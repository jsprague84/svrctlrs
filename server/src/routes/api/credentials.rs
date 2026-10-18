//! Credentials API endpoints
//!
//! Provides CRUD operations for managing SSH keys, API tokens, and other credentials.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{queries::credentials, CreateCredential, UpdateCredential};
use tracing::{error, info, instrument};

use super::ApiError;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;
use svrctlrs_database::models::NewAuditEntry;

/// Create credentials router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_credentials).post(create_credential))
        .route("/generate-key", post(generate_key))
        .route(
            "/{id}",
            get(get_credential)
                .put(update_credential)
                .delete(delete_credential),
        )
}

/// List all credentials
///
/// Note: Credential values are redacted for security.
#[instrument(skip(state))]
async fn list_credentials(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let creds = credentials::list_credentials(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list credentials");
            ApiError::internal_error(format!("Failed to list credentials: {}", e))
        })?;

    // Which servers use each credential: the list is what tells you whether deleting is safe
    let servers = svrctlrs_database::queries::servers::list_servers(&state.pool)
        .await
        .unwrap_or_default();

    // Redact credential values for security; derive what the page can show instead
    let redacted: Vec<_> = creds
        .into_iter()
        .map(|c| {
            let used_by: Vec<&str> = servers
                .iter()
                .filter(|s| s.credential_id == Some(c.id))
                .map(|s| s.name.as_str())
                .collect();
            let public_key = crate::ssh_keys::public_key_of(&c);
            let key_source = if !c.is_ssh_key() {
                None
            } else if c.value_is_key_material() {
                Some("key")
            } else {
                Some("path")
            };
            json!({
                "id": c.id,
                "name": c.name,
                "credential_type": c.credential_type_str,
                "description": c.description,
                "username": c.username,
                "has_value": !c.value.is_empty(),
                "has_passphrase": c.passphrase().is_some(),
                "key_source": key_source,
                "fingerprint": public_key.as_deref().map(crate::host_keys::fingerprint),
                "in_use": !used_by.is_empty(),
                "used_by": used_by,
                "created_at": c.created_at,
                "updated_at": c.updated_at
            })
        })
        .collect();

    Ok(Json(json!({
        "credentials": redacted
    })))
}

/// Get credential by ID
///
/// Note: Credential value is redacted for security.
#[instrument(skip(state))]
async fn get_credential(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let cred = credentials::get_credential(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to get credential");
            ApiError::not_found("Credential")
        })?;

    // Check if in use
    let in_use = credentials::credential_in_use(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to check credential usage");
            ApiError::internal_error(format!("Failed to check credential usage: {}", e))
        })?;

    Ok(Json(json!({
        "id": cred.id,
        "name": cred.name,
        "credential_type": cred.credential_type_str,
        "description": cred.description,
        "username": cred.username,
        "has_value": !cred.value.is_empty(),
        "has_passphrase": cred.passphrase().is_some(),
        "public_key": crate::ssh_keys::public_key_of(&cred),
        "in_use": in_use,
        "created_at": cred.created_at,
        "updated_at": cred.updated_at
    })))
}

/// Generate-key input
#[derive(Debug, Deserialize)]
struct GenerateKeyInput {
    name: String,
    #[serde(default)]
    description: Option<String>,
    /// Comment at the end of the public key line (defaults to `svrctlrs:<name>`)
    #[serde(default)]
    comment: Option<String>,
}

/// Generate an ed25519 key pair, store the private half as an encrypted `ssh_key`
/// credential, and return the public half for `authorized_keys`.
#[instrument(skip(state, input))]
async fn generate_key(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<GenerateKeyInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("name is required"));
    }
    let comment = input
        .comment
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| format!("svrctlrs:{name}"));
    let generated = crate::ssh_keys::generate_ed25519(&comment)
        .map_err(|e| ApiError::internal_error(format!("Failed to generate key: {e}")))?;

    let create = CreateCredential {
        name: name.clone(),
        credential_type: svrctlrs_database::models::CredentialType::SshKey,
        value: generated.private_openssh,
        description: input.description,
        username: None,
        metadata: None,
    };
    let id = credentials::create_credential(&state.pool, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to store generated key");
            ApiError::internal_error(format!("Failed to store generated key: {e}"))
        })?;
    info!(id, "Generated SSH key credential");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("credential.generated")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("credential", id, &name)
            .details(json!({ "type": "ssh_key", "algorithm": "ed25519" })),
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "public_key": generated.public_openssh,
            "message": "Key generated"
        })),
    ))
}

/// Create credential input
#[derive(Debug, Deserialize)]
struct CreateCredentialInput {
    name: String,
    credential_type: String,
    value: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    username: Option<String>,
    /// Passphrase of an encrypted SSH key; stored encrypted in the credential's metadata.
    #[serde(default)]
    passphrase: Option<String>,
}

/// Credential metadata carrying the passphrase, or none when it is empty.
fn passphrase_metadata(passphrase: Option<String>) -> Option<serde_json::Value> {
    passphrase
        .filter(|p| !p.is_empty())
        .map(|p| json!({ "passphrase": p }))
}

/// Create a new credential
#[instrument(skip(state, input))]
async fn create_credential(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<CreateCredentialInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(name = %input.name, "Creating credential");

    // Parse credential type
    let credential_type = match input.credential_type.as_str() {
        "ssh_key" => svrctlrs_database::models::CredentialType::SshKey,
        "password" => svrctlrs_database::models::CredentialType::Password,
        "api_token" => svrctlrs_database::models::CredentialType::ApiToken,
        _ => {
            return Err(ApiError::bad_request(format!(
                "Invalid credential type: {}. Valid types: ssh_key, password, api_token",
                input.credential_type
            )))
        }
    };

    if credential_type == svrctlrs_database::models::CredentialType::SshKey {
        if let Some(problem) = svrctlrs_database::models::ssh_key_value_problem(&input.value) {
            return Err(ApiError::bad_request(problem));
        }
    }

    let create = CreateCredential {
        name: input.name,
        credential_type,
        value: input.value,
        description: input.description,
        username: input.username,
        metadata: passphrase_metadata(input.passphrase),
    };

    let id = credentials::create_credential(&state.pool, &create)
        .await
        .map_err(|e| match e {
            svrctlrs_core::Error::Conflict(msg) => ApiError::conflict(msg),
            e => {
                error!(error = %e, "Failed to create credential");
                ApiError::internal_error(format!("Failed to create credential: {}", e))
            }
        })?;

    info!(id = id, "Credential created successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("credential.created")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("credential", id, &create.name)
            .details(json!({ "type": create.credential_type.as_str() })),
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Credential created successfully"
        })),
    ))
}

/// Update credential input
#[derive(Debug, Deserialize)]
struct UpdateCredentialInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    username: Option<String>,
    /// New passphrase for an encrypted SSH key. Omit to keep the current one; send an empty
    /// string to remove it.
    #[serde(default)]
    passphrase: Option<String>,
}

/// Update an existing credential
#[instrument(skip(state, input))]
async fn update_credential(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<UpdateCredentialInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Updating credential");

    // Verify credential exists
    let existing = credentials::get_credential(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Credential"))?;
    if existing.is_ssh_key() {
        if let Some(problem) = input
            .value
            .as_deref()
            .and_then(svrctlrs_database::models::ssh_key_value_problem)
        {
            return Err(ApiError::bad_request(problem));
        }
    }

    let update = UpdateCredential {
        name: input.name,
        value: input.value,
        description: input.description,
        username: input.username,
        // Some("") clears the passphrase by storing an empty metadata object.
        metadata: input
            .passphrase
            .map(|p| passphrase_metadata(Some(p)).unwrap_or_else(|| json!({}))),
    };

    credentials::update_credential(&state.pool, id, &update)
        .await
        .map_err(|e| match e {
            svrctlrs_core::Error::Conflict(msg) => ApiError::conflict(msg),
            e => {
                error!(error = %e, id = id, "Failed to update credential");
                ApiError::internal_error(format!("Failed to update credential: {}", e))
            }
        })?;

    info!(id = id, "Credential updated successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("credential.updated")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("credential", id, "")
            .details(json!({
                "value_changed": update.value.is_some(),
                "passphrase_changed": update.metadata.is_some(),
            })),
    );

    Ok(Json(json!({
        "id": id,
        "message": "Credential updated successfully"
    })))
}

/// Delete a credential
#[instrument(skip(state))]
async fn delete_credential(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Deleting credential");

    // Check if in use
    let in_use = credentials::credential_in_use(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to check credential usage");
            ApiError::internal_error(format!("Failed to check credential usage: {}", e))
        })?;

    if in_use {
        return Err(ApiError::conflict(
            "Cannot delete credential: it is in use by one or more servers",
        ));
    }

    credentials::delete_credential(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to delete credential");
            ApiError::internal_error(format!("Failed to delete credential: {}", e))
        })?;

    info!(id = id, "Credential deleted successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("credential.deleted")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("credential", id, ""),
    );

    Ok(Json(json!({
        "id": id,
        "message": "Credential deleted successfully"
    })))
}

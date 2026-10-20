//! Credentials API endpoints
//!
//! Provides CRUD operations for managing SSH keys, API tokens, and other credentials.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::json;
use svrctlrs_api::credentials::{
    CredentialChanges, CredentialDetail, CredentialList, CredentialSummary, GenerateKey,
    GeneratedKey, NewCredential,
};
use svrctlrs_api::{ErrorBody, IdMessage};
use svrctlrs_database::{
    models::{CreateCredential, UpdateCredential},
    queries::credentials,
};
use tracing::{info, instrument, warn};

use super::ApiError;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;
use svrctlrs_database::models::NewAuditEntry;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Create credentials router
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_credentials, create_credential))
        .routes(routes!(generate_key))
        .routes(routes!(
            get_credential,
            update_credential,
            delete_credential
        ))
}

/// List all credentials
///
/// Note: Credential values are redacted for security.
#[utoipa::path(get, path = "/", tag = "credentials",
    responses((status = 200, body = CredentialList)))]
#[instrument(skip(state))]
async fn list_credentials(State(state): State<AppState>) -> Result<Json<CredentialList>, ApiError> {
    let creds = credentials::list_credentials(&state.pool).await?;

    // Which servers use each credential: the list is what tells you whether deleting is safe
    let servers = svrctlrs_database::queries::servers::list_servers(&state.pool)
        .await
        .unwrap_or_default();

    // Values stay redacted; derive what the page can show instead. The list query redacts in
    // SQL, so an SSH key's source, passphrase and fingerprint come from its decrypted row.
    let mut redacted = Vec::with_capacity(creds.len());
    for c in creds {
        let used_by: Vec<String> = servers
            .iter()
            .filter(|s| s.credential_id == Some(c.id))
            .map(|s| s.name.clone())
            .collect();
        let summary = if c.is_ssh_key() {
            match credentials::get_credential(&state.pool, c.id).await {
                Ok(full) => crate::ssh_keys::key_summary(&full),
                Err(e) => {
                    warn!(credential_id = c.id, error = %e, "Could not read SSH key for the list");
                    None
                }
            }
        } else {
            None
        };
        redacted.push(CredentialSummary {
            id: c.id,
            has_value: !c.value.is_empty(),
            name: c.name,
            credential_type: c.credential_type_str,
            description: c.description,
            username: c.username,
            has_passphrase: summary.as_ref().is_some_and(|s| s.has_passphrase),
            key_source: summary.as_ref().map(|s| s.source.to_string()),
            fingerprint: summary.and_then(|s| s.fingerprint),
            in_use: !used_by.is_empty(),
            used_by,
            created_at: c.created_at,
            updated_at: c.updated_at,
        });
    }

    Ok(Json(CredentialList {
        credentials: redacted,
    }))
}

/// Get credential by ID
///
/// Note: Credential value is redacted for security.
#[utoipa::path(get, path = "/{id}", tag = "credentials", params(("id" = i64, Path)),
    responses((status = 200, body = CredentialDetail), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn get_credential(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<CredentialDetail>, ApiError> {
    let cred = credentials::get_credential(&state.pool, id).await?;

    // Check if in use
    let in_use = credentials::credential_in_use(&state.pool, id).await?;
    let summary = crate::ssh_keys::key_summary(&cred);

    Ok(Json(CredentialDetail {
        has_value: !cred.value.is_empty(),
        has_passphrase: cred.passphrase().is_some(),
        key_source: summary.as_ref().map(|s| s.source.to_string()),
        fingerprint: summary.and_then(|s| s.fingerprint),
        public_key: crate::ssh_keys::public_key_of(&cred),
        id: cred.id,
        name: cred.name,
        credential_type: cred.credential_type_str,
        description: cred.description,
        username: cred.username,
        in_use,
        created_at: cred.created_at,
        updated_at: cred.updated_at,
    }))
}

/// Generate an ed25519 key pair, store the private half as an encrypted `ssh_key`
/// credential, and return the public half for `authorized_keys`.
#[utoipa::path(post, path = "/generate-key", tag = "credentials", request_body = GenerateKey,
    responses(
        (status = 201, body = GeneratedKey),
        (status = 400, body = ErrorBody),
        (status = 409, description = "The name is taken", body = ErrorBody),
    ))]
#[instrument(skip(state, input))]
async fn generate_key(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<GenerateKey>,
) -> Result<(StatusCode, Json<GeneratedKey>), ApiError> {
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
    let id = credentials::create_credential(&state.pool, &create).await?;
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
        Json(GeneratedKey {
            id,
            public_key: generated.public_openssh,
            message: "Key generated".to_string(),
        }),
    ))
}

/// Credential metadata carrying the passphrase, or none when it is empty.
fn passphrase_metadata(passphrase: Option<String>) -> Option<serde_json::Value> {
    passphrase
        .filter(|p| !p.is_empty())
        .map(|p| json!({ "passphrase": p }))
}

/// Create a new credential
#[utoipa::path(post, path = "/", tag = "credentials", request_body = NewCredential,
    responses(
        (status = 201, body = IdMessage),
        (status = 400, body = ErrorBody),
        (status = 409, description = "The name is taken", body = ErrorBody),
    ))]
#[instrument(skip(state, input))]
async fn create_credential(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<NewCredential>,
) -> Result<(StatusCode, Json<IdMessage>), ApiError> {
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

    let id = credentials::create_credential(&state.pool, &create).await?;

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
        Json(IdMessage::new(id, "Credential created successfully")),
    ))
}

/// Update an existing credential
#[utoipa::path(put, path = "/{id}", tag = "credentials", params(("id" = i64, Path)),
    request_body = CredentialChanges,
    responses(
        (status = 200, body = IdMessage),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, description = "The name is taken", body = ErrorBody),
    ))]
#[instrument(skip(state, input))]
async fn update_credential(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<CredentialChanges>,
) -> Result<Json<IdMessage>, ApiError> {
    info!(id = id, "Updating credential");

    // Verify credential exists
    let existing = credentials::get_credential(&state.pool, id).await?;
    if existing.is_ssh_key() {
        if let Some(problem) = input
            .value
            .as_deref()
            .and_then(svrctlrs_database::models::ssh_key_value_problem)
        {
            return Err(ApiError::bad_request(problem));
        }
    }

    let audit_name = input.name.clone().unwrap_or_else(|| existing.name.clone());
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

    credentials::update_credential(&state.pool, id, &update).await?;

    info!(id = id, "Credential updated successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("credential.updated")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("credential", id, &audit_name)
            .details(json!({
                "value_changed": update.value.is_some(),
                "passphrase_changed": update.metadata.is_some(),
            })),
    );

    Ok(Json(IdMessage::new(id, "Credential updated successfully")))
}

/// Delete a credential; one a server still uses is refused
#[utoipa::path(delete, path = "/{id}", tag = "credentials", params(("id" = i64, Path)),
    responses(
        (status = 200, body = IdMessage),
        (status = 202, description = "Held for the owner's approval (a token set to ask first)"),
        (status = 404, body = ErrorBody),
        (status = 409, description = "A server uses it", body = ErrorBody),
    ))]
#[instrument(skip(state))]
async fn delete_credential(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<IdMessage>, ApiError> {
    info!(id = id, "Deleting credential");
    let name = credentials::get_credential(&state.pool, id).await?.name;

    // Check if in use
    let in_use = credentials::credential_in_use(&state.pool, id).await?;

    if in_use {
        return Err(ApiError::conflict(
            "Cannot delete credential: it is in use by one or more servers",
        ));
    }

    credentials::delete_credential(&state.pool, id).await?;

    info!(id = id, "Credential deleted successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("credential.deleted")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("credential", id, &name),
    );

    Ok(Json(IdMessage::new(id, "Credential deleted successfully")))
}

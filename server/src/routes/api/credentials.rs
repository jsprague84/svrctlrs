//! Credentials API endpoints
//!
//! Provides CRUD operations for managing SSH keys, API tokens, and other credentials.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{queries::credentials, CreateCredential, UpdateCredential};
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create credentials router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_credentials).post(create_credential))
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

    // Redact credential values for security
    let redacted: Vec<_> = creds
        .into_iter()
        .map(|c| {
            json!({
                "id": c.id,
                "name": c.name,
                "credential_type": c.credential_type_str,
                "description": c.description,
                "username": c.username,
                "has_value": !c.value.is_empty(),
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
        .unwrap_or(false);

    Ok(Json(json!({
        "id": cred.id,
        "name": cred.name,
        "credential_type": cred.credential_type_str,
        "description": cred.description,
        "username": cred.username,
        "has_value": !cred.value.is_empty(),
        "in_use": in_use,
        "created_at": cred.created_at,
        "updated_at": cred.updated_at
    })))
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
}

/// Create a new credential
#[instrument(skip(state, input))]
async fn create_credential(
    State(state): State<AppState>,
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

    let create = CreateCredential {
        name: input.name,
        credential_type,
        value: input.value,
        description: input.description,
        username: input.username,
        metadata: None,
    };

    let id = credentials::create_credential(&state.pool, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create credential");
            ApiError::internal_error(format!("Failed to create credential: {}", e))
        })?;

    info!(id = id, "Credential created successfully");

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
}

/// Update an existing credential
#[instrument(skip(state, input))]
async fn update_credential(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateCredentialInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Updating credential");

    // Verify credential exists
    credentials::get_credential(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Credential"))?;

    let update = UpdateCredential {
        name: input.name,
        value: input.value,
        description: input.description,
        username: input.username,
        metadata: None,
    };

    credentials::update_credential(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update credential");
            ApiError::internal_error(format!("Failed to update credential: {}", e))
        })?;

    info!(id = id, "Credential updated successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Credential updated successfully"
    })))
}

/// Delete a credential
#[instrument(skip(state))]
async fn delete_credential(
    State(state): State<AppState>,
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

    Ok(Json(json!({
        "id": id,
        "message": "Credential deleted successfully"
    })))
}

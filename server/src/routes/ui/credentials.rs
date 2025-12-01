//! Credentials management routes

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::{get, post, put},
    Router,
};
use axum_extra::extract::Form;
use serde::Deserialize;
use svrctlrs_database::{
    models::credential::{CreateCredential, Credential, CredentialType, UpdateCredential},
    queries,
};

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::User};

/// Create credentials router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/credentials",
            get(credentials_page).post(credential_create),
        )
        .route("/credentials/new", get(credential_form_new))
        .route("/credentials/{id}/edit", get(credential_form_edit))
        .route(
            "/credentials/{id}",
            put(credential_update).delete(credential_delete),
        )
        .route("/credentials/{id}/test", post(test_connection))
}

// ============================================================================
// Template Structs
// ============================================================================

#[derive(Template)]
#[template(path = "pages/credentials.html")]
pub struct CredentialsTemplate {
    pub user: Option<User>,
    pub credentials: Vec<CredentialDisplay>,
}

#[derive(Template)]
#[template(path = "components/credential_list.html")]
pub struct CredentialListTemplate {
    pub credentials: Vec<CredentialDisplay>,
}

#[derive(Template)]
#[template(path = "components/credential_form.html")]
pub struct CredentialFormTemplate {
    pub credential: Option<CredentialDisplay>,
    pub error: Option<String>,
}

/// Credential display model - hides sensitive data in list views
#[derive(Debug, Clone)]
pub struct CredentialDisplay {
    pub id: i64,
    pub name: String,
    #[allow(dead_code)]
    pub credential_type: String,
    pub credential_type_display: String,
    pub auth_type: String, // Alias for credential_type (for template compatibility)
    pub description: String, // Empty string if None (display-ready)
    #[allow(dead_code)]
    pub value_preview: String, // Masked or path only
    pub username: String,  // Empty string if None (display-ready)
    pub server_count: i64,
    #[allow(dead_code)]
    pub created_at: String,
    #[allow(dead_code)]
    pub updated_at: String,
}

impl From<Credential> for CredentialDisplay {
    fn from(cred: Credential) -> Self {
        let credential_type_display = match cred.credential_type_str.as_str() {
            "ssh_key" => "SSH Key",
            "api_token" => "API Token",
            "password" => "Password",
            "certificate" => "Certificate",
            _ => "Unknown",
        };

        let value_preview = match cred.credential_type_str.as_str() {
            "ssh_key" | "certificate" => {
                // Show path for file-based credentials
                cred.value.clone()
            }
            "api_token" | "password" => {
                // Mask sensitive values
                if cred.value.len() > 8 {
                    format!(
                        "{}...{}",
                        &cred.value[..4],
                        &cred.value[cred.value.len() - 4..]
                    )
                } else {
                    "****".to_string()
                }
            }
            _ => "****".to_string(),
        };

        Self {
            id: cred.id,
            name: cred.name,
            credential_type: cred.credential_type_str.clone(),
            credential_type_display: credential_type_display.to_string(),
            auth_type: cred.credential_type_str, // Alias for templates
            description: cred.description.unwrap_or_default(), // Convert Option → String
            value_preview,
            username: cred.username.unwrap_or_default(), // Convert Option → String
            server_count: 0,                             // TODO: Query actual count from database
            created_at: cred.created_at.format("%Y-%m-%d %H:%M").to_string(),
            updated_at: cred.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        }
    }
}

// ============================================================================
// Form Input Structs
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateCredentialInput {
    pub name: String,
    pub credential_type: String,
    pub description: Option<String>,
    pub value: String,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCredentialInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub username: Option<String>,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// Credentials page handler
async fn credentials_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Load credentials from database
    let db = state.db().await;
    let db_credentials = queries::credentials::list_credentials(db.pool()).await?;
    let credentials = db_credentials
        .into_iter()
        .map(CredentialDisplay::from)
        .collect();

    let template = CredentialsTemplate { user, credentials };
    Ok(Html(template.render()?))
}

/// New credential form
async fn credential_form_new() -> Result<Html<String>, AppError> {
    let template = CredentialFormTemplate {
        credential: None,
        error: None,
    };
    Ok(Html(template.render()?))
}

/// Edit credential form
async fn credential_form_edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    // Load credential from database
    let db = state.db().await;
    let db_credential = queries::credentials::get_credential(db.pool(), id).await;

    let (credential, error) = match db_credential {
        Ok(c) => (Some(CredentialDisplay::from(c)), None),
        Err(e) => {
            tracing::warn!("Failed to load credential {}: {}", id, e);
            (None, Some(format!("Credential with ID {} not found", id)))
        }
    };

    let template = CredentialFormTemplate { credential, error };
    Ok(Html(template.render()?))
}

/// Create credential handler
async fn credential_create(
    State(state): State<AppState>,
    Form(input): Form<CreateCredentialInput>,
) -> Result<Html<String>, AppError> {
    // Validate
    if input.name.trim().is_empty() {
        let template = CredentialFormTemplate {
            credential: None,
            error: Some("Name is required".to_string()),
        };
        return Ok(Html(template.render()?));
    }

    if input.value.trim().is_empty() {
        let template = CredentialFormTemplate {
            credential: None,
            error: Some("Value is required".to_string()),
        };
        return Ok(Html(template.render()?));
    }

    // Parse credential type
    let credential_type = match CredentialType::from_str(&input.credential_type) {
        Some(t) => t,
        None => {
            let template = CredentialFormTemplate {
                credential: None,
                error: Some(format!(
                    "Invalid credential type: {}",
                    input.credential_type
                )),
            };
            return Ok(Html(template.render()?));
        }
    };

    // Create credential
    tracing::info!(
        "Creating credential: {} (type: {})",
        input.name,
        input.credential_type
    );
    let db = state.db().await;

    let create_credential = CreateCredential {
        name: input.name.clone(),
        credential_type,
        description: input.description,
        value: input.value,
        username: input.username,
        metadata: None,
    };

    // Try to create, handle duplicate name error
    match queries::credentials::create_credential(db.pool(), &create_credential).await {
        Ok(_) => {
            // Success - return updated list with success message
            let db_credentials = queries::credentials::list_credentials(db.pool()).await?;
            let credentials = db_credentials
                .into_iter()
                .map(CredentialDisplay::from)
                .collect();
            let template = CredentialListTemplate { credentials };
            let list_html = template.render()?;

            // Prepend success message
            Ok(Html(format!(
                r#"<div class="alert alert-success alert-auto-dismiss">✓ Credential '{}' created successfully!</div>{}"#,
                input.name, list_html
            )))
        }
        Err(e) => {
            // Check if it's a duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("credentials.name") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ A credential with the name '{}' already exists. Please use a different name.</div>"#,
                    input.name
                )))
            } else {
                // Other database error
                tracing::error!("Failed to create credential: {}", e);
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ Failed to create credential: {}</div>"#,
                    e
                )))
            }
        }
    }
}

/// Update credential handler
async fn credential_update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateCredentialInput>,
) -> Result<Html<String>, AppError> {
    // Validate
    if let Some(ref name) = input.name {
        if name.trim().is_empty() {
            return Ok(Html(
                r#"<div class="alert alert-error alert-auto-dismiss">✗ Name cannot be empty</div>"#.to_string(),
            ));
        }
    }

    if let Some(ref value) = input.value {
        if value.trim().is_empty() {
            return Ok(Html(
                r#"<div class="alert alert-error alert-auto-dismiss">✗ Value cannot be empty</div>"#.to_string(),
            ));
        }
    }

    // Update in database
    tracing::info!("Updating credential {}: {:?}", id, input);
    let db = state.db().await;

    // Get the credential name for the success message
    let credential_name = if let Some(ref name) = input.name {
        name.clone()
    } else {
        // If name wasn't changed, get it from database
        queries::credentials::get_credential(db.pool(), id)
            .await?
            .name
    };

    let update_credential = UpdateCredential {
        name: input.name,
        description: input.description,
        value: input.value,
        username: input.username,
        metadata: None,
    };

    // Try to update, handle duplicate name error
    match queries::credentials::update_credential(db.pool(), id, &update_credential).await {
        Ok(_) => {
            // Success - return updated list with success message
            let db_credentials = queries::credentials::list_credentials(db.pool()).await?;
            let credentials = db_credentials
                .into_iter()
                .map(CredentialDisplay::from)
                .collect();
            let template = CredentialListTemplate { credentials };
            let list_html = template.render()?;

            // Prepend success message
            Ok(Html(format!(
                r#"<div class="alert alert-success alert-auto-dismiss">✓ Credential '{}' updated successfully!</div>{}"#,
                credential_name, list_html
            )))
        }
        Err(e) => {
            // Check if it's a duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("credentials.name") {
                Ok(Html(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ A credential with that name already exists. Please use a different name.</div>"#.to_string()
                ))
            } else {
                // Other database error
                tracing::error!("Failed to update credential: {}", e);
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ Failed to update credential: {}</div>"#,
                    e
                )))
            }
        }
    }
}

/// Delete credential handler
async fn credential_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    // Get credential name before deleting
    let db = state.db().await;
    let credential_name = queries::credentials::get_credential(db.pool(), id)
        .await
        .map(|c| c.name)
        .unwrap_or_else(|_| format!("Credential {}", id));

    // Try to delete (will fail if in use)
    tracing::info!("Deleting credential {}", id);
    match queries::credentials::delete_credential(db.pool(), id).await {
        Ok(_) => {
            // Success
            Ok(Html(format!(
                r#"<div class="alert alert-success alert-auto-dismiss">✓ Credential '{}' deleted successfully!</div>"#,
                credential_name
            )))
        }
        Err(e) => {
            // Check if credential is in use
            let error_msg = e.to_string();
            if error_msg.contains("in use") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ Cannot delete credential '{}': it is in use by one or more servers</div>"#,
                    credential_name
                )))
            } else {
                // Other error
                tracing::error!("Failed to delete credential: {}", e);
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ Failed to delete credential: {}</div>"#,
                    e
                )))
            }
        }
    }
}

/// Test SSH connection input
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TestConnectionInput {
    host: String,
    port: Option<i32>,
    username: Option<String>,
    ssh_key_path: Option<String>,
}

/// Test SSH connection handler
async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Testing SSH connection with credential {}", id);

    // Load credential from database
    let db = state.db().await;
    let credential = queries::credentials::get_credential(db.pool(), id).await?;

    // Verify it's an SSH key credential
    if !credential.is_ssh_key() {
        return Ok(Html(format!(
            r#"<div class="alert alert-error alert-auto-dismiss">✗ Credential '{}' is not an SSH key. Only SSH key credentials can be tested.</div>"#,
            credential.name
        )));
    }

    // For testing, we need more information - this endpoint should be called with form data
    // For now, just return a message indicating the credential exists
    Ok(Html(format!(
        r#"<div class="alert alert-info">ℹ Credential '{}' loaded successfully. SSH key path: {}</div>"#,
        credential.name, credential.value
    )))
}

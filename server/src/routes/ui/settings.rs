//! Settings and notification backend management routes

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::{get, post, put},
    Form, Router,
};
use serde::Deserialize;
use svrctlrs_database::queries;

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::*};

/// Create settings router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(settings_page))
        .route("/settings/notifications", get(notifications_page))
        .route("/settings/notifications/new", get(notification_form_new))
        .route("/settings/notifications", post(notification_create))
        .route(
            "/settings/notifications/{id}/edit",
            get(notification_form_edit),
        )
        .route(
            "/settings/notifications/{id}",
            put(notification_update).delete(notification_delete),
        )
}

/// Helper to convert DB notification backend to UI model
fn db_notification_to_ui(
    db: svrctlrs_database::models::notification::NotificationBackend,
) -> NotificationBackend {
    NotificationBackend {
        id: db.id,
        backend_type: db.backend_type,
        name: db.name,
        enabled: db.enabled,
        priority: db.priority,
    }
}

/// Settings page handler
async fn settings_page() -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    let template = SettingsTemplate { user };
    Ok(Html(template.render()?))
}

/// Notifications page handler
async fn notifications_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Load notification backends from database
    let db = state.db().await;
    let db_notifications = queries::notifications::list_notification_channels(db.pool()).await?;
    let notifications = db_notifications
        .into_iter()
        .map(db_notification_to_ui)
        .collect();

    let template = NotificationsTemplate {
        user,
        notifications,
    };
    Ok(Html(template.render()?))
}

/// New notification backend form
async fn notification_form_new() -> Result<Html<String>, AppError> {
    tracing::info!("notification_form_new called - loading add backend form");
    let template = NotificationFormTemplate {
        notification: None,
        config_url: String::new(),
        config_token: String::new(),
        config_topic: String::new(),
        config_username: String::new(),
        config_password: String::new(),
        error: None,
    };
    Ok(Html(template.render()?))
}

/// Edit notification backend form
async fn notification_form_edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    // Load notification backend from database
    let db = state.db().await;
    let db_notification = queries::notifications::get_notification_channel(db.pool(), id).await;

    let (notification, error) = match db_notification {
        Ok(n) => {
            let config = n.get_config();
            let template_notification = Some(db_notification_to_ui(n));
            let config_url = config
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let config_token = config
                .get("token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let config_topic = config
                .get("topic")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let config_username = config
                .get("username")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let config_password = config
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let template = NotificationFormTemplate {
                notification: template_notification,
                config_url,
                config_token,
                config_topic,
                config_username,
                config_password,
                error: None,
            };
            return Ok(Html(template.render()?));
        }
        Err(e) => {
            tracing::warn!("Failed to load notification backend {}: {}", id, e);
            (
                None,
                Some(format!("Notification backend with ID {} not found", id)),
            )
        }
    };

    let template = NotificationFormTemplate {
        notification,
        config_url: String::new(),
        config_token: String::new(),
        config_topic: String::new(),
        config_username: String::new(),
        config_password: String::new(),
        error,
    };
    Ok(Html(template.render()?))
}

/// Create notification backend input
#[derive(Debug, Deserialize)]
struct CreateNotificationInput {
    backend_type: String,
    name: String,
    url: Option<String>,
    token: Option<String>,
    topic: Option<String>,
    username: Option<String>,
    password: Option<String>,
    priority: Option<i32>,
}

/// Create notification backend handler
async fn notification_create(
    State(state): State<AppState>,
    Form(input): Form<CreateNotificationInput>,
) -> Result<Html<String>, AppError> {
    tracing::info!(
        "notification_create called with: name={}, type={}, url={:?}, token={:?}, topic={:?}",
        input.name,
        input.backend_type,
        input.url,
        input.token,
        input.topic
    );

    // Validate
    if input.name.is_empty() || input.backend_type.is_empty() {
        let template = NotificationFormTemplate {
            notification: None,
            config_url: String::new(),
            config_token: String::new(),
            config_topic: String::new(),
            config_username: String::new(),
            config_password: String::new(),
            error: Some("Name and backend type are required".to_string()),
        };
        return Ok(Html(template.render()?));
    }

    // Build config JSON based on backend type
    let config_json = if input.backend_type == "gotify" {
        serde_json::json!({
            "url": input.url.unwrap_or_default(),
            "token": input.token.unwrap_or_default(),
        })
    } else {
        // ntfy backend - include username/password if provided
        let mut config = serde_json::json!({
            "url": input.url.unwrap_or_default(),
            "topic": input.topic.unwrap_or_default(),
        });

        // Add token if provided
        if let Some(token) = input.token {
            if !token.trim().is_empty() {
                config["token"] = serde_json::json!(token);
            }
        }

        // Add username/password if provided
        if let Some(username) = input.username {
            if !username.trim().is_empty() {
                config["username"] = serde_json::json!(username);
                if let Some(password) = input.password {
                    config["password"] = serde_json::json!(password);
                }
            }
        }

        config
    };

    // Save to database
    tracing::info!(
        "Creating notification backend: {} ({})",
        input.name,
        input.backend_type
    );
    let db = state.db().await;

    let create_backend = svrctlrs_database::models::notification::CreateNotificationBackend {
        backend_type: input.backend_type.clone(),
        name: input.name.clone(),
        config: config_json,
        priority: input.priority.unwrap_or(5),
    };

    match queries::notifications::create_notification_backend(db.pool(), &create_backend).await {
        Ok(_) => {
            // Success - return updated list with success message
            let db_notifications =
                queries::notifications::list_notification_channels(db.pool()).await?;
            let notifications = db_notifications
                .into_iter()
                .map(db_notification_to_ui)
                .collect();
            let template = NotificationListTemplate { notifications };
            let list_html = template.render()?;

            Ok(Html(format!(
                r#"<div class="alert alert-success">✓ Notification backend '{}' ({}) created successfully!</div>{}"#,
                input.name, input.backend_type, list_html
            )))
        }
        Err(e) => {
            // Check if it's a duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error">✗ A notification backend with the name '{}' already exists. Please use a different name.</div>"#,
                    input.name
                )))
            } else {
                Err(e.into())
            }
        }
    }
}

/// Update notification backend input
#[derive(Debug, Deserialize)]
struct UpdateNotificationInput {
    name: Option<String>,
    enabled: Option<String>,
    url: Option<String>,
    token: Option<String>,
    topic: Option<String>,
    username: Option<String>,
    password: Option<String>,
    priority: Option<i32>,
}

/// Update notification backend handler
async fn notification_update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateNotificationInput>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Updating notification backend {}: {:?}", id, input);

    // Get existing backend to determine type
    let db = state.db().await;
    let existing = queries::notifications::get_notification_channel(db.pool(), id).await?;

    // Build config JSON based on backend type
    let config_json = if existing.backend_type == "gotify" {
        serde_json::json!({
            "url": input.url.unwrap_or_default(),
            "token": input.token.unwrap_or_default(),
        })
    } else {
        // ntfy backend - include username/password if provided
        let mut config = serde_json::json!({
            "url": input.url.unwrap_or_default(),
            "topic": input.topic.unwrap_or_default(),
        });

        // Add token if provided
        if let Some(token) = input.token {
            if !token.trim().is_empty() {
                config["token"] = serde_json::json!(token);
            }
        }

        // Add username/password if provided
        if let Some(username) = input.username {
            if !username.trim().is_empty() {
                config["username"] = serde_json::json!(username);
                if let Some(password) = input.password {
                    config["password"] = serde_json::json!(password);
                }
            }
        }

        config
    };

    // Get backend name for success message
    let backend_name = if let Some(ref name) = input.name {
        name.clone()
    } else {
        existing.name.clone()
    };

    // Update in database
    let update_backend = svrctlrs_database::models::notification::UpdateNotificationBackend {
        name: input.name,
        enabled: input.enabled.map(|s| s == "on"),
        config: Some(config_json),
        priority: input.priority,
    };

    match queries::notifications::update_notification_backend(db.pool(), id, &update_backend).await
    {
        Ok(_) => {
            // Success - return updated list with success message
            let db_notifications =
                queries::notifications::list_notification_channels(db.pool()).await?;
            let notifications = db_notifications
                .into_iter()
                .map(db_notification_to_ui)
                .collect();
            let template = NotificationListTemplate { notifications };
            let list_html = template.render()?;

            Ok(Html(format!(
                r#"<div class="alert alert-success">✓ Notification backend '{}' updated successfully!</div>{}"#,
                backend_name, list_html
            )))
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") {
                Ok(Html(
                    r#"<div class="alert alert-error">✗ A notification backend with that name already exists. Please use a different name.</div>"#.to_string()
                ))
            } else {
                Err(e.into())
            }
        }
    }
}

/// Delete notification backend handler
async fn notification_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    // Get backend name before deleting
    let db = state.db().await;
    let backend_name = queries::notifications::get_notification_channel(db.pool(), id)
        .await
        .map(|b| b.name)
        .unwrap_or_else(|_| format!("Backend {}", id));

    tracing::info!("Deleting notification backend {}", id);
    queries::notifications::delete_notification_backend(db.pool(), id).await?;

    // Return success message
    Ok(Html(format!(
        r#"<div class="alert alert-success">✓ Notification backend '{}' deleted successfully!</div>"#,
        backend_name
    )))
}

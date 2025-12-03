//! Notifications API endpoints
//!
//! Provides CRUD for notification channels and policies, plus testing.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{
    queries::notifications, ChannelType, CreateNotificationChannel, CreateNotificationPolicy,
    UpdateNotificationChannel, UpdateNotificationPolicy,
};
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create notifications router
pub fn routes() -> Router<AppState> {
    Router::new()
        .nest("/channels", channels_routes())
        .nest("/policies", policies_routes())
        .route("/history", get(list_notification_history))
}

fn channels_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_channels).post(create_channel))
        .route(
            "/{id}",
            get(get_channel).put(update_channel).delete(delete_channel),
        )
        .route("/{id}/test", post(test_channel))
}

fn policies_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_policies).post(create_policy))
        .route(
            "/{id}",
            get(get_policy).put(update_policy).delete(delete_policy),
        )
}

// === Notification Channels ===

/// List all notification channels
#[instrument(skip(state))]
async fn list_channels(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let channels = notifications::list_notification_channels(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list notification channels");
            ApiError::internal_error(format!("Failed to list notification channels: {}", e))
        })?;

    // Redact sensitive config values
    let redacted: Vec<_> = channels
        .into_iter()
        .map(|c| {
            json!({
                "id": c.id,
                "name": c.name,
                "channel_type": c.channel_type_str,
                "description": c.description,
                "enabled": c.enabled,
                "created_at": c.created_at,
                "updated_at": c.updated_at
            })
        })
        .collect();

    Ok(Json(json!({ "channels": redacted })))
}

/// Get notification channel by ID
#[instrument(skip(state))]
async fn get_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let channel = notifications::get_notification_channel(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to get notification channel");
            ApiError::not_found("Notification channel")
        })?;

    // Include config but redact tokens/passwords
    let mut config = channel.get_config();
    if let Some(obj) = config.as_object_mut() {
        if obj.contains_key("token") {
            obj.insert("token".to_string(), json!("***REDACTED***"));
        }
        if obj.contains_key("password") {
            obj.insert("password".to_string(), json!("***REDACTED***"));
        }
    }

    Ok(Json(json!({
        "id": channel.id,
        "name": channel.name,
        "channel_type": channel.channel_type_str,
        "description": channel.description,
        "config": config,
        "enabled": channel.enabled,
        "default_priority": channel.default_priority,
        "created_at": channel.created_at,
        "updated_at": channel.updated_at
    })))
}

/// Create channel input
#[derive(Debug, Deserialize)]
struct CreateChannelInput {
    name: String,
    channel_type: String, // "gotify" or "ntfy"
    config: serde_json::Value,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default = "default_priority")]
    default_priority: i32,
}

fn default_enabled() -> bool {
    true
}

fn default_priority() -> i32 {
    3
}

/// Create a new notification channel
#[instrument(skip(state, input))]
async fn create_channel(
    State(state): State<AppState>,
    Json(input): Json<CreateChannelInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(name = %input.name, channel_type = %input.channel_type, "Creating notification channel");

    // Parse and validate channel type
    let channel_type = ChannelType::from_str(&input.channel_type).ok_or_else(|| {
        ApiError::bad_request(format!(
            "Invalid channel type: {}. Valid types: gotify, ntfy, email, slack, discord, webhook",
            input.channel_type
        ))
    })?;

    let create = CreateNotificationChannel {
        name: input.name,
        channel_type,
        config: input.config,
        description: input.description,
        enabled: input.enabled,
        default_priority: input.default_priority,
        metadata: None,
    };

    let id = notifications::create_notification_channel(&state.pool, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create notification channel");
            ApiError::internal_error(format!("Failed to create notification channel: {}", e))
        })?;

    info!(id = id, "Notification channel created successfully");

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Notification channel created successfully"
        })),
    ))
}

/// Update channel input
#[derive(Debug, Deserialize)]
struct UpdateChannelInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    config: Option<serde_json::Value>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    default_priority: Option<i32>,
}

/// Update an existing notification channel
#[instrument(skip(state, input))]
async fn update_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateChannelInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Updating notification channel");

    // Verify exists
    notifications::get_notification_channel(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Notification channel"))?;

    let update = UpdateNotificationChannel {
        name: input.name,
        config: input.config,
        description: input.description,
        enabled: input.enabled,
        default_priority: input.default_priority,
        metadata: None,
    };

    notifications::update_notification_channel(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update notification channel");
            ApiError::internal_error(format!("Failed to update notification channel: {}", e))
        })?;

    info!(id = id, "Notification channel updated successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Notification channel updated successfully"
    })))
}

/// Delete a notification channel
#[instrument(skip(state))]
async fn delete_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Deleting notification channel");

    notifications::delete_notification_channel(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to delete notification channel");
            ApiError::internal_error(format!("Failed to delete notification channel: {}", e))
        })?;

    info!(id = id, "Notification channel deleted successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Notification channel deleted successfully"
    })))
}

/// Test channel input
#[derive(Debug, Deserialize, Default)]
struct TestChannelInput {
    #[serde(default)]
    topic: Option<String>,
}

/// Test a notification channel by sending a test message
#[instrument(skip(state, input))]
async fn test_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<TestChannelInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    use svrctlrs_core::{
        GotifyBackend, NotificationBackend as CoreBackend, NotificationMessage, NtfyBackend,
    };

    info!(id = id, "Testing notification channel");

    let channel = notifications::get_notification_channel(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Notification channel"))?;

    if !channel.enabled {
        return Err(ApiError::bad_request(format!(
            "Channel \"{}\" is disabled",
            channel.name
        )));
    }

    let test_message = NotificationMessage {
        title: format!("Test from {}", channel.name),
        body: "This is a test notification from SvrCtlRS API.".to_string(),
        priority: 3,
        actions: vec![],
    };

    let client = reqwest::Client::new();

    let result = match channel.channel_type_str.as_str() {
        "gotify" => {
            let config = channel.get_config();
            let url = config["url"].as_str().unwrap_or_default();
            let token = config["token"].as_str().unwrap_or_default();

            let gotify = GotifyBackend::with_url_and_key(client, url, token).map_err(|e| {
                ApiError::internal_error(format!("Failed to create Gotify backend: {}", e))
            })?;

            gotify.send(&test_message).await
        }
        "ntfy" => {
            let config = channel.get_config();
            let url = config["url"].as_str().unwrap_or("https://ntfy.sh");
            let topic = input
                .topic
                .as_deref()
                .or_else(|| config["topic"].as_str())
                .unwrap_or_default();

            let mut ntfy = NtfyBackend::with_url_and_topic(client, url, topic).map_err(|e| {
                ApiError::internal_error(format!("Failed to create ntfy backend: {}", e))
            })?;

            // Add authentication if configured
            if let Some(token) = config.get("token").and_then(|t| t.as_str()) {
                if !token.trim().is_empty() {
                    ntfy = ntfy.with_token(token.trim());
                }
            } else if let (Some(username), Some(password)) = (
                config.get("username").and_then(|u| u.as_str()),
                config.get("password").and_then(|p| p.as_str()),
            ) {
                if !username.trim().is_empty() && !password.trim().is_empty() {
                    ntfy = ntfy.with_basic_auth(username.trim(), password.trim());
                }
            }

            ntfy.register_service("test", topic);
            ntfy.send_for_service("test", &test_message).await
        }
        _ => {
            return Err(ApiError::bad_request(format!(
                "Unknown channel type: {}",
                channel.channel_type_str
            )))
        }
    };

    match result {
        Ok(()) => {
            info!(id = id, "Test notification sent successfully");
            Ok(Json(json!({
                "success": true,
                "message": format!("Test notification sent to \"{}\"", channel.name)
            })))
        }
        Err(e) => {
            error!(error = %e, id = id, "Failed to send test notification");
            Err(ApiError::internal_error(format!(
                "Failed to send test notification: {}",
                e
            )))
        }
    }
}

// === Notification Policies ===

/// List all notification policies
#[instrument(skip(state))]
async fn list_policies(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let policies = notifications::list_notification_policies(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list notification policies");
            ApiError::internal_error(format!("Failed to list notification policies: {}", e))
        })?;

    Ok(Json(json!({ "policies": policies })))
}

/// Get notification policy by ID
#[instrument(skip(state))]
async fn get_policy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let policy = notifications::get_notification_policy(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to get notification policy");
            ApiError::not_found("Notification policy")
        })?;

    Ok(Json(json!({ "policy": policy })))
}

/// Create policy input
#[derive(Debug, Deserialize)]
struct CreatePolicyInput {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    on_success: bool,
    #[serde(default = "default_enabled")]
    on_failure: bool,
    #[serde(default = "default_enabled")]
    on_timeout: bool,
    #[serde(default)]
    job_type_filter: Option<Vec<String>>,
    #[serde(default)]
    server_filter: Option<Vec<i64>>,
    #[serde(default)]
    tag_filter: Option<Vec<String>>,
    #[serde(default = "default_min_severity")]
    min_severity: i32,
    #[serde(default)]
    max_per_hour: Option<i32>,
    #[serde(default)]
    title_template: Option<String>,
    #[serde(default)]
    body_template: Option<String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_min_severity() -> i32 {
    1
}

/// Create a new notification policy
#[instrument(skip(state, input))]
async fn create_policy(
    State(state): State<AppState>,
    Json(input): Json<CreatePolicyInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(name = %input.name, "Creating notification policy");

    let create = CreateNotificationPolicy {
        name: input.name,
        description: input.description,
        on_success: input.on_success,
        on_failure: input.on_failure,
        on_timeout: input.on_timeout,
        job_type_filter: input.job_type_filter,
        server_filter: input.server_filter,
        tag_filter: input.tag_filter,
        min_severity: input.min_severity,
        max_per_hour: input.max_per_hour,
        title_template: input.title_template,
        body_template: input.body_template,
        enabled: input.enabled,
        metadata: None,
    };

    let id = notifications::create_notification_policy(&state.pool, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create notification policy");
            ApiError::internal_error(format!("Failed to create notification policy: {}", e))
        })?;

    info!(id = id, "Notification policy created successfully");

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Notification policy created successfully"
        })),
    ))
}

/// Update policy input
#[derive(Debug, Deserialize)]
struct UpdatePolicyInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    on_success: Option<bool>,
    #[serde(default)]
    on_failure: Option<bool>,
    #[serde(default)]
    on_timeout: Option<bool>,
    #[serde(default)]
    job_type_filter: Option<Vec<String>>,
    #[serde(default)]
    server_filter: Option<Vec<i64>>,
    #[serde(default)]
    tag_filter: Option<Vec<String>>,
    #[serde(default)]
    min_severity: Option<i32>,
    #[serde(default)]
    max_per_hour: Option<i32>,
    #[serde(default)]
    title_template: Option<String>,
    #[serde(default)]
    body_template: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
}

/// Update an existing notification policy
#[instrument(skip(state, input))]
async fn update_policy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdatePolicyInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Updating notification policy");

    // Verify exists
    notifications::get_notification_policy(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Notification policy"))?;

    let update = UpdateNotificationPolicy {
        name: input.name,
        description: input.description,
        on_success: input.on_success,
        on_failure: input.on_failure,
        on_timeout: input.on_timeout,
        job_type_filter: input.job_type_filter,
        server_filter: input.server_filter,
        tag_filter: input.tag_filter,
        min_severity: input.min_severity,
        max_per_hour: input.max_per_hour,
        title_template: input.title_template,
        body_template: input.body_template,
        enabled: input.enabled,
        metadata: None,
    };

    notifications::update_notification_policy(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update notification policy");
            ApiError::internal_error(format!("Failed to update notification policy: {}", e))
        })?;

    info!(id = id, "Notification policy updated successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Notification policy updated successfully"
    })))
}

/// Delete a notification policy
#[instrument(skip(state))]
async fn delete_policy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Deleting notification policy");

    notifications::delete_notification_policy(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to delete notification policy");
            ApiError::internal_error(format!("Failed to delete notification policy: {}", e))
        })?;

    info!(id = id, "Notification policy deleted successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Notification policy deleted successfully"
    })))
}

// === Notification History ===

/// List recent notification history
#[instrument(skip(state))]
async fn list_notification_history(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let history = notifications::get_notification_log(&state.pool, 100, 0)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list notification history");
            ApiError::internal_error(format!("Failed to list notification history: {}", e))
        })?;

    Ok(Json(json!({ "notifications": history })))
}

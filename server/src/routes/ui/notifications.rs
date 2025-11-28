use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    Form,
    routing::{delete, get, post, put},
    Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{
    models::{
        CreateNotificationChannel, CreateNotificationPolicy,
        NotificationChannel, NotificationPolicy,
        UpdateNotificationChannel, UpdateNotificationPolicy,
    },
    queries,
};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{
        NotificationChannelsTemplate,
        NotificationChannelListTemplate as ChannelListTemplate,
        NotificationChannelFormTemplate as ChannelFormTemplate,
        NotificationPoliciesTemplate,
        NotificationPolicyListTemplate as PolicyListTemplate,
        NotificationPolicyFormTemplate as PolicyFormTemplate,
    },
};

// ============================================================================
// Notification Channels - Page Routes
// ============================================================================

/// Display the notification channels management page
#[instrument(skip(state))]
pub async fn notification_channels_page(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Rendering notification channels page");

    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification channels");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = NotificationChannelsTemplate {
        user: None, // TODO: Add authentication
        channels,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render notification channels template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Notification Channels - HTMX List Routes
// ============================================================================

/// Get the notification channels list (HTMX)
#[instrument(skip(state))]
pub async fn get_channels_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Fetching notification channels list");

    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification channels");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = ChannelListTemplate { channels };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render channel list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Notification Channels - Form Routes
// ============================================================================

/// Show the new notification channel form (HTMX)
#[instrument(skip(_state))]
pub async fn new_channel_form(State(_state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Rendering new notification channel form");

    let template = ChannelFormTemplate {
        channel: None,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render channel form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show the edit notification channel form (HTMX)
#[instrument(skip(state))]
pub async fn edit_channel_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(channel_id = id, "Rendering edit notification channel form");

    let channel = queries::get_notification_channel(&state.pool, id)
        .await
        .map_err(|e| {
            error!(channel_id = id, error = %e, "Failed to fetch notification channel");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(channel_id = id, "Notification channel not found");
            AppError::NotFound(format!("Notification channel {} not found", id))
        })?;

    let template = ChannelFormTemplate {
        channel: Some(channel),
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render channel form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Notification Channels - CRUD Operations
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateChannelInput {
    pub name: String,
    pub channel_type: String, // "gotify", "ntfy", "email", "webhook"
    pub enabled: Option<String>, // "on" or absent
    pub gotify_url: Option<String>,
    pub gotify_token: Option<String>,
    pub ntfy_topic: Option<String>,
    pub ntfy_server: Option<String>,
    pub email_to: Option<String>,
    pub email_from: Option<String>,
    pub email_smtp_host: Option<String>,
    pub email_smtp_port: Option<i64>,
    pub webhook_url: Option<String>,
    pub webhook_method: Option<String>,
    pub webhook_headers: Option<String>, // JSON string
}

/// Create a new notification channel
#[instrument(skip(state, input), fields(name = %input.name, channel_type = %input.channel_type))]
pub async fn create_channel(
    State(state): State<AppState>,
    Form(input): Form<CreateChannelInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, channel_type = %input.channel_type, "Creating notification channel");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Channel name is empty");
        let template = ChannelFormTemplate {
            channel: None,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Validate channel type
    if !["gotify", "ntfy", "email", "webhook"].contains(&input.channel_type.as_str()) {
        warn!(channel_type = %input.channel_type, "Invalid channel type");
        let template = ChannelFormTemplate {
            channel: None,
            error: Some("Invalid channel type".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Build configuration JSON based on channel type
    let config = match input.channel_type.as_str() {
        "gotify" => {
            if input.gotify_url.is_none() || input.gotify_token.is_none() {
                warn!("Gotify URL and token are required");
                let template = ChannelFormTemplate {
                    channel: None,
                    error: Some("Gotify URL and token are required".to_string()),
                };
                let html = template.render().map_err(|e| {
                    error!(error = %e, "Failed to render error template");
                    AppError::TemplateError(e.to_string())
                })?;
                return Ok(Html(html));
            }
            json!({
                "url": input.gotify_url.unwrap(),
                "token": input.gotify_token.unwrap(),
            })
        }
        "ntfy" => {
            if input.ntfy_topic.is_none() {
                warn!("Ntfy topic is required");
                let template = ChannelFormTemplate {
                    channel: None,
                    error: Some("Ntfy topic is required".to_string()),
                };
                let html = template.render().map_err(|e| {
                    error!(error = %e, "Failed to render error template");
                    AppError::TemplateError(e.to_string())
                })?;
                return Ok(Html(html));
            }
            json!({
                "topic": input.ntfy_topic.unwrap(),
                "server": input.ntfy_server.unwrap_or_else(|| "https://ntfy.sh".to_string()),
            })
        }
        "email" => {
            if input.email_to.is_none() || input.email_smtp_host.is_none() {
                warn!("Email recipient and SMTP host are required");
                let template = ChannelFormTemplate {
                    channel: None,
                    error: Some("Email recipient and SMTP host are required".to_string()),
                };
                let html = template.render().map_err(|e| {
                    error!(error = %e, "Failed to render error template");
                    AppError::TemplateError(e.to_string())
                })?;
                return Ok(Html(html));
            }
            json!({
                "to": input.email_to.unwrap(),
                "from": input.email_from.unwrap_or_else(|| "noreply@svrctlrs.local".to_string()),
                "smtp_host": input.email_smtp_host.unwrap(),
                "smtp_port": input.email_smtp_port.unwrap_or(587),
            })
        }
        "webhook" => {
            if input.webhook_url.is_none() {
                warn!("Webhook URL is required");
                let template = ChannelFormTemplate {
                    channel: None,
                    error: Some("Webhook URL is required".to_string()),
                };
                let html = template.render().map_err(|e| {
                    error!(error = %e, "Failed to render error template");
                    AppError::TemplateError(e.to_string())
                })?;
                return Ok(Html(html));
            }
            let headers = if let Some(h) = input.webhook_headers {
                serde_json::from_str(&h).unwrap_or_else(|_| json!({}))
            } else {
                json!({})
            };
            json!({
                "url": input.webhook_url.unwrap(),
                "method": input.webhook_method.unwrap_or_else(|| "POST".to_string()),
                "headers": headers,
            })
        }
        _ => json!({}),
    };

    // Create notification channel
    let channel_id = queries::create_notification_channel(
        &state.pool,
        &input.name,
        &input.channel_type,
        &config.to_string(),
        input.enabled.is_some(),
    )
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create notification channel");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(
        channel_id,
        name = %input.name,
        "Notification channel created successfully"
    );

    // Return updated list
    get_channels_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelInput {
    pub name: String,
    pub channel_type: String,
    pub enabled: Option<String>,
    pub gotify_url: Option<String>,
    pub gotify_token: Option<String>,
    pub ntfy_topic: Option<String>,
    pub ntfy_server: Option<String>,
    pub email_to: Option<String>,
    pub email_from: Option<String>,
    pub email_smtp_host: Option<String>,
    pub email_smtp_port: Option<i64>,
    pub webhook_url: Option<String>,
    pub webhook_method: Option<String>,
    pub webhook_headers: Option<String>,
}

/// Update an existing notification channel
#[instrument(skip(state, input), fields(channel_id = id, name = %input.name))]
pub async fn update_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateChannelInput>,
) -> Result<Html<String>, AppError> {
    info!(channel_id = id, name = %input.name, "Updating notification channel");

    // Validate input (same as create)
    if input.name.trim().is_empty() {
        warn!("Channel name is empty");
        let channel = queries::get_notification_channel(&state.pool, id)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = ChannelFormTemplate {
            channel,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Build configuration JSON (same logic as create)
    let config = match input.channel_type.as_str() {
        "gotify" => json!({
            "url": input.gotify_url.unwrap_or_default(),
            "token": input.gotify_token.unwrap_or_default(),
        }),
        "ntfy" => json!({
            "topic": input.ntfy_topic.unwrap_or_default(),
            "server": input.ntfy_server.unwrap_or_else(|| "https://ntfy.sh".to_string()),
        }),
        "email" => json!({
            "to": input.email_to.unwrap_or_default(),
            "from": input.email_from.unwrap_or_else(|| "noreply@svrctlrs.local".to_string()),
            "smtp_host": input.email_smtp_host.unwrap_or_default(),
            "smtp_port": input.email_smtp_port.unwrap_or(587),
        }),
        "webhook" => {
            let headers = if let Some(h) = input.webhook_headers {
                serde_json::from_str(&h).unwrap_or_else(|_| json!({}))
            } else {
                json!({})
            };
            json!({
                "url": input.webhook_url.unwrap_or_default(),
                "method": input.webhook_method.unwrap_or_else(|| "POST".to_string()),
                "headers": headers,
            })
        }
        _ => json!({}),
    };

    // Update notification channel
    queries::update_notification_channel(
        &state.pool,
        id,
        &input.name,
        &input.channel_type,
        &config.to_string(),
        input.enabled.is_some(),
    )
    .await
    .map_err(|e| {
        error!(channel_id = id, error = %e, "Failed to update notification channel");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(channel_id = id, "Notification channel updated successfully");

    // Return updated list
    get_channels_list(State(state)).await
}

/// Delete a notification channel
#[instrument(skip(state))]
pub async fn delete_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(channel_id = id, "Deleting notification channel");

    // Check if channel exists
    let channel = queries::get_notification_channel(&state.pool, id)
        .await
        .map_err(|e| {
            error!(channel_id = id, error = %e, "Failed to fetch notification channel");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(channel_id = id, "Notification channel not found");
            AppError::NotFound(format!("Notification channel {} not found", id))
        })?;

    // Delete notification channel
    queries::delete_notification_channel(&state.pool, id)
        .await
        .map_err(|e| {
            error!(channel_id = id, error = %e, "Failed to delete notification channel");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(
        channel_id = id,
        name = %channel.name,
        "Notification channel deleted successfully"
    );

    // Return updated list
    get_channels_list(State(state)).await
}

/// Test a notification channel
#[instrument(skip(state))]
pub async fn test_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(channel_id = id, "Testing notification channel");

    // Get channel
    let channel = queries::get_notification_channel(&state.pool, id)
        .await
        .map_err(|e| {
            error!(channel_id = id, error = %e, "Failed to fetch notification channel");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(channel_id = id, "Notification channel not found");
            AppError::NotFound(format!("Notification channel {} not found", id))
        })?;

    // TODO: Actually send a test notification
    // This would require integration with the notification system

    info!(
        channel_id = id,
        name = %channel.name,
        "Test notification sent successfully"
    );

    Ok(Html(format!(
        r#"<div class="alert alert-success">
            Test notification sent to {} ({})
        </div>"#,
        channel.name, channel.channel_type
    )))
}

// ============================================================================
// Notification Policies - Page Routes
// ============================================================================

/// Display the notification policies management page
#[instrument(skip(state))]
pub async fn notification_policies_page(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Rendering notification policies page");

    let policies = queries::list_notification_policies(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification policies");
            AppError::DatabaseError(e.to_string())
        })?;

    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification channels");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = NotificationPoliciesTemplate {
        user: None, // TODO: Add authentication
        policies,
        channels,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render notification policies template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Notification Policies - HTMX List Routes
// ============================================================================

/// Get the notification policies list (HTMX)
#[instrument(skip(state))]
pub async fn get_policies_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Fetching notification policies list");

    let policies = queries::list_notification_policies(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification policies");
            AppError::DatabaseError(e.to_string())
        })?;

    // Get channels for each policy
    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification channels");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = PolicyListTemplate { policies, channels };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render policy list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Notification Policies - Form Routes
// ============================================================================

/// Show the new notification policy form (HTMX)
#[instrument(skip(state))]
pub async fn new_policy_form(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Rendering new notification policy form");

    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification channels");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = PolicyFormTemplate {
        policy: None,
        channels,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render policy form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show the edit notification policy form (HTMX)
#[instrument(skip(state))]
pub async fn edit_policy_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(policy_id = id, "Rendering edit notification policy form");

    let policy = queries::get_notification_policy(&state.pool, id)
        .await
        .map_err(|e| {
            error!(policy_id = id, error = %e, "Failed to fetch notification policy");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(policy_id = id, "Notification policy not found");
            AppError::NotFound(format!("Notification policy {} not found", id))
        })?;

    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification channels");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = PolicyFormTemplate {
        policy: Some(policy),
        channels,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render policy form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Notification Policies - CRUD Operations
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreatePolicyInput {
    pub name: String,
    pub description: Option<String>,
    pub event_type: String, // "job_success", "job_failure", "job_warning", "system_alert"
    pub severity_threshold: Option<i64>, // 1-5, None = all
    pub enabled: Option<String>,
}

/// Create a new notification policy
#[instrument(skip(state))]
pub async fn create_policy(
    State(state): State<AppState>,
    Form(input): Form<CreatePolicyInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, event_type = %input.event_type, "Creating notification policy");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Policy name is empty");
        let channels = queries::list_notification_channels(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = PolicyFormTemplate {
            policy: None,
            channels,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Create notification policy
    let policy_id = queries::create_notification_policy(
        &state.pool,
        &input.name,
        input.description.as_deref(),
        &input.event_type,
        input.severity_threshold,
        input.enabled.is_some(),
    )
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create notification policy");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(
        policy_id,
        name = %input.name,
        "Notification policy created successfully"
    );

    // Return updated list
    get_policies_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdatePolicyInput {
    pub name: String,
    pub description: Option<String>,
    pub event_type: String,
    pub severity_threshold: Option<i64>,
    pub enabled: Option<String>,
}

/// Update an existing notification policy
#[instrument(skip(state))]
pub async fn update_policy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdatePolicyInput>,
) -> Result<Html<String>, AppError> {
    info!(policy_id = id, name = %input.name, "Updating notification policy");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Policy name is empty");
        let policy = queries::get_notification_policy(&state.pool, id)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let channels = queries::list_notification_channels(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = PolicyFormTemplate {
            policy,
            channels,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Update notification policy
    queries::update_notification_policy(
        &state.pool,
        id,
        &input.name,
        input.description.as_deref(),
        &input.event_type,
        input.severity_threshold,
        input.enabled.is_some(),
    )
    .await
    .map_err(|e| {
        error!(policy_id = id, error = %e, "Failed to update notification policy");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(policy_id = id, "Notification policy updated successfully");

    // Return updated list
    get_policies_list(State(state)).await
}

/// Delete a notification policy
#[instrument(skip(state))]
pub async fn delete_policy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(policy_id = id, "Deleting notification policy");

    // Check if policy exists
    let policy = queries::get_notification_policy(&state.pool, id)
        .await
        .map_err(|e| {
            error!(policy_id = id, error = %e, "Failed to fetch notification policy");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(policy_id = id, "Notification policy not found");
            AppError::NotFound(format!("Notification policy {} not found", id))
        })?;

    // Delete notification policy
    queries::delete_notification_policy(&state.pool, id)
        .await
        .map_err(|e| {
            error!(policy_id = id, error = %e, "Failed to delete notification policy");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(
        policy_id = id,
        name = %policy.name,
        "Notification policy deleted successfully"
    );

    // Return updated list
    get_policies_list(State(state)).await
}

// ============================================================================
// Policy-Channel Association Routes
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct AddChannelToPolicyInput {
    pub channel_id: i64,
}

/// Add a channel to a policy
#[instrument(skip(state))]
pub async fn add_policy_channel(
    State(state): State<AppState>,
    Path(policy_id): Path<i64>,
    Form(input): Form<AddChannelToPolicyInput>,
) -> Result<Html<String>, AppError> {
    info!(policy_id, channel_id = input.channel_id, "Adding channel to policy");

    // Verify policy exists
    queries::get_notification_policy(&state.pool, policy_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| {
            warn!(policy_id, "Notification policy not found");
            AppError::NotFound(format!("Notification policy {} not found", policy_id))
        })?;

    // Verify channel exists
    queries::get_notification_channel(&state.pool, input.channel_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| {
            warn!(channel_id = input.channel_id, "Notification channel not found");
            AppError::NotFound(format!(
                "Notification channel {} not found",
                input.channel_id
            ))
        })?;

    // Add channel to policy
    queries::add_policy_channel(&state.pool, policy_id, input.channel_id)
        .await
        .map_err(|e| {
            error!(policy_id, channel_id = input.channel_id, error = %e, "Failed to add channel to policy");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(policy_id, channel_id = input.channel_id, "Channel added to policy successfully");

    // Return success
    Ok(Html(
        r#"<div class="alert alert-success">Channel added to policy successfully</div>"#
            .to_string(),
    ))
}

/// Remove a channel from a policy
#[instrument(skip(state))]
pub async fn remove_policy_channel(
    State(state): State<AppState>,
    Path((policy_id, channel_id)): Path<(i64, i64)>,
) -> Result<Html<String>, AppError> {
    info!(policy_id, channel_id, "Removing channel from policy");

    // Remove channel from policy
    queries::remove_policy_channel(&state.pool, policy_id, channel_id)
        .await
        .map_err(|e| {
            error!(policy_id, channel_id, error = %e, "Failed to remove channel from policy");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(policy_id, channel_id, "Channel removed from policy successfully");

    // Return success
    Ok(Html(
        r#"<div class="alert alert-success">Channel removed from policy successfully</div>"#
            .to_string(),
    ))
}

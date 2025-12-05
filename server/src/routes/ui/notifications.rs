use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    routing::{get, post, put},
    Router,
};
use axum_extra::extract::Form;
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{
    models::{
        CreateNotificationChannel, CreateNotificationPolicy, UpdateNotificationChannel,
        UpdateNotificationPolicy,
    },
    queries::job_templates,
    queries::job_types,
    queries::notifications as queries,
    queries::servers,
    queries::tags,
    sqlx,
};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{
        NotificationChannelFormTemplate as ChannelFormTemplate,
        NotificationChannelListTemplate as ChannelListTemplate, NotificationChannelsTemplate,
        NotificationLogDisplay, NotificationLogPageTemplate, NotificationPoliciesTemplate,
        NotificationPolicyDisplay, NotificationPolicyFormTemplate as PolicyFormTemplate,
        NotificationPolicyListTemplate as PolicyListTemplate, PolicyChannelAssignment,
    },
};

/// Create notifications router
pub fn routes() -> Router<AppState> {
    Router::new()
        // Notification Channels
        .route(
            "/settings/notifications/channels",
            get(notification_channels_page).post(create_channel),
        )
        .route(
            "/settings/notifications/channels/list",
            get(get_channels_list),
        )
        .route(
            "/settings/notifications/channels/new",
            get(new_channel_form),
        )
        .route(
            "/settings/notifications/channels/{id}/edit",
            get(edit_channel_form),
        )
        .route(
            "/settings/notifications/channels/{id}",
            put(update_channel).delete(delete_channel),
        )
        .route(
            "/settings/notifications/channels/{id}/test",
            post(test_channel),
        )
        // Notification Policies
        .route(
            "/settings/notifications/policies",
            get(notification_policies_page).post(create_policy),
        )
        .route(
            "/settings/notifications/policies/list",
            get(get_policies_list),
        )
        .route("/settings/notifications/policies/new", get(new_policy_form))
        .route(
            "/settings/notifications/policies/{id}/edit",
            get(edit_policy_form),
        )
        .route(
            "/settings/notifications/policies/{id}",
            put(update_policy).delete(delete_policy),
        )
        .route(
            "/settings/notifications/policies/{id}/toggle",
            post(toggle_policy),
        )
        // Notification Log (Audit Trail)
        .route("/settings/notifications/log", get(notification_log_page))
}

// ============================================================================
// Notification Channels
// ============================================================================

/// Display notification channels page
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
        user: None,
        channels: channels.into_iter().map(Into::into).collect(),
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render notification channels template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Get channels list (HTMX)
#[instrument(skip(state))]
pub async fn get_channels_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Fetching notification channels list");

    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification channels");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = ChannelListTemplate {
        channels: channels.into_iter().map(Into::into).collect(),
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render channels list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show new channel form (HTMX)
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

/// Show edit channel form (HTMX)
#[instrument(skip(state))]
pub async fn edit_channel_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(channel_id = id, "Rendering edit notification channel form");

    let channel = queries::get_notification_channel(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(channel_id = id, error = %e, "Notification channel not found");
            AppError::NotFound(format!("Notification channel {} not found", id))
        })?;

    let template = ChannelFormTemplate {
        channel: Some(channel.into()),
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render channel form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelInput {
    pub name: String,
    pub channel_type: String, // gotify, ntfy, email, slack, etc.
    pub description: Option<String>,
    pub config: String, // JSON string
    pub enabled: Option<String>,
}

/// Create a new notification channel
#[instrument(skip(state))]
pub async fn create_channel(
    State(state): State<AppState>,
    Form(input): Form<CreateChannelInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, channel_type = %input.channel_type, "Creating notification channel");

    if input.name.trim().is_empty() {
        return Err(AppError::ValidationError(
            "Channel name is required".to_string(),
        ));
    }

    // Parse config JSON
    let config = serde_json::from_str(&input.config).unwrap_or_else(|_| json!({}));

    // Parse channel type
    let channel_type =
        svrctlrs_database::ChannelType::from_str(&input.channel_type).ok_or_else(|| {
            AppError::ValidationError(format!("Invalid channel type: {}", input.channel_type))
        })?;

    let create_input = CreateNotificationChannel {
        name: input.name.clone(),
        channel_type,
        description: input.description,
        config,
        enabled: input.enabled.is_some(),
        default_priority: 5, // Default medium priority
        metadata: None,
    };

    queries::create_notification_channel(&state.pool, &create_input)
        .await
        .map_err(|e| {
            error!(name = %input.name, error = %e, "Failed to create notification channel");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(name = %input.name, "Notification channel created successfully");

    // Return updated list
    get_channels_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub config: Option<String>,
    pub enabled: Option<String>,
}

/// Update a notification channel
#[instrument(skip(state))]
pub async fn update_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateChannelInput>,
) -> Result<Html<String>, AppError> {
    info!(channel_id = id, "Updating notification channel");

    // Parse config JSON if provided
    let config = input
        .config
        .as_ref()
        .and_then(|c| serde_json::from_str(c).ok());

    let update_input = UpdateNotificationChannel {
        name: input.name,
        description: input.description,
        config,
        enabled: Some(input.enabled.is_some()),
        default_priority: None, // Keep existing priority
        metadata: None,
    };

    queries::update_notification_channel(&state.pool, id, &update_input)
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

    queries::delete_notification_channel(&state.pool, id)
        .await
        .map_err(|e| {
            error!(channel_id = id, error = %e, "Failed to delete notification channel");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(channel_id = id, "Notification channel deleted successfully");

    // Return empty response (HTMX will remove the element)
    Ok(Html(String::new()))
}

// ============================================================================
// Notification Policies
// ============================================================================

/// Display notification policies page
#[instrument(skip(state))]
pub async fn notification_policies_page(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Rendering notification policies page");

    let policies_raw = queries::list_notification_policies(&state.pool)
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

    // Convert policies and populate channel names
    let mut policies: Vec<NotificationPolicyDisplay> = Vec::new();
    for policy in policies_raw {
        let mut display: NotificationPolicyDisplay = policy.clone().into();

        // Query the notification_policy_channels table to get channel_id
        let policy_channel: Option<(i64,)> = sqlx::query_as(
            "SELECT channel_id FROM notification_policy_channels WHERE policy_id = ? LIMIT 1",
        )
        .bind(policy.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch policy channel");
            AppError::DatabaseError(e.to_string())
        })?;

        // Find and set channel name
        if let Some((channel_id,)) = policy_channel {
            if let Some(channel) = channels.iter().find(|c| c.id == channel_id) {
                display.channel_id = channel.id;
                display.channel_name = channel.name.clone();
            }
        }

        policies.push(display);
    }

    let template = NotificationPoliciesTemplate {
        user: None,
        policies,
        channels: channels.into_iter().map(Into::into).collect(),
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render notification policies template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Get policies list (HTMX)
#[instrument(skip(state))]
pub async fn get_policies_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Fetching notification policies list");

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

    // Convert policies and populate policy_channels for each
    let mut policy_displays: Vec<NotificationPolicyDisplay> =
        policies.into_iter().map(Into::into).collect();

    // Populate policy_channels for each policy
    for policy in &mut policy_displays {
        let policy_channels = queries::get_policy_channel_assignments(&state.pool, policy.id)
            .await
            .map_err(|e| {
                warn!(policy_id = policy.id, error = %e, "Failed to fetch policy channels");
                e
            })
            .unwrap_or_default();

        policy.policy_channels = policy_channels
            .into_iter()
            .map(|pc| PolicyChannelAssignment {
                channel_id: pc.channel_id,
                channel_name: pc.channel_name,
                priority_override: pc.priority_override,
            })
            .collect();
    }

    let template = PolicyListTemplate {
        policies: policy_displays,
        channels: channels.into_iter().map(Into::into).collect(),
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render policies list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show new policy form (HTMX)
#[instrument(skip(state))]
pub async fn new_policy_form(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Rendering new notification policy form");

    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let job_types_list = job_types::list_job_types(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let job_templates_list = job_templates::list_job_templates_with_counts(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let servers_list = servers::list_servers_with_details(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let tags_list = tags::get_tags_with_counts(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let template = PolicyFormTemplate {
        policy: None,
        channels: channels.into_iter().map(Into::into).collect(),
        job_types: job_types_list.into_iter().map(Into::into).collect(),
        job_templates: job_templates_list.into_iter().map(Into::into).collect(),
        servers: servers_list.into_iter().map(Into::into).collect(),
        tags: tags_list.into_iter().map(Into::into).collect(),
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render policy form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show edit policy form (HTMX)
#[instrument(skip(state))]
pub async fn edit_policy_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(policy_id = id, "Rendering edit notification policy form");

    let policy = queries::get_notification_policy(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(policy_id = id, error = %e, "Notification policy not found");
            AppError::NotFound(format!("Notification policy {} not found", id))
        })?;

    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let job_types_list = job_types::list_job_types(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let job_templates_list = job_templates::list_job_templates_with_counts(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let servers_list = servers::list_servers_with_details(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let tags_list = tags::get_tags_with_counts(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    // Fetch policy channels for multi-channel support
    let policy_channels = queries::get_policy_channels(&state.pool, id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    // Convert policy to display model and populate policy_channels
    let mut policy_display: NotificationPolicyDisplay = policy.into();
    policy_display.policy_channels = policy_channels
        .into_iter()
        .map(|c| PolicyChannelAssignment {
            channel_id: c.id,
            channel_name: c.name,
            priority_override: None, // TODO: Get from notification_policy_channels
        })
        .collect();

    let template = PolicyFormTemplate {
        policy: Some(policy_display),
        channels: channels.into_iter().map(Into::into).collect(),
        job_types: job_types_list.into_iter().map(Into::into).collect(),
        job_templates: job_templates_list.into_iter().map(Into::into).collect(),
        servers: servers_list.into_iter().map(Into::into).collect(),
        tags: tags_list.into_iter().map(Into::into).collect(),
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render policy form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct CreatePolicyInput {
    pub name: String,
    pub description: Option<String>,
    pub channel_ids: Vec<i64>, // Multiple channel IDs from checkboxes
    pub on_success: Option<String>,
    pub on_failure: Option<String>,
    pub on_timeout: Option<String>,
    pub job_type_filter: Option<String>, // Comma-separated or JSON
    pub server_filter: Option<String>,   // Comma-separated IDs or JSON
    pub tag_filter: Option<String>,      // Comma-separated or JSON
    pub min_severity: Option<i32>,
    pub max_per_hour: Option<i32>,
    pub title_template: Option<String>,
    pub body_template: Option<String>,
    pub enabled: Option<String>,
}

/// Create a new notification policy
#[instrument(skip(state))]
pub async fn create_policy(
    State(state): State<AppState>,
    Form(input): Form<CreatePolicyInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, "Creating notification policy");

    if input.name.trim().is_empty() {
        return Err(AppError::ValidationError(
            "Policy name is required".to_string(),
        ));
    }

    // Parse filters
    let job_type_filter = input.job_type_filter.and_then(|s| {
        if s.trim().is_empty() {
            None
        } else {
            Some(s.split(',').map(|s| s.trim().to_string()).collect())
        }
    });

    let server_filter = input.server_filter.and_then(|s| {
        if s.trim().is_empty() {
            None
        } else {
            Some(s.split(',').filter_map(|s| s.trim().parse().ok()).collect())
        }
    });

    let tag_filter = input.tag_filter.and_then(|s| {
        if s.trim().is_empty() {
            None
        } else {
            Some(s.split(',').map(|s| s.trim().to_string()).collect())
        }
    });

    let create_input = CreateNotificationPolicy {
        name: input.name.clone(),
        description: input.description,
        on_success: input.on_success.is_some(),
        on_failure: input.on_failure.is_some(),
        on_timeout: input.on_timeout.is_some(),
        job_type_filter,
        server_filter,
        tag_filter,
        min_severity: input.min_severity.unwrap_or(1),
        max_per_hour: input.max_per_hour,
        title_template: input.title_template,
        body_template: input.body_template,
        enabled: input.enabled.is_some(),
        metadata: None,
    };

    let policy_id = queries::create_notification_policy(&state.pool, &create_input)
        .await
        .map_err(|e| {
            error!(name = %input.name, error = %e, "Failed to create notification policy");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(name = %input.name, policy_id = policy_id, "Notification policy created successfully");

    // Add policy channels
    for channel_id in &input.channel_ids {
        queries::add_policy_channel(&state.pool, policy_id, *channel_id, None)
            .await
            .map_err(|e| {
                error!(
                    policy_id = policy_id,
                    channel_id = channel_id,
                    error = %e,
                    "Failed to add channel to policy"
                );
                AppError::DatabaseError(e.to_string())
            })?;
    }

    info!(
        policy_id = policy_id,
        channel_count = input.channel_ids.len(),
        "Policy channels linked successfully"
    );

    // Return updated list
    get_policies_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdatePolicyInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub channel_ids: Vec<i64>, // Multiple channel IDs from checkboxes
    pub on_success: Option<String>,
    pub on_failure: Option<String>,
    pub on_timeout: Option<String>,
    pub job_type_filter: Option<String>, // Comma-separated or JSON
    pub server_filter: Option<String>,   // Comma-separated IDs or JSON
    pub tag_filter: Option<String>,      // Comma-separated or JSON
    pub min_severity: Option<i32>,
    pub max_per_hour: Option<i32>,
    pub title_template: Option<String>,
    pub body_template: Option<String>,
    pub enabled: Option<String>,
}

/// Update a notification policy
#[instrument(skip(state))]
pub async fn update_policy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdatePolicyInput>,
) -> Result<Html<String>, AppError> {
    info!(policy_id = id, "Updating notification policy");

    // Parse filters (same logic as create_policy)
    let job_type_filter = input.job_type_filter.map(|s| {
        if s.trim().is_empty() {
            Vec::new() // Empty filter means "no filter" (allow all)
        } else {
            s.split(',').map(|s| s.trim().to_string()).collect()
        }
    });

    let server_filter = input.server_filter.map(|s| {
        if s.trim().is_empty() {
            Vec::new()
        } else {
            s.split(',').filter_map(|s| s.trim().parse().ok()).collect()
        }
    });

    let tag_filter = input.tag_filter.map(|s| {
        if s.trim().is_empty() {
            Vec::new()
        } else {
            s.split(',').map(|s| s.trim().to_string()).collect()
        }
    });

    let update_input = UpdateNotificationPolicy {
        name: input.name,
        description: input.description,
        on_success: Some(input.on_success.is_some()),
        on_failure: Some(input.on_failure.is_some()),
        on_timeout: Some(input.on_timeout.is_some()),
        job_type_filter,
        server_filter,
        tag_filter,
        min_severity: input.min_severity,
        max_per_hour: input.max_per_hour,
        title_template: input.title_template,
        body_template: input.body_template,
        enabled: Some(input.enabled.is_some()),
        metadata: None,
    };

    queries::update_notification_policy(&state.pool, id, &update_input)
        .await
        .map_err(|e| {
            error!(policy_id = id, error = %e, "Failed to update notification policy");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(policy_id = id, "Notification policy updated successfully");

    // Delete all existing policy channels
    sqlx::query("DELETE FROM notification_policy_channels WHERE policy_id = ?")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            error!(policy_id = id, error = %e, "Failed to delete existing policy channels");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(policy_id = id, "Existing policy channels deleted");

    // Add the new policy channels
    for channel_id in &input.channel_ids {
        queries::add_policy_channel(&state.pool, id, *channel_id, None)
            .await
            .map_err(|e| {
                error!(
                    policy_id = id,
                    channel_id = channel_id,
                    error = %e,
                    "Failed to add channel to policy"
                );
                AppError::DatabaseError(e.to_string())
            })?;
    }

    info!(
        policy_id = id,
        channel_count = input.channel_ids.len(),
        "Policy channels updated successfully"
    );

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

    queries::delete_notification_policy(&state.pool, id)
        .await
        .map_err(|e| {
            error!(policy_id = id, error = %e, "Failed to delete notification policy");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(policy_id = id, "Notification policy deleted successfully");

    // Return empty response (HTMX will remove the element)
    Ok(Html(String::new()))
}

/// Toggle a notification policy enabled/disabled
#[instrument(skip(state))]
pub async fn toggle_policy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(policy_id = id, "Toggling notification policy");

    // Get current policy
    let policy = queries::get_notification_policy(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(policy_id = id, error = %e, "Notification policy not found");
            AppError::NotFound(format!("Notification policy {} not found", id))
        })?;

    // Toggle enabled
    let update_input = UpdateNotificationPolicy {
        name: None,
        description: None,
        on_success: None,
        on_failure: None,
        on_timeout: None,
        job_type_filter: None,
        server_filter: None,
        tag_filter: None,
        min_severity: None,
        max_per_hour: None,
        title_template: None,
        body_template: None,
        enabled: Some(!policy.enabled),
        metadata: None,
    };

    queries::update_notification_policy(&state.pool, id, &update_input)
        .await
        .map_err(|e| {
            error!(policy_id = id, error = %e, "Failed to toggle notification policy");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(
        policy_id = id,
        enabled = !policy.enabled,
        "Notification policy toggled successfully"
    );

    // Return updated list (so the entire policy card is refreshed with new state)
    get_policies_list(State(state)).await
}

/// Test a notification channel
#[instrument(skip(state))]
pub async fn test_channel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    use svrctlrs_database::models::notification::ChannelType;

    info!(channel_id = id, "Testing notification channel");

    // Fetch the channel
    let channel = queries::get_notification_channel(&state.pool, id)
        .await
        .map_err(|e| {
            error!(channel_id = id, error = %e, "Failed to fetch notification channel");
            AppError::DatabaseError(e.to_string())
        })?;

    // Parse config
    let config: serde_json::Value = serde_json::from_str(&channel.config).unwrap_or(json!({}));

    // Parse channel type
    let channel_type: ChannelType =
        serde_json::from_str(&format!("\"{}\"", channel.channel_type_str))
            .unwrap_or(ChannelType::Gotify);

    // Send test notification based on channel type
    let result = match channel_type {
        ChannelType::Gotify => {
            // Config may have "url" (from settings form) or "endpoint" (legacy)
            let endpoint = config["url"]
                .as_str()
                .or_else(|| config["endpoint"].as_str())
                .unwrap_or("http://localhost:8080");
            let token = config["token"].as_str().unwrap_or("");

            if token.is_empty() {
                return Ok(Html(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ No token configured</div>"#.to_string(),
                ));
            }

            // Send test message to Gotify
            let client = reqwest::Client::new();
            let url = format!("{}/message", endpoint.trim_end_matches('/'));

            match client
                .post(&url)
                .header("X-Gotify-Key", token)
                .json(&json!({
                    "title": "Test Notification",
                    "message": format!("Test message from SvrCtlRS channel: {}", channel.name),
                    "priority": 5
                }))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => Ok(format!(
                    "✓ Test notification sent successfully to Gotify at {}",
                    endpoint
                )),
                Ok(resp) => Err(format!(
                    "✗ Gotify returned status {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                )),
                Err(e) => Err(format!("✗ Failed to connect to Gotify: {}", e)),
            }
        }
        ChannelType::Ntfy => {
            // Config may have "url" (from settings form) or "endpoint" (legacy)
            let endpoint = config["url"]
                .as_str()
                .or_else(|| config["endpoint"].as_str())
                .unwrap_or("https://ntfy.sh");
            let topic = config["topic"].as_str().unwrap_or("");

            if topic.is_empty() {
                return Ok(Html(
                    r#"<div class="alert alert-error alert-auto-dismiss">✗ No topic configured</div>"#.to_string(),
                ));
            }

            // Build request
            let client = reqwest::Client::new();
            let url = format!("{}/{}", endpoint.trim_end_matches('/'), topic);
            let mut request = client.post(&url);

            // Add authentication if configured
            if let Some(token) = config["token"].as_str() {
                if !token.is_empty() {
                    request = request.header("Authorization", format!("Bearer {}", token));
                }
            } else if let (Some(username), Some(password)) =
                (config["username"].as_str(), config["password"].as_str())
            {
                if !username.is_empty() && !password.is_empty() {
                    request = request.basic_auth(username, Some(password));
                }
            }

            // Send test message
            match request
                .header("Title", "Test Notification")
                .header("Priority", "default")
                .header("Tags", "test,svrctlrs")
                .body(format!(
                    "Test message from SvrCtlRS channel: {}",
                    channel.name
                ))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => Ok(format!(
                    "✓ Test notification sent successfully to ntfy topic: {}",
                    topic
                )),
                Ok(resp) => Err(format!(
                    "✗ ntfy returned status {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                )),
                Err(e) => Err(format!("✗ Failed to connect to ntfy: {}", e)),
            }
        }
        _ => {
            // Other channel types not yet implemented
            Err(format!(
                "✗ Testing not yet implemented for {:?} channels",
                channel_type
            ))
        }
    };

    match result {
        Ok(msg) => Ok(Html(format!(
            r#"<div class="alert alert-success alert-auto-dismiss">{}</div>"#,
            msg
        ))),
        Err(msg) => Ok(Html(format!(
            r#"<div class="alert alert-error alert-auto-dismiss">{}</div>"#,
            msg
        ))),
    }
}

// ============================================================================
// Notification Log (Audit Trail)
// ============================================================================

/// Display notification audit log page
#[instrument(skip(state))]
pub async fn notification_log_page(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Rendering notification audit log page");

    // Fetch recent notification logs (limit 100)
    let logs_raw = queries::get_notification_log(&state.pool, 100, 0)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification logs");
            AppError::DatabaseError(e.to_string())
        })?;

    // Fetch channels for display
    let channels = queries::list_notification_channels(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch notification channels");
            AppError::DatabaseError(e.to_string())
        })?;

    // Convert to display models
    let mut logs: Vec<NotificationLogDisplay> = Vec::new();
    for log in logs_raw {
        // Find channel name
        let channel_name = channels
            .iter()
            .find(|c| c.id == log.channel_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("Channel #{}", log.channel_id));

        // Fetch policy name if available
        let policy_name = if let Some(policy_id) = log.policy_id {
            queries::get_notification_policy(&state.pool, policy_id)
                .await
                .ok()
                .map(|p| p.name)
        } else {
            None
        };

        logs.push(NotificationLogDisplay {
            id: log.id,
            channel_id: log.channel_id,
            channel_name,
            policy_id: log.policy_id,
            policy_name,
            job_run_id: log.job_run_id,
            title: log.title,
            body: log.body,
            priority: log.priority,
            success: log.success,
            error_message: log.error_message,
            retry_count: log.retry_count,
            sent_at: log
                .sent_at
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        });
    }

    let template = NotificationLogPageTemplate {
        user: None,
        logs,
        channels: channels.into_iter().map(Into::into).collect(),
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render notification log template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

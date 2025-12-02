//! Terminal page routes - Multi-terminal server interface
//!
//! Provides a page with multiple terminal panes for simultaneously
//! executing commands across multiple servers.

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use svrctlrs_database::{models::CreateTerminalProfile, queries};

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::*};

/// Create terminal router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/terminal", get(terminal_page))
        // Terminal profile API endpoints
        .route("/terminal/profiles", get(list_profiles).post(create_profile))
        .route(
            "/terminal/profiles/{id}",
            get(get_profile).delete(delete_profile),
        )
}

/// Terminal page handler - provides multi-terminal interface
async fn terminal_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Get all enabled servers for the terminal dropdowns
    let db = state.db().await;
    let servers_with_details = queries::servers::list_servers_with_details(db.pool()).await?;

    // Convert to display format and filter to enabled only
    let servers: Vec<ServerDisplay> = servers_with_details
        .into_iter()
        .filter(|s| s.server.enabled)
        .map(Into::into)
        .collect();

    // Get all tags for server groups feature
    let tags_list = queries::tags::get_tags_with_counts(db.pool()).await?;
    let tags: Vec<TagDisplay> = tags_list.into_iter().map(Into::into).collect();

    // Get terminal profiles
    let profiles_list = queries::terminal_profiles::list_terminal_profiles(db.pool()).await?;
    let profiles: Vec<TerminalProfileDisplay> = profiles_list.into_iter().map(Into::into).collect();

    let template = TerminalPageTemplate {
        user,
        servers,
        tags,
        profiles,
    };
    Ok(Html(template.render()?))
}

// ============================================================================
// Terminal Profile API Endpoints
// ============================================================================

/// Response for profile operations
#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<ProfileData>>,
}

#[derive(Debug, Serialize)]
pub struct ProfileData {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub layout: String,
    pub pane_configs: serde_json::Value,
    pub quick_commands: serde_json::Value,
    pub is_default: bool,
}

impl From<svrctlrs_database::models::TerminalProfile> for ProfileData {
    fn from(p: svrctlrs_database::models::TerminalProfile) -> Self {
        let pane_configs = p
            .pane_configs
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!([]));
        let quick_commands = p
            .quick_commands
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!([]));

        Self {
            id: p.id,
            name: p.name,
            description: p.description,
            layout: p.layout,
            pane_configs,
            quick_commands,
            is_default: p.is_default,
        }
    }
}

/// Input for creating a profile
#[derive(Debug, Deserialize)]
pub struct CreateProfileInput {
    pub name: String,
    pub description: Option<String>,
    pub layout: String,
    pub pane_configs: Option<serde_json::Value>,
    pub quick_commands: Option<serde_json::Value>,
    pub is_default: Option<bool>,
}

/// List all terminal profiles
async fn list_profiles(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let db = state.db().await;
    let profiles_list = queries::terminal_profiles::list_terminal_profiles(db.pool()).await?;
    let profiles: Vec<ProfileData> = profiles_list.into_iter().map(Into::into).collect();

    Ok(Json(ProfileResponse {
        success: true,
        message: "Profiles loaded".to_string(),
        profile: None,
        profiles: Some(profiles),
    }))
}

/// Get a single terminal profile
async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let db = state.db().await;
    match queries::terminal_profiles::get_terminal_profile(db.pool(), id).await {
        Ok(profile) => Ok(Json(ProfileResponse {
            success: true,
            message: "Profile loaded".to_string(),
            profile: Some(profile.into()),
            profiles: None,
        })),
        Err(e) => Ok(Json(ProfileResponse {
            success: false,
            message: format!("Profile not found: {}", e),
            profile: None,
            profiles: None,
        })),
    }
}

/// Create a new terminal profile
async fn create_profile(
    State(state): State<AppState>,
    Json(input): Json<CreateProfileInput>,
) -> Result<impl IntoResponse, AppError> {
    // Validate name
    if input.name.trim().is_empty() {
        return Ok(Json(ProfileResponse {
            success: false,
            message: "Profile name is required".to_string(),
            profile: None,
            profiles: None,
        }));
    }

    let db = state.db().await;

    let create = CreateTerminalProfile {
        name: input.name.clone(),
        description: input.description,
        layout: input.layout,
        pane_configs: input.pane_configs,
        quick_commands: input.quick_commands,
        is_default: input.is_default.unwrap_or(false),
    };

    match queries::terminal_profiles::create_terminal_profile(db.pool(), &create).await {
        Ok(id) => {
            tracing::info!("Created terminal profile '{}' with id {}", input.name, id);
            let profile = queries::terminal_profiles::get_terminal_profile(db.pool(), id).await?;
            Ok(Json(ProfileResponse {
                success: true,
                message: format!("Profile '{}' created successfully", input.name),
                profile: Some(profile.into()),
                profiles: None,
            }))
        }
        Err(e) => {
            tracing::error!("Failed to create terminal profile: {}", e);
            Ok(Json(ProfileResponse {
                success: false,
                message: format!("Failed to create profile: {}", e),
                profile: None,
                profiles: None,
            }))
        }
    }
}

/// Delete a terminal profile
async fn delete_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let db = state.db().await;

    // Get profile name for response message
    let profile_name = queries::terminal_profiles::get_terminal_profile(db.pool(), id)
        .await
        .map(|p| p.name)
        .unwrap_or_else(|_| format!("Profile {}", id));

    match queries::terminal_profiles::delete_terminal_profile(db.pool(), id).await {
        Ok(_) => {
            tracing::info!("Deleted terminal profile '{}'", profile_name);
            Ok(Json(ProfileResponse {
                success: true,
                message: format!("Profile '{}' deleted successfully", profile_name),
                profile: None,
                profiles: None,
            }))
        }
        Err(e) => {
            tracing::error!("Failed to delete terminal profile: {}", e);
            Ok(Json(ProfileResponse {
                success: false,
                message: format!("Failed to delete profile: {}", e),
                profile: None,
                profiles: None,
            }))
        }
    }
}

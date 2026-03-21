//! Terminal profiles API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::queries::terminal_profiles;
use tower_sessions::Session;
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

const USER_ID_KEY: &str = "user_id";

/// Create profiles router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_profiles).post(create_profile))
        .route(
            "/{id}",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
}

/// Extract user_id from session or return 401
async fn get_user_id(session: &Session) -> Result<i64, (StatusCode, Json<ApiError>)> {
    session
        .get::<i64>(USER_ID_KEY)
        .await
        .ok()
        .flatten()
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiError::new("UNAUTHORIZED", "Not authenticated")),
            )
        })
}

/// List profiles for authenticated user
#[instrument(skip(state, session))]
async fn list_profiles(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let user_id = get_user_id(&session).await?;

    let profiles = terminal_profiles::list_terminal_profiles_for_user(&state.pool, user_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list profiles");
            ApiError::internal_error(format!("Failed to list profiles: {}", e))
        })?;

    Ok(Json(json!({ "profiles": profiles })))
}

/// Get profile by ID
#[instrument(skip(state, _session))]
async fn get_profile(
    State(state): State<AppState>,
    _session: Session,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let profile = terminal_profiles::get_terminal_profile(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to get profile");
            ApiError::not_found("Profile")
        })?;

    Ok(Json(json!({ "profile": profile })))
}

/// Create profile input
#[derive(Debug, Deserialize)]
struct CreateProfileInput {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "default_layout")]
    layout: String,
    #[serde(default)]
    pane_configs: Option<serde_json::Value>,
    #[serde(default)]
    is_default: bool,
}

fn default_layout() -> String {
    "single".to_string()
}

/// Create a new profile
#[instrument(skip(state, session, input))]
async fn create_profile(
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<CreateProfileInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let user_id = get_user_id(&session).await?;

    info!(name = %input.name, "Creating terminal profile");

    let create = svrctlrs_database::CreateTerminalProfile {
        name: input.name,
        description: input.description,
        layout: input.layout,
        pane_configs: input.pane_configs,
        quick_commands: None,
        is_default: input.is_default,
    };

    let id = terminal_profiles::create_terminal_profile(&state.pool, &create, Some(user_id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create profile");
            ApiError::internal_error(format!("Failed to create profile: {}", e))
        })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id, "message": "Profile created" })),
    ))
}

/// Update profile input
#[derive(Debug, Deserialize)]
struct UpdateProfileInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    layout: Option<String>,
    #[serde(default)]
    pane_configs: Option<serde_json::Value>,
    #[serde(default)]
    is_default: Option<bool>,
}

/// Update an existing profile
#[instrument(skip(state, _session, input))]
async fn update_profile(
    State(state): State<AppState>,
    _session: Session,
    Path(id): Path<i64>,
    Json(input): Json<UpdateProfileInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let update = svrctlrs_database::UpdateTerminalProfile {
        name: input.name,
        description: input.description,
        layout: input.layout,
        pane_configs: input.pane_configs,
        quick_commands: None,
        is_default: input.is_default,
    };

    terminal_profiles::update_terminal_profile(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update profile");
            ApiError::internal_error(format!("Failed to update profile: {}", e))
        })?;

    Ok(Json(json!({ "id": id, "message": "Profile updated" })))
}

/// Delete a profile
#[instrument(skip(state, _session))]
async fn delete_profile(
    State(state): State<AppState>,
    _session: Session,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    terminal_profiles::delete_terminal_profile(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to delete profile");
            ApiError::internal_error(format!("Failed to delete profile: {}", e))
        })?;

    Ok(Json(json!({ "id": id, "message": "Profile deleted" })))
}

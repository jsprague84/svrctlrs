//! Terminal profiles API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::queries::terminal_profiles;
use tracing::{info, instrument};

use super::ApiError;
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;
use utoipa_axum::router::OpenApiRouter;

/// Create profiles router
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .route("/", get(list_profiles).post(create_profile))
        .route(
            "/{id}",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
}

/// List profiles for authenticated user
#[instrument(skip(state))]
async fn list_profiles(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = current_user.0;

    let profiles = terminal_profiles::list_terminal_profiles_for_user(&state.pool, user_id).await?;

    Ok(Json(json!({ "profiles": profiles })))
}

/// Get profile by ID
#[instrument(skip(state))]
async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
) -> Result<impl IntoResponse, ApiError> {
    let profile = terminal_profiles::get_terminal_profile(&state.pool, id, current_user.0).await?;

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
#[instrument(skip(state, input))]
async fn create_profile(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(input): Json<CreateProfileInput>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = current_user.0;

    info!(name = %input.name, "Creating terminal profile");

    let create = svrctlrs_database::models::CreateTerminalProfile {
        name: input.name,
        description: input.description,
        layout: input.layout,
        pane_configs: input.pane_configs,
        is_default: input.is_default,
    };

    let id =
        terminal_profiles::create_terminal_profile(&state.pool, &create, Some(user_id)).await?;

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
#[instrument(skip(state, input))]
async fn update_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
    Json(input): Json<UpdateProfileInput>,
) -> Result<impl IntoResponse, ApiError> {
    let update = svrctlrs_database::models::UpdateTerminalProfile {
        name: input.name,
        description: input.description,
        layout: input.layout,
        pane_configs: input.pane_configs,
        is_default: input.is_default,
    };

    terminal_profiles::update_terminal_profile(&state.pool, id, current_user.0, &update).await?;

    Ok(Json(json!({ "id": id, "message": "Profile updated" })))
}

/// Delete a profile
#[instrument(skip(state))]
async fn delete_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
) -> Result<impl IntoResponse, ApiError> {
    terminal_profiles::delete_terminal_profile(&state.pool, id, current_user.0).await?;

    Ok(Json(json!({ "id": id, "message": "Profile deleted" })))
}

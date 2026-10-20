//! Terminal profiles API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use svrctlrs_api::profiles::{NewProfile, Profile, ProfileChanges, ProfileList, ProfileOne};
use svrctlrs_api::{ErrorBody, IdMessage};
use svrctlrs_database::queries::terminal_profiles;
use tracing::{info, instrument};

use super::ApiError;
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Create profiles router
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_profiles, create_profile))
        .routes(routes!(get_profile, update_profile, delete_profile))
}

fn profile(p: svrctlrs_database::models::TerminalProfile) -> Profile {
    Profile {
        id: p.id,
        name: p.name,
        description: p.description,
        layout: p.layout,
        pane_configs: p.pane_configs,
        is_default: p.is_default,
        user_id: p.user_id,
        created_at: p.created_at,
        updated_at: p.updated_at,
    }
}

/// The caller's profiles (and shared ones)
#[utoipa::path(get, path = "/", tag = "profiles",
    responses((status = 200, body = ProfileList)))]
#[instrument(skip(state))]
async fn list_profiles(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<ProfileList>, ApiError> {
    let profiles = terminal_profiles::list_terminal_profiles_for_user(&state.pool, current_user.0)
        .await?
        .into_iter()
        .map(profile)
        .collect();
    Ok(Json(ProfileList { profiles }))
}

/// One profile; another user's is 404
#[utoipa::path(get, path = "/{id}", tag = "profiles", params(("id" = i64, Path)),
    responses((status = 200, body = ProfileOne), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
) -> Result<Json<ProfileOne>, ApiError> {
    let p = terminal_profiles::get_terminal_profile(&state.pool, id, current_user.0).await?;
    Ok(Json(ProfileOne {
        profile: profile(p),
    }))
}

/// Create a new profile
#[utoipa::path(post, path = "/", tag = "profiles", request_body = NewProfile,
    responses((status = 201, body = IdMessage), (status = 400, body = ErrorBody)))]
#[instrument(skip(state, input))]
async fn create_profile(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(input): Json<NewProfile>,
) -> Result<(StatusCode, Json<IdMessage>), ApiError> {
    info!(name = %input.name, "Creating terminal profile");
    let create = svrctlrs_database::models::CreateTerminalProfile {
        name: input.name,
        description: input.description,
        layout: input.layout,
        pane_configs: input.pane_configs,
        is_default: input.is_default,
    };
    let id = terminal_profiles::create_terminal_profile(&state.pool, &create, Some(current_user.0))
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(IdMessage::new(id, "Profile created")),
    ))
}

/// Update an existing profile
#[utoipa::path(put, path = "/{id}", tag = "profiles", params(("id" = i64, Path)),
    request_body = ProfileChanges,
    responses((status = 200, body = IdMessage), (status = 404, body = ErrorBody)))]
#[instrument(skip(state, input))]
async fn update_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
    Json(input): Json<ProfileChanges>,
) -> Result<Json<IdMessage>, ApiError> {
    let update = svrctlrs_database::models::UpdateTerminalProfile {
        name: input.name,
        description: input.description,
        layout: input.layout,
        pane_configs: input.pane_configs,
        is_default: input.is_default,
    };
    terminal_profiles::update_terminal_profile(&state.pool, id, current_user.0, &update).await?;
    Ok(Json(IdMessage::new(id, "Profile updated")))
}

/// Delete a profile
#[utoipa::path(delete, path = "/{id}", tag = "profiles", params(("id" = i64, Path)),
    responses((status = 200, body = IdMessage), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn delete_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
) -> Result<Json<IdMessage>, ApiError> {
    terminal_profiles::delete_terminal_profile(&state.pool, id, current_user.0).await?;
    Ok(Json(IdMessage::new(id, "Profile deleted")))
}

//! User accounts API. Every account is an administrator until roles exist (see SECURITY.md),
//! so any user may manage accounts; the only guards are "not yourself" and "not the last one".

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{models::CreateUser, queries::users};
use tracing::{error, info, instrument};

use super::ApiError;
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_users).post(create_user))
        .route("/me/password", put(change_password))
        .route("/{id}", axum::routing::delete(delete_user))
}

fn user_json(u: &svrctlrs_database::models::User, me: i64) -> serde_json::Value {
    json!({
        "id": u.id,
        "username": u.username,
        "is_me": u.id == me,
        "created_at": u.created_at,
        "updated_at": u.updated_at,
    })
}

fn map_err(e: svrctlrs_core::Error, what: &str) -> (StatusCode, Json<ApiError>) {
    match e {
        svrctlrs_core::Error::NotFound(_) => ApiError::not_found("User"),
        svrctlrs_core::Error::ConfigError(msg) => ApiError::bad_request(msg),
        e => {
            error!(error = %e, "Failed to {what}");
            ApiError::internal_error(format!("Failed to {what}: {e}"))
        }
    }
}

#[instrument(skip(state))]
async fn list_users(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let list = users::list_users(&state.pool)
        .await
        .map_err(|e| map_err(e, "list users"))?;
    let list: Vec<_> = list.iter().map(|u| user_json(u, me)).collect();
    Ok(Json(json!({ "users": list })))
}

#[derive(Debug, Deserialize)]
struct CreateUserInput {
    username: String,
    password: String,
}

#[instrument(skip(state, input))]
async fn create_user(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Json(input): Json<CreateUserInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let username = input.username.trim().to_string();
    if username.is_empty() || username.chars().count() > 64 {
        return Err(ApiError::bad_request("username must be 1–64 characters"));
    }
    if users::get_user_by_username(&state.pool, &username)
        .await
        .map_err(|e| map_err(e, "check username"))?
        .is_some()
    {
        return Err(ApiError::conflict("username already exists"));
    }
    let id = users::create_user(
        &state.pool,
        &CreateUser {
            username: username.clone(),
            password: input.password,
        },
    )
    .await
    .map_err(|e| map_err(e, "create user"))?;
    info!(by = me, id, username = %username, "User created");
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id, "message": "User created" })),
    ))
}

#[instrument(skip(state))]
async fn delete_user(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    if id == me {
        return Err(ApiError::bad_request("you cannot delete your own account"));
    }
    let total = users::count_users(&state.pool)
        .await
        .map_err(|e| map_err(e, "count users"))?;
    if total <= 1 {
        return Err(ApiError::bad_request("the last account cannot be deleted"));
    }
    users::delete_user(&state.pool, id)
        .await
        .map_err(|e| map_err(e, "delete user"))?;
    info!(by = me, id, "User deleted");
    Ok(Json(json!({ "id": id, "message": "User deleted" })))
}

#[derive(Debug, Deserialize)]
struct ChangePasswordInput {
    current_password: String,
    new_password: String,
}

/// Change the caller's own password; the current password is required so a hijacked session
/// alone cannot lock the owner out.
#[instrument(skip(state, input))]
async fn change_password(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Json(input): Json<ChangePasswordInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let user = users::get_user(&state.pool, me)
        .await
        .map_err(|e| map_err(e, "load user"))?
        .ok_or_else(|| ApiError::not_found("User"))?;
    let valid = users::verify_password(&input.current_password, &user.password_hash)
        .map_err(|e| map_err(e, "verify password"))?;
    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiError::new(
                "UNAUTHORIZED",
                "current password is incorrect",
            )),
        ));
    }
    users::update_password(&state.pool, me, &input.new_password)
        .await
        .map_err(|e| map_err(e, "update password"))?;
    info!(user_id = me, "Password changed");
    Ok(Json(json!({ "message": "Password changed" })))
}

//! Quick commands API endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::queries::quick_commands;
use tracing::{error, info, instrument};

use super::ApiError;
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;

/// Map a query error: a row that is missing or belongs to someone else is 404.
fn command_error(e: anyhow::Error, action: &str, id: i64) -> (StatusCode, Json<ApiError>) {
    match e.downcast_ref::<svrctlrs_core::Error>() {
        Some(svrctlrs_core::Error::NotFound(_)) => ApiError::not_found("Quick command"),
        _ => {
            error!(error = %e, id = id, "Failed to {action} quick command");
            ApiError::internal_error(format!("Failed to {action} quick command: {e}"))
        }
    }
}

/// Create quick commands router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_commands).post(create_command))
        .route(
            "/{id}",
            get(get_command).put(update_command).delete(delete_command),
        )
}

/// Query parameters for listing commands
#[derive(Debug, Deserialize)]
struct ListQuery {
    server_id: Option<i64>,
}

/// List quick commands for authenticated user
#[instrument(skip(state))]
async fn list_commands(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let user_id = current_user.0;

    let commands = if let Some(server_id) = query.server_id {
        quick_commands::list_quick_commands_for_server(&state.pool, user_id, server_id).await
    } else {
        quick_commands::list_quick_commands(&state.pool, user_id).await
    }
    .map_err(|e| {
        error!(error = %e, "Failed to list quick commands");
        ApiError::internal_error(format!("Failed to list quick commands: {}", e))
    })?;

    Ok(Json(json!({ "commands": commands })))
}

/// Get command by ID
#[instrument(skip(state))]
async fn get_command(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let command = quick_commands::get_quick_command(&state.pool, id, current_user.0)
        .await
        .map_err(|e| command_error(e, "get", id))?;

    Ok(Json(json!({ "command": command })))
}

/// Create command input
#[derive(Debug, Deserialize)]
struct CreateCommandInput {
    name: String,
    command: String,
    #[serde(default)]
    server_id: Option<i64>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    sort_order: Option<i32>,
}

/// Create a new quick command
#[instrument(skip(state, input))]
async fn create_command(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(input): Json<CreateCommandInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let user_id = current_user.0;

    info!(name = %input.name, "Creating quick command");

    let create = svrctlrs_database::CreateQuickCommand {
        name: input.name,
        command: input.command,
        server_id: input.server_id,
        category: input.category,
        sort_order: input.sort_order,
    };

    let id = quick_commands::create_quick_command(&state.pool, user_id, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create quick command");
            ApiError::internal_error(format!("Failed to create quick command: {}", e))
        })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id, "message": "Quick command created" })),
    ))
}

/// Update command input
#[derive(Debug, Deserialize)]
struct UpdateCommandInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    server_id: Option<i64>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    sort_order: Option<i32>,
}

/// Update an existing quick command
#[instrument(skip(state, input))]
async fn update_command(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
    Json(input): Json<UpdateCommandInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let update = svrctlrs_database::UpdateQuickCommand {
        name: input.name,
        command: input.command,
        server_id: input.server_id,
        category: input.category,
        sort_order: input.sort_order,
    };

    quick_commands::update_quick_command(&state.pool, id, current_user.0, &update)
        .await
        .map_err(|e| command_error(e, "update", id))?;

    Ok(Json(
        json!({ "id": id, "message": "Quick command updated" }),
    ))
}

/// Delete a quick command
#[instrument(skip(state))]
async fn delete_command(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    quick_commands::delete_quick_command(&state.pool, id, current_user.0)
        .await
        .map_err(|e| command_error(e, "delete", id))?;

    Ok(Json(
        json!({ "id": id, "message": "Quick command deleted" }),
    ))
}

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
use tower_sessions::Session;
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

const USER_ID_KEY: &str = "user_id";

/// Create quick commands router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_commands).post(create_command))
        .route(
            "/{id}",
            get(get_command).put(update_command).delete(delete_command),
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

/// Query parameters for listing commands
#[derive(Debug, Deserialize)]
struct ListQuery {
    server_id: Option<i64>,
}

/// List quick commands for authenticated user
#[instrument(skip(state, session))]
async fn list_commands(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let user_id = get_user_id(&session).await?;

    let commands = if let Some(server_id) = query.server_id {
        quick_commands::list_quick_commands_for_server(&state.pool, user_id, server_id)
            .await
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
#[instrument(skip(state, _session))]
async fn get_command(
    State(state): State<AppState>,
    _session: Session,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let command = quick_commands::get_quick_command(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to get quick command");
            ApiError::not_found("Quick command")
        })?;

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
#[instrument(skip(state, session, input))]
async fn create_command(
    State(state): State<AppState>,
    session: Session,
    Json(input): Json<CreateCommandInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let user_id = get_user_id(&session).await?;

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
#[instrument(skip(state, _session, input))]
async fn update_command(
    State(state): State<AppState>,
    _session: Session,
    Path(id): Path<i64>,
    Json(input): Json<UpdateCommandInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let update = svrctlrs_database::UpdateQuickCommand {
        name: input.name,
        command: input.command,
        server_id: input.server_id,
        category: input.category,
        sort_order: input.sort_order,
    };

    quick_commands::update_quick_command(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update quick command");
            ApiError::internal_error(format!("Failed to update quick command: {}", e))
        })?;

    Ok(Json(json!({ "id": id, "message": "Quick command updated" })))
}

/// Delete a quick command
#[instrument(skip(state, _session))]
async fn delete_command(
    State(state): State<AppState>,
    _session: Session,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    quick_commands::delete_quick_command(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to delete quick command");
            ApiError::internal_error(format!("Failed to delete quick command: {}", e))
        })?;

    Ok(Json(json!({ "id": id, "message": "Quick command deleted" })))
}

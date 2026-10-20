//! Quick commands API endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use svrctlrs_api::quick_commands::{
    NewQuickCommand, QuickCommand, QuickCommandChanges, QuickCommandList, QuickCommandOne,
    QuickCommandQuery,
};
use svrctlrs_api::{ErrorBody, IdMessage};
use svrctlrs_database::queries::quick_commands;
use tracing::{info, instrument};

use super::ApiError;
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Create quick commands router
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_commands, create_command))
        .routes(routes!(get_command, update_command, delete_command))
}

fn command(c: svrctlrs_database::models::QuickCommand) -> QuickCommand {
    QuickCommand {
        id: c.id,
        user_id: c.user_id,
        name: c.name,
        command: c.command,
        server_id: c.server_id,
        category: c.category,
        sort_order: c.sort_order,
        created_at: c.created_at,
        updated_at: c.updated_at,
    }
}

/// The caller's quick commands
#[utoipa::path(get, path = "/", tag = "quick-commands", params(QuickCommandQuery),
    responses((status = 200, body = QuickCommandList)))]
#[instrument(skip(state))]
async fn list_commands(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<QuickCommandQuery>,
) -> Result<Json<QuickCommandList>, ApiError> {
    let user_id = current_user.0;
    let commands = if let Some(server_id) = query.server_id {
        quick_commands::list_quick_commands_for_server(&state.pool, user_id, server_id).await
    } else {
        quick_commands::list_quick_commands(&state.pool, user_id).await
    }?;
    Ok(Json(QuickCommandList {
        commands: commands.into_iter().map(command).collect(),
    }))
}

/// One quick command; another user's is 404
#[utoipa::path(get, path = "/{id}", tag = "quick-commands", params(("id" = i64, Path)),
    responses((status = 200, body = QuickCommandOne), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn get_command(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
) -> Result<Json<QuickCommandOne>, ApiError> {
    let c = quick_commands::get_quick_command(&state.pool, id, current_user.0).await?;
    Ok(Json(QuickCommandOne {
        command: command(c),
    }))
}

/// Create a new quick command
#[utoipa::path(post, path = "/", tag = "quick-commands", request_body = NewQuickCommand,
    responses((status = 201, body = IdMessage), (status = 400, body = ErrorBody)))]
#[instrument(skip(state, input))]
async fn create_command(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(input): Json<NewQuickCommand>,
) -> Result<(StatusCode, Json<IdMessage>), ApiError> {
    info!(name = %input.name, "Creating quick command");
    let create = svrctlrs_database::models::CreateQuickCommand {
        name: input.name,
        command: input.command,
        server_id: input.server_id,
        category: input.category,
        sort_order: input.sort_order,
    };
    let id = quick_commands::create_quick_command(&state.pool, current_user.0, &create).await?;
    Ok((
        StatusCode::CREATED,
        Json(IdMessage::new(id, "Quick command created")),
    ))
}

/// Update an existing quick command
#[utoipa::path(put, path = "/{id}", tag = "quick-commands", params(("id" = i64, Path)),
    request_body = QuickCommandChanges,
    responses((status = 200, body = IdMessage), (status = 404, body = ErrorBody)))]
#[instrument(skip(state, input))]
async fn update_command(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
    Json(input): Json<QuickCommandChanges>,
) -> Result<Json<IdMessage>, ApiError> {
    let update = svrctlrs_database::models::UpdateQuickCommand {
        name: input.name,
        command: input.command,
        server_id: input.server_id,
        category: input.category,
        sort_order: input.sort_order,
    };
    quick_commands::update_quick_command(&state.pool, id, current_user.0, &update).await?;
    Ok(Json(IdMessage::new(id, "Quick command updated")))
}

/// Delete a quick command
#[utoipa::path(delete, path = "/{id}", tag = "quick-commands", params(("id" = i64, Path)),
    responses((status = 200, body = IdMessage), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn delete_command(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    current_user: CurrentUser,
) -> Result<Json<IdMessage>, ApiError> {
    quick_commands::delete_quick_command(&state.pool, id, current_user.0).await?;
    Ok(Json(IdMessage::new(id, "Quick command deleted")))
}

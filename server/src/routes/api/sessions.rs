//! Live interactive sessions of the calling user: list them (to offer "resume" after a
//! reload) and close one for good.

use axum::{
    extract::{Path, State},
    Json,
};
use svrctlrs_api::sessions::{SessionClosed, SessionList, SessionSummary};
use tracing::instrument;

use super::ApiError;
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_sessions))
        .routes(routes!(close_session))
}

/// The caller's live shells, attached or waiting to be resumed.
#[utoipa::path(get, path = "/", tag = "sessions",
    responses((status = 200, body = SessionList)))]
#[instrument(skip(state))]
async fn list_sessions(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
) -> Json<SessionList> {
    let sessions = state
        .sessions
        .list_for_user(me)
        .await
        .into_iter()
        .map(|s| SessionSummary {
            id: s.id,
            server_id: s.server_id,
            server_name: s.server_name,
            created_at: s.created_at,
            attachments: s.attachments,
            latest_seq: s.latest_seq,
            recorded: s.recorded,
        })
        .collect();
    Json(SessionList {
        sessions,
        detach_ttl_secs: state.sessions.detach_ttl().as_secs(),
    })
}

/// End the shell behind a session, attached or not.
#[utoipa::path(delete, path = "/{id}", tag = "sessions",
    params(("id" = String, Path, description = "Session id")),
    responses(
        (status = 200, body = SessionClosed),
        (status = 404, body = svrctlrs_api::ErrorBody),
    ))]
#[instrument(skip(state))]
async fn close_session(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<SessionClosed>, ApiError> {
    if !state.sessions.close_session(&id, me).await {
        return Err(ApiError::not_found("Session"));
    }
    Ok(Json(SessionClosed {
        id,
        message: "Session closed".to_string(),
    }))
}

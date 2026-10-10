//! Live interactive sessions of the calling user: list them (to offer "resume" after a
//! reload) and close one for good.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use tracing::instrument;

use super::ApiError;
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_sessions))
        .route("/{id}", axum::routing::delete(close_session))
}

#[instrument(skip(state))]
async fn list_sessions(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
) -> impl IntoResponse {
    let sessions = state.sessions.list_for_user(me).await;
    Json(json!({
        "sessions": sessions,
        "detach_ttl_secs": state.sessions.detach_ttl().as_secs(),
    }))
}

/// End the shell behind a session, attached or not.
#[instrument(skip(state))]
async fn close_session(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let Some(live) = state.sessions.get(&id, me).await else {
        return Err(ApiError::not_found("Session"));
    };
    live.close().await;
    live.mark_closed("Session closed");
    state.sessions.remove(&id).await;
    Ok(Json(json!({ "id": id, "message": "Session closed" })))
}

//! Audit log API

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::queries::audit;
use tracing::{error, instrument};

use super::ApiError;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(list_audit))
}

#[derive(Debug, Deserialize)]
struct AuditQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    before: Option<i64>,
}

fn default_limit() -> i64 {
    100
}

/// Newest first; `next_before` is the cursor for the following page, absent on the last one.
#[instrument(skip(state))]
async fn list_audit(
    State(state): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let limit = q.limit.clamp(1, 500);
    let entries = audit::list(&state.pool, limit, q.before)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list audit log");
            ApiError::internal_error(format!("Failed to list audit log: {e}"))
        })?;
    let next_before = if entries.len() as i64 == limit {
        entries.last().map(|e| e.id)
    } else {
        None
    };
    Ok(Json(
        json!({ "entries": entries, "next_before": next_before }),
    ))
}

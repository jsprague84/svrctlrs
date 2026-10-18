//! Audit log API

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::queries::audit;
use tracing::instrument;

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
    /// Comma-separated action prefixes, e.g. `login,session.`
    action: Option<String>,
    user_id: Option<i64>,
    /// Free-text search over action, target, details, user and address
    q: Option<String>,
}

fn default_limit() -> i64 {
    100
}

/// Newest first; `next_before` is the cursor for the following page, absent on the last one.
#[instrument(skip(state))]
async fn list_audit(
    State(state): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = q.limit.clamp(1, 500);
    let filter = audit::AuditFilter {
        action_prefixes: q
            .action
            .as_deref()
            .map(|a| a.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default(),
        user_id: q.user_id,
        text: q.q,
    };
    let entries = audit::list_filtered(&state.pool, limit, q.before, &filter).await?;
    let next_before = if entries.len() as i64 == limit {
        entries.last().map(|e| e.id)
    } else {
        None
    };
    Ok(Json(
        json!({ "entries": entries, "next_before": next_before }),
    ))
}

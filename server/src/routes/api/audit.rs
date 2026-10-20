//! Audit log API

use axum::{
    extract::{Query, State},
    Json,
};
use svrctlrs_api::audit::{AuditEntry, AuditPage, AuditQuery};
use svrctlrs_database::queries::audit;
use tracing::instrument;

use super::ApiError;
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(list_audit))
}

fn entry(e: svrctlrs_database::models::AuditEntry) -> AuditEntry {
    AuditEntry {
        id: e.id,
        at: e.at,
        user_id: e.user_id,
        username: e.username,
        client_ip: e.client_ip,
        action: e.action,
        target_type: e.target_type,
        target_id: e.target_id,
        target_name: e.target_name,
        details: e.details,
        token_id: e.token_id,
        token_name: e.token_name,
    }
}

/// Newest first; `next_before` is the cursor for the following page, null on the last one.
#[utoipa::path(get, path = "/", tag = "audit", params(AuditQuery),
    responses((status = 200, body = AuditPage)))]
#[instrument(skip(state))]
async fn list_audit(
    State(state): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> Result<Json<AuditPage>, ApiError> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
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
    Ok(Json(AuditPage {
        entries: entries.into_iter().map(entry).collect(),
        next_before,
    }))
}

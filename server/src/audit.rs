//! Fire-and-forget audit recording. A failed write is logged and never fails the request.
//!
//! A request that came in with an API token runs inside [`with_token`] (the auth middleware
//! sets it up), and every entry recorded while it runs names that token, so no handler has
//! to remember to. Work that leaves the request's task (a WebSocket after its upgrade) carries
//! the token over with [`current_token`] and [`with_token`].

use std::future::Future;

use svrctlrs_database::{models::NewAuditEntry, queries::audit, SqlxPool, SqlxSqlite};
use tracing::error;

use crate::api_tokens::TokenAuth;

tokio::task_local! {
    static TOKEN: Option<TokenAuth>;
}

/// Run `f` with `token` as the API token behind everything it records.
pub async fn with_token<F: Future>(token: Option<TokenAuth>, f: F) -> F::Output {
    TOKEN.scope(token, f).await
}

/// The API token behind the current request, if any.
pub fn current_token() -> Option<TokenAuth> {
    TOKEN.try_with(Clone::clone).ok().flatten()
}

/// Record an entry without waiting for the insert.
pub fn record(pool: &SqlxPool<SqlxSqlite>, entry: NewAuditEntry) {
    let entry = match current_token() {
        Some(t) if entry.token_id.is_none() => entry.token(t.id, &t.name),
        _ => entry,
    };
    let pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = audit::record(&pool, &entry).await {
            error!(error = %e, action = %entry.action, "Failed to write audit entry");
        }
    });
}

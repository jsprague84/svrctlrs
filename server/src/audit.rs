//! Fire-and-forget audit recording. A failed write is logged and never fails the request.

use svrctlrs_database::{models::NewAuditEntry, queries::audit, SqlxPool, SqlxSqlite};
use tracing::error;

/// Record an entry without waiting for the insert.
pub fn record(pool: &SqlxPool<SqlxSqlite>, entry: NewAuditEntry) {
    let pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = audit::record(&pool, &entry).await {
            error!(error = %e, action = %entry.action, "Failed to write audit entry");
        }
    });
}

/// Commands are recorded for the audit trail; keep them to a sane size.
pub fn truncate_for_log(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}

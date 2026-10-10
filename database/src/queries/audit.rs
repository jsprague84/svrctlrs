//! Audit log queries

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{AuditEntry, NewAuditEntry};

/// Append one entry. Returns its id.
#[instrument(skip(pool, entry), fields(action = %entry.action))]
pub async fn record(pool: &Pool<Sqlite>, entry: &NewAuditEntry) -> Result<i64> {
    let details = entry
        .details
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| Error::Other(format!("audit details: {e}")))?;
    let result = sqlx::query(
        r#"
        INSERT INTO audit_log (user_id, username, client_ip, action, target_type, target_id,
                               target_name, details)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(entry.user_id)
    .bind(&entry.username)
    .bind(&entry.client_ip)
    .bind(&entry.action)
    .bind(&entry.target_type)
    .bind(entry.target_id)
    .bind(&entry.target_name)
    .bind(details)
    .execute(pool)
    .await
    .context("Failed to record audit entry")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;
    Ok(result.last_insert_rowid())
}

/// What to show: all three filters are ANDed; an empty filter matches everything.
#[derive(Debug, Default, Clone)]
pub struct AuditFilter {
    /// Keep actions starting with any of these (`login`, `session.`, `server.` …)
    pub action_prefixes: Vec<String>,
    pub user_id: Option<i64>,
    /// Case-insensitive substring of the action, target name, details, username or address
    pub text: Option<String>,
}

/// Newest entries first, `limit` at a time; pass the smallest `id` of the previous page as
/// `before` to continue. The username shown is the account's current one while it exists.
#[instrument(skip(pool))]
pub async fn list(pool: &Pool<Sqlite>, limit: i64, before: Option<i64>) -> Result<Vec<AuditEntry>> {
    list_filtered(pool, limit, before, &AuditFilter::default()).await
}

/// [`list`] restricted by an [`AuditFilter`].
#[instrument(skip(pool))]
pub async fn list_filtered(
    pool: &Pool<Sqlite>,
    limit: i64,
    before: Option<i64>,
    filter: &AuditFilter,
) -> Result<Vec<AuditEntry>> {
    let mut qb: sqlx::QueryBuilder<Sqlite> = sqlx::QueryBuilder::new(
        "SELECT a.id, a.at, a.user_id, COALESCE(u.username, a.username) AS username, a.client_ip, \
         a.action, a.target_type, a.target_id, a.target_name, a.details \
         FROM audit_log a LEFT JOIN users u ON u.id = a.user_id WHERE 1 = 1",
    );
    if let Some(before) = before {
        qb.push(" AND a.id < ").push_bind(before);
    }
    if let Some(uid) = filter.user_id {
        qb.push(" AND a.user_id = ").push_bind(uid);
    }
    let prefixes: Vec<&String> = filter
        .action_prefixes
        .iter()
        .filter(|p| !p.is_empty())
        .collect();
    if !prefixes.is_empty() {
        qb.push(" AND (");
        for (i, p) in prefixes.iter().enumerate() {
            if i > 0 {
                qb.push(" OR ");
            }
            // A prefix matches the action itself and dotted sub-actions
            qb.push("a.action = ")
                .push_bind((*p).clone())
                .push(" OR a.action LIKE ")
                .push_bind(format!("{}%", escape_like(p)))
                .push(" ESCAPE '\\'");
        }
        qb.push(")");
    }
    if let Some(text) = filter
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let pat = format!("%{}%", escape_like(text));
        qb.push(" AND (");
        for (i, col) in [
            "a.action",
            "a.target_name",
            "a.details",
            "COALESCE(u.username, a.username)",
            "a.client_ip",
        ]
        .iter()
        .enumerate()
        {
            if i > 0 {
                qb.push(" OR ");
            }
            qb.push(*col)
                .push(" LIKE ")
                .push_bind(pat.clone())
                .push(" ESCAPE '\\'");
        }
        qb.push(")");
    }
    qb.push(" ORDER BY a.id DESC LIMIT ")
        .push_bind(limit.clamp(1, 500));
    qb.build_query_as::<AuditEntry>()
        .fetch_all(pool)
        .await
        .context("Failed to list audit entries")
        .map_err(|e| Error::DatabaseError(e.to_string()))
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// The most recent successful login per user id.
#[instrument(skip(pool))]
pub async fn last_logins(pool: &Pool<Sqlite>) -> Result<Vec<(i64, chrono::DateTime<chrono::Utc>)>> {
    sqlx::query_as::<_, (i64, chrono::DateTime<chrono::Utc>)>(
        "SELECT user_id, MAX(at) FROM audit_log WHERE action = 'login' AND user_id IS NOT NULL \
         GROUP BY user_id",
    )
    .fetch_all(pool)
    .await
    .context("Failed to read last logins")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Delete entries older than `days`. Returns how many were removed.
#[instrument(skip(pool))]
pub async fn prune_older_than_days(pool: &Pool<Sqlite>, days: i64) -> Result<u64> {
    let result = sqlx::query("DELETE FROM audit_log WHERE at < datetime('now', ? || ' days')")
        .bind(format!("-{days}"))
        .execute(pool)
        .await
        .context("Failed to prune audit log")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::query("INSERT INTO users (username, password_hash) VALUES ('alice', 'x')")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn records_lists_newest_first_and_pages() {
        let pool = setup().await;
        record(
            &pool,
            &NewAuditEntry::new("login.failed")
                .username("ghost")
                .client_ip(Some("10.0.0.9".into())),
        )
        .await
        .unwrap();
        record(
            &pool,
            &NewAuditEntry::new("server.created")
                .user(1)
                .target("server", 7, "web-01")
                .details(serde_json::json!({"port": 22})),
        )
        .await
        .unwrap();
        record(&pool, &NewAuditEntry::new("logout").user(1))
            .await
            .unwrap();

        let page = list(&pool, 2, None).await.unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].action, "logout");
        assert_eq!(
            page[0].username.as_deref(),
            Some("alice"),
            "joined from users"
        );
        assert_eq!(page[1].action, "server.created");
        assert_eq!(page[1].target_name.as_deref(), Some("web-01"));
        assert_eq!(page[1].details.as_deref(), Some(r#"{"port":22}"#));

        let rest = list(&pool, 2, Some(page[1].id)).await.unwrap();
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].action, "login.failed");
        assert_eq!(
            rest[0].username.as_deref(),
            Some("ghost"),
            "kept when there is no user"
        );
        assert_eq!(rest[0].client_ip.as_deref(), Some("10.0.0.9"));
    }

    #[tokio::test]
    async fn prune_removes_only_old_rows() {
        let pool = setup().await;
        record(&pool, &NewAuditEntry::new("recent")).await.unwrap();
        sqlx::query(
            "INSERT INTO audit_log (at, action) VALUES (datetime('now', '-400 days'), 'ancient')",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(prune_older_than_days(&pool, 180).await.unwrap(), 1);
        let left = list(&pool, 10, None).await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].action, "recent");
    }
}

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
        .map_err(|e| Error::InvalidInput(format!("audit details: {e}")))?;
    let result = sqlx::query(
        r#"
        INSERT INTO audit_log (user_id, username, client_ip, action, target_type, target_id,
                               target_name, details, token_id, token_name)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
    .bind(entry.token_id)
    .bind(&entry.token_name)
    .execute(pool)
    .await
    .context("Failed to record audit entry")
    .map_err(Error::Database)?;
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
         a.action, a.target_type, a.target_id, a.target_name, a.details, a.token_id, \
         a.token_name FROM audit_log a LEFT JOIN users u ON u.id = a.user_id WHERE 1 = 1",
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
            "a.token_name",
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
        .map_err(Error::Database)
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
    .map_err(Error::Database)
}

/// The servers `user_id` opened shells on most recently, newest first, as `(id, name)`.
/// Only servers that still exist and are enabled. Feeds the installed app's home-screen
/// shortcuts, so it is per user like everything else a user sees.
#[instrument(skip(pool))]
pub async fn recent_servers(
    pool: &Pool<Sqlite>,
    user_id: i64,
    limit: i64,
) -> Result<Vec<(i64, String)>> {
    sqlx::query_as::<_, (i64, String)>(
        "SELECT s.id, s.name FROM audit_log a JOIN servers s ON s.id = a.target_id \
         WHERE a.action = 'session.pty_opened' AND a.target_type = 'server' \
           AND a.user_id = ? AND s.enabled = 1 \
         GROUP BY s.id ORDER BY MAX(a.id) DESC LIMIT ?",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("Failed to read recent servers")
    .map_err(Error::Database)
}

/// Delete entries older than `days`. Returns how many were removed.
#[instrument(skip(pool))]
pub async fn prune_older_than_days(pool: &Pool<Sqlite>, days: i64) -> Result<u64> {
    let result = sqlx::query("DELETE FROM audit_log WHERE at < datetime('now', ? || ' days')")
        .bind(format!("-{days}"))
        .execute(pool)
        .await
        .context("Failed to prune audit log")
        .map_err(Error::Database)?;
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
    async fn recent_servers_are_per_user_newest_first_and_enabled_only() {
        let pool = setup().await;
        sqlx::query("INSERT INTO users (username, password_hash) VALUES ('bob', 'x')")
            .execute(&pool)
            .await
            .unwrap();
        for (name, enabled) in [("web", 1), ("db", 1), ("old", 0)] {
            sqlx::query("INSERT INTO servers (name, hostname, enabled) VALUES (?, '10.0.0.1', ?)")
                .bind(name)
                .bind(enabled)
                .execute(&pool)
                .await
                .unwrap();
        }
        let ids: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM servers ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        let (web, db, old) = (ids[0].0, ids[1].0, ids[2].0);
        let opened = |user: i64, id: i64, name: &str| {
            NewAuditEntry::new("session.pty_opened")
                .user(user)
                .target("server", id, name)
        };
        for entry in [
            opened(1, web, "web"),
            opened(1, db, "db"),
            opened(1, old, "old"),
            opened(1, web, "web"),
            opened(2, db, "db"),
            // Not a shell: does not count
            NewAuditEntry::new("server.updated")
                .user(1)
                .target("server", db, "db"),
        ] {
            record(&pool, &entry).await.unwrap();
        }

        let alice = recent_servers(&pool, 1, 3).await.unwrap();
        assert_eq!(
            alice,
            vec![(web, "web".to_string()), (db, "db".to_string())],
            "newest first, the disabled one left out"
        );
        assert_eq!(recent_servers(&pool, 1, 1).await.unwrap().len(), 1);
        assert_eq!(
            recent_servers(&pool, 2, 3).await.unwrap(),
            vec![(db, "db".to_string())],
            "only bob's own shells"
        );
        assert!(recent_servers(&pool, 99, 3).await.unwrap().is_empty());
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

//! Approval requests. Reads and decisions are scoped by `user_id`; the decision is one
//! conditional UPDATE, so a request is decided at most once and never after it expired.

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{Approval, NewApproval};

const COLUMNS: &str = "id, user_id, token_id, token_name, action, method, path, target_type, \
                       target_id, target_name, client_ip, url, status, created_at, expires_at, \
                       decided_at, decided_by, result_status, result";

/// Store a new pending request that expires `ttl_minutes` from now.
#[instrument(skip(pool, a), fields(action = %a.action, token_id = a.token_id))]
pub async fn create(pool: &Pool<Sqlite>, a: &NewApproval) -> Result<Approval> {
    sqlx::query_as::<_, Approval>(&format!(
        r#"
        INSERT INTO approvals (id, user_id, token_id, token_name, action, method, path,
                               target_type, target_id, target_name, client_ip, url, expires_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now', '+' || ? || ' minutes'))
        RETURNING {COLUMNS}
        "#
    ))
    .bind(&a.id)
    .bind(a.user_id)
    .bind(a.token_id)
    .bind(&a.token_name)
    .bind(&a.action)
    .bind(&a.method)
    .bind(&a.path)
    .bind(&a.target_type)
    .bind(a.target_id)
    .bind(&a.target_name)
    .bind(&a.client_ip)
    .bind(&a.url)
    .bind(a.ttl_minutes)
    .fetch_one(pool)
    .await
    .context("Failed to create approval request")
    .map_err(Error::Database)
}

/// A request from this token for this call that is still waiting, so asking again does not
/// send a second notification.
#[instrument(skip(pool))]
pub async fn find_waiting(
    pool: &Pool<Sqlite>,
    token_id: i64,
    method: &str,
    path: &str,
) -> Result<Option<Approval>> {
    sqlx::query_as::<_, Approval>(&format!(
        "SELECT {COLUMNS} FROM approvals WHERE token_id = ? AND method = ? AND path = ? \
         AND status = 'pending' AND expires_at > CURRENT_TIMESTAMP \
         ORDER BY created_at DESC LIMIT 1"
    ))
    .bind(token_id)
    .bind(method)
    .bind(path)
    .fetch_optional(pool)
    .await
    .context("Failed to look up approval requests")
    .map_err(Error::Database)
}

/// One of the user's requests; another user's or a missing one is [`Error::NotFound`].
#[instrument(skip(pool))]
pub async fn get(pool: &Pool<Sqlite>, user_id: i64, id: &str) -> Result<Approval> {
    sqlx::query_as::<_, Approval>(&format!(
        "SELECT {COLUMNS} FROM approvals WHERE id = ? AND user_id = ?"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get approval request")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound("approval request".to_string()))
}

/// The user's requests, newest first.
#[instrument(skip(pool))]
pub async fn list_for_user(pool: &Pool<Sqlite>, user_id: i64, limit: i64) -> Result<Vec<Approval>> {
    sqlx::query_as::<_, Approval>(&format!(
        "SELECT {COLUMNS} FROM approvals WHERE user_id = ? ORDER BY created_at DESC, rowid DESC \
         LIMIT ?"
    ))
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("Failed to list approval requests")
    .map_err(Error::Database)
}

/// Approve or deny one of the user's requests if it is still pending and unexpired; None
/// otherwise (already decided, expired, or not theirs).
#[instrument(skip(pool))]
pub async fn decide(
    pool: &Pool<Sqlite>,
    user_id: i64,
    id: &str,
    approve: bool,
) -> Result<Option<Approval>> {
    sqlx::query_as::<_, Approval>(&format!(
        "UPDATE approvals SET status = ?, decided_at = CURRENT_TIMESTAMP, decided_by = ? \
         WHERE id = ? AND user_id = ? AND status = 'pending' \
         AND expires_at > CURRENT_TIMESTAMP RETURNING {COLUMNS}"
    ))
    .bind(if approve { "approved" } else { "denied" })
    .bind(user_id)
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to decide approval request")
    .map_err(Error::Database)
}

/// Store what the approved call answered.
#[instrument(skip(pool, result))]
pub async fn set_result(
    pool: &Pool<Sqlite>,
    id: &str,
    status: u16,
    result: &str,
) -> Result<Approval> {
    sqlx::query_as::<_, Approval>(&format!(
        "UPDATE approvals SET result_status = ?, result = ? WHERE id = ? RETURNING {COLUMNS}"
    ))
    .bind(i64::from(status))
    .bind(result)
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to store approval result")
    .map_err(Error::Database)
}

/// How many of the user's requests are waiting now.
#[instrument(skip(pool))]
pub async fn count_waiting(pool: &Pool<Sqlite>, user_id: i64) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM approvals WHERE user_id = ? AND status = 'pending' \
         AND expires_at > CURRENT_TIMESTAMP",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Failed to count approval requests")
    .map_err(Error::Database)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn db() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        for name in ["ann", "bob"] {
            sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, 'x')")
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query(
            "INSERT INTO api_tokens (user_id, name, prefix, token_hash, preset) \
             VALUES (1, 'hermes', 'svr_abcdef', 'h', 'servers_keys')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn new(id: &str, path: &str) -> NewApproval {
        NewApproval {
            id: id.to_string(),
            user_id: 1,
            token_id: 1,
            token_name: "hermes".into(),
            action: "server.delete".into(),
            method: "DELETE".into(),
            path: path.into(),
            target_type: "server".into(),
            target_id: 5,
            target_name: Some("web-01".into()),
            client_ip: Some("10.0.0.9".into()),
            url: format!("https://svrctlrs.example/approvals?id={id}"),
            ttl_minutes: 60,
        }
    }

    #[tokio::test]
    async fn a_request_is_decided_once_by_its_owner() {
        let pool = db().await;
        let a = create(&pool, &new("a1", "/api/v1/servers/5"))
            .await
            .unwrap();
        let now = chrono::Utc::now();
        assert_eq!(a.state_at(now), "pending");
        assert_eq!(
            find_waiting(&pool, 1, "DELETE", "/api/v1/servers/5")
                .await
                .unwrap()
                .map(|x| x.id),
            Some("a1".into())
        );
        assert_eq!(count_waiting(&pool, 1).await.unwrap(), 1);

        assert!(matches!(get(&pool, 2, "a1").await, Err(Error::NotFound(_))));
        assert!(decide(&pool, 2, "a1", true).await.unwrap().is_none());

        let approved = decide(&pool, 1, "a1", true).await.unwrap().unwrap();
        assert_eq!(approved.state_at(now), "approved");
        assert!(
            decide(&pool, 1, "a1", false).await.unwrap().is_none(),
            "once"
        );
        let done = set_result(&pool, "a1", 200, "{}").await.unwrap();
        assert_eq!(done.state_at(now), "done");
        let failed = set_result(&pool, "a1", 409, "{}").await.unwrap();
        assert_eq!(failed.state_at(now), "failed");
        assert_eq!(count_waiting(&pool, 1).await.unwrap(), 0);

        create(&pool, &new("a2", "/api/v1/servers/6"))
            .await
            .unwrap();
        let denied = decide(&pool, 1, "a2", false).await.unwrap().unwrap();
        assert_eq!(denied.state_at(now), "denied");
        assert_eq!(list_for_user(&pool, 1, 10).await.unwrap().len(), 2);
        assert!(list_for_user(&pool, 2, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_expired_request_cannot_be_approved() {
        let pool = db().await;
        create(&pool, &new("old", "/api/v1/servers/5"))
            .await
            .unwrap();
        sqlx::query("UPDATE approvals SET expires_at = datetime('now', '-1 minute')")
            .execute(&pool)
            .await
            .unwrap();
        let a = get(&pool, 1, "old").await.unwrap();
        assert_eq!(a.state_at(chrono::Utc::now()), "expired");
        assert!(decide(&pool, 1, "old", true).await.unwrap().is_none());
        assert!(find_waiting(&pool, 1, "DELETE", "/api/v1/servers/5")
            .await
            .unwrap()
            .is_none());
    }
}

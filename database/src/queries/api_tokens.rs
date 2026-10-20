//! API tokens. Everything a user does with their tokens is scoped by `user_id`; only the
//! authentication lookup ([`find_by_hash`]) goes by the secret's hash alone.

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{ApiToken, NewApiToken};

const COLUMNS: &str = "id, user_id, name, prefix, preset, created_at, expires_at, last_used_at, \
                       last_used_ip, revoked_at, require_approval";

/// Store a new token and return it. A name the user already has is a [`Error::Conflict`].
#[instrument(skip(pool, token), fields(user_id = token.user_id, name = %token.name))]
pub async fn create(pool: &Pool<Sqlite>, token: &NewApiToken) -> Result<ApiToken> {
    sqlx::query_as::<_, ApiToken>(&format!(
        r#"
        INSERT INTO api_tokens (user_id, name, prefix, token_hash, preset, expires_at,
                                require_approval)
        VALUES (?, ?, ?, ?, ?,
                CASE WHEN ? IS NULL THEN NULL ELSE datetime('now', '+' || ? || ' days') END, ?)
        RETURNING {COLUMNS}
        "#
    ))
    .bind(token.user_id)
    .bind(&token.name)
    .bind(&token.prefix)
    .bind(&token.token_hash)
    .bind(&token.preset)
    .bind(token.expires_in_days)
    .bind(token.expires_in_days)
    .bind(token.require_approval)
    .fetch_one(pool)
    .await
    .map_err(super::write_error("token", "create"))
}

/// The user's tokens, newest first, revoked and expired ones included.
#[instrument(skip(pool))]
pub async fn list_for_user(pool: &Pool<Sqlite>, user_id: i64) -> Result<Vec<ApiToken>> {
    sqlx::query_as::<_, ApiToken>(&format!(
        "SELECT {COLUMNS} FROM api_tokens WHERE user_id = ? ORDER BY created_at DESC, id DESC"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list tokens")
    .map_err(Error::Database)
}

/// The token with this secret hash, whatever its state; the caller decides whether a revoked
/// or expired one may pass (it may not) and can say which it was.
#[instrument(skip(pool, token_hash))]
pub async fn find_by_hash(pool: &Pool<Sqlite>, token_hash: &str) -> Result<Option<ApiToken>> {
    sqlx::query_as::<_, ApiToken>(&format!(
        "SELECT {COLUMNS} FROM api_tokens WHERE token_hash = ?"
    ))
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .context("Failed to look up token")
    .map_err(Error::Database)
}

/// The token with this id, whatever its state and owner (for re-checking a token the server
/// already authenticated, never for a request's own lookups).
#[instrument(skip(pool))]
pub async fn get(pool: &Pool<Sqlite>, id: i64) -> Result<Option<ApiToken>> {
    sqlx::query_as::<_, ApiToken>(&format!("SELECT {COLUMNS} FROM api_tokens WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to get token")
        .map_err(Error::Database)
}

/// Revoke one of the user's tokens (idempotent) and return it. Another user's token, or a
/// missing one, is [`Error::NotFound`].
#[instrument(skip(pool))]
pub async fn revoke(pool: &Pool<Sqlite>, user_id: i64, id: i64) -> Result<ApiToken> {
    sqlx::query_as::<_, ApiToken>(&format!(
        "UPDATE api_tokens SET revoked_at = COALESCE(revoked_at, CURRENT_TIMESTAMP) \
         WHERE id = ? AND user_id = ? RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to revoke token")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound(format!("token {id}")))
}

/// Turn approvals on or off for one of the user's tokens and return it.
#[instrument(skip(pool))]
pub async fn set_require_approval(
    pool: &Pool<Sqlite>,
    user_id: i64,
    id: i64,
    on: bool,
) -> Result<ApiToken> {
    sqlx::query_as::<_, ApiToken>(&format!(
        "UPDATE api_tokens SET require_approval = ? WHERE id = ? AND user_id = ? RETURNING {COLUMNS}"
    ))
    .bind(on)
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to update token")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound(format!("token {id}")))
}

/// Remove one of the user's tokens and return what it was. Another user's token, or a
/// missing one, is [`Error::NotFound`].
#[instrument(skip(pool))]
pub async fn delete(pool: &Pool<Sqlite>, user_id: i64, id: i64) -> Result<ApiToken> {
    sqlx::query_as::<_, ApiToken>(&format!(
        "DELETE FROM api_tokens WHERE id = ? AND user_id = ? RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to delete token")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound(format!("token {id}")))
}

/// Note that the token was just used, from `ip`.
#[instrument(skip(pool))]
pub async fn touch(pool: &Pool<Sqlite>, id: i64, ip: &str) -> Result<()> {
    sqlx::query(
        "UPDATE api_tokens SET last_used_at = CURRENT_TIMESTAMP, last_used_ip = ? WHERE id = ?",
    )
    .bind(ip)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to record token use")
    .map_err(Error::Database)?;
    Ok(())
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
        pool
    }

    fn new(user_id: i64, name: &str, hash: &str, days: Option<u32>) -> NewApiToken {
        NewApiToken {
            user_id,
            name: name.to_string(),
            prefix: "svr_abcdef".to_string(),
            token_hash: hash.to_string(),
            preset: "read_only".to_string(),
            expires_in_days: days,
            require_approval: false,
        }
    }

    #[tokio::test]
    async fn tokens_are_per_user_and_names_unique_per_user() {
        let pool = db().await;
        let t = create(&pool, &new(1, "hermes", "h1", Some(30)))
            .await
            .unwrap();
        assert_eq!(t.user_id, 1);
        let expires = t.expires_at.expect("30 days");
        let days = (expires - t.created_at).num_days();
        assert!((29..=30).contains(&days), "{days}");
        let never = create(&pool, &new(1, "claude", "h2", None)).await.unwrap();
        assert!(never.expires_at.is_none());

        let dup = create(&pool, &new(1, "hermes", "h3", None)).await;
        assert!(matches!(dup, Err(Error::Conflict(_))), "{dup:?}");
        // Another user may use the same name
        create(&pool, &new(2, "hermes", "h4", None)).await.unwrap();

        assert_eq!(list_for_user(&pool, 1).await.unwrap().len(), 2);
        assert_eq!(list_for_user(&pool, 2).await.unwrap().len(), 1);
        assert_eq!(
            find_by_hash(&pool, "h1").await.unwrap().map(|t| t.name),
            Some("hermes".to_string())
        );
        assert!(find_by_hash(&pool, "nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn revoke_and_delete_only_touch_the_owners_token() {
        let pool = db().await;
        let t = create(&pool, &new(1, "hermes", "h1", None)).await.unwrap();

        assert!(matches!(
            revoke(&pool, 2, t.id).await,
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            delete(&pool, 2, t.id).await,
            Err(Error::NotFound(_))
        ));

        let revoked = revoke(&pool, 1, t.id).await.unwrap();
        let at = revoked.revoked_at.expect("revoked");
        assert_eq!(revoked.status_at(chrono::Utc::now()), "revoked");
        // Revoking again keeps the first time
        assert_eq!(revoke(&pool, 1, t.id).await.unwrap().revoked_at, Some(at));

        let on = set_require_approval(&pool, 1, t.id, true).await.unwrap();
        assert!(on.require_approval);
        assert!(matches!(
            set_require_approval(&pool, 2, t.id, false).await,
            Err(Error::NotFound(_))
        ));

        delete(&pool, 1, t.id).await.unwrap();
        assert!(find_by_hash(&pool, "h1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn touch_records_time_and_address_and_users_cascade() {
        let pool = db().await;
        let t = create(&pool, &new(2, "bot", "h1", None)).await.unwrap();
        assert!(t.last_used_at.is_none());
        touch(&pool, t.id, "10.0.0.9").await.unwrap();
        let t = find_by_hash(&pool, "h1").await.unwrap().unwrap();
        assert!(t.last_used_at.is_some());
        assert_eq!(t.last_used_ip.as_deref(), Some("10.0.0.9"));

        sqlx::query("DELETE FROM users WHERE id = 2")
            .execute(&pool)
            .await
            .unwrap();
        assert!(find_by_hash(&pool, "h1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn expiry_and_status() {
        let pool = db().await;
        let t = create(&pool, &new(1, "old", "h1", Some(1))).await.unwrap();
        let now = chrono::Utc::now();
        assert_eq!(t.status_at(now), "active");
        assert_eq!(t.status_at(now + chrono::Duration::days(2)), "expired");
    }
}

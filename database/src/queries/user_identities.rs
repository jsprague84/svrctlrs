//! Identities at an OpenID Connect provider, linked to accounts. A sign-in finds the account
//! by `(issuer, subject)` alone ([`find`]); everything else is scoped by `user_id`.

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{NewUserIdentity, UserIdentity};

const COLUMNS: &str = "id, user_id, issuer, subject, email, display_name, created_at, last_used_at";

/// The identity the provider vouched for, whoever it belongs to.
#[instrument(skip(pool))]
pub async fn find(
    pool: &Pool<Sqlite>,
    issuer: &str,
    subject: &str,
) -> Result<Option<UserIdentity>> {
    sqlx::query_as::<_, UserIdentity>(&format!(
        "SELECT {COLUMNS} FROM user_identities WHERE issuer = ? AND subject = ?"
    ))
    .bind(issuer)
    .bind(subject)
    .fetch_optional(pool)
    .await
    .context("Failed to look up an identity")
    .map_err(Error::Database)
}

/// The user's linked identities, oldest first.
#[instrument(skip(pool))]
pub async fn list_for_user(pool: &Pool<Sqlite>, user_id: i64) -> Result<Vec<UserIdentity>> {
    sqlx::query_as::<_, UserIdentity>(&format!(
        "SELECT {COLUMNS} FROM user_identities WHERE user_id = ? ORDER BY created_at, id"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list identities")
    .map_err(Error::Database)
}

/// How many accounts have at least one linked identity (start-up warns at zero when
/// password sign-in is off).
#[instrument(skip(pool))]
pub async fn count_linked_users(pool: &Pool<Sqlite>) -> Result<i64> {
    sqlx::query_scalar("SELECT COUNT(DISTINCT user_id) FROM user_identities")
        .fetch_one(pool)
        .await
        .context("Failed to count linked accounts")
        .map_err(Error::Database)
}

/// Link an identity. One that is already linked (to this account or another) is a
/// [`Error::Conflict`].
#[instrument(skip(pool, identity), fields(user_id = identity.user_id))]
pub async fn link(pool: &Pool<Sqlite>, identity: &NewUserIdentity) -> Result<UserIdentity> {
    sqlx::query_as::<_, UserIdentity>(&format!(
        r#"
        INSERT INTO user_identities (user_id, issuer, subject, email, display_name)
        VALUES (?, ?, ?, ?, ?)
        RETURNING {COLUMNS}
        "#
    ))
    .bind(identity.user_id)
    .bind(&identity.issuer)
    .bind(&identity.subject)
    .bind(&identity.email)
    .bind(&identity.display_name)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            Error::Conflict("That identity is already linked to an account".into())
        }
        _ => Error::Database(anyhow::Error::new(e).context("Failed to link an identity")),
    })
}

/// Record a sign-in with the identity, and refresh what the provider says about it.
#[instrument(skip(pool))]
pub async fn touch(
    pool: &Pool<Sqlite>,
    id: i64,
    email: Option<&str>,
    display_name: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE user_identities SET last_used_at = CURRENT_TIMESTAMP, \
         email = COALESCE(?, email), display_name = COALESCE(?, display_name) WHERE id = ?",
    )
    .bind(email)
    .bind(display_name)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to record an identity's use")
    .map_err(Error::Database)?;
    Ok(())
}

/// Unlink one of the user's identities. Another user's id is [`Error::NotFound`].
#[instrument(skip(pool))]
pub async fn unlink(pool: &Pool<Sqlite>, user_id: i64, id: i64) -> Result<UserIdentity> {
    sqlx::query_as::<_, UserIdentity>(&format!(
        "DELETE FROM user_identities WHERE id = ? AND user_id = ? RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to unlink an identity")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound("identity".into()))
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

    fn identity(user_id: i64, subject: &str) -> NewUserIdentity {
        NewUserIdentity {
            user_id,
            issuer: "https://id.example".into(),
            subject: subject.into(),
            email: Some("ann@example.com".into()),
            display_name: None,
        }
    }

    #[tokio::test]
    async fn an_identity_belongs_to_one_account_and_is_found_by_issuer_and_subject() {
        let pool = db().await;
        let linked = link(&pool, &identity(1, "sub-1")).await.unwrap();
        assert!(matches!(
            link(&pool, &identity(2, "sub-1")).await,
            Err(Error::Conflict(_))
        ));
        let found = find(&pool, "https://id.example", "sub-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.user_id, 1);
        assert!(find(&pool, "https://other.example", "sub-1")
            .await
            .unwrap()
            .is_none());
        assert_eq!(count_linked_users(&pool).await.unwrap(), 1);

        touch(&pool, linked.id, None, Some("Ann")).await.unwrap();
        let listed = list_for_user(&pool, 1).await.unwrap();
        assert_eq!(listed[0].display_name.as_deref(), Some("Ann"));
        assert_eq!(listed[0].email.as_deref(), Some("ann@example.com"));
        assert!(listed[0].last_used_at.is_some());
        assert!(list_for_user(&pool, 2).await.unwrap().is_empty());

        // Bob cannot unlink Ann's identity
        assert!(matches!(
            unlink(&pool, 2, linked.id).await,
            Err(Error::NotFound(_))
        ));
        unlink(&pool, 1, linked.id).await.unwrap();
        assert_eq!(count_linked_users(&pool).await.unwrap(), 0);
    }
}

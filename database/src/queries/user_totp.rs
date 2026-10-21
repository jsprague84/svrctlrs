//! TOTP enrolments and recovery codes. Every query takes the `user_id`; the secret is stored
//! encrypted with the credential key (`svrctlrs_core::encryption`) and refused without one.

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{encryption, Error, Result};
use tracing::instrument;

use crate::models::UserTotp;

fn db(context: &'static str) -> impl Fn(sqlx::Error) -> Error {
    move |e| Error::Database(anyhow::Error::new(e).context(context))
}

/// The user's enrolment, confirmed or pending, with the secret decrypted.
#[instrument(skip(pool))]
pub async fn get(pool: &Pool<Sqlite>, user_id: i64) -> Result<Option<UserTotp>> {
    let row = sqlx::query_as::<_, UserTotp>(
        "SELECT user_id, secret, confirmed_at, last_step, created_at FROM user_totp WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(db("Failed to read the TOTP enrolment"))?;
    row.map(|mut t| {
        t.secret = encryption::decrypt(&t.secret)?;
        Ok(t)
    })
    .transpose()
}

/// Whether the user has a confirmed enrolment (a password sign-in then needs a code).
#[instrument(skip(pool))]
pub async fn is_enabled(pool: &Pool<Sqlite>, user_id: i64) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM user_totp WHERE user_id = ? AND confirmed_at IS NOT NULL)",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Failed to read the TOTP enrolment")
    .map_err(Error::Database)
}

/// Start (or restart) an enrolment with `secret`. A confirmed enrolment is not replaced:
/// [`Error::Conflict`], since turning it off takes the password.
#[instrument(skip(pool, secret))]
pub async fn begin(pool: &Pool<Sqlite>, user_id: i64, secret: &str) -> Result<()> {
    if !encryption::is_initialized() {
        return Err(Error::Config(
            "credential encryption is not initialised; refusing to store a TOTP secret".into(),
        ));
    }
    let stored = encryption::encrypt(secret)?;
    let result = sqlx::query(
        r#"
        INSERT INTO user_totp (user_id, secret) VALUES (?, ?)
        ON CONFLICT(user_id) DO UPDATE SET
            secret = excluded.secret, last_step = 0, created_at = CURRENT_TIMESTAMP
        WHERE user_totp.confirmed_at IS NULL
        "#,
    )
    .bind(user_id)
    .bind(&stored)
    .execute(pool)
    .await
    .map_err(db("Failed to start the TOTP enrolment"))?;
    if result.rows_affected() == 0 {
        return Err(Error::Conflict(
            "two-factor sign-in is already on; turn it off first".into(),
        ));
    }
    Ok(())
}

/// Confirm a pending enrolment with the step of the first valid code, and store the recovery
/// codes (their hashes) in place of any old ones. False when there was nothing pending.
#[instrument(skip(pool, recovery_hashes))]
pub async fn confirm(
    pool: &Pool<Sqlite>,
    user_id: i64,
    step: i64,
    recovery_hashes: &[String],
) -> Result<bool> {
    let mut tx = pool
        .begin()
        .await
        .map_err(db("Failed to start a transaction"))?;
    let updated = sqlx::query(
        "UPDATE user_totp SET confirmed_at = CURRENT_TIMESTAMP, last_step = ? \
         WHERE user_id = ? AND confirmed_at IS NULL",
    )
    .bind(step)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(db("Failed to confirm the TOTP enrolment"))?;
    if updated.rows_affected() == 0 {
        return Ok(false);
    }
    store_codes(&mut tx, user_id, recovery_hashes).await?;
    tx.commit().await.map_err(db("Failed to commit"))?;
    Ok(true)
}

/// Accept a code's time step: true only when it is later than the last one accepted, so the
/// same code (or an older one) cannot be used twice, even by two requests at once.
#[instrument(skip(pool))]
pub async fn accept_step(pool: &Pool<Sqlite>, user_id: i64, step: i64) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE user_totp SET last_step = ? \
         WHERE user_id = ? AND confirmed_at IS NOT NULL AND last_step < ?",
    )
    .bind(step)
    .bind(user_id)
    .bind(step)
    .execute(pool)
    .await
    .map_err(db("Failed to record the TOTP step"))?;
    Ok(result.rows_affected() == 1)
}

/// Remove the enrolment and the recovery codes. True when there was an enrolment.
#[instrument(skip(pool))]
pub async fn disable(pool: &Pool<Sqlite>, user_id: i64) -> Result<bool> {
    let mut tx = pool
        .begin()
        .await
        .map_err(db("Failed to start a transaction"))?;
    let removed = sqlx::query("DELETE FROM user_totp WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(db("Failed to remove the TOTP enrolment"))?;
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(db("Failed to remove recovery codes"))?;
    tx.commit().await.map_err(db("Failed to commit"))?;
    Ok(removed.rows_affected() > 0)
}

/// Replace the user's recovery codes (their hashes).
#[instrument(skip(pool, recovery_hashes))]
pub async fn replace_recovery_codes(
    pool: &Pool<Sqlite>,
    user_id: i64,
    recovery_hashes: &[String],
) -> Result<()> {
    let mut tx = pool
        .begin()
        .await
        .map_err(db("Failed to start a transaction"))?;
    store_codes(&mut tx, user_id, recovery_hashes).await?;
    tx.commit().await.map_err(db("Failed to commit"))
}

async fn store_codes(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    user_id: i64,
    recovery_hashes: &[String],
) -> Result<()> {
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(db("Failed to remove recovery codes"))?;
    for hash in recovery_hashes {
        sqlx::query("INSERT INTO user_recovery_codes (user_id, code_hash) VALUES (?, ?)")
            .bind(user_id)
            .bind(hash)
            .execute(&mut **tx)
            .await
            .map_err(db("Failed to store a recovery code"))?;
    }
    Ok(())
}

/// Use up one of the user's recovery codes by its hash. True when an unused one matched.
#[instrument(skip(pool, code_hash))]
pub async fn use_recovery_code(pool: &Pool<Sqlite>, user_id: i64, code_hash: &str) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE user_recovery_codes SET used_at = CURRENT_TIMESTAMP \
         WHERE id = (SELECT id FROM user_recovery_codes \
                     WHERE user_id = ? AND code_hash = ? AND used_at IS NULL LIMIT 1)",
    )
    .bind(user_id)
    .bind(code_hash)
    .execute(pool)
    .await
    .map_err(db("Failed to use a recovery code"))?;
    Ok(result.rows_affected() == 1)
}

/// How many unused recovery codes the user has.
#[instrument(skip(pool))]
pub async fn recovery_codes_left(pool: &Pool<Sqlite>, user_id: i64) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_recovery_codes WHERE user_id = ? AND used_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Failed to count recovery codes")
    .map_err(Error::Database)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn db_with_users() -> Pool<Sqlite> {
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
        if !encryption::is_initialized() {
            let _ = encryption::init_with_hex_key(&"ab".repeat(32));
        }
        pool
    }

    #[tokio::test]
    async fn enrolment_is_pending_until_confirmed_and_then_not_replaced() {
        let pool = db_with_users().await;
        begin(&pool, 1, "JBSWY3DPEHPK3PXP").await.unwrap();
        let t = get(&pool, 1).await.unwrap().unwrap();
        assert_eq!(t.secret, "JBSWY3DPEHPK3PXP");
        assert!(!t.is_enabled());
        assert!(!is_enabled(&pool, 1).await.unwrap());
        // Stored encrypted
        let raw: String = sqlx::query_scalar("SELECT secret FROM user_totp WHERE user_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_ne!(raw, "JBSWY3DPEHPK3PXP");

        // A pending enrolment can be restarted
        begin(&pool, 1, "KRSXG5CTMVRXEZLU").await.unwrap();
        assert!(confirm(&pool, 1, 100, &["h1".into(), "h2".into()])
            .await
            .unwrap());
        assert!(is_enabled(&pool, 1).await.unwrap());
        assert_eq!(
            get(&pool, 1).await.unwrap().unwrap().secret,
            "KRSXG5CTMVRXEZLU"
        );
        assert!(matches!(
            begin(&pool, 1, "JBSWY3DPEHPK3PXP").await,
            Err(Error::Conflict(_))
        ));
        // Nothing pending any more
        assert!(!confirm(&pool, 1, 101, &[]).await.unwrap());
        // Another user is untouched
        assert!(get(&pool, 2).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_step_is_accepted_once_and_never_an_earlier_one() {
        let pool = db_with_users().await;
        begin(&pool, 1, "JBSWY3DPEHPK3PXP").await.unwrap();
        // Not before confirmation
        assert!(!accept_step(&pool, 1, 5).await.unwrap());
        confirm(&pool, 1, 10, &[]).await.unwrap();
        assert!(!accept_step(&pool, 1, 10).await.unwrap());
        assert!(!accept_step(&pool, 1, 9).await.unwrap());
        assert!(accept_step(&pool, 1, 11).await.unwrap());
        assert!(!accept_step(&pool, 1, 11).await.unwrap());
    }

    #[tokio::test]
    async fn recovery_codes_work_once_per_user_and_go_with_the_enrolment() {
        let pool = db_with_users().await;
        begin(&pool, 1, "JBSWY3DPEHPK3PXP").await.unwrap();
        confirm(&pool, 1, 1, &["aa".into(), "bb".into()])
            .await
            .unwrap();
        assert_eq!(recovery_codes_left(&pool, 1).await.unwrap(), 2);
        assert!(!use_recovery_code(&pool, 2, "aa").await.unwrap());
        assert!(use_recovery_code(&pool, 1, "aa").await.unwrap());
        assert!(!use_recovery_code(&pool, 1, "aa").await.unwrap());
        assert_eq!(recovery_codes_left(&pool, 1).await.unwrap(), 1);

        replace_recovery_codes(&pool, 1, &["cc".into()])
            .await
            .unwrap();
        assert!(!use_recovery_code(&pool, 1, "bb").await.unwrap());
        assert_eq!(recovery_codes_left(&pool, 1).await.unwrap(), 1);

        assert!(disable(&pool, 1).await.unwrap());
        assert!(!disable(&pool, 1).await.unwrap());
        assert_eq!(recovery_codes_left(&pool, 1).await.unwrap(), 0);
        assert!(get(&pool, 1).await.unwrap().is_none());
    }
}

//! The commands that run next to the database, without the network: `backup` (a consistent
//! copy), `restore` (stage a copy for the next start) and `reset-password` (lockout recovery).
//! Meant for `docker exec`.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use svrctlrs_database::models::{NewAuditEntry, NewBackupRun};
use svrctlrs_database::sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use svrctlrs_database::{backup, queries};
use tower_sessions::{session::Id, SessionStore};
use tower_sessions_sqlx_store::SqliteStore;

/// Where the server keeps its generated key file: next to the database.
fn key_file_for(database_url: &str) -> Option<PathBuf> {
    let db = backup::db_path(database_url)?;
    Some(
        db.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .join("encryption.key"),
    )
}

fn key_in_environment() -> bool {
    std::env::var("ENCRYPTION_KEY").is_ok_and(|k| !k.trim().is_empty())
}

async fn connect(database_url: &str) -> anyhow::Result<Pool<Sqlite>> {
    Ok(SqlitePoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?)
}

/// The folder backups go to: `--out`, else `BACKUP_DIR`, else `data/backups` (the server's
/// own default, beside the database).
pub fn backup_dir(out: Option<PathBuf>) -> PathBuf {
    out.or_else(|| {
        std::env::var("BACKUP_DIR")
            .ok()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
    })
    .unwrap_or_else(|| PathBuf::from("data/backups"))
}

/// The same copy the server makes every night (`svrctlrs_database::backup`), recorded in the
/// run history as `svrctl`. `keep` defaults to the `backup.keep` setting; 0 keeps all.
pub async fn handle_backup(
    database_url: &str,
    out: &Path,
    keep: Option<usize>,
    with_key: bool,
) -> anyhow::Result<()> {
    let pool = connect(database_url).await?;
    let keep = match keep {
        Some(k) => k,
        None => queries::settings::get_setting_value_or(&pool, "backup.keep", "14")
            .await
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(14),
    };
    let started_at = chrono::Utc::now();
    let result = backup::create(&pool, out, keep).await;
    let (file_name, size, removed, error) = match &result {
        Ok(c) => (
            Some(c.file.name.as_str()),
            Some(c.file.bytes as i64),
            c.removed as i64,
            None,
        ),
        Err(e) => (None, None, 0, Some(e.detail())),
    };
    // A database from before migration 111 has no history table; the copy is what matters
    let _ = queries::backup_runs::record(
        &pool,
        &NewBackupRun {
            trigger: "svrctl",
            started_at,
            file_name,
            size_bytes: size,
            removed,
            error: error.as_deref(),
        },
    )
    .await;
    pool.close().await;
    let created = result.map_err(|e| anyhow::anyhow!(e.detail()))?;

    // The key: without it the backup's credentials are unreadable. It is copied only when
    // asked: a key in the same folder as the copies defeats the encryption once that folder
    // is shipped somewhere.
    let key = if key_in_environment() {
        "in ENCRYPTION_KEY — back up the environment (.env) separately".to_string()
    } else if let Some(key_file) = key_file_for(database_url).filter(|p| p.exists()) {
        if with_key {
            let dest = out.join("encryption.key");
            std::fs::copy(&key_file, &dest)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600))?;
            }
            "copied to encryption.key".to_string()
        } else {
            format!(
                "in {} — not copied (pass --with-key, or back it up separately)",
                key_file.display()
            )
        }
    } else {
        "not found — credentials in this backup cannot be decrypted without it".to_string()
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "backup": created.file.path,
            "bytes": created.file.bytes,
            "key": key,
            "kept": backup::list(out).map(|l| l.len()).unwrap_or(0),
            "removed": created.removed,
        }))?
    );
    Ok(())
}

/// Check a copy and, unless `check_only`, stage it for the next start
/// (`svrctlrs_database::backup`). The running server is not touched: it swaps the copy in
/// when it restarts.
pub async fn handle_restore(
    database_url: &str,
    file: &Path,
    check_only: bool,
) -> anyhow::Result<()> {
    let key_file = key_file_for(database_url)
        .ok_or_else(|| anyhow::anyhow!("{database_url} is not a database file"))?;
    svrctlrs_core::encryption::init_existing_key(&key_file)
        .map_err(|e| anyhow::anyhow!(e.detail()))?;
    let found = backup::inspect(file)
        .await
        .map_err(|e| anyhow::anyhow!(e.detail()))?;
    if check_only {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "checked": file,
                "restorable": true,
                "schema_version": found.schema_version,
                "users": found.users,
                "servers": found.servers,
                "credentials_checked": found.credentials,
            }))?
        );
        return Ok(());
    }
    let db = backup::db_path(database_url)
        .ok_or_else(|| anyhow::anyhow!("{database_url} is not a database file"))?;
    let replaced = backup::staged_path(&db).exists();
    let staged =
        backup::stage_restore(database_url, file).map_err(|e| anyhow::anyhow!(e.detail()))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "staged": staged,
            "from": file,
            "replaced_staged": replaced,
            "schema_version": found.schema_version,
            "users": found.users,
            "servers": found.servers,
            "credentials_checked": found.credentials,
            "next": "restart the server (docker restart svrctlrs): it moves the current database \
                     into before-restore-<time>/ and starts on the copy; everyone signs in again",
        }))?
    );
    Ok(())
}

/// A random password of 24 characters from an unambiguous alphabet (~139 bits).
fn generate_password() -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rngs::OsRng;
    (0..24)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

/// The new password: from stdin, generated, or asked for twice without echo.
fn new_password(password_stdin: bool, generate: bool) -> anyhow::Result<(String, bool)> {
    use std::io::{BufRead, IsTerminal};
    if generate {
        return Ok((generate_password(), true));
    }
    if password_stdin {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        return Ok((line.trim_end_matches(['\r', '\n']).to_string(), false));
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "no terminal to ask on: run with `docker exec -it`, or pass --password-stdin or --generate"
        );
    }
    let first = rpassword::prompt_password("New password (at least 12 characters): ")?;
    let again = rpassword::prompt_password("Repeat it: ")?;
    if first != again {
        anyhow::bail!("the two passwords differ; nothing was changed");
    }
    Ok((first, false))
}

/// Sign-ins that ended
#[derive(Debug, PartialEq)]
struct ResetOutcome {
    user_id: i64,
    sessions_ended: usize,
    totp_cleared: bool,
}

/// Set `username`'s password and, with `end_sessions`, delete every session record of that
/// account (browser cookies and native-app session tokens alike) so whoever had it is signed
/// out. API tokens are separate credentials and stay. Audited.
async fn reset_password(
    pool: &Pool<Sqlite>,
    username: &str,
    password: &str,
    end_sessions: bool,
    clear_totp: bool,
) -> anyhow::Result<ResetOutcome> {
    let user = queries::users::get_user_by_username(pool, username)
        .await?
        .ok_or_else(|| anyhow::anyhow!("there is no account called {username:?}"))?;
    queries::users::update_password(pool, user.id, password).await?;

    let mut sessions_ended = 0;
    if end_sessions {
        let store = SqliteStore::new(pool.clone());
        // The table exists once the server has started; before that there is nobody to sign out
        let ids: Vec<(String,)> =
            svrctlrs_database::sqlx::query_as("SELECT id FROM tower_sessions")
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        for (raw,) in ids {
            let Ok(id) = raw.parse::<Id>() else { continue };
            let Ok(Some(record)) = store.load(&id).await else {
                continue;
            };
            if record.data.get("user_id").and_then(Value::as_i64) == Some(user.id) {
                store.delete(&id).await?;
                sessions_ended += 1;
            }
        }
    }

    // Removing the enrolment needs no key: it deletes the (encrypted) secret
    let totp_cleared = clear_totp && queries::user_totp::disable(pool, user.id).await?;

    queries::audit::record(
        pool,
        &NewAuditEntry::new("user.password_reset")
            .target("user", user.id, &user.username)
            .details(json!({
                "via": "svrctl",
                "sessions_ended": sessions_ended,
                "totp_cleared": totp_cleared,
            })),
    )
    .await?;
    Ok(ResetOutcome {
        user_id: user.id,
        sessions_ended,
        totp_cleared,
    })
}

pub async fn handle_reset_password(
    database_url: &str,
    username: &str,
    password_stdin: bool,
    generate: bool,
    end_sessions: bool,
    clear_totp: bool,
) -> anyhow::Result<()> {
    let (password, generated) = new_password(password_stdin, generate)?;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    let outcome = reset_password(&pool, username, &password, end_sessions, clear_totp).await?;
    pool.close().await;
    let mut report = json!({
        "user": username,
        "password": "changed",
        "sessions_ended": outcome.sessions_ended,
        "two_factor": if outcome.totp_cleared { "removed" } else if clear_totp { "was off" } else { "unchanged" },
    });
    if generated {
        report["new_password"] = json!(password);
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_file_sits_next_to_the_database() {
        assert_eq!(
            key_file_for("sqlite:/app/data/svrctlrs.db"),
            Some(PathBuf::from("/app/data/encryption.key"))
        );
        assert_eq!(
            key_file_for("sqlite:data/svrctlrs.db"),
            Some(PathBuf::from("data/encryption.key"))
        );
        assert_eq!(key_file_for("sqlite::memory:"), None);
        assert_eq!(key_file_for("postgres://x"), None);
    }

    async fn database() -> (Pool<Sqlite>, SqliteStore) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        svrctlrs_database::run_migrations(&pool).await.unwrap();
        let store = SqliteStore::new(pool.clone());
        store.migrate().await.unwrap();
        for name in ["admin", "bob"] {
            queries::users::create_user(
                &pool,
                &svrctlrs_database::models::CreateUser {
                    username: name.into(),
                    password: "old-password-123".into(),
                },
            )
            .await
            .unwrap();
        }
        (pool, store)
    }

    async fn signed_in(store: &SqliteStore, user_id: i64) -> Id {
        let mut record = tower_sessions::session::Record {
            id: Id::default(),
            data: std::collections::HashMap::from([("user_id".to_string(), json!(user_id))]),
            expiry_date: time::OffsetDateTime::now_utc() + time::Duration::hours(1),
        };
        store.create(&mut record).await.unwrap();
        record.id
    }

    #[tokio::test]
    async fn reset_sets_the_password_and_signs_only_that_account_out() {
        let (pool, store) = database().await;
        let admin = queries::users::get_user_by_username(&pool, "admin")
            .await
            .unwrap()
            .unwrap();
        let a1 = signed_in(&store, admin.id).await;
        let a2 = signed_in(&store, admin.id).await;
        let bob = signed_in(&store, admin.id + 1).await;

        let outcome = reset_password(&pool, "admin", "a-brand-new-password", true, false)
            .await
            .unwrap();
        assert_eq!(outcome.sessions_ended, 2);
        let after = queries::users::get_user(&pool, admin.id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            queries::users::verify_password("a-brand-new-password", &after.password_hash).unwrap()
        );
        assert!(
            !queries::users::verify_password("old-password-123", &after.password_hash).unwrap()
        );
        assert!(store.load(&a1).await.unwrap().is_none());
        assert!(store.load(&a2).await.unwrap().is_none());
        assert!(
            store.load(&bob).await.unwrap().is_some(),
            "another account stays signed in"
        );

        let audit = queries::audit::list(&pool, 10, None).await.unwrap();
        assert_eq!(audit[0].action, "user.password_reset");
        assert_eq!(audit[0].target_id, Some(admin.id));
    }

    #[tokio::test]
    async fn reset_refuses_an_unknown_account_and_a_short_password() {
        let (pool, store) = database().await;
        let s = signed_in(&store, 1).await;
        assert!(
            reset_password(&pool, "nobody", "a-brand-new-password", true, false)
                .await
                .is_err()
        );
        assert!(reset_password(&pool, "admin", "short", true, false)
            .await
            .is_err());
        assert!(
            store.load(&s).await.unwrap().is_some(),
            "nothing ended on a refusal"
        );
        let kept = reset_password(&pool, "admin", "a-brand-new-password", false, false)
            .await
            .unwrap();
        assert_eq!(kept.sessions_ended, 0);
        assert!(store.load(&s).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn clear_totp_turns_two_factor_off_and_only_when_asked() {
        let (pool, _store) = database().await;
        let admin = queries::users::get_user_by_username(&pool, "admin")
            .await
            .unwrap()
            .unwrap();
        // An enrolment as the server writes it (the secret is opaque here; nothing decrypts it)
        svrctlrs_database::sqlx::query(
            "INSERT INTO user_totp (user_id, secret, confirmed_at) VALUES (?, 'x', CURRENT_TIMESTAMP)",
        )
        .bind(admin.id)
        .execute(&pool)
        .await
        .unwrap();
        let kept = reset_password(&pool, "admin", "a-brand-new-password", false, false)
            .await
            .unwrap();
        assert!(!kept.totp_cleared);
        assert!(queries::user_totp::is_enabled(&pool, admin.id)
            .await
            .unwrap());

        let cleared = reset_password(&pool, "admin", "a-brand-new-password", false, true)
            .await
            .unwrap();
        assert!(cleared.totp_cleared);
        assert!(!queries::user_totp::is_enabled(&pool, admin.id)
            .await
            .unwrap());
    }

    #[test]
    fn generated_passwords_are_long_and_differ() {
        let a = generate_password();
        assert_eq!(a.chars().count(), 24);
        assert_ne!(a, generate_password());
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}

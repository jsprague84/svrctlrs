//! The commands that run next to the database, without the network: `backup` (a consistent
//! copy) and `reset-password` (lockout recovery). Meant for `docker exec`.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use svrctlrs_database::sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use svrctlrs_database::{models::NewAuditEntry, queries};
use tower_sessions::{session::Id, SessionStore};
use tower_sessions_sqlx_store::SqliteStore;

/// Where the server keeps its generated key file: next to the database.
fn key_file_for(database_url: &str) -> Option<PathBuf> {
    let path = database_url
        .strip_prefix("sqlite:")?
        .trim_start_matches("//");
    if path.is_empty() || path == ":memory:" {
        return None;
    }
    Some(
        Path::new(path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("encryption.key"),
    )
}

/// `VACUUM INTO` a fresh file: a consistent snapshot while the server keeps running.
async fn backup_database(database_url: &str, target: &Path) -> anyhow::Result<()> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    let target_str = target
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("backup path is not valid UTF-8"))?
        .replace('\'', "''");
    svrctlrs_database::sqlx::query(&format!("VACUUM INTO '{target_str}'"))
        .execute(&pool)
        .await?;
    pool.close().await;
    Ok(())
}

/// Backups beyond `keep`, oldest first, by file name (names carry the timestamp).
fn prunable(mut backups: Vec<PathBuf>, keep: usize) -> Vec<PathBuf> {
    backups.sort();
    if backups.len() <= keep {
        return Vec::new();
    }
    let excess = backups.len() - keep;
    backups.into_iter().take(excess).collect()
}

pub async fn handle_backup(database_url: &str, out: &Path, keep: usize) -> anyhow::Result<()> {
    std::fs::create_dir_all(out)?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%SZ");
    let target = out.join(format!("svrctlrs-{stamp}.db"));
    backup_database(database_url, &target).await?;
    let size = std::fs::metadata(&target)?.len();

    // The key: without it the backup's credentials are unreadable.
    let key = if std::env::var("ENCRYPTION_KEY").is_ok_and(|k| !k.trim().is_empty()) {
        "in ENCRYPTION_KEY — back up the environment (.env) with this file"
    } else if let Some(key_file) = key_file_for(database_url).filter(|p| p.exists()) {
        let dest = out.join("encryption.key");
        std::fs::copy(&key_file, &dest)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600))?;
        }
        "copied to encryption.key"
    } else {
        "not found — credentials in this backup cannot be decrypted without it"
    };

    let existing: Vec<PathBuf> = std::fs::read_dir(out)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("svrctlrs-") && n.ends_with(".db"))
        })
        .collect();
    let kept = existing.len().min(keep.max(1));
    let removed = prunable(existing, keep.max(1));
    for old in &removed {
        std::fs::remove_file(old)?;
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "backup": target,
            "bytes": size,
            "key": key,
            "kept": kept,
            "removed": removed.len(),
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

    #[test]
    fn prunes_oldest_beyond_keep() {
        let files: Vec<PathBuf> = [
            "svrctlrs-20260103-000000Z.db",
            "svrctlrs-20260101-000000Z.db",
            "svrctlrs-20260102-000000Z.db",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let gone = prunable(files.clone(), 2);
        assert_eq!(gone, vec![PathBuf::from("svrctlrs-20260101-000000Z.db")]);
        assert!(prunable(files, 3).is_empty());
    }

    #[tokio::test]
    async fn backup_is_a_consistent_copy() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("live.db");
        let url = format!("sqlite:{}?mode=rwc", db.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        svrctlrs_database::sqlx::query("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('hello')")
            .execute(&pool)
            .await
            .unwrap();

        let target = dir.path().join("copy.db");
        backup_database(&url, &target).await.unwrap();

        let copy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}", target.display()))
            .await
            .unwrap();
        let (v,): (String,) = svrctlrs_database::sqlx::query_as("SELECT v FROM t")
            .fetch_one(&copy)
            .await
            .unwrap();
        assert_eq!(v, "hello");
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

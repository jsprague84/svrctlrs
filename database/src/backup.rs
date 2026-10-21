//! Backups of the database file (ROADMAP 24, docs/DEPLOYMENT.md "Backups").
//!
//! A backup is `BACKUP_DIR/svrctlrs-<UTC time>.db`: a `VACUUM INTO` copy, consistent while
//! the server keeps writing, with the sign-ins removed (`tower_sessions` holds live session
//! ids, browser and desktop app alike, so a copy that kept them would be a bag of valid
//! cookies). It is written as a hidden `.tmp` file and renamed into place, so a crash never
//! leaves a half copy under a name that counts; folder 0700, file 0600. It does not hold the
//! encryption key (credentials, TOTP secrets and recordings stay unreadable without it) nor
//! the recordings directory.
//!
//! Restoring is two steps so that no database is swapped under a running server:
//! [`inspect`] checks a copy (integrity, schema not newer than this binary, every encrypted
//! value readable with the running key) and [`stage_restore`] puts it next to the database as
//! `<db>.restore`; the next start calls [`apply_staged_restore`] before opening the pool,
//! which moves the current database aside into `before-restore-<time>/` and the copy into
//! its place. The server and `svrctl` share all of it.

use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::{info, instrument, warn};

/// Copies are `svrctlrs-<STAMP>.db`; only names of this shape are listed or pruned.
const PREFIX: &str = "svrctlrs-";
const SUFFIX: &str = ".db";
const STAMP: &str = "%Y%m%d-%H%M%SZ";

/// A copy in the backup folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupFile {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    /// When the copy was made (from its name; the file's mtime for a renamed one)
    pub created_at: DateTime<Utc>,
}

/// A copy just written, and how many older ones retention removed.
#[derive(Debug, Clone)]
pub struct Created {
    pub file: BackupFile,
    pub removed: usize,
}

/// What [`inspect`] found in a copy that can be restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    /// The newest migration the copy has applied
    pub schema_version: i64,
    pub users: i64,
    pub servers: i64,
    /// Encrypted credentials, all of which decrypted with the running key
    pub credentials: i64,
}

/// A restore that the start-up applied.
#[derive(Debug, Clone)]
pub struct Applied {
    /// Where the database it replaced went
    pub previous: PathBuf,
}

/// The database file behind a `sqlite:` URL (`sqlite:data/x.db`, `sqlite:///abs/x.db`,
/// options after `?` ignored); `None` for memory databases and other schemes.
pub fn db_path(database_url: &str) -> Option<PathBuf> {
    let rest = database_url.strip_prefix("sqlite:")?;
    let rest = rest.strip_prefix("//").unwrap_or(rest);
    let path = rest.split('?').next().unwrap_or_default();
    if path.is_empty() || path == ":memory:" {
        return None;
    }
    Some(PathBuf::from(path))
}

/// Where a restore waits for the next start: `<db>.restore`.
pub fn staged_path(db: &Path) -> PathBuf {
    let mut name = db.as_os_str().to_owned();
    name.push(".restore");
    PathBuf::from(name)
}

/// The newest migration this binary knows.
pub fn latest_schema_version() -> i64 {
    crate::MIGRATOR
        .iter()
        .map(|m| m.version)
        .max()
        .unwrap_or(crate::BASELINE_VERSION)
}

fn config(msg: impl Into<String>) -> Error {
    Error::Config(msg.into())
}

fn io(context: String) -> impl FnOnce(std::io::Error) -> Error {
    move |e| Error::Other(anyhow::Error::new(e).context(context))
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(io(format!(
        "Could not set the permissions of {}",
        path.display()
    )))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

/// One connection to a file that is not the server's database.
async fn open_file(path: &Path, read_only: bool) -> Result<Pool<Sqlite>> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(read_only);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .with_context(|| format!("Could not open {}", path.display()))
        .map_err(Error::Database)
}

async fn table_exists(pool: &Pool<Sqlite>, name: &str) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?)",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .context("Failed to read the schema")
    .map_err(Error::Database)
}

/// Write a copy of the database behind `pool` into `dir`, then keep only the newest `keep`
/// copies there (0 keeps all). Older copies are removed only after this one is in place.
#[instrument(skip(pool))]
pub async fn create(pool: &Pool<Sqlite>, dir: &Path, keep: usize) -> Result<Created> {
    std::fs::create_dir_all(dir).map_err(io(format!("Could not create {}", dir.display())))?;
    set_mode(dir, 0o700)?;
    remove_leftovers(dir);

    let now = Utc::now();
    let name = format!("{PREFIX}{}{SUFFIX}", now.format(STAMP));
    let target = dir.join(&name);
    if target.exists() {
        return Err(Error::Conflict(format!(
            "{name} already exists: a backup was made this second"
        )));
    }
    let tmp = dir.join(format!(".{name}.tmp"));
    let tmp_str = tmp
        .to_str()
        .ok_or_else(|| config("The backup folder's path is not valid UTF-8"))?;

    let result = async {
        sqlx::query("VACUUM INTO ?")
            .bind(tmp_str)
            .execute(pool)
            .await
            .context("VACUUM INTO failed")
            .map_err(Error::Database)?;
        strip_sign_ins(&tmp).await?;
        set_mode(&tmp, 0o600)?;
        std::fs::File::open(&tmp)
            .and_then(|f| f.sync_all())
            .map_err(io(format!("Could not flush {}", tmp.display())))?;
        std::fs::rename(&tmp, &target).map_err(io(format!(
            "Could not move the copy to {}",
            target.display()
        )))?;
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok::<_, Error>(())
    }
    .await;
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    let bytes = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
    let removed = prune(dir, keep)?;
    info!(file = %name, bytes, removed, "Database backup written");
    Ok(Created {
        file: BackupFile {
            name,
            path: target,
            bytes,
            created_at: now,
        },
        removed,
    })
}

/// Remove the sign-ins from a fresh copy, leave no trace of them in free pages, and switch
/// it out of WAL so it is one self-contained file.
async fn strip_sign_ins(copy: &Path) -> Result<()> {
    let pool = open_file(copy, false).await?;
    let result = async {
        if table_exists(&pool, "tower_sessions").await? {
            sqlx::query("DELETE FROM tower_sessions")
                .execute(&pool)
                .await
                .context("Could not remove sign-ins from the copy")
                .map_err(Error::Database)?;
        }
        for statement in ["VACUUM", "PRAGMA journal_mode = DELETE"] {
            sqlx::query(statement)
                .execute(&pool)
                .await
                .with_context(|| format!("{statement} on the copy failed"))
                .map_err(Error::Database)?;
        }
        Ok(())
    }
    .await;
    pool.close().await;
    result
}

/// Hidden `.tmp` files a crash left behind.
fn remove_leftovers(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with(&format!(".{PREFIX}")) && name.ends_with(".tmp") {
            if let Err(e) = std::fs::remove_file(entry.path()) {
                warn!(file = name, error = %e, "Could not remove a half-written backup");
            }
        }
    }
}

/// The copies in `dir`, newest first; an absent folder has none.
pub fn list(dir: &Path) -> Result<Vec<BackupFile>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(format!("Could not read {}", dir.display()))(e)),
    };
    let mut files: Vec<BackupFile> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            let stamp = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?.to_string();
            let meta = entry.metadata().ok().filter(|m| m.is_file())?;
            let created_at = NaiveDateTime::parse_from_str(&stamp, STAMP)
                .map(|t| Utc.from_utc_datetime(&t))
                .ok()
                .or_else(|| meta.modified().ok().map(DateTime::<Utc>::from))?;
            Some(BackupFile {
                path: entry.path(),
                name,
                bytes: meta.len(),
                created_at,
            })
        })
        .collect();
    files.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.name.cmp(&a.name)));
    Ok(files)
}

/// Delete all but the newest `keep` copies (0 keeps all). Returns how many went.
pub fn prune(dir: &Path, keep: usize) -> Result<usize> {
    if keep == 0 {
        return Ok(0);
    }
    let mut removed = 0;
    for old in list(dir)?.into_iter().skip(keep) {
        std::fs::remove_file(&old.path).map_err(io(format!("Could not remove {}", old.name)))?;
        removed += 1;
    }
    Ok(removed)
}

/// Check that `path` is a svrctlrs database this binary can run and this key can read.
/// Needs the encryption key initialised. Errors are `InvalidInput` with the reason.
pub async fn inspect(path: &Path) -> Result<Inspection> {
    if !path.is_file() {
        return Err(Error::InvalidInput(format!(
            "{} is not a file",
            path.display()
        )));
    }
    let pool = open_file(path, true)
        .await
        .map_err(|e| Error::InvalidInput(format!("Not a database: {}", e.detail())))?;
    let result = inspect_pool(&pool).await;
    pool.close().await;
    result
}

async fn inspect_pool(pool: &Pool<Sqlite>) -> Result<Inspection> {
    let bad = |m: String| Error::InvalidInput(m);
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(pool)
        .await
        .map_err(|e| bad(format!("Not a database: {e}")))?;
    if integrity != "ok" {
        return Err(bad(format!("The copy is damaged: {integrity}")));
    }
    for table in ["_sqlx_migrations", "users", "servers", "credentials"] {
        if !table_exists(pool, table).await? {
            return Err(bad(format!(
                "Not a svrctlrs database (it has no {table} table)"
            )));
        }
    }
    let schema_version: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
    )
    .fetch_one(pool)
    .await
    .context("Failed to read the copy's migrations")
    .map_err(Error::Database)?;
    let known = latest_schema_version();
    if schema_version > known {
        return Err(bad(format!(
            "The copy has migration {schema_version}, newer than this version knows ({known}): \
             restore it with the release that made it"
        )));
    }

    let count = |sql: &'static str| async move {
        sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(pool)
            .await
            .context("Failed to count rows in the copy")
            .map_err(Error::Database)
    };
    let users = count("SELECT COUNT(*) FROM users").await?;
    let servers = count("SELECT COUNT(*) FROM servers").await?;

    // Every encrypted value must open with the running key; one that does not means the copy
    // was made under another ENCRYPTION_KEY, and restoring it would lose every credential.
    let key_hint = "the copy was made with another encryption key: set ENCRYPTION_KEY (or \
                    data/encryption.key) to the key it was made with";
    let rows: Vec<(String, String, bool, Option<String>, bool)> = sqlx::query_as(
        "SELECT name, value, encrypted, metadata, metadata_encrypted FROM credentials",
    )
    .fetch_all(pool)
    .await
    .context("Failed to read the copy's credentials")
    .map_err(Error::Database)?;
    let mut credentials = 0;
    for (name, value, encrypted, metadata, metadata_encrypted) in rows {
        if encrypted {
            credentials += 1;
            svrctlrs_core::encryption::decrypt(&value).map_err(|e| match e {
                Error::Config(m) => Error::Config(m),
                _ => bad(format!("Credential {name:?} does not decrypt: {key_hint}")),
            })?;
        }
        if let (Some(m), true) = (metadata, metadata_encrypted) {
            svrctlrs_core::encryption::decrypt(&m).map_err(|e| match e {
                Error::Config(m) => Error::Config(m),
                _ => bad(format!("Credential {name:?} does not decrypt: {key_hint}")),
            })?;
        }
    }
    if table_exists(pool, "user_totp").await? {
        let secrets: Vec<String> = sqlx::query_scalar("SELECT secret FROM user_totp")
            .fetch_all(pool)
            .await
            .context("Failed to read the copy's two-factor secrets")
            .map_err(Error::Database)?;
        for secret in secrets {
            svrctlrs_core::encryption::decrypt(&secret).map_err(|e| match e {
                Error::Config(m) => Error::Config(m),
                _ => bad(format!("A two-factor secret does not decrypt: {key_hint}")),
            })?;
        }
    }

    Ok(Inspection {
        schema_version,
        users,
        servers,
        credentials,
    })
}

/// Copy `backup` next to the database behind `database_url` as `<db>.restore`, for the next
/// start to put in place. Check it with [`inspect`] first.
pub fn stage_restore(database_url: &str, backup: &Path) -> Result<PathBuf> {
    let db = db_path(database_url)
        .ok_or_else(|| config(format!("{database_url} is not a database file")))?;
    let staged = staged_path(&db);
    let mut tmp = staged.clone().into_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::copy(backup, &tmp).map_err(io(format!(
        "Could not copy {} next to the database",
        backup.display()
    )))?;
    set_mode(&tmp, 0o600)?;
    std::fs::File::open(&tmp)
        .and_then(|f| f.sync_all())
        .map_err(io(format!("Could not flush {}", tmp.display())))?;
    std::fs::rename(&tmp, &staged).map_err(io(format!(
        "Could not move the copy to {}",
        staged.display()
    )))?;
    Ok(staged)
}

/// At start-up, before the database is opened: put a staged restore in place. The current
/// database (with its `-wal` and `-shm`, which belong to it) moves into
/// `before-restore-<time>/` beside it. `None` when nothing is staged.
pub fn apply_staged_restore(database_url: &str) -> Result<Option<Applied>> {
    let Some(db) = db_path(database_url) else {
        return Ok(None);
    };
    let staged = staged_path(&db);
    if !staged.is_file() {
        return Ok(None);
    }
    let parent = db
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let previous = parent.join(format!("before-restore-{}", Utc::now().format(STAMP)));
    std::fs::create_dir_all(&previous)
        .map_err(io(format!("Could not create {}", previous.display())))?;
    set_mode(&previous, 0o700)?;
    let file_name = db
        .file_name()
        .ok_or_else(|| config(format!("{} has no file name", db.display())))?
        .to_owned();
    for suffix in ["", "-wal", "-shm"] {
        let mut name = file_name.clone();
        name.push(suffix);
        let from = parent.join(&name);
        if from.exists() {
            std::fs::rename(&from, previous.join(&name))
                .map_err(io(format!("Could not move {} aside", from.display())))?;
        }
    }
    std::fs::rename(&staged, &db).map_err(io(format!(
        "Could not put the restored copy in place as {}",
        db.display()
    )))?;
    info!(previous = %previous.display(), "Restored the database from a backup");
    Ok(Some(Applied { previous }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn init_key() {
        if !svrctlrs_core::encryption::is_initialized() {
            let _ = svrctlrs_core::encryption::init_with_hex_key(KEY);
        }
    }

    /// A migrated database file with one encrypted credential and one sign-in.
    async fn database(dir: &Path) -> (String, Pool<Sqlite>) {
        init_key();
        let url = format!("sqlite:{}", dir.join("svrctlrs.db").display());
        let db = crate::Database::new(&url).await.unwrap();
        db.migrate().await.unwrap();
        let pool = db.pool().clone();
        crate::queries::credentials::create_credential(
            &pool,
            &crate::models::CreateCredential {
                name: "pw".into(),
                credential_type: crate::models::CredentialType::Password,
                description: None,
                value: "hunter2hunter2".into(),
                username: Some("root".into()),
                metadata: None,
            },
        )
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE tower_sessions (id TEXT PRIMARY KEY, data BLOB, expiry_date INTEGER)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO tower_sessions VALUES ('live-cookie-id', x'00', 0)")
            .execute(&pool)
            .await
            .unwrap();
        (url, pool)
    }

    #[test]
    fn db_path_reads_the_url_shapes() {
        assert_eq!(
            db_path("sqlite:data/svrctlrs.db"),
            Some("data/svrctlrs.db".into())
        );
        assert_eq!(
            db_path("sqlite:/app/data/x.db"),
            Some("/app/data/x.db".into())
        );
        assert_eq!(
            db_path("sqlite:///app/data/x.db"),
            Some("/app/data/x.db".into())
        );
        assert_eq!(
            db_path("sqlite:data/x.db?mode=rwc"),
            Some("data/x.db".into())
        );
        assert_eq!(db_path("sqlite::memory:"), None);
        assert_eq!(db_path("postgres://x"), None);
        assert_eq!(
            staged_path(Path::new("/app/data/svrctlrs.db")),
            PathBuf::from("/app/data/svrctlrs.db.restore")
        );
    }

    #[tokio::test]
    async fn a_backup_is_a_private_copy_without_sign_ins() {
        let tmp = tempfile::tempdir().unwrap();
        let (_url, pool) = database(tmp.path()).await;
        let dir = tmp.path().join("backups");
        let created = create(&pool, &dir, 14).await.unwrap();
        assert!(created.file.name.starts_with("svrctlrs-") && created.file.name.ends_with(".db"));
        assert!(created.file.bytes > 0);
        assert_eq!(created.removed, 0);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&dir), 0o700);
            assert_eq!(mode(&created.file.path), 0o600);
        }
        // Nothing half-written is left, and the copy is one file (not WAL)
        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, vec![created.file.name.clone()]);

        let copy = open_file(&created.file.path, true).await.unwrap();
        let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tower_sessions")
            .fetch_one(&copy)
            .await
            .unwrap();
        assert_eq!(sessions, 0, "sign-ins must not travel in a backup");
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&copy)
            .await
            .unwrap();
        assert_eq!(mode, "delete");
        copy.close().await;
        // Nor in its free pages
        let raw = std::fs::read(&created.file.path).unwrap();
        assert!(!raw.windows(14).any(|w| w == b"live-cookie-id"));

        // The live database still has its sign-in
        let live: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tower_sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(live, 1);
    }

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"x").unwrap();
    }

    #[test]
    fn lists_newest_first_and_prunes_beyond_keep() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for name in [
            "svrctlrs-20261001-030000Z.db",
            "svrctlrs-20261003-030000Z.db",
            "svrctlrs-20261002-030000Z.db",
            "notes.txt",
            "svrctlrs.db",
            ".svrctlrs-20261004-030000Z.db.tmp",
        ] {
            touch(dir, name);
        }
        let names: Vec<String> = list(dir).unwrap().into_iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            [
                "svrctlrs-20261003-030000Z.db",
                "svrctlrs-20261002-030000Z.db",
                "svrctlrs-20261001-030000Z.db"
            ]
        );
        assert_eq!(prune(dir, 0).unwrap(), 0);
        assert_eq!(prune(dir, 2).unwrap(), 1);
        assert!(!dir.join("svrctlrs-20261001-030000Z.db").exists());
        assert!(dir.join("notes.txt").exists() && dir.join("svrctlrs.db").exists());
        remove_leftovers(dir);
        assert!(!dir.join(".svrctlrs-20261004-030000Z.db.tmp").exists());
        assert!(list(&dir.join("absent")).unwrap().is_empty());
    }

    #[tokio::test]
    async fn retention_runs_after_the_new_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let (_url, pool) = database(tmp.path()).await;
        let dir = tmp.path().join("backups");
        std::fs::create_dir_all(&dir).unwrap();
        for day in 1..=3 {
            touch(&dir, &format!("svrctlrs-2026100{day}-030000Z.db"));
        }
        let created = create(&pool, &dir, 2).await.unwrap();
        assert_eq!(created.removed, 2);
        let names: Vec<String> = list(&dir).unwrap().into_iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            [
                created.file.name.clone(),
                "svrctlrs-20261003-030000Z.db".into()
            ]
        );
    }

    #[tokio::test]
    async fn inspect_accepts_a_good_copy_and_refuses_the_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let (_url, pool) = database(tmp.path()).await;
        let created = create(&pool, &tmp.path().join("b"), 0).await.unwrap();
        let found = inspect(&created.file.path).await.unwrap();
        assert_eq!(found.schema_version, latest_schema_version());
        assert_eq!(found.credentials, 1);

        // Not a database
        let junk = tmp.path().join("junk.db");
        std::fs::write(&junk, b"this is not sqlite at all, not even close......").unwrap();
        assert!(matches!(inspect(&junk).await, Err(Error::InvalidInput(_))));
        assert!(matches!(
            inspect(&tmp.path().join("missing.db")).await,
            Err(Error::InvalidInput(_))
        ));

        // A credential encrypted under another key
        let other = tmp.path().join("other.db");
        std::fs::copy(&created.file.path, &other).unwrap();
        let rw = open_file(&other, false).await.unwrap();
        // 12-byte nonce + 16-byte tag + data, valid base64, but not from this key
        sqlx::query(
            "UPDATE credentials SET value = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'",
        )
        .execute(&rw)
        .await
        .unwrap();
        sqlx::query("INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (9999, 'future', 1, x'00', 0)")
            .execute(&rw)
            .await
            .unwrap();
        rw.close().await;
        let err = inspect(&other).await.unwrap_err().to_string();
        assert!(err.contains("newer than this version knows"), "{err}");

        let rw = open_file(&other, false).await.unwrap();
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 9999")
            .execute(&rw)
            .await
            .unwrap();
        rw.close().await;
        let err = inspect(&other).await.unwrap_err().to_string();
        assert!(err.contains("another encryption key"), "{err}");
    }

    #[tokio::test]
    async fn a_staged_restore_replaces_the_database_at_the_next_start() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, pool) = database(tmp.path()).await;
        let created = create(&pool, &tmp.path().join("b"), 0).await.unwrap();
        // A change after the backup, which the restore must undo
        crate::queries::settings::set_setting(&pool, "backup.keep", "5")
            .await
            .unwrap();
        pool.close().await;

        assert!(
            apply_staged_restore(&url).unwrap().is_none(),
            "nothing staged yet"
        );
        let staged = stage_restore(&url, &created.file.path).unwrap();
        assert!(staged.ends_with("svrctlrs.db.restore"));
        let applied = apply_staged_restore(&url).unwrap().unwrap();
        assert!(!staged.exists());
        assert!(applied.previous.join("svrctlrs.db").is_file());
        assert!(!tmp.path().join("svrctlrs.db-wal").exists());

        let db = crate::Database::new(&url).await.unwrap();
        db.migrate().await.unwrap();
        let keep = crate::queries::settings::get_setting_value_or(db.pool(), "backup.keep", "")
            .await
            .unwrap();
        assert_eq!(keep, "14", "the restored copy predates the change");
        let cred = crate::queries::credentials::get_credential_by_name(db.pool(), "pw")
            .await
            .unwrap();
        assert_eq!(cred.value, "hunter2hunter2");
    }
}

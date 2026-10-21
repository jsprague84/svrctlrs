//! Nightly backups, run by the server itself (ROADMAP 24; the copy is
//! `svrctlrs_database::backup`, shared with `svrctl backup`).
//!
//! Every night at `backup.hour` (UTC, default 3) the server writes a copy of its database into
//! `BACKUP_DIR` (default `data/backups`, beside the database) and keeps the newest
//! `backup.keep` copies (default 14; 0 turns nightly copies off). Each daily slot gets one
//! attempt: a server that was down at that hour makes it a few minutes after it starts, and a
//! restart after the attempt does not repeat it. Every attempt (nightly, "Back up now",
//! `svrctl backup`) leaves a row in `backup_runs`, which About shows. A failed nightly copy is
//! notified once (Gotify/ntfy, when notifications are on), and so is the first good one after.
//! The folder comes from the environment only: no request can point a copy anywhere else.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Days, Utc};
use svrctlrs_core::notifications::NotificationClient;
use svrctlrs_database::backup::{self, Created};
use svrctlrs_database::models::NewBackupRun;
use svrctlrs_database::queries::{backup_runs, settings};
use svrctlrs_database::{SqlxPool, SqlxSqlite};
use tracing::{error, info, warn};

type Pool = SqlxPool<SqlxSqlite>;

/// Default `backup.keep`.
pub const DEFAULT_KEEP: i64 = 14;
/// Default `backup.hour` (UTC).
pub const DEFAULT_HOUR: u32 = 3;
/// The most copies `backup.keep` may ask for.
pub const MAX_KEEP: i64 = 365;
/// The first check after a start: late enough that a deploy is over.
const FIRST_CHECK: Duration = Duration::from_secs(120);
/// How often the schedule looks at the clock.
const CHECK_EVERY: Duration = Duration::from_secs(300);

/// Where copies go, and whether one is being written.
pub struct Backups {
    dir: PathBuf,
    key_source: &'static str,
    running: tokio::sync::Mutex<()>,
    /// The last nightly attempt failed (and that was notified)
    failing: AtomicBool,
}

impl Backups {
    pub fn new(dir: impl Into<PathBuf>) -> Arc<Self> {
        let key_source = if std::env::var("ENCRYPTION_KEY").is_ok_and(|k| !k.trim().is_empty()) {
            "environment"
        } else {
            "file"
        };
        Arc::new(Self {
            dir: dir.into(),
            key_source,
            running: tokio::sync::Mutex::new(()),
            failing: AtomicBool::new(false),
        })
    }

    /// `BACKUP_DIR`, default `data/backups` (beside the database, like `RECORDINGS_DIR`).
    pub fn from_env() -> Arc<Self> {
        let dir = std::env::var("BACKUP_DIR")
            .ok()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "data/backups".to_string());
        Self::new(dir)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// `environment` (`ENCRYPTION_KEY`) or `file` (`data/encryption.key`).
    pub fn key_source(&self) -> &'static str {
        self.key_source
    }

    pub fn is_running(&self) -> bool {
        self.running.try_lock().is_err()
    }
}

/// `backup.keep`, read leniently: anything unparsable or out of range is the default.
pub async fn keep(pool: &Pool) -> i64 {
    settings::get_setting_value_or(pool, "backup.keep", &DEFAULT_KEEP.to_string())
        .await
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|k| (0..=MAX_KEEP).contains(k))
        .unwrap_or(DEFAULT_KEEP)
}

/// `backup.hour`, read leniently.
pub async fn hour(pool: &Pool) -> u32 {
    settings::get_setting_value_or(pool, "backup.hour", &DEFAULT_HOUR.to_string())
        .await
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|h| *h < 24)
        .unwrap_or(DEFAULT_HOUR)
}

/// The latest nightly slot at or before `now`.
pub fn latest_slot(now: DateTime<Utc>, hour: u32) -> DateTime<Utc> {
    let today = now
        .date_naive()
        .and_hms_opt(hour, 0, 0)
        .map(|t| t.and_utc())
        .unwrap_or(now);
    if today <= now {
        today
    } else {
        today.checked_sub_days(Days::new(1)).unwrap_or(today)
    }
}

/// A slot gets one attempt: due when the last nightly attempt started before the latest slot.
pub fn due(now: DateTime<Utc>, hour: u32, last_attempt: Option<DateTime<Utc>>) -> bool {
    last_attempt.is_none_or(|t| t < latest_slot(now, hour))
}

/// When the next nightly copy runs: the latest slot if it has not been tried (the schedule
/// catches up within minutes), otherwise the next one.
pub fn next_run(
    now: DateTime<Utc>,
    hour: u32,
    last_attempt: Option<DateTime<Utc>>,
) -> DateTime<Utc> {
    let slot = latest_slot(now, hour);
    if due(now, hour, last_attempt) {
        slot
    } else {
        slot.checked_add_days(Days::new(1)).unwrap_or(slot)
    }
}

/// Write a copy now and record the attempt. One at a time: a second caller waits for the
/// first. Retention follows `backup.keep` (0 keeps every copy).
pub async fn run(pool: &Pool, backups: &Backups, trigger: &str) -> svrctlrs_core::Result<Created> {
    let _one_at_a_time = backups.running.lock().await;
    let keep = keep(pool).await as usize;
    let started_at = Utc::now();
    let result = backup::create(pool, &backups.dir, keep).await;
    let detail = result.as_ref().err().map(|e| e.detail());
    let record = NewBackupRun {
        trigger,
        started_at,
        file_name: result.as_ref().ok().map(|c| c.file.name.as_str()),
        size_bytes: result.as_ref().ok().map(|c| c.file.bytes as i64),
        removed: result.as_ref().map(|c| c.removed as i64).unwrap_or(0),
        error: detail.as_deref(),
    };
    if let Err(e) = backup_runs::record(pool, &record).await {
        warn!(error = %e, "Could not record the backup attempt");
    }
    result
}

/// The nightly schedule; see the module docs.
pub fn start_schedule(pool: Pool, backups: Arc<Backups>) {
    tokio::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        if let Ok(Some(last)) = backup_runs::latest(&pool, Some("nightly")).await {
            backups
                .failing
                .store(last.error.is_some(), Ordering::Relaxed);
        }
        loop {
            tick(&pool, &backups, Utc::now()).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

async fn tick(pool: &Pool, backups: &Backups, now: DateTime<Utc>) {
    if keep(pool).await == 0 {
        return;
    }
    let hour = hour(pool).await;
    let last = match backup_runs::latest(pool, Some("nightly")).await {
        Ok(last) => last.map(|r| r.started_at),
        Err(e) => {
            error!(error = %e, "Could not read the backup history; skipping this check");
            return;
        }
    };
    if !due(now, hour, last) {
        return;
    }
    match run(pool, backups, "nightly").await {
        Ok(created) => {
            if backups.failing.swap(false, Ordering::Relaxed) {
                notify(
                    pool,
                    "Backups working again",
                    &format!("Nightly backup {} written", created.file.name),
                    3,
                )
                .await;
            }
        }
        Err(e) => {
            error!(error = %e.detail(), "Nightly backup failed");
            if !backups.failing.swap(true, Ordering::Relaxed) {
                notify(
                    pool,
                    "Backup failed",
                    &format!("The nightly database backup failed: {}", e.detail()),
                    8,
                )
                .await;
            }
        }
    }
}

async fn notify(pool: &Pool, title: &str, message: &str, priority: u8) {
    let config = match settings::get_notification_config(pool).await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Could not read the notification settings");
            return;
        }
    };
    if let Err(e) = NotificationClient::new(config)
        .send(title, message, priority)
        .await
    {
        warn!(error = %e, "Could not send the backup notification");
    } else {
        info!(title, "Backup notification sent");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn the_latest_slot_is_today_or_yesterday() {
        assert_eq!(
            latest_slot(at("2026-10-22T03:00:00Z"), 3),
            at("2026-10-22T03:00:00Z")
        );
        assert_eq!(
            latest_slot(at("2026-10-22T14:20:00Z"), 3),
            at("2026-10-22T03:00:00Z")
        );
        assert_eq!(
            latest_slot(at("2026-10-22T02:59:59Z"), 3),
            at("2026-10-21T03:00:00Z")
        );
        assert_eq!(
            latest_slot(at("2026-11-01T00:10:00Z"), 23),
            at("2026-10-31T23:00:00Z")
        );
    }

    #[test]
    fn one_attempt_per_slot_and_a_catch_up_after_downtime() {
        let hour = 3;
        // Never tried: due at once (a fresh install makes its first copy minutes after start)
        assert!(due(at("2026-10-22T14:00:00Z"), hour, None));
        // Tried at 03:02 today: not again today, whatever restarts happen
        let tried = Some(at("2026-10-22T03:02:00Z"));
        assert!(!due(at("2026-10-22T03:07:00Z"), hour, tried));
        assert!(!due(at("2026-10-23T02:59:00Z"), hour, tried));
        assert!(due(at("2026-10-23T03:00:00Z"), hour, tried));
        // Down from 01:00 to 09:00: the 03:00 slot is made up when the server is back
        let yesterday = Some(at("2026-10-21T03:01:00Z"));
        assert!(due(at("2026-10-22T09:02:00Z"), hour, yesterday));
        // The hour moved earlier after today's copy: tomorrow at the new hour
        assert!(!due(at("2026-10-22T14:00:00Z"), 1, tried));
    }

    #[test]
    fn next_run_says_when() {
        let tried = Some(at("2026-10-22T03:02:00Z"));
        assert_eq!(
            next_run(at("2026-10-22T14:00:00Z"), 3, tried),
            at("2026-10-23T03:00:00Z")
        );
        assert_eq!(
            next_run(at("2026-10-22T14:00:00Z"), 3, None),
            at("2026-10-22T03:00:00Z")
        );
    }

    async fn pool() -> Pool {
        let pool = svrctlrs_database::sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        svrctlrs_database::run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn settings_are_read_leniently() {
        let pool = pool().await;
        assert_eq!((keep(&pool).await, hour(&pool).await), (14, 3));
        settings::set_setting(&pool, "backup.keep", "lots")
            .await
            .unwrap();
        settings::set_setting(&pool, "backup.hour", "25")
            .await
            .unwrap();
        assert_eq!((keep(&pool).await, hour(&pool).await), (14, 3));
        settings::set_setting(&pool, "backup.keep", "0")
            .await
            .unwrap();
        settings::set_setting(&pool, "backup.hour", "23")
            .await
            .unwrap();
        assert_eq!((keep(&pool).await, hour(&pool).await), (0, 23));
    }

    #[tokio::test]
    async fn a_tick_backs_up_once_per_slot_and_records_failures() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("live.db");
        let database = svrctlrs_database::Database::new(&format!("sqlite:{}", db.display()))
            .await
            .unwrap();
        database.migrate().await.unwrap();
        let pool = database.pool().clone();
        let backups = Backups::new(tmp.path().join("backups"));

        let now = Utc::now();
        tick(&pool, &backups, now).await;
        tick(&pool, &backups, now).await;
        assert_eq!(backup::list(backups.dir()).unwrap().len(), 1);
        let last = backup_runs::latest(&pool, Some("nightly"))
            .await
            .unwrap()
            .unwrap();
        assert!(last.error.is_none() && last.file_name.is_some());

        // Nightly off: nothing, even when due
        settings::set_setting(&pool, "backup.keep", "0")
            .await
            .unwrap();
        tick(&pool, &backups, now + chrono::Duration::days(1)).await;
        assert_eq!(backup::list(backups.dir()).unwrap().len(), 1);

        // A folder that cannot be created: the attempt is recorded with its error
        settings::set_setting(&pool, "backup.keep", "14")
            .await
            .unwrap();
        std::fs::write(tmp.path().join("blocked"), b"a file, not a folder").unwrap();
        let blocked = Backups::new(tmp.path().join("blocked").join("backups"));
        tick(&pool, &blocked, now + chrono::Duration::days(1)).await;
        let last = backup_runs::latest(&pool, None).await.unwrap().unwrap();
        assert!(last.error.is_some(), "{last:?}");
        assert!(blocked.failing.load(Ordering::Relaxed));
        let good = backup_runs::latest_success(&pool).await.unwrap().unwrap();
        assert_eq!(good.trigger, "nightly");
    }
}

//! Session recording.
//!
//! A server with `record_sessions` set has every shell opened on it recorded for the user who
//! opened it: what the terminal showed (the shell's output, never the keystrokes, so a password
//! typed at a prompt is not in it) as an [asciicast v2] file with the terminal's resizes. A
//! recorder task follows the live session the way an attached socket does
//! ([`crate::sessions::Follower`]), so a recording is complete whatever the clients do.
//!
//! On disk a recording is `RECORDINGS_DIR/<user id>/<random>.cast.enc` (directory 0700, file
//! 0600): one line per chunk of whole asciicast lines, each chunk encrypted with the
//! credential key (AES-256-GCM, base64; `core/src/encryption.rs`), so the files are as useless
//! without the key as the stored credentials. [`read_cast`] turns a file back into a plain
//! `.cast`; the API serves that to the recording's owner and nobody else.
//!
//! Limits and housekeeping: a recording stops at `RECORDING_MAX_MB` (default 100) of asciicast
//! and the shell goes on unrecorded; at start-up, recordings a stopped server left running are
//! marked interrupted; every hour, finished recordings older than the
//! `recording.retention_days` setting (default 30, 0 keeps them) are deleted with their files,
//! and recording files that no row knows are removed.
//!
//! [asciicast v2]: https://docs.asciinema.org/manual/asciicast/v2/

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::Stream;
use svrctlrs_database::models::{NewRecording, RecordingStatus};
use svrctlrs_database::queries::{recordings, settings};
use svrctlrs_database::{SqlxPool, SqlxSqlite};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_util::task::TaskTracker;
use tracing::{error, info, warn};

use crate::sessions::{Followed, LiveSession};

type Pool = SqlxPool<SqlxSqlite>;

/// The file suffix of a recording; housekeeping only ever deletes files that carry it.
const SUFFIX: &str = ".cast.enc";
/// Plaintext gathered before a chunk is encrypted and written, besides the time-based flush.
const CHUNK_BYTES: usize = 64 * 1024;
/// How often buffered lines are written out (and readable by a download in progress).
const FLUSH_EVERY: Duration = Duration::from_secs(1);
/// How often a running recording's duration and size are written to its row.
const PROGRESS_EVERY: Duration = Duration::from_secs(15);
/// Default `recording.retention_days`.
pub const DEFAULT_RETENTION_DAYS: i64 = 30;

/// Where recordings live, how large one may grow, and the recorder tasks still writing.
pub struct RecordingStore {
    dir: PathBuf,
    max_bytes: u64,
    tasks: TaskTracker,
}

impl RecordingStore {
    pub const DEFAULT_MAX_MB: u64 = 100;

    pub fn new(dir: impl Into<PathBuf>, max_bytes: u64) -> Arc<Self> {
        Arc::new(Self {
            dir: dir.into(),
            max_bytes: max_bytes.max(1),
            tasks: TaskTracker::new(),
        })
    }

    /// `RECORDINGS_DIR` (default `data/recordings`, beside the database) and
    /// `RECORDING_MAX_MB` (default 100).
    pub fn from_env() -> Arc<Self> {
        let dir = std::env::var("RECORDINGS_DIR")
            .ok()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "data/recordings".to_string());
        let max_mb = std::env::var("RECORDING_MAX_MB")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|mb| *mb > 0)
            .unwrap_or(Self::DEFAULT_MAX_MB);
        Self::new(dir, max_mb * 1024 * 1024)
    }

    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    /// The file behind a recording row. Both parts come from the database, never from a
    /// request; a name with a path separator is refused all the same.
    pub fn path_of(&self, user_id: i64, file_name: &str) -> Option<PathBuf> {
        if file_name.is_empty() || file_name.contains(['/', '\\']) || file_name.starts_with('.') {
            return None;
        }
        Some(self.dir.join(user_id.to_string()).join(file_name))
    }

    /// Delete a recording's file; one that is already gone is fine.
    pub async fn remove_file(&self, user_id: i64, file_name: &str) {
        let Some(path) = self.path_of(user_id, file_name) else {
            return;
        };
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                warn!(path = %path.display(), error = %e, "Could not delete a recording file")
            }
        }
    }

    /// Shutdown: give running recorders up to `timeout` to write their last lines (their
    /// sessions have been closed, so they are finishing).
    pub async fn finish_all(&self, timeout: Duration) {
        self.tasks.close();
        if tokio::time::timeout(timeout, self.tasks.wait())
            .await
            .is_err()
        {
            warn!("Recorders still writing at shutdown; their recordings will read as interrupted");
        }
    }
}

/// Start recording a session whose shell is about to start. Returns the recording id. On an
/// error nothing is recorded (a row, if one was made, says why) and the caller tells the
/// user; the shell itself goes ahead.
pub async fn start(
    pool: &Pool,
    store: &Arc<RecordingStore>,
    live: &Arc<LiveSession>,
    cols: u32,
    rows: u32,
) -> Result<i64, String> {
    let file_name = new_file_name();
    // The row first: housekeeping deletes files that no row knows.
    let id = recordings::create(
        pool,
        &NewRecording {
            user_id: live.user_id,
            server_id: live.server_id,
            server_name: live.server_name.clone(),
            session_id: live.id.clone(),
            file_name: file_name.clone(),
            cols: i64::from(cols),
            rows: i64::from(rows),
        },
    )
    .await
    .map_err(|e| format!("could not register the recording: {e}"))?;

    let file = match create_file(store, live.user_id, &file_name).await {
        Ok(f) => f,
        Err(e) => {
            let reason = format!("could not create the recording file: {e}");
            let _ =
                recordings::finish(pool, id, RecordingStatus::Failed, 0, 0, Some(&reason)).await;
            return Err(reason);
        }
    };

    // From the shell's first frame: whatever it printed before this point is in the ring,
    // and times count from the session's start.
    let follow = live.follow(0);
    live.set_recorded(true);
    let started = live.started;
    let mut writer = CastWriter::new(file);
    writer.header(cols, rows, &live.server_name, live.created_at.timestamp());
    let recorder = Recorder {
        pool: pool.clone(),
        id,
        live: Arc::clone(live),
        max_bytes: store.max_bytes,
        started,
        carry: Vec::new(),
        writer,
    };
    store.tasks.spawn(recorder.run(follow));
    info!(recording_id = id, session_id = %live.id, server = %live.server_name, "Recording session");
    Ok(id)
}

async fn create_file(
    store: &RecordingStore,
    user_id: i64,
    file_name: &str,
) -> std::io::Result<tokio::fs::File> {
    let path = store
        .path_of(user_id, file_name)
        .ok_or_else(|| std::io::Error::other("bad file name"))?;
    if let Some(dir) = path.parent() {
        let mut builder = tokio::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        builder.mode(0o700);
        builder.create(dir).await?;
    }
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(&path).await
}

fn new_file_name() -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    format!(
        "{}{SUFFIX}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    )
}

/// One recording being written.
struct Recorder {
    pool: Pool,
    id: i64,
    live: Arc<LiveSession>,
    max_bytes: u64,
    started: Instant,
    /// The start of a UTF-8 sequence the last frame cut off
    carry: Vec<u8>,
    writer: CastWriter,
}

/// How a recording ended.
struct Ending {
    status: RecordingStatus,
    reason: String,
}

impl Recorder {
    async fn run(mut self, mut follow: crate::sessions::Follower) {
        let mut flush = tokio::time::interval(FLUSH_EVERY);
        flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_progress = Instant::now();

        let ending = loop {
            let step = tokio::select! {
                item = follow.next() => self.take(item),
                _ = flush.tick() => {
                    if last_progress.elapsed() >= PROGRESS_EVERY {
                        last_progress = Instant::now();
                        let _ = recordings::update_progress(
                            &self.pool,
                            self.id,
                            self.elapsed_ms(),
                            self.writer.plain_bytes() as i64,
                        )
                        .await;
                    }
                    self.writer.flush().await.map(|()| None).map_err(failed)
                }
            };
            match step {
                Ok(None) => {}
                Ok(Some(ending)) | Err(ending) => break ending,
            }
            if self.writer.plain_bytes() >= self.max_bytes {
                let mb = self.max_bytes / (1024 * 1024);
                let t = self.started.elapsed();
                self.writer
                    .event(t, "m", &format!("Recording stopped at the {mb} MB limit"));
                break Ending {
                    status: RecordingStatus::Truncated,
                    reason: format!("Stopped at the {mb} MB size limit; the shell went on"),
                };
            }
            if self.writer.buffered() >= CHUNK_BYTES {
                if let Err(e) = self.writer.flush().await {
                    break failed(e);
                }
            }
        };

        // What a cut-off final frame left behind, as best it can be shown
        if !self.carry.is_empty() {
            let rest = String::from_utf8_lossy(&std::mem::take(&mut self.carry)).into_owned();
            self.writer.event(self.started.elapsed(), "o", &rest);
        }
        let ending = match self.writer.flush().await {
            Ok(()) => ending,
            Err(e) => failed(e),
        };
        if ending.status != RecordingStatus::Complete {
            self.live.set_recorded(false);
        }
        match ending.status {
            RecordingStatus::Failed => {
                error!(recording_id = self.id, reason = %ending.reason, "Recording failed")
            }
            _ => info!(
                recording_id = self.id,
                status = ending.status.as_str(),
                bytes = self.writer.plain_bytes(),
                "Recording finished"
            ),
        }
        if let Err(e) = recordings::finish(
            &self.pool,
            self.id,
            ending.status,
            self.elapsed_ms(),
            self.writer.plain_bytes() as i64,
            Some(&ending.reason),
        )
        .await
        {
            error!(recording_id = self.id, error = %e, "Could not finish the recording's row");
        }
    }

    /// Write one followed item; `Some` once the recording is over.
    fn take(&mut self, item: Option<Followed>) -> Result<Option<Ending>, Ending> {
        match item {
            Some(Followed::Output(frame)) => {
                let text = decode_utf8_stream(&mut self.carry, &frame.data);
                if !text.is_empty() {
                    let t = frame.at.saturating_duration_since(self.started);
                    self.writer.event(t, "o", &text);
                }
                Ok(None)
            }
            Some(Followed::Resized { cols, rows, at }) => {
                let t = at.saturating_duration_since(self.started);
                self.writer.event(t, "r", &format!("{cols}x{rows}"));
                Ok(None)
            }
            Some(Followed::WindowLost) => {
                // Only if writing stalled for longer than the ring covers
                let t = self.started.elapsed();
                self.writer
                    .event(t, "m", "Output lost here: the recorder fell behind");
                Ok(None)
            }
            Some(Followed::Closed(reason)) => Ok(Some(Ending {
                status: RecordingStatus::Complete,
                reason: reason.to_string(),
            })),
            // A kept shell let go for a restart (or a lost connection): this recording ends
            // here, and a reattach starts the next one
            Some(Followed::Released) => Ok(Some(Ending {
                status: RecordingStatus::Complete,
                reason: "Kept shell disconnected (it lives on in tmux)".to_string(),
            })),
            None => Ok(Some(Ending {
                status: RecordingStatus::Complete,
                reason: "Session ended".to_string(),
            })),
        }
    }

    fn elapsed_ms(&self) -> i64 {
        i64::try_from(self.started.elapsed().as_millis()).unwrap_or(i64::MAX)
    }
}

fn failed(error: String) -> Ending {
    Ending {
        status: RecordingStatus::Failed,
        reason: format!("Writing the recording failed: {error}"),
    }
}

/// Buffers asciicast lines and writes them as encrypted chunks, one per line of the file.
struct CastWriter {
    file: tokio::fs::File,
    buf: String,
    plain_bytes: u64,
}

impl CastWriter {
    fn new(file: tokio::fs::File) -> Self {
        Self {
            file,
            buf: String::new(),
            plain_bytes: 0,
        }
    }

    fn line(&mut self, line: &str) {
        self.buf.push_str(line);
        self.buf.push('\n');
        self.plain_bytes += line.len() as u64 + 1;
    }

    fn header(&mut self, cols: u32, rows: u32, title: &str, unix_time: i64) {
        let header = serde_json::json!({
            "version": 2,
            "width": cols,
            "height": rows,
            "timestamp": unix_time,
            "title": title,
            "env": { "TERM": "xterm-256color" },
        });
        self.line(&header.to_string());
    }

    /// `[time, kind, data]`: `o` output, `r` resize (`COLSxROWS`), `m` marker.
    fn event(&mut self, at: Duration, kind: &str, data: &str) {
        let secs = at.as_micros() as f64 / 1_000_000.0;
        let line = serde_json::json!([secs, kind, data]);
        self.line(&line.to_string());
    }

    fn buffered(&self) -> usize {
        self.buf.len()
    }

    fn plain_bytes(&self) -> u64 {
        self.plain_bytes
    }

    async fn flush(&mut self) -> Result<(), String> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let mut chunk = svrctlrs_core::encryption::encrypt(&self.buf).map_err(|e| e.to_string())?;
        chunk.push('\n');
        self.file
            .write_all(chunk.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.file.flush().await.map_err(|e| e.to_string())?;
        self.buf.clear();
        Ok(())
    }
}

/// Decode a stream of UTF-8 split at arbitrary byte boundaries: whole characters come out, a
/// sequence cut off at the end of `bytes` waits in `carry` for the next call, and invalid
/// bytes become U+FFFD. asciicast data is text, and PTY reads split characters.
pub fn decode_utf8_stream(carry: &mut Vec<u8>, bytes: &[u8]) -> String {
    let mut buf = std::mem::take(carry);
    buf.extend_from_slice(bytes);
    let mut out = String::with_capacity(buf.len());
    let mut rest: &[u8] = &buf;
    loop {
        match std::str::from_utf8(rest) {
            Ok(s) => {
                out.push_str(s);
                break;
            }
            Err(e) => {
                let (valid, after) = rest.split_at(e.valid_up_to());
                out.push_str(&String::from_utf8_lossy(valid));
                match e.error_len() {
                    // Incomplete at the end: the next frame has the rest
                    None => {
                        carry.extend_from_slice(after);
                        break;
                    }
                    Some(bad) => {
                        out.push(char::REPLACEMENT_CHARACTER);
                        rest = &after[bad..];
                    }
                }
            }
        }
    }
    out
}

/// A recording file as plain asciicast, chunk by chunk. A line that does not decrypt (the
/// end of a recording still being written, or damage) ends the stream there.
pub fn read_cast(path: PathBuf) -> impl Stream<Item = std::io::Result<String>> + Send + 'static {
    futures_util::stream::try_unfold(None, move |lines| {
        let path = path.clone();
        async move {
            let mut lines = match lines {
                Some(lines) => lines,
                None => BufReader::new(tokio::fs::File::open(&path).await?).lines(),
            };
            loop {
                let Some(line) = lines.next_line().await? else {
                    return Ok(None);
                };
                if line.trim().is_empty() {
                    continue;
                }
                return match svrctlrs_core::encryption::decrypt(line.trim()) {
                    Ok(chunk) => Ok(Some((chunk, Some(lines)))),
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "Recording chunk does not decrypt; stopping there");
                        Ok(None)
                    }
                };
            }
        }
    })
}

/// Start-up and hourly housekeeping; see the module docs.
pub fn start_housekeeping(pool: Pool, store: Arc<RecordingStore>) {
    tokio::spawn(async move {
        match recordings::mark_interrupted(&pool).await {
            Ok(0) => {}
            Ok(n) => info!(
                count = n,
                "Marked recordings left running by the last start as interrupted"
            ),
            Err(e) => error!(error = %e, "Could not mark interrupted recordings"),
        }
        loop {
            match sweep(&pool, &store).await {
                Ok((0, 0)) => {}
                Ok((expired, orphans)) => {
                    info!(expired, orphans, "Removed old and orphaned recordings")
                }
                Err(e) => error!(error = %e, "Recording housekeeping failed"),
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}

/// `recording.retention_days`, read leniently: anything unparsable is the default.
pub async fn retention_days(pool: &Pool) -> i64 {
    settings::get_setting_value_or(
        pool,
        "recording.retention_days",
        &DEFAULT_RETENTION_DAYS.to_string(),
    )
    .await
    .ok()
    .and_then(|v| v.trim().parse::<i64>().ok())
    .filter(|d| *d >= 0)
    .unwrap_or(DEFAULT_RETENTION_DAYS)
}

/// Delete expired recordings and orphaned files. Returns how many of each.
async fn sweep(pool: &Pool, store: &RecordingStore) -> svrctlrs_core::Result<(usize, usize)> {
    let mut expired = 0;
    let days = retention_days(pool).await;
    if days > 0 {
        for r in recordings::older_than_days(pool, days).await? {
            store.remove_file(r.user_id, &r.file_name).await;
            recordings::delete(pool, r.id).await?;
            expired += 1;
        }
    }

    let known: HashSet<(i64, String)> = recordings::all_files(pool).await?.into_iter().collect();
    let orphans = remove_orphans(&store.dir, &known).await;
    Ok((expired, orphans))
}

/// Recording files under `dir/<user id>/` that no row knows.
async fn remove_orphans(dir: &Path, known: &HashSet<(i64, String)>) -> usize {
    let mut removed = 0;
    let Ok(mut users) = tokio::fs::read_dir(dir).await else {
        return 0; // nothing recorded yet
    };
    while let Ok(Some(user_dir)) = users.next_entry().await {
        let Some(user_id) = user_dir
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<i64>().ok())
        else {
            continue;
        };
        let Ok(mut files) = tokio::fs::read_dir(user_dir.path()).await else {
            continue;
        };
        while let Ok(Some(file)) = files.next_entry().await {
            let Some(name) = file.file_name().to_str().map(String::from) else {
                continue;
            };
            if !name.ends_with(SUFFIX) || known.contains(&(user_id, name.clone())) {
                continue;
            }
            match tokio::fs::remove_file(file.path()).await {
                Ok(()) => removed += 1,
                Err(e) => {
                    warn!(path = %file.path().display(), error = %e, "Could not remove an orphaned recording")
                }
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{database, ensure_encryption};
    use futures_util::StreamExt;
    use svrctlrs_database::queries;
    use tokio::sync::mpsc;

    #[test]
    fn utf8_split_across_frames_is_reassembled_and_bad_bytes_are_replaced() {
        let mut carry = Vec::new();
        // "╔" is E2 95 94; the first frame ends after two of its bytes
        assert_eq!(decode_utf8_stream(&mut carry, b"ab\xE2\x95"), "ab");
        assert_eq!(carry, [0xE2, 0x95]);
        assert_eq!(decode_utf8_stream(&mut carry, b"\x94cd"), "╔cd");
        assert!(carry.is_empty());
        assert_eq!(decode_utf8_stream(&mut carry, b"x\xFFy"), "x\u{FFFD}y");
        // An emoji split three ways
        let crab = "🦀".as_bytes();
        let mut out = String::new();
        for b in crab {
            out += &decode_utf8_stream(&mut carry, std::slice::from_ref(b));
        }
        assert_eq!(out, "🦀");
    }

    struct Fixture {
        pool: Pool,
        store: Arc<RecordingStore>,
        live: Arc<LiveSession>,
        _pty: mpsc::Receiver<crate::sessions::PtyCommand>,
        _dir: tempfile::TempDir,
    }

    async fn fixture(max_bytes: u64) -> Fixture {
        ensure_encryption();
        let pool = database().await;
        svrctlrs_database::sqlx::query(
            "INSERT INTO users (username, password_hash) VALUES ('ann', 'x')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let server = crate::test_support::add_server(&pool, "web", 22, "ann", None, None).await;
        let dir = tempfile::tempdir().unwrap();
        let store = RecordingStore::new(dir.path().join("recordings"), max_bytes);
        let (tx, rx) = mpsc::channel(64);
        let live = LiveSession::new(1, server, "web".into(), tx);
        Fixture {
            pool,
            store,
            live,
            _pty: rx,
            _dir: dir,
        }
    }

    async fn cast_text(store: &RecordingStore, user_id: i64, file: &str) -> String {
        let path = store.path_of(user_id, file).unwrap();
        let chunks: Vec<String> = read_cast(path)
            .map(|c| c.unwrap())
            .collect::<Vec<_>>()
            .await;
        chunks.concat()
    }

    fn events(cast: &str) -> (serde_json::Value, Vec<(f64, String, String)>) {
        let mut lines = cast.lines();
        let header: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        let events = lines
            .map(|l| serde_json::from_str::<(f64, String, String)>(l).unwrap())
            .collect();
        (header, events)
    }

    #[tokio::test]
    async fn a_session_is_recorded_as_encrypted_asciicast_with_resizes() {
        let f = fixture(1 << 20).await;
        let id = start(&f.pool, &f.store, &f.live, 80, 24).await.unwrap();
        assert!(f.live.is_recorded());

        f.live.push_text("hello ").await;
        f.live.push_output(vec![0xE2, 0x95]).await; // half a box-drawing character
        f.live.push_output(vec![0x94]).await;
        f.live.resize(100, 30).await.unwrap();
        f.live.push_text("\r\n$ ").await;
        f.live.mark_closed("Shell exited with status 0");
        f.store.finish_all(Duration::from_secs(30)).await;

        let row = queries::recordings::get_for_user(&f.pool, id, 1)
            .await
            .unwrap();
        assert_eq!(row.status, "complete");
        assert_eq!(
            row.end_reason.as_deref(),
            Some("Shell exited with status 0")
        );
        assert!(row.ended_at.is_some());

        // Encrypted at rest: the file holds none of the text (base64 has no spaces, so a match
        // on "hello " cannot be chance)
        let path = f.store.path_of(1, &row.file_name).unwrap();
        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.windows(6).any(|w| w == b"hello "));
        assert!(!raw.contains(&b'"'), "no plaintext JSON");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        let cast = cast_text(&f.store, 1, &row.file_name).await;
        assert_eq!(
            row.size_bytes as usize,
            cast.len(),
            "size is the plaintext's"
        );
        let (header, events) = events(&cast);
        assert_eq!(header["version"], 2);
        assert_eq!(
            (header["width"].as_u64(), header["height"].as_u64()),
            (Some(80), Some(24))
        );
        assert_eq!(header["title"], "web");
        let kinds: Vec<(&str, &str)> = events
            .iter()
            .map(|(_, k, d)| (k.as_str(), d.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("o", "hello "),
                ("o", "╔"),
                ("r", "100x30"),
                ("o", "\r\n$ ")
            ]
        );
        assert!(events.windows(2).all(|w| w[0].0 <= w[1].0), "times ascend");
    }

    #[tokio::test]
    async fn a_recording_stops_at_the_size_limit_and_the_shell_goes_on() {
        let f = fixture(2_000).await;
        let id = start(&f.pool, &f.store, &f.live, 80, 24).await.unwrap();
        for _ in 0..100 {
            f.live.push_text(&"x".repeat(100)).await;
        }
        // The recorder gives up on its own; the session is still open
        for _ in 0..3000 {
            if !f.live.is_recorded() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!f.live.is_recorded(), "no longer recorded");
        assert!(!f.live.is_closed());
        f.live.mark_closed("done");
        f.store.finish_all(Duration::from_secs(30)).await;

        let row = queries::recordings::get_for_user(&f.pool, id, 1)
            .await
            .unwrap();
        assert_eq!(row.status, "truncated");
        let cast = cast_text(&f.store, 1, &row.file_name).await;
        assert!(cast.len() < 4_000, "{}", cast.len());
        let (_, events) = events(&cast);
        assert_eq!(events.last().unwrap().1, "m", "ends with a marker");
    }

    #[tokio::test]
    async fn housekeeping_removes_expired_recordings_and_orphaned_files() {
        let f = fixture(1 << 20).await;
        let id = start(&f.pool, &f.store, &f.live, 80, 24).await.unwrap();
        f.live.push_text("old").await;
        f.live.mark_closed("done");
        f.store.finish_all(Duration::from_secs(30)).await;
        let row = queries::recordings::get_for_user(&f.pool, id, 1)
            .await
            .unwrap();
        let path = f.store.path_of(1, &row.file_name).unwrap();
        let dir = path.parent().unwrap().to_path_buf();
        let orphan = dir.join(format!("orphan{SUFFIX}"));
        let other = dir.join("notes.txt");
        std::fs::write(&orphan, "x").unwrap();
        std::fs::write(&other, "x").unwrap();

        // Recent: kept; the orphan goes, a file that is not a recording stays
        assert_eq!(sweep(&f.pool, &f.store).await.unwrap(), (0, 1));
        assert!(path.exists() && !orphan.exists() && other.exists());

        svrctlrs_database::sqlx::query(
            "UPDATE session_recordings SET started_at = datetime('now', '-31 days')",
        )
        .execute(&f.pool)
        .await
        .unwrap();
        // 0 keeps everything
        queries::settings::set_setting(&f.pool, "recording.retention_days", "0")
            .await
            .unwrap();
        assert_eq!(sweep(&f.pool, &f.store).await.unwrap(), (0, 0));
        queries::settings::set_setting(&f.pool, "recording.retention_days", "30")
            .await
            .unwrap();
        assert_eq!(sweep(&f.pool, &f.store).await.unwrap(), (1, 0));
        assert!(!path.exists());
        assert!(queries::recordings::get_for_user(&f.pool, id, 1)
            .await
            .is_err());
    }

    #[test]
    fn file_paths_come_only_from_plain_names() {
        let store = RecordingStore::new("/data/rec", 1);
        assert_eq!(
            store.path_of(3, "abc.cast.enc"),
            Some(PathBuf::from("/data/rec/3/abc.cast.enc"))
        );
        for bad in ["", "../x.cast.enc", "a/b", ".hidden", "a\\b"] {
            assert!(store.path_of(3, bad).is_none(), "{bad}");
        }
    }
}

//! Server-owned interactive sessions.
//!
//! A PTY used to live exactly as long as the WebSocket that opened it: a dropped socket — a
//! reload, a backgrounded tab, a flaky link — killed the shell. Here the server owns the
//! session. Output is numbered and kept in a bounded replay ring; a client that reconnects
//! sends the last sequence it saw and gets everything it missed, then live output. A session
//! with no attached client survives for a grace period and is closed when that runs out: the
//! user's own choice (15 minutes, 1 hour or 8 hours, capped by `SESSION_DETACH_TTL_MAX_SECS`),
//! else `SESSION_DETACH_TTL_SECS` (default 15 minutes). Modelled on RelayTerm's session manager.
//!
//! A *persistent* session (a server with `persistent_shell`; `persistent_shells.rs`) runs in
//! tmux on that server. On shutdown it is released rather than closed: the SSH connection goes,
//! tmux keeps the shell, attached clients are told to reconnect, and the restarted server
//! attaches a new connection to the same tmux session when they do.
//!
//! Frames hold the PTY's raw bytes: a multibyte character split across two SSH reads must
//! reach the terminal intact, which a per-chunk UTF-8 conversion cannot guarantee.
//!
//! Everything that consumes a session's output — each attached socket, and the recorder when
//! the server records sessions — does so through a [`Follower`]: the ring first, then live
//! frames, and the ring again whenever it falls behind. The PTY driver never waits for any of
//! them, so a slow client costs only its own view (measured with
//! `scripts/dev/backpressure-check.py`: memory stays bounded by the ring and the broadcast
//! buffer however long a client stops reading).

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// What the socket side can ask the PTY driver to do.
#[derive(Debug)]
pub enum PtyCommand {
    Data(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

/// One numbered chunk of terminal output, as the PTY produced it, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFrame {
    pub seq: u64,
    pub data: Arc<[u8]>,
    pub at: Instant,
}

/// What a session broadcasts to its followers.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    Output(OutputFrame),
    /// A client resized the terminal.
    Resized {
        cols: u32,
        rows: u32,
        at: Instant,
    },
    /// The shell ended (or the session was closed); the reason is shown to the user.
    Closed(Arc<str>),
    /// A persistent session let go of its shell for a restart: clients reconnect and reattach.
    Released,
}

/// Bounded FIFO of recent output, by frame count and by bytes.
#[derive(Debug)]
pub struct ReplayRing {
    frames: VecDeque<OutputFrame>,
    bytes: usize,
    max_frames: usize,
    max_bytes: usize,
}

impl ReplayRing {
    pub const DEFAULT_MAX_FRAMES: usize = 4096;
    pub const DEFAULT_MAX_BYTES: usize = 2 * 1024 * 1024;

    pub fn new(max_frames: usize, max_bytes: usize) -> Self {
        Self {
            frames: VecDeque::new(),
            bytes: 0,
            max_frames: max_frames.max(1),
            max_bytes: max_bytes.max(1),
        }
    }

    pub fn push(&mut self, frame: OutputFrame) {
        self.bytes += frame.data.len();
        self.frames.push_back(frame);
        // Always keep at least the newest frame, however large.
        while self.frames.len() > 1
            && (self.frames.len() > self.max_frames || self.bytes > self.max_bytes)
        {
            if let Some(old) = self.frames.pop_front() {
                self.bytes -= old.data.len();
            }
        }
    }

    pub fn latest_seq(&self) -> u64 {
        self.frames.back().map(|f| f.seq).unwrap_or(0)
    }

    pub fn oldest_seq(&self) -> Option<u64> {
        self.frames.front().map(|f| f.seq)
    }

    /// Whether frames after `seq` have already been evicted: the ring moved past it, leaving a
    /// gap nobody can fill.
    pub fn lost_after(&self, seq: u64) -> bool {
        self.oldest_seq().is_some_and(|oldest| oldest > seq + 1)
    }

    /// The held frames with `seq > after`, in order.
    pub fn frames_after(&self, after: u64) -> Vec<OutputFrame> {
        self.frames
            .iter()
            .filter(|f| f.seq > after)
            .cloned()
            .collect()
    }
}

/// A running PTY and everything a client needs to (re)attach to it.
pub struct LiveSession {
    pub id: String,
    pub user_id: i64,
    pub server_id: i64,
    pub server_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// `created_at` on the monotonic clock; frame times are measured against it
    pub started: Instant,
    command_tx: mpsc::Sender<PtyCommand>,
    events: broadcast::Sender<SessionEvent>,
    replay: Mutex<ReplayRing>,
    seq: AtomicU64,
    attachments: AtomicUsize,
    detach_timer: Mutex<Option<JoinHandle<()>>>,
    /// Set once, when the shell ends: why.
    closed: OnceLock<Arc<str>>,
    /// A recorder is writing this session's output (`recordings.rs`)
    recorded: AtomicBool,
    /// The shell runs in tmux on the server and outlives this process
    pub persistent: bool,
    /// Let go for a restart (persistent sessions only): the shell lives on in tmux
    released: AtomicBool,
}

impl LiveSession {
    /// Frames the broadcast buffer holds for followers that have not read them yet; one that
    /// falls further behind is caught up from the ring.
    pub const BROADCAST_CAPACITY: usize = 512;

    pub fn new(
        user_id: i64,
        server_id: i64,
        server_name: String,
        command_tx: mpsc::Sender<PtyCommand>,
    ) -> Arc<Self> {
        Self::with_buffers(
            user_id,
            server_id,
            server_name,
            command_tx,
            ReplayRing::new(
                ReplayRing::DEFAULT_MAX_FRAMES,
                ReplayRing::DEFAULT_MAX_BYTES,
            ),
            Self::BROADCAST_CAPACITY,
        )
    }

    /// A session whose shell runs in tmux on the server, under the id `id`. A session revived
    /// after a restart numbers its output from `seq_base` on, the last frame the reattaching
    /// client saw, so that client takes the new output as what comes next.
    pub fn persistent(
        id: String,
        user_id: i64,
        server_id: i64,
        server_name: String,
        command_tx: mpsc::Sender<PtyCommand>,
        created_at: chrono::DateTime<chrono::Utc>,
        seq_base: u64,
    ) -> Arc<Self> {
        let (events, _) = broadcast::channel(Self::BROADCAST_CAPACITY);
        Arc::new(Self {
            id,
            user_id,
            server_id,
            server_name,
            created_at,
            started: Instant::now(),
            command_tx,
            events,
            replay: Mutex::new(ReplayRing::new(
                ReplayRing::DEFAULT_MAX_FRAMES,
                ReplayRing::DEFAULT_MAX_BYTES,
            )),
            seq: AtomicU64::new(seq_base),
            attachments: AtomicUsize::new(0),
            detach_timer: Mutex::new(None),
            closed: OnceLock::new(),
            recorded: AtomicBool::new(false),
            persistent: true,
            released: AtomicBool::new(false),
        })
    }

    fn with_buffers(
        user_id: i64,
        server_id: i64,
        server_name: String,
        command_tx: mpsc::Sender<PtyCommand>,
        ring: ReplayRing,
        broadcast_capacity: usize,
    ) -> Arc<Self> {
        let (events, _) = broadcast::channel(broadcast_capacity);
        Arc::new(Self {
            id: new_session_id(),
            user_id,
            server_id,
            server_name,
            created_at: chrono::Utc::now(),
            started: Instant::now(),
            command_tx,
            events,
            replay: Mutex::new(ring),
            seq: AtomicU64::new(0),
            attachments: AtomicUsize::new(0),
            detach_timer: Mutex::new(None),
            closed: OnceLock::new(),
            recorded: AtomicBool::new(false),
            persistent: false,
            released: AtomicBool::new(false),
        })
    }

    /// The raw event stream; consumers use [`LiveSession::follow`].
    #[cfg(test)]
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.events.subscribe()
    }

    pub fn attachments(&self) -> usize {
        self.attachments.load(Ordering::SeqCst)
    }

    pub fn is_closed(&self) -> bool {
        self.closed.get().is_some()
    }

    /// Whether a recorder is writing this session's output right now.
    pub fn is_recorded(&self) -> bool {
        self.recorded.load(Ordering::SeqCst)
    }

    pub fn set_recorded(&self, on: bool) {
        self.recorded.store(on, Ordering::SeqCst);
    }

    /// Let go of a persistent shell (a restart, a lost connection): attached clients are told
    /// to reconnect, and the shell stays in tmux on the server.
    pub fn release(&self) {
        if !self.released.swap(true, Ordering::SeqCst) {
            let _ = self.events.send(SessionEvent::Released);
        }
    }

    /// Let go for a restart: the shell stays in tmux on the server.
    pub fn is_released(&self) -> bool {
        self.released.load(Ordering::SeqCst)
    }

    /// Why the shell ended, once it has.
    pub fn close_reason(&self) -> Option<Arc<str>> {
        self.closed.get().cloned()
    }

    /// Stream this session's output from after `after_seq` on; see [`Follower`].
    pub fn follow(self: &Arc<Self>, after_seq: u64) -> Follower {
        // Subscribe first: a close after this point reaches the receiver, one before it is
        // seen by `is_closed`, and one in between is seen by both (harmless).
        let events = self.events.subscribe();
        Follower {
            ended_before: self.is_closed(),
            events,
            live: Arc::clone(self),
            sent: after_seq,
            pending: VecDeque::new(),
            closing: None,
            behind: false,
            done: false,
        }
    }

    /// Record and broadcast a chunk of PTY output. Returns its sequence number.
    pub async fn push_output(&self, bytes: Vec<u8>) -> u64 {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let frame = OutputFrame {
            seq,
            data: Arc::from(bytes),
            at: Instant::now(),
        };
        self.replay.lock().await.push(frame.clone());
        // No subscribers is fine: the ring keeps it for whoever attaches next.
        let _ = self.events.send(SessionEvent::Output(frame));
        seq
    }

    /// A server-side notice (banner, exit status) written into the stream like output.
    pub async fn push_text(&self, text: &str) -> u64 {
        self.push_output(text.as_bytes().to_vec()).await
    }

    /// Everything the ring holds.
    #[cfg(test)]
    pub async fn held_frames(&self) -> Vec<OutputFrame> {
        self.replay.lock().await.frames_after(0)
    }

    pub async fn latest_seq(&self) -> u64 {
        self.replay.lock().await.latest_seq()
    }

    pub async fn send_input(&self, data: &[u8]) -> Result<(), String> {
        self.command_tx
            .send(PtyCommand::Data(data.to_vec()))
            .await
            .map_err(|_| "PTY is gone".to_string())
    }

    pub async fn resize(&self, cols: u32, rows: u32) -> Result<(), String> {
        self.command_tx
            .send(PtyCommand::Resize { cols, rows })
            .await
            .map_err(|_| "PTY is gone".to_string())?;
        let _ = self.events.send(SessionEvent::Resized {
            cols,
            rows,
            at: Instant::now(),
        });
        Ok(())
    }

    /// Ask the PTY driver to end the shell. The driver then calls [`LiveSession::mark_closed`].
    pub async fn close(&self) {
        let _ = self.command_tx.send(PtyCommand::Close).await;
    }

    /// The shell ended. Tells every follower and is idempotent: the first reason sticks.
    pub fn mark_closed(&self, reason: &str) {
        let reason: Arc<str> = Arc::from(reason);
        if self.closed.set(reason.clone()).is_ok() {
            let _ = self.events.send(SessionEvent::Closed(reason));
        }
    }
}

/// What a follower hands on, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Followed {
    Output(OutputFrame),
    /// The follower fell further behind than the ring reaches. What it missed is gone; the
    /// frames that come next are everything the ring still holds, to redraw from scratch.
    WindowLost,
    Resized {
        cols: u32,
        rows: u32,
        at: Instant,
    },
    /// The shell ended. Nothing follows.
    Closed(Arc<str>),
    /// The server let go of a persistent shell for a restart; reconnect. Nothing follows.
    Released,
}

/// One consumer's view of a session's output from a sequence number on: what the ring holds
/// after it ([`Follower::catch_up`]), then live frames ([`Follower::next`]). Every frame after
/// the starting point is handed out once, in order, or, when some are gone (the follower fell
/// behind the broadcast buffer *and* the ring), [`Followed::WindowLost`] comes first and then
/// everything the ring still holds. It subscribes when created, so nothing produced after
/// [`LiveSession::follow`] returns is missed.
pub struct Follower {
    live: Arc<LiveSession>,
    events: broadcast::Receiver<SessionEvent>,
    sent: u64,
    pending: VecDeque<Followed>,
    /// The shell had ended before this follower subscribed, so no `Closed` will arrive
    ended_before: bool,
    /// The shell ended (and why); finish from the ring
    closing: Option<Arc<str>>,
    /// Frames were missed from the broadcast; catch up from the ring
    behind: bool,
    done: bool,
}

impl Follower {
    /// What the ring holds after the last frame handed out: the missed frames in order, or
    /// `WindowLost` and then everything still held.
    pub async fn catch_up(&mut self) -> Vec<Followed> {
        let ring = self.live.replay.lock().await;
        let lost = ring.lost_after(self.sent);
        let frames = ring.frames_after(if lost { 0 } else { self.sent });
        drop(ring);
        self.hand_out(lost, frames)
    }

    /// Everything the ring holds, for a consumer that has nothing yet and wants the current
    /// screen (a socket attaching with `last_seq` 0). Nothing was owed to it before, so unlike
    /// [`Follower::catch_up`] this never reports a lost window.
    pub async fn redraw(&mut self) -> Vec<Followed> {
        let frames = self.live.replay.lock().await.frames_after(0);
        self.hand_out(false, frames)
    }

    fn hand_out(&mut self, lost: bool, frames: Vec<OutputFrame>) -> Vec<Followed> {
        if let Some(last) = frames.last() {
            self.sent = last.seq;
        }
        lost.then_some(Followed::WindowLost)
            .into_iter()
            .chain(frames.into_iter().map(Followed::Output))
            .collect()
    }

    /// The next item, waiting for the shell if need be; `None` after `Closed`.
    ///
    /// Cancel-safe, so it can sit in a `select!` beside a timer: what an event means is
    /// recorded in `self` before the next await, and a dropped call resumes from there.
    pub async fn next(&mut self) -> Option<Followed> {
        loop {
            if let Some(item) = self.pending.pop_front() {
                return Some(item);
            }
            if self.done {
                return None;
            }
            if self.ended_before && self.closing.is_none() {
                // Its `Closed` went to nobody; the ring holds all of its output.
                self.closing = Some(
                    self.live
                        .close_reason()
                        .unwrap_or_else(|| Arc::from("Session ended")),
                );
            }
            if let Some(reason) = self.closing.clone() {
                // Output is in the ring before it is broadcast, and the close comes last
                let rest = self.catch_up().await;
                self.pending.extend(rest);
                self.pending.push_back(Followed::Closed(reason));
                self.done = true;
                continue;
            }
            if self.behind {
                let items = self.catch_up().await;
                self.behind = false;
                self.pending.extend(items);
                continue;
            }
            match self.events.recv().await {
                Ok(SessionEvent::Output(frame)) => {
                    if frame.seq <= self.sent {
                        continue; // already handed out from the ring
                    }
                    if frame.seq > self.sent + 1 {
                        // A gap without a lag report: frames pushed before this follower
                        // subscribed and not caught up yet. The ring has this one too.
                        self.behind = true;
                        continue;
                    }
                    self.sent = frame.seq;
                    return Some(Followed::Output(frame));
                }
                Ok(SessionEvent::Resized { cols, rows, at }) => {
                    return Some(Followed::Resized { cols, rows, at });
                }
                Ok(SessionEvent::Closed(reason)) => self.closing = Some(reason),
                Ok(SessionEvent::Released) => {
                    self.done = true;
                    return Some(Followed::Released);
                }
                Err(RecvError::Lagged(missed)) => {
                    debug!(
                        session_id = %self.live.id,
                        missed,
                        "Slow follower; catching up from the replay ring"
                    );
                    self.behind = true;
                }
                Err(RecvError::Closed) => self.done = true,
            }
        }
    }
}

/// Summary for the sessions API.
#[derive(Debug, serde::Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub server_id: i64,
    pub server_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub attachments: usize,
    pub latest_seq: u64,
    /// Its output is being recorded
    pub recorded: bool,
    /// It runs in tmux on the server
    pub persistent: bool,
}

/// All live sessions on this server, with how long a detached one is kept.
pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, Arc<LiveSession>>>,
    default_ttl: Duration,
    max_ttl: Duration,
    max_per_user: usize,
}

impl SessionRegistry {
    pub const DEFAULT_DETACH_TTL: Duration = Duration::from_secs(15 * 60);
    pub const DEFAULT_MAX_DETACH_TTL: Duration = Duration::from_secs(8 * 60 * 60);
    pub const DEFAULT_MAX_PER_USER: usize = 16;
    /// What a user can choose for their own shells: 15 minutes, 1 hour, 8 hours.
    pub const DETACH_TTL_CHOICES: [u64; 3] = [15 * 60, 60 * 60, 8 * 60 * 60];

    /// `default_ttl` for users who chose nothing, `max_ttl` caps what they choose (and the
    /// default; a maximum below it is raised to it).
    pub fn new(default_ttl: Duration, max_ttl: Duration, max_per_user: usize) -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
            default_ttl,
            max_ttl: max_ttl.max(default_ttl),
            max_per_user: max_per_user.max(1),
        })
    }

    /// Read `SESSION_DETACH_TTL_SECS` (default 900), `SESSION_DETACH_TTL_MAX_SECS` (default
    /// 28800) and `SESSION_MAX_PER_USER` (default 16).
    pub fn from_env() -> Arc<Self> {
        let secs = |name: &str| {
            std::env::var(name)
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs)
        };
        let max = std::env::var("SESSION_MAX_PER_USER")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(Self::DEFAULT_MAX_PER_USER);
        Self::new(
            secs("SESSION_DETACH_TTL_SECS").unwrap_or(Self::DEFAULT_DETACH_TTL),
            secs("SESSION_DETACH_TTL_MAX_SECS").unwrap_or(Self::DEFAULT_MAX_DETACH_TTL),
            max,
        )
    }

    /// The longest a user may choose
    pub fn max_ttl(&self) -> Duration {
        self.max_ttl
    }

    /// The choices a user has on this server: those within the maximum.
    pub fn ttl_choices(&self) -> Vec<u64> {
        Self::DETACH_TTL_CHOICES
            .into_iter()
            .filter(|s| Duration::from_secs(*s) <= self.max_ttl)
            .collect()
    }

    /// How long a user's shell is kept, given what they chose (if anything): the choice
    /// capped by the maximum, else the default.
    pub fn ttl_for(&self, chosen_secs: Option<u64>) -> Duration {
        chosen_secs
            .map(|s| Duration::from_secs(s).min(self.max_ttl))
            .unwrap_or(self.default_ttl)
    }

    /// Register a freshly started session. Refuses when the user already holds the quota.
    pub async fn insert(&self, session: Arc<LiveSession>) -> Result<(), String> {
        let mut map = self.sessions.lock().await;
        let mine = map
            .values()
            .filter(|s| s.user_id == session.user_id)
            .count();
        if mine >= self.max_per_user {
            return Err(format!(
                "You already have {mine} live sessions (limit {}); close one first",
                self.max_per_user
            ));
        }
        map.insert(session.id.clone(), session);
        Ok(())
    }

    /// Attach a client to a session it owns: cancels any pending close. `None` when the
    /// session does not exist, belongs to someone else, or has ended.
    pub async fn attach(&self, id: &str, user_id: i64) -> Option<Arc<LiveSession>> {
        let session = self.sessions.lock().await.get(id).cloned()?;
        if session.user_id != user_id || session.is_closed() {
            return None;
        }
        if let Some(timer) = session.detach_timer.lock().await.take() {
            timer.abort();
        }
        session.attachments.fetch_add(1, Ordering::SeqCst);
        Some(session)
    }

    /// Count a brand-new session's first attachment (the socket that started it).
    pub fn attach_new(&self, session: &LiveSession) {
        session.attachments.fetch_add(1, Ordering::SeqCst);
    }

    /// A client went away. With no clients left, the session is closed after `ttl` (the
    /// user's keep time, [`SessionRegistry::ttl_for`]) unless someone reattaches first.
    pub async fn detach(self: &Arc<Self>, session: &Arc<LiveSession>, ttl: Duration) {
        let left = session
            .attachments
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                Some(n.saturating_sub(1))
            })
            .map(|n| n.saturating_sub(1))
            .unwrap_or(0);
        if left > 0 || session.is_closed() {
            return;
        }
        self.arm_timer(session, ttl).await;
    }

    /// Close `session` after `ttl` unless a client attaches first (replacing any timer).
    async fn arm_timer(self: &Arc<Self>, session: &Arc<LiveSession>, ttl: Duration) {
        let registry = Arc::downgrade(self);
        let weak = Arc::downgrade(session);
        let id = session.id.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(ttl).await;
            if let Some(session) = weak.upgrade() {
                if session.attachments() == 0 && !session.is_closed() {
                    info!(session_id = %session.id, server = %session.server_name,
                        "Detached session expired; closing shell");
                    session.close().await;
                    session.mark_closed("Session closed after being left unattended");
                }
            }
            if let Some(registry) = registry.upgrade() {
                registry.remove(&id).await;
            }
        });
        if let Some(old) = session.detach_timer.lock().await.replace(handle) {
            old.abort();
        }
    }

    /// The user changed their keep time: their detached sessions are kept `ttl` from now.
    pub async fn retime_detached(self: &Arc<Self>, user_id: i64, ttl: Duration) {
        let detached: Vec<Arc<LiveSession>> = self
            .sessions
            .lock()
            .await
            .values()
            .filter(|s| s.user_id == user_id && s.attachments() == 0 && !s.is_closed())
            .cloned()
            .collect();
        for session in detached {
            self.arm_timer(&session, ttl).await;
        }
    }

    pub async fn remove(&self, id: &str) {
        if let Some(session) = self.sessions.lock().await.remove(id) {
            if let Some(timer) = session.detach_timer.lock().await.take() {
                timer.abort();
            }
        }
    }

    /// End a session the user owns, attached or not: the shell is told to exit, every attached
    /// socket gets `Closed`, and the session leaves the registry. `false` when there is no such
    /// session for this user (unknown id, someone else's, or already gone).
    pub async fn close_session(&self, id: &str, user_id: i64) -> bool {
        let Some(session) = self.get(id, user_id).await else {
            return false;
        };
        session.close().await;
        session.mark_closed("Session closed");
        self.remove(id).await;
        true
    }

    pub async fn get(&self, id: &str, user_id: i64) -> Option<Arc<LiveSession>> {
        self.sessions
            .lock()
            .await
            .get(id)
            .filter(|s| s.user_id == user_id)
            .cloned()
    }

    /// Whether a session with this id is live (anyone's): the expiry sweep after a restart
    /// leaves revived shells alone.
    pub async fn contains(&self, id: &str) -> bool {
        self.sessions.lock().await.contains_key(id)
    }

    pub async fn list_for_user(&self, user_id: i64) -> Vec<SessionSummary> {
        let sessions: Vec<Arc<LiveSession>> = self
            .sessions
            .lock()
            .await
            .values()
            .filter(|s| s.user_id == user_id && !s.is_closed())
            .cloned()
            .collect();
        let mut out = Vec::with_capacity(sessions.len());
        for s in sessions {
            out.push(SessionSummary {
                id: s.id.clone(),
                server_id: s.server_id,
                server_name: s.server_name.clone(),
                created_at: s.created_at,
                attachments: s.attachments(),
                latest_seq: s.latest_seq().await,
                recorded: s.is_recorded(),
                persistent: s.persistent,
            });
        }
        out.sort_by_key(|s| s.created_at);
        out
    }

    /// Server shutdown. Plain sessions are closed. Persistent ones are released: the SSH
    /// connection closes, which only detaches tmux, and attached clients are told to
    /// reconnect, which reaches the restarted server.
    pub async fn close_all(&self) {
        let sessions: Vec<Arc<LiveSession>> =
            self.sessions.lock().await.values().cloned().collect();
        for s in sessions {
            if s.persistent {
                s.release();
                s.close().await;
            } else {
                s.close().await;
                s.mark_closed("Server shutting down");
            }
        }
        if !self.sessions.lock().await.is_empty() {
            warn!("Closed live sessions for shutdown");
        }
    }
}

pub fn new_session_id() -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(seq: u64, s: &str) -> OutputFrame {
        OutputFrame {
            seq,
            data: Arc::from(s.as_bytes()),
            at: Instant::now(),
        }
    }

    #[test]
    fn ring_evicts_by_count_and_bytes_but_keeps_the_newest() {
        let mut ring = ReplayRing::new(3, 100);
        for i in 1..=5 {
            ring.push(frame(i, "ab"));
        }
        assert_eq!(ring.oldest_seq(), Some(3));
        assert_eq!(ring.latest_seq(), 5);

        let mut ring = ReplayRing::new(100, 10);
        ring.push(frame(1, "12345"));
        ring.push(frame(2, "12345"));
        ring.push(frame(3, "123456789012")); // alone exceeds the byte cap; still kept
        assert_eq!(ring.oldest_seq(), Some(3));
        assert_eq!(ring.latest_seq(), 3);
    }

    #[test]
    fn the_ring_returns_what_follows_and_knows_what_it_lost() {
        let mut ring = ReplayRing::new(10, 1000);
        for i in 1..=6 {
            ring.push(frame(i, "x"));
        }
        let got = ring.frames_after(4);
        assert_eq!(got.iter().map(|f| f.seq).collect::<Vec<_>>(), [5, 6]);
        assert!(ring.frames_after(6).is_empty());
        assert!(!ring.lost_after(0) && !ring.lost_after(4));
        // Everything, for a client that has nothing yet
        assert_eq!(ring.frames_after(0).len(), 6);

        let mut small = ReplayRing::new(2, 1000);
        for i in 1..=6 {
            small.push(frame(i, "x"));
        }
        assert_eq!(small.oldest_seq(), Some(5));
        assert!(small.lost_after(2), "3 and 4 are gone");
        // last_seen == oldest - 1 is contiguous, so it is not a gap
        assert!(!small.lost_after(4));
        assert_eq!(small.frames_after(4).len(), 2);
        assert!(small.lost_after(0), "1 to 4 are gone");
        assert!(
            !ReplayRing::new(2, 1000).lost_after(0),
            "an empty ring lost nothing"
        );
    }

    fn session(user: i64) -> (Arc<LiveSession>, mpsc::Receiver<PtyCommand>) {
        let (tx, rx) = mpsc::channel(8);
        (LiveSession::new(user, 1, "web".into(), tx), rx)
    }

    #[tokio::test]
    async fn output_is_numbered_broadcast_and_replayable() {
        let (s, _rx) = session(1);
        let mut sub = s.subscribe();
        assert_eq!(s.push_output(b"a".to_vec()).await, 1);
        assert_eq!(s.push_text("b").await, 2);
        // Raw bytes survive, including a UTF-8 sequence cut in half by the SSH read
        assert_eq!(s.push_output(vec![0xE2, 0x94]).await, 3);
        match sub.recv().await.unwrap() {
            SessionEvent::Output(f) => assert_eq!((f.seq, &*f.data), (1, &b"a"[..])),
            other => panic!("{other:?}"),
        }
        let late = s.held_frames().await.split_off(1);
        assert_eq!(late.len(), 2);
        assert_eq!(&*late[0].data, b"b");
        assert_eq!(&*late[1].data, &[0xE2, 0x94][..]);
        s.mark_closed("done");
        s.mark_closed("twice is harmless");
        assert!(s.is_closed());
    }

    /// The sequence numbers of the output items, with `L` for a lost window
    fn seqs(items: &[Followed]) -> String {
        items
            .iter()
            .filter_map(|i| match i {
                Followed::Output(f) => Some(f.seq.to_string()),
                Followed::WindowLost => Some("L".into()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    async fn next_n(follow: &mut Follower, n: usize) -> Vec<Followed> {
        let mut out = Vec::new();
        for _ in 0..n {
            let item = tokio::time::timeout(Duration::from_secs(20), follow.next())
                .await
                .expect("an item in time")
                .expect("not finished");
            out.push(item);
        }
        out
    }

    #[tokio::test]
    async fn a_follower_replays_the_ring_then_follows_live_output_resizes_and_the_close() {
        let (s, _rx) = session(1);
        s.push_text("one").await;
        s.push_text("two").await;
        let mut follow = s.follow(0);
        assert_eq!(seqs(&follow.catch_up().await), "1,2");

        s.push_text("three").await;
        s.resize(120, 40).await.unwrap();
        s.mark_closed("Shell exited with status 0");
        let items = next_n(&mut follow, 3).await;
        assert_eq!(seqs(&items), "3");
        assert!(matches!(
            items[1],
            Followed::Resized {
                cols: 120,
                rows: 40,
                ..
            }
        ));
        assert_eq!(
            items[2],
            Followed::Closed(Arc::from("Shell exited with status 0"))
        );
        assert!(follow.next().await.is_none());
    }

    #[tokio::test]
    async fn a_follower_of_an_ended_session_finishes_from_the_ring() {
        let (s, _rx) = session(1);
        for t in ["a", "b", "c"] {
            s.push_text(t).await;
        }
        s.mark_closed("done");
        // Subscribed after the close was broadcast: the ring and the stored reason finish it
        let mut follow = s.follow(1);
        let items = next_n(&mut follow, 3).await;
        assert_eq!(seqs(&items), "2,3");
        assert_eq!(items[2], Followed::Closed(Arc::from("done")));
        assert!(follow.next().await.is_none());
    }

    fn small(
        ring_frames: usize,
        broadcast: usize,
    ) -> (Arc<LiveSession>, mpsc::Receiver<PtyCommand>) {
        let (tx, rx) = mpsc::channel(8);
        let s = LiveSession::with_buffers(
            1,
            1,
            "web".into(),
            tx,
            ReplayRing::new(ring_frames, 1 << 20),
            broadcast,
        );
        (s, rx)
    }

    #[tokio::test]
    async fn a_follower_that_lags_the_broadcast_is_caught_up_from_the_ring() {
        let (s, _rx) = small(100, 2);
        let mut follow = s.follow(0);
        for i in 0..10 {
            s.push_text(&i.to_string()).await;
        }
        // Eight of the ten fell out of the broadcast buffer; the ring still has them all
        let items = next_n(&mut follow, 10).await;
        assert_eq!(seqs(&items), "1,2,3,4,5,6,7,8,9,10");
    }

    #[tokio::test]
    async fn a_follower_behind_the_ring_gets_a_lost_window_and_a_redraw() {
        let (s, _rx) = small(3, 2);
        let mut follow = s.follow(0);
        for i in 0..10 {
            s.push_text(&i.to_string()).await;
        }
        // 1–7 are gone everywhere; the follower is told, then gets what the ring holds
        let items = next_n(&mut follow, 4).await;
        assert_eq!(seqs(&items), "L,8,9,10");
        s.push_text("live again").await;
        assert_eq!(seqs(&next_n(&mut follow, 1).await), "11");
    }

    #[tokio::test]
    async fn registry_attach_is_owner_only_and_detach_expires_after_ttl() {
        let registry =
            SessionRegistry::new(Duration::from_millis(60), Duration::from_secs(3600), 16);
        let (s, mut rx) = session(7);
        let id = s.id.clone();
        registry.insert(s.clone()).await.unwrap();
        registry.attach_new(&s);

        assert!(registry.attach(&id, 8).await.is_none(), "another user");
        assert!(registry.attach("nope", 7).await.is_none());

        registry.detach(&s, registry.ttl_for(None)).await;
        assert_eq!(s.attachments(), 0);
        // Reattach within the grace period cancels the close
        assert!(registry.attach(&id, 7).await.is_some());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!s.is_closed(), "reattach must cancel the timer");
        assert!(registry.get(&id, 7).await.is_some());

        // Detach again and let it expire
        registry.detach(&s, registry.ttl_for(None)).await;
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(s.is_closed());
        assert!(matches!(rx.try_recv(), Ok(PtyCommand::Close)));
        assert!(
            registry.get(&id, 7).await.is_none(),
            "removed from the registry"
        );
    }

    #[test]
    fn a_users_keep_time_is_their_choice_within_the_maximum() {
        let registry =
            SessionRegistry::new(Duration::from_secs(900), Duration::from_secs(3600), 16);
        assert_eq!(registry.ttl_for(None), Duration::from_secs(900));
        assert_eq!(registry.ttl_for(Some(3600)), Duration::from_secs(3600));
        assert_eq!(
            registry.ttl_for(Some(28800)),
            Duration::from_secs(3600),
            "capped"
        );
        assert_eq!(registry.ttl_choices(), vec![900, 3600]);

        // A maximum below the default is raised to it
        let odd = SessionRegistry::new(Duration::from_secs(7200), Duration::from_secs(60), 16);
        assert_eq!(odd.max_ttl(), Duration::from_secs(7200));
        assert_eq!(odd.ttl_for(None), Duration::from_secs(7200));
    }

    #[tokio::test]
    async fn a_new_keep_time_applies_to_shells_already_detached() {
        let registry = SessionRegistry::new(Duration::from_secs(60), Duration::from_secs(3600), 16);
        let (s, _rx) = session(7);
        let (other, _ro) = session(8);
        for x in [&s, &other] {
            registry.insert(x.clone()).await.unwrap();
            registry.attach_new(x);
            registry.detach(x, Duration::from_secs(60)).await;
        }
        registry.retime_detached(7, Duration::from_millis(50)).await;
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(s.is_closed(), "user 7's shell now expires sooner");
        assert!(!other.is_closed(), "user 8's keeps its own time");
    }

    #[tokio::test]
    async fn shutdown_releases_persistent_sessions_and_closes_the_rest() {
        let registry = SessionRegistry::new(Duration::from_secs(60), Duration::from_secs(3600), 16);
        let (plain, mut plain_rx) = session(1);
        let (tx, mut kept_rx) = mpsc::channel(8);
        let kept = LiveSession::persistent(
            "kept".into(),
            1,
            1,
            "web".into(),
            tx,
            chrono::Utc::now(),
            40,
        );
        registry.insert(plain.clone()).await.unwrap();
        registry.insert(kept.clone()).await.unwrap();
        let mut plain_follow = plain.follow(0);
        let mut kept_follow = kept.follow(40);

        registry.close_all().await;

        assert!(matches!(plain_rx.try_recv(), Ok(PtyCommand::Close)));
        assert!(matches!(
            plain_follow.next().await,
            Some(Followed::Closed(_))
        ));
        // The persistent one lets go of its connection but is not ended: its clients are told
        // to reconnect, and nothing says the shell is gone
        assert!(matches!(kept_rx.try_recv(), Ok(PtyCommand::Close)));
        assert!(kept.is_released());
        assert!(!kept.is_closed());
        assert_eq!(kept_follow.next().await, Some(Followed::Released));
        assert_eq!(kept_follow.next().await, None);
    }

    #[tokio::test]
    async fn a_revived_session_numbers_its_output_after_what_the_client_saw() {
        let (tx, _rx) = mpsc::channel(8);
        let kept = LiveSession::persistent(
            "kept".into(),
            1,
            1,
            "web".into(),
            tx,
            chrono::Utc::now(),
            500,
        );
        let mut follow = kept.follow(500);
        assert!(follow.catch_up().await.is_empty());
        kept.push_text("redrawn").await;
        match follow.next().await {
            Some(Followed::Output(f)) => assert_eq!(f.seq, 501),
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn listing_reports_what_the_resume_list_shows_and_skips_ended_sessions() {
        let registry = SessionRegistry::new(Duration::from_secs(60), Duration::from_secs(3600), 16);
        let (attached, _ra) = session(1);
        let (detached, _rd) = session(1);
        let (ended, _re) = session(1);
        let (theirs, _rt) = session(2);
        for s in [&attached, &detached, &ended, &theirs] {
            registry.insert(s.clone()).await.unwrap();
        }
        registry.attach_new(&attached);
        detached.push_text("one").await;
        detached.push_text("two").await;
        ended.mark_closed("exited");

        let mine = registry.list_for_user(1).await;
        assert_eq!(mine.len(), 2, "closed sessions are not offered for resume");
        let by_id = |id: &str| mine.iter().find(|s| s.id == id).unwrap();
        assert_eq!(by_id(&attached.id).attachments, 1);
        assert_eq!(by_id(&attached.id).latest_seq, 0);
        assert_eq!(by_id(&detached.id).attachments, 0);
        assert_eq!(by_id(&detached.id).latest_seq, 2);
        assert_eq!(by_id(&detached.id).server_name, "web");

        // The JSON the UI reads
        let json = serde_json::to_value(by_id(&detached.id)).unwrap();
        for key in [
            "id",
            "server_id",
            "server_name",
            "created_at",
            "attachments",
            "latest_seq",
            "recorded",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
    }

    #[tokio::test]
    async fn close_session_ends_the_shell_tells_listeners_and_is_owner_only() {
        let registry = SessionRegistry::new(Duration::from_secs(60), Duration::from_secs(3600), 16);
        let (s, mut rx) = session(7);
        let id = s.id.clone();
        registry.insert(s.clone()).await.unwrap();
        registry.attach_new(&s);
        let mut sub = s.subscribe();

        assert!(!registry.close_session(&id, 8).await, "not the owner");
        assert!(!registry.close_session("nope", 7).await);
        assert!(!s.is_closed());
        assert!(
            rx.try_recv().is_err(),
            "a refused close must not touch the PTY"
        );

        assert!(registry.close_session(&id, 7).await);
        assert!(s.is_closed());
        assert!(matches!(rx.try_recv(), Ok(PtyCommand::Close)));
        assert!(matches!(sub.recv().await.unwrap(), SessionEvent::Closed(_)));
        assert!(registry.get(&id, 7).await.is_none());
        assert!(registry.attach(&id, 7).await.is_none(), "cannot be resumed");
        assert!(registry.list_for_user(7).await.is_empty());
        assert!(!registry.close_session(&id, 7).await, "already gone");
    }

    #[tokio::test]
    async fn registry_enforces_the_per_user_quota() {
        let registry = SessionRegistry::new(Duration::from_secs(60), Duration::from_secs(3600), 1);
        let (a, _ra) = session(1);
        let (b, _rb) = session(1);
        let (c, _rc) = session(2);
        registry.insert(a).await.unwrap();
        assert!(registry.insert(b).await.unwrap_err().contains("limit 1"));
        registry.insert(c).await.unwrap();
        assert_eq!(registry.list_for_user(1).await.len(), 1);
    }
}

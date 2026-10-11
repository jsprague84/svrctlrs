//! Server-owned interactive sessions.
//!
//! A PTY used to live exactly as long as the WebSocket that opened it: a dropped socket — a
//! reload, a backgrounded tab, a flaky link — killed the shell. Here the server owns the
//! session. Output is numbered and kept in a bounded replay ring; a client that reconnects
//! sends the last sequence it saw and gets everything it missed, then live output. A session
//! with no attached client survives for a grace period (`SESSION_DETACH_TTL_SECS`, default
//! 15 minutes) and is closed when that runs out. Modelled on RelayTerm's session manager.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::{info, warn};

/// What the socket side can ask the PTY driver to do.
#[derive(Debug)]
pub enum PtyCommand {
    Data(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

/// One numbered chunk of terminal output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFrame {
    pub seq: u64,
    pub data: Arc<str>,
}

/// What attached sockets receive.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    Output(OutputFrame),
    /// The shell ended (or the session was closed); the reason is shown to the user.
    Closed(Arc<str>),
}

/// A reconnecting client asked for more history than the ring still holds.
#[derive(Debug, PartialEq, Eq)]
pub struct WindowLost;

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

    /// Frames with `seq > last_seen`. `Err(WindowLost)` when `last_seen` predates the ring,
    /// meaning the client must redraw from scratch; a `last_seen` of 0 asks for everything
    /// that is still held and never fails.
    pub fn replay_since(&self, last_seen: u64) -> Result<Vec<OutputFrame>, WindowLost> {
        if last_seen != 0 {
            match self.oldest_seq() {
                // Ring moved past what the client saw: there is a gap we cannot fill.
                Some(oldest) if last_seen + 1 < oldest => return Err(WindowLost),
                _ => {}
            }
        }
        Ok(self
            .frames
            .iter()
            .filter(|f| f.seq > last_seen)
            .cloned()
            .collect())
    }
}

/// A running PTY and everything a client needs to (re)attach to it.
pub struct LiveSession {
    pub id: String,
    pub user_id: i64,
    pub server_id: i64,
    pub server_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    command_tx: mpsc::Sender<PtyCommand>,
    events: broadcast::Sender<SessionEvent>,
    replay: Mutex<ReplayRing>,
    seq: AtomicU64,
    attachments: AtomicUsize,
    detach_timer: Mutex<Option<JoinHandle<()>>>,
    closed: AtomicBool,
}

impl LiveSession {
    pub fn new(
        user_id: i64,
        server_id: i64,
        server_name: String,
        command_tx: mpsc::Sender<PtyCommand>,
    ) -> Arc<Self> {
        let (events, _) = broadcast::channel(512);
        Arc::new(Self {
            id: new_session_id(),
            user_id,
            server_id,
            server_name,
            created_at: chrono::Utc::now(),
            command_tx,
            events,
            replay: Mutex::new(ReplayRing::new(
                ReplayRing::DEFAULT_MAX_FRAMES,
                ReplayRing::DEFAULT_MAX_BYTES,
            )),
            seq: AtomicU64::new(0),
            attachments: AtomicUsize::new(0),
            detach_timer: Mutex::new(None),
            closed: AtomicBool::new(false),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.events.subscribe()
    }

    pub fn attachments(&self) -> usize {
        self.attachments.load(Ordering::SeqCst)
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Record and broadcast a chunk of PTY output. Returns its sequence number.
    pub async fn push_output(&self, text: String) -> u64 {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let frame = OutputFrame {
            seq,
            data: Arc::from(text),
        };
        self.replay.lock().await.push(frame.clone());
        // No subscribers is fine: the ring keeps it for whoever attaches next.
        let _ = self.events.send(SessionEvent::Output(frame));
        seq
    }

    pub async fn replay_since(&self, last_seen: u64) -> Result<Vec<OutputFrame>, WindowLost> {
        self.replay.lock().await.replay_since(last_seen)
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
            .map_err(|_| "PTY is gone".to_string())
    }

    /// Ask the PTY driver to end the shell. The driver then calls [`LiveSession::mark_closed`].
    pub async fn close(&self) {
        let _ = self.command_tx.send(PtyCommand::Close).await;
    }

    /// The shell ended. Tells every attached socket and is idempotent.
    pub fn mark_closed(&self, reason: &str) {
        if !self.closed.swap(true, Ordering::SeqCst) {
            let _ = self.events.send(SessionEvent::Closed(Arc::from(reason)));
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
}

/// All live sessions on this server, with the detach grace period.
pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, Arc<LiveSession>>>,
    detach_ttl: Duration,
    max_per_user: usize,
}

impl SessionRegistry {
    pub const DEFAULT_DETACH_TTL: Duration = Duration::from_secs(15 * 60);
    pub const DEFAULT_MAX_PER_USER: usize = 16;

    pub fn new(detach_ttl: Duration, max_per_user: usize) -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
            detach_ttl,
            max_per_user: max_per_user.max(1),
        })
    }

    /// Read `SESSION_DETACH_TTL_SECS` (default 900) and `SESSION_MAX_PER_USER` (default 16).
    pub fn from_env() -> Arc<Self> {
        let ttl = std::env::var("SESSION_DETACH_TTL_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Self::DEFAULT_DETACH_TTL);
        let max = std::env::var("SESSION_MAX_PER_USER")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(Self::DEFAULT_MAX_PER_USER);
        Self::new(ttl, max)
    }

    pub fn detach_ttl(&self) -> Duration {
        self.detach_ttl
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

    /// A client went away. With no clients left, the session is closed after the grace
    /// period unless someone reattaches first.
    pub async fn detach(self: &Arc<Self>, session: &Arc<LiveSession>) {
        let left = session
            .attachments
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                Some(n.saturating_sub(1))
            })
            .map(|n| n.saturating_sub(1))
            .unwrap_or(0);
        if left > 0 || session.is_closed() {
            return;
        }
        let registry = Arc::downgrade(self);
        let weak = Arc::downgrade(session);
        let ttl = self.detach_ttl;
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
        *session.detach_timer.lock().await = Some(handle);
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
            });
        }
        out.sort_by_key(|s| s.created_at);
        out
    }

    /// Close every session (server shutdown).
    pub async fn close_all(&self) {
        let sessions: Vec<Arc<LiveSession>> =
            self.sessions.lock().await.values().cloned().collect();
        for s in sessions {
            s.close().await;
            s.mark_closed("Server shutting down");
        }
        if !self.sessions.lock().await.is_empty() {
            warn!("Closed live sessions for shutdown");
        }
    }
}

fn new_session_id() -> String {
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
            data: Arc::from(s),
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
    fn replay_returns_the_gap_or_reports_a_lost_window() {
        let mut ring = ReplayRing::new(10, 1000);
        for i in 1..=6 {
            ring.push(frame(i, "x"));
        }
        let got = ring.replay_since(4).unwrap();
        assert_eq!(got.iter().map(|f| f.seq).collect::<Vec<_>>(), [5, 6]);
        assert!(ring.replay_since(6).unwrap().is_empty());
        // Everything, for a client that has nothing yet
        assert_eq!(ring.replay_since(0).unwrap().len(), 6);

        let mut small = ReplayRing::new(2, 1000);
        for i in 1..=6 {
            small.push(frame(i, "x"));
        }
        assert_eq!(small.oldest_seq(), Some(5));
        assert_eq!(small.replay_since(2), Err(WindowLost));
        // last_seen == oldest - 1 is contiguous, so it is not a gap
        assert_eq!(small.replay_since(4).unwrap().len(), 2);
        assert!(small.replay_since(0).is_ok());
    }

    fn session(user: i64) -> (Arc<LiveSession>, mpsc::Receiver<PtyCommand>) {
        let (tx, rx) = mpsc::channel(8);
        (LiveSession::new(user, 1, "web".into(), tx), rx)
    }

    #[tokio::test]
    async fn output_is_numbered_broadcast_and_replayable() {
        let (s, _rx) = session(1);
        let mut sub = s.subscribe();
        assert_eq!(s.push_output("a".into()).await, 1);
        assert_eq!(s.push_output("b".into()).await, 2);
        match sub.recv().await.unwrap() {
            SessionEvent::Output(f) => assert_eq!((f.seq, &*f.data), (1, "a")),
            other => panic!("{other:?}"),
        }
        let late = s.replay_since(1).await.unwrap();
        assert_eq!(late.len(), 1);
        assert_eq!(&*late[0].data, "b");
        s.mark_closed("done");
        s.mark_closed("twice is harmless");
        assert!(s.is_closed());
    }

    #[tokio::test]
    async fn registry_attach_is_owner_only_and_detach_expires_after_ttl() {
        let registry = SessionRegistry::new(Duration::from_millis(60), 16);
        let (s, mut rx) = session(7);
        let id = s.id.clone();
        registry.insert(s.clone()).await.unwrap();
        registry.attach_new(&s);

        assert!(registry.attach(&id, 8).await.is_none(), "another user");
        assert!(registry.attach("nope", 7).await.is_none());

        registry.detach(&s).await;
        assert_eq!(s.attachments(), 0);
        // Reattach within the grace period cancels the close
        assert!(registry.attach(&id, 7).await.is_some());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!s.is_closed(), "reattach must cancel the timer");
        assert!(registry.get(&id, 7).await.is_some());

        // Detach again and let it expire
        registry.detach(&s).await;
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(s.is_closed());
        assert!(matches!(rx.try_recv(), Ok(PtyCommand::Close)));
        assert!(
            registry.get(&id, 7).await.is_none(),
            "removed from the registry"
        );
    }

    #[tokio::test]
    async fn listing_reports_what_the_resume_list_shows_and_skips_ended_sessions() {
        let registry = SessionRegistry::new(Duration::from_secs(60), 16);
        let (attached, _ra) = session(1);
        let (detached, _rd) = session(1);
        let (ended, _re) = session(1);
        let (theirs, _rt) = session(2);
        for s in [&attached, &detached, &ended, &theirs] {
            registry.insert(s.clone()).await.unwrap();
        }
        registry.attach_new(&attached);
        detached.push_output("one".into()).await;
        detached.push_output("two".into()).await;
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
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
    }

    #[tokio::test]
    async fn close_session_ends_the_shell_tells_listeners_and_is_owner_only() {
        let registry = SessionRegistry::new(Duration::from_secs(60), 16);
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
        let registry = SessionRegistry::new(Duration::from_secs(60), 1);
        let (a, _ra) = session(1);
        let (b, _rb) = session(1);
        let (c, _rc) = session(2);
        registry.insert(a).await.unwrap();
        assert!(registry.insert(b).await.unwrap_err().contains("limit 1"));
        registry.insert(c).await.unwrap();
        assert_eq!(registry.list_for_user(1).await.len(), 1);
    }
}

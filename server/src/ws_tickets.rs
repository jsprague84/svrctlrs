//! One-time tickets for WebSocket authentication.
//!
//! A browser cannot set headers on a WebSocket handshake, so a native client used to put
//! its session token in the URL, where proxies and logs could keep it. Instead the client
//! asks `POST /api/v1/auth/ws-ticket` (with its normal credentials) for a ticket, which is
//! random, bound to the user, valid for seconds and consumed by the first upgrade that
//! presents it. A leaked URL is then worthless almost immediately.

use base64::Engine;
use rand::RngCore;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct WsTickets {
    ttl: Duration,
    tickets: Mutex<HashMap<String, (i64, Instant)>>,
}

impl Default for WsTickets {
    fn default() -> Self {
        Self::new(Self::DEFAULT_TTL)
    }
}

impl WsTickets {
    pub const DEFAULT_TTL: Duration = Duration::from_secs(30);

    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            tickets: Mutex::new(HashMap::new()),
        }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Mint a ticket for `user_id`. Expired tickets are dropped while we hold the lock.
    pub fn issue(&self, user_id: i64) -> String {
        self.issue_at(user_id, Instant::now())
    }

    /// Exchange a ticket for the user it was issued to. A ticket redeems once.
    pub fn redeem(&self, ticket: &str) -> Option<i64> {
        self.redeem_at(ticket, Instant::now())
    }

    fn issue_at(&self, user_id: i64, now: Instant) -> String {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let ticket = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);

        let mut map = self.lock();
        map.retain(|_, (_, issued)| now.duration_since(*issued) < self.ttl);
        map.insert(ticket.clone(), (user_id, now));
        ticket
    }

    fn redeem_at(&self, ticket: &str, now: Instant) -> Option<i64> {
        let (user_id, issued) = self.lock().remove(ticket)?;
        (now.duration_since(issued) < self.ttl).then_some(user_id)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, (i64, Instant)>> {
        self.tickets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ticket_redeems_once_for_its_user() {
        let tickets = WsTickets::default();
        let t = tickets.issue(42);
        assert_eq!(t.len(), 43, "32 random bytes, base64url without padding");
        assert_eq!(tickets.redeem(&t), Some(42));
        assert_eq!(tickets.redeem(&t), None, "second use is refused");
        assert_eq!(tickets.redeem("not-a-ticket"), None);
    }

    #[test]
    fn tickets_expire_and_are_pruned() {
        let tickets = WsTickets::new(Duration::from_secs(30));
        let t0 = Instant::now();
        let stale = tickets.issue_at(1, t0);
        assert_eq!(
            tickets.redeem_at(&stale, t0 + Duration::from_secs(31)),
            None
        );

        let old = tickets.issue_at(1, t0);
        let fresh = tickets.issue_at(2, t0 + Duration::from_secs(60));
        // Issuing `fresh` pruned `old`; `fresh` itself is still live.
        assert!(!tickets.lock().contains_key(&old));
        assert_eq!(
            tickets.redeem_at(&fresh, t0 + Duration::from_secs(70)),
            Some(2)
        );
    }

    #[test]
    fn tickets_are_distinct() {
        let tickets = WsTickets::default();
        assert_ne!(tickets.issue(1), tickets.issue(1));
    }
}

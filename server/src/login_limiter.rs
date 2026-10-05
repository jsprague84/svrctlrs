//! Brute-force protection for the login endpoints.
//!
//! A sliding window of recent failures per key. Two keys are checked for every attempt: the
//! client address and the lowercased username, so an attacker cannot escape the limit by
//! rotating addresses against one account, nor lock everyone out by failing against many
//! accounts from one address. State is in memory; a restart clears it, which is acceptable
//! for a single-process server whose other control is the edge passkey login.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct LoginLimiter {
    max_failures: usize,
    window: Duration,
    failures: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl LoginLimiter {
    pub const DEFAULT_MAX_FAILURES: usize = 10;
    pub const DEFAULT_WINDOW: Duration = Duration::from_secs(15 * 60);

    pub fn new(max_failures: usize, window: Duration) -> Self {
        Self {
            max_failures,
            window,
            failures: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(Self::DEFAULT_MAX_FAILURES, Self::DEFAULT_WINDOW)
    }

    /// The two keys an attempt is judged by.
    pub fn keys(client_ip: &str, username: &str) -> [String; 2] {
        [
            format!("ip:{client_ip}"),
            format!("user:{}", username.trim().to_ascii_lowercase()),
        ]
    }

    /// `Err(retry_after)` when `key` has reached the failure limit inside the window.
    pub fn check(&self, key: &str) -> Result<(), Duration> {
        self.check_at(key, Instant::now())
    }

    pub fn record_failure(&self, key: &str) {
        self.record_failure_at(key, Instant::now());
    }

    /// A successful login forgets the key's history.
    pub fn clear(&self, key: &str) {
        self.lock().remove(key);
    }

    fn check_at(&self, key: &str, now: Instant) -> Result<(), Duration> {
        let mut map = self.lock();
        let Some(times) = map.get_mut(key) else {
            return Ok(());
        };
        while times
            .front()
            .is_some_and(|t| now.duration_since(*t) >= self.window)
        {
            times.pop_front();
        }
        if times.is_empty() {
            map.remove(key);
            return Ok(());
        }
        if times.len() >= self.max_failures {
            let oldest = times[0];
            return Err(self.window.saturating_sub(now.duration_since(oldest)));
        }
        Ok(())
    }

    fn record_failure_at(&self, key: &str, now: Instant) {
        self.lock()
            .entry(key.to_string())
            .or_default()
            .push_back(now);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, VecDeque<Instant>>> {
        // A poisoned lock only means another thread panicked mid-update; the map is still
        // a valid map and refusing to limit would be the worse failure.
        self.failures
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_until_the_limit_then_refuses_with_retry_after() {
        let limiter = LoginLimiter::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..3 {
            assert!(limiter.check_at("ip:1.2.3.4", t0).is_ok(), "attempt {i}");
            limiter.record_failure_at("ip:1.2.3.4", t0 + Duration::from_secs(i));
        }
        let retry = limiter
            .check_at("ip:1.2.3.4", t0 + Duration::from_secs(10))
            .unwrap_err();
        assert_eq!(retry, Duration::from_secs(50));
    }

    #[test]
    fn failures_expire_with_the_window() {
        let limiter = LoginLimiter::new(2, Duration::from_secs(60));
        let t0 = Instant::now();
        limiter.record_failure_at("user:admin", t0);
        limiter.record_failure_at("user:admin", t0 + Duration::from_secs(1));
        assert!(limiter
            .check_at("user:admin", t0 + Duration::from_secs(30))
            .is_err());
        // The first failure falls out of the window; one slot opens.
        assert!(limiter
            .check_at("user:admin", t0 + Duration::from_secs(60))
            .is_ok());
        // Both gone: the key is dropped from the map entirely.
        assert!(limiter
            .check_at("user:admin", t0 + Duration::from_secs(61))
            .is_ok());
        assert!(limiter.lock().is_empty());
    }

    #[test]
    fn success_clears_only_that_key() {
        let limiter = LoginLimiter::new(1, Duration::from_secs(60));
        limiter.record_failure("ip:1.2.3.4");
        limiter.record_failure("user:admin");
        limiter.clear("user:admin");
        assert!(limiter.check("user:admin").is_ok());
        assert!(limiter.check("ip:1.2.3.4").is_err());
    }

    #[test]
    fn keys_normalise_the_username() {
        assert_eq!(
            LoginLimiter::keys("10.0.0.1", "  Admin "),
            ["ip:10.0.0.1".to_string(), "user:admin".to_string()]
        );
    }
}

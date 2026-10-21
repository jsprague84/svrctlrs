//! Brute-force protection for every sign-in step: the password, a TOTP or recovery code, and
//! a provider callback that fails its checks.
//!
//! A sliding window of recent failures per key. Two keys are checked for every attempt: the
//! client address and the lowercased username, so an attacker cannot escape the limit by
//! rotating addresses against one account, nor lock everyone out by failing against many
//! accounts from one address. The failures live in the database (`login_failures`), so a
//! restart — every deploy is one — does not hand out a fresh allowance.

use std::time::Duration;

use svrctlrs_core::Result;
use svrctlrs_database::queries::login_failures;
use svrctlrs_database::{SqlxPool, SqlxSqlite};

pub struct LoginLimiter {
    pool: SqlxPool<SqlxSqlite>,
    max_failures: usize,
    window: Duration,
}

/// The first of an attempt's keys that has reached the limit, and how long until it is free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limited {
    pub key: String,
    pub retry_after: Duration,
}

impl LoginLimiter {
    pub const DEFAULT_MAX_FAILURES: usize = 10;
    pub const DEFAULT_WINDOW: Duration = Duration::from_secs(15 * 60);

    pub fn new(pool: SqlxPool<SqlxSqlite>, max_failures: usize, window: Duration) -> Self {
        Self {
            pool,
            max_failures: max_failures.max(1),
            window,
        }
    }

    pub fn with_defaults(pool: SqlxPool<SqlxSqlite>) -> Self {
        Self::new(pool, Self::DEFAULT_MAX_FAILURES, Self::DEFAULT_WINDOW)
    }

    /// The two keys an attempt is judged by.
    pub fn keys(client_ip: &str, username: &str) -> [String; 2] {
        [
            format!("ip:{client_ip}"),
            format!("user:{}", username.trim().to_ascii_lowercase()),
        ]
    }

    /// `Ok(Some(..))` when one of `keys` has reached the failure limit inside the window. A
    /// database error is an error: the caller refuses the attempt rather than skip the limit.
    pub async fn check(&self, keys: &[String]) -> Result<Option<Limited>> {
        self.check_at(keys, now()).await
    }

    pub async fn record_failure(&self, keys: &[String]) -> Result<()> {
        self.record_failure_at(keys, now()).await
    }

    /// A successful sign-in forgets the keys' history.
    pub async fn clear(&self, keys: &[String]) -> Result<()> {
        for key in keys {
            login_failures::clear(&self.pool, key).await?;
        }
        Ok(())
    }

    fn window_secs(&self) -> i64 {
        i64::try_from(self.window.as_secs()).unwrap_or(i64::MAX)
    }

    async fn check_at(&self, keys: &[String], now: i64) -> Result<Option<Limited>> {
        let window = self.window_secs();
        for key in keys {
            let times = login_failures::failures_since(&self.pool, key, now - window).await?;
            if times.len() >= self.max_failures {
                // The attempt is allowed again once enough of the oldest failures have left
                // the window to bring the count below the limit.
                let freeing = times[times.len() - self.max_failures];
                let secs = (freeing + window - now).max(1);
                return Ok(Some(Limited {
                    key: key.clone(),
                    retry_after: Duration::from_secs(secs.unsigned_abs()),
                }));
            }
        }
        Ok(None)
    }

    async fn record_failure_at(&self, keys: &[String], now: i64) -> Result<()> {
        login_failures::record(&self.pool, keys, now, now - self.window_secs()).await
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn limiter(max: usize, window_secs: u64) -> LoginLimiter {
        LoginLimiter::new(
            crate::test_support::database().await,
            max,
            Duration::from_secs(window_secs),
        )
    }

    fn keys(k: &[&str]) -> Vec<String> {
        k.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn allows_until_the_limit_then_refuses_with_retry_after() {
        let limiter = limiter(3, 60).await;
        let ip = keys(&["ip:1.2.3.4"]);
        let t0 = 1_000_000;
        for i in 0..3 {
            assert_eq!(
                limiter.check_at(&ip, t0 + i).await.unwrap(),
                None,
                "attempt {i}"
            );
            limiter.record_failure_at(&ip, t0 + i).await.unwrap();
        }
        let limited = limiter.check_at(&ip, t0 + 10).await.unwrap().unwrap();
        assert_eq!(limited.key, "ip:1.2.3.4");
        assert_eq!(limited.retry_after, Duration::from_secs(50));
    }

    #[tokio::test]
    async fn failures_expire_with_the_window_and_are_pruned() {
        let limiter = limiter(2, 60).await;
        let user = keys(&["user:admin"]);
        let t0 = 1_000_000;
        limiter.record_failure_at(&user, t0).await.unwrap();
        limiter.record_failure_at(&user, t0 + 1).await.unwrap();
        assert!(limiter.check_at(&user, t0 + 30).await.unwrap().is_some());
        // The first failure leaves the window; one slot opens.
        assert_eq!(limiter.check_at(&user, t0 + 60).await.unwrap(), None);
        // A later failure elsewhere prunes rows the window has passed.
        limiter
            .record_failure_at(&keys(&["ip:9.9.9.9"]), t0 + 120)
            .await
            .unwrap();
        let rows: i64 =
            svrctlrs_database::sqlx::query_scalar("SELECT COUNT(*) FROM login_failures")
                .fetch_one(&limiter.pool)
                .await
                .unwrap();
        assert_eq!(rows, 1);
    }

    #[tokio::test]
    async fn retry_after_counts_from_the_failure_that_frees_a_slot() {
        // Five failures against a limit of three: the third-oldest must expire first.
        let limiter = limiter(3, 100).await;
        let ip = keys(&["ip:1.2.3.4"]);
        for t in [0, 10, 20, 30, 40] {
            limiter.record_failure_at(&ip, 1_000 + t).await.unwrap();
        }
        let limited = limiter.check_at(&ip, 1_050).await.unwrap().unwrap();
        assert_eq!(limited.retry_after, Duration::from_secs(70));
    }

    #[tokio::test]
    async fn success_clears_only_those_keys() {
        let limiter = limiter(1, 60).await;
        limiter
            .record_failure(&keys(&["ip:1.2.3.4", "user:admin"]))
            .await
            .unwrap();
        limiter.clear(&keys(&["user:admin"])).await.unwrap();
        assert_eq!(limiter.check(&keys(&["user:admin"])).await.unwrap(), None);
        assert!(limiter
            .check(&keys(&["ip:1.2.3.4"]))
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn the_state_survives_a_new_limiter_on_the_same_database() {
        // What a restart looks like: the same file, a new process.
        let pool = crate::test_support::database().await;
        let user = keys(&["user:admin"]);
        let first = LoginLimiter::new(pool.clone(), 2, Duration::from_secs(900));
        first.record_failure(&user).await.unwrap();
        first.record_failure(&user).await.unwrap();
        drop(first);
        let second = LoginLimiter::new(pool, 2, Duration::from_secs(900));
        assert!(second.check(&user).await.unwrap().is_some());
    }

    #[test]
    fn keys_normalise_the_username() {
        assert_eq!(
            LoginLimiter::keys("10.0.0.1", "  Admin "),
            ["ip:10.0.0.1".to_string(), "user:admin".to_string()]
        );
    }
}

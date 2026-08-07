//! Per-visitor rate limiting for the form-submit path (`docs/design/
//! sites.md`, form flow): an in-memory sliding window keyed by the client
//! address. The key is used **transiently** — it exists only in this
//! process's map while inside the window and is never persisted, logged, or
//! attached to a submission (the privacy model stores no connection data).
//!
//! The budget ([`MAX_PER_WINDOW`] per [`WINDOW`]) is an anti-abuse bound,
//! not a quota: generous enough that an office behind one NAT address never
//! notices it, and tight enough that a flood through one address is
//! pointless. State is per-instance by design — a second replica doubling
//! the ceiling is acceptable for abuse control.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How many submissions one client key may make per [`WINDOW`].
pub const MAX_PER_WINDOW: usize = 10;
/// The sliding window length.
pub const WINDOW: Duration = Duration::from_secs(600);
/// Hard cap on distinct tracked keys. When full (after dropping expired
/// entries) new keys are refused instead of growing memory: an attacker
/// cycling addresses buys nothing, and real traffic never gets near it.
const MAX_TRACKED_KEYS: usize = 4096;

/// The in-memory sliding-window limiter, shared across request tasks. A
/// poisoned lock is taken anyway — the map holds only timestamps, and the
/// public service must keep answering.
#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    /// Registers an attempt by `key` at `now`. `Ok(())` means the attempt is
    /// within budget and now counts against it; `Err(seconds)` means it is
    /// refused (nothing recorded — refusals are cheap) and carries the
    /// `Retry-After` hint, always at least 1.
    pub fn allow(&self, key: &str, now: Instant) -> Result<(), u64> {
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        if !windows.contains_key(key) && windows.len() >= MAX_TRACKED_KEYS {
            windows.retain(|_, stamps| {
                stamps
                    .last()
                    .is_some_and(|last| now.duration_since(*last) < WINDOW)
            });
            if windows.len() >= MAX_TRACKED_KEYS {
                return Err(WINDOW.as_secs());
            }
        }
        let stamps = windows.entry(key.to_owned()).or_default();
        stamps.retain(|stamp| now.duration_since(*stamp) < WINDOW);
        if stamps.len() >= MAX_PER_WINDOW {
            // The budget frees when the oldest counted attempt ages out.
            let oldest = stamps.iter().min().copied().unwrap_or(now);
            let wait = WINDOW.saturating_sub(now.duration_since(oldest));
            return Err(wait.as_secs().max(1));
        }
        stamps.push(now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn budget_then_refusal_with_a_retry_hint_and_independent_keys() {
        let limiter = RateLimiter::default();
        let t0 = Instant::now();
        for n in 0..MAX_PER_WINDOW {
            assert!(limiter.allow("a", t0).is_ok(), "attempt {n} within budget");
        }
        let wait = limiter.allow("a", t0).unwrap_err();
        assert!((1..=WINDOW.as_secs()).contains(&wait));
        assert!(limiter.allow("b", t0).is_ok(), "other keys are unaffected");
    }

    #[test]
    fn refusals_do_not_extend_the_window_and_budget_returns() {
        let limiter = RateLimiter::default();
        let t0 = Instant::now();
        for _ in 0..MAX_PER_WINDOW {
            limiter.allow("a", t0).unwrap();
        }
        assert!(limiter.allow("a", t0 + WINDOW / 2).is_err());
        // Just past the window every counted attempt has expired — the
        // mid-window refusal must not have restarted the clock.
        assert!(
            limiter
                .allow("a", t0 + WINDOW + Duration::from_secs(1))
                .is_ok()
        );
    }

    #[test]
    fn tracked_keys_are_bounded_until_old_windows_expire() {
        let limiter = RateLimiter::default();
        let t0 = Instant::now();
        for n in 0..4096 {
            limiter.allow(&format!("key-{n}"), t0).unwrap();
        }
        assert_eq!(limiter.allow("fresh", t0).unwrap_err(), WINDOW.as_secs());
        assert!(
            limiter
                .allow("fresh", t0 + WINDOW + Duration::from_secs(1))
                .is_ok()
        );
    }
}

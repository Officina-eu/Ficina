//! Per-IP connection admission for the SMTP listeners: a concurrency
//! cap and a per-minute accept-rate cap, both keyed on the peer
//! address. This is the **pre-DATA** abuse control — Rspamd only sees
//! a message at DATA, so a peer opening hundreds of sessions (slow
//! loris, RCPT probing, greeting floods) must be stopped at accept.
//! Complements, never replaces, the global connection semaphore.
//!
//! In-memory and single-node by design (one MX process); entries are
//! bounded and expire, so an address-diverse attacker degrades to the
//! global cap rather than growing the table without limit.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The per-IP limits. A value of `0` disables that check.
#[derive(Debug, Clone, Copy)]
pub struct ConnLimits {
    /// Concurrent sessions allowed per address.
    pub max_concurrent: usize,
    /// New connections allowed per address per minute.
    pub max_per_minute: u32,
}

impl Default for ConnLimits {
    /// Generous defaults: a legitimate MTA retries with a handful of
    /// connections; only floods cross these.
    fn default() -> Self {
        Self {
            max_concurrent: 16,
            max_per_minute: 60,
        }
    }
}

/// The rolling per-address book-keeping.
struct IpEntry {
    active: usize,
    window_start: Instant,
    in_window: u32,
    last_seen: Instant,
}

/// Above this many tracked addresses, stale inactive entries are
/// swept. Far above anything legitimate; bounds hostile diversity.
const SWEEP_THRESHOLD: usize = 4096;
/// An inactive entry older than this is forgotten during a sweep.
const STALE_AFTER: Duration = Duration::from_secs(120);
/// The rate window.
const WINDOW: Duration = Duration::from_secs(60);

/// The admission decision for one incoming connection.
pub enum Admission {
    /// Admitted; drop the permit when the session ends.
    Granted(ConnPermit),
    /// Over a per-IP limit — greet with 421 and close.
    TooMany,
}

/// Releases the per-IP concurrency slot on drop.
pub struct ConnPermit {
    gate: Arc<Mutex<HashMap<IpAddr, IpEntry>>>,
    ip: IpAddr,
}

impl Drop for ConnPermit {
    fn drop(&mut self) {
        if let Ok(mut map) = self.gate.lock()
            && let Some(entry) = map.get_mut(&self.ip)
        {
            entry.active = entry.active.saturating_sub(1);
        }
    }
}

/// The shared per-IP gate, one per listener.
pub struct ConnGate {
    limits: ConnLimits,
    inner: Arc<Mutex<HashMap<IpAddr, IpEntry>>>,
}

impl ConnGate {
    /// Builds a gate with the given limits.
    pub fn new(limits: ConnLimits) -> Self {
        Self {
            limits,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Decides admission for a connection from `ip`. Never blocks; a
    /// poisoned lock (a panicked holder) admits rather than taking the
    /// listener down with it.
    pub fn admit(&self, ip: IpAddr) -> Admission {
        let now = Instant::now();
        let Ok(mut map) = self.inner.lock() else {
            return Admission::Granted(ConnPermit {
                gate: Arc::clone(&self.inner),
                ip,
            });
        };
        if map.len() > SWEEP_THRESHOLD {
            map.retain(|_, e| e.active > 0 || now.duration_since(e.last_seen) < STALE_AFTER);
        }
        let entry = map.entry(ip).or_insert(IpEntry {
            active: 0,
            window_start: now,
            in_window: 0,
            last_seen: now,
        });
        entry.last_seen = now;
        if now.duration_since(entry.window_start) >= WINDOW {
            entry.window_start = now;
            entry.in_window = 0;
        }
        let over_rate =
            self.limits.max_per_minute != 0 && entry.in_window >= self.limits.max_per_minute;
        let over_concurrent =
            self.limits.max_concurrent != 0 && entry.active >= self.limits.max_concurrent;
        if over_rate || over_concurrent {
            return Admission::TooMany;
        }
        entry.in_window += 1;
        entry.active += 1;
        drop(map);
        Admission::Granted(ConnPermit {
            gate: Arc::clone(&self.inner),
            ip,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn ip(last: u8) -> IpAddr {
        IpAddr::from([203, 0, 113, last])
    }

    #[test]
    fn concurrency_cap_enforced_and_released_on_drop() {
        let gate = ConnGate::new(ConnLimits {
            max_concurrent: 2,
            max_per_minute: 0, // rate check off
        });
        let a = match gate.admit(ip(1)) {
            Admission::Granted(p) => p,
            Admission::TooMany => panic!("first admit"),
        };
        let _b = match gate.admit(ip(1)) {
            Admission::Granted(p) => p,
            Admission::TooMany => panic!("second admit"),
        };
        assert!(matches!(gate.admit(ip(1)), Admission::TooMany));
        // Another address is unaffected.
        assert!(matches!(gate.admit(ip(2)), Admission::Granted(_)));
        // Releasing one slot readmits.
        drop(a);
        assert!(matches!(gate.admit(ip(1)), Admission::Granted(_)));
    }

    #[test]
    fn rate_cap_enforced_within_the_window() {
        let gate = ConnGate::new(ConnLimits {
            max_concurrent: 0, // concurrency check off
            max_per_minute: 3,
        });
        for _ in 0..3 {
            match gate.admit(ip(9)) {
                Admission::Granted(permit) => drop(permit), // short sessions
                Admission::TooMany => panic!("within budget"),
            }
        }
        // The 4th in the same window is refused even though nothing is
        // concurrent — the rate is what's exhausted.
        assert!(matches!(gate.admit(ip(9)), Admission::TooMany));
        assert!(matches!(gate.admit(ip(10)), Admission::Granted(_)));
    }

    #[test]
    fn zero_disables_both_checks() {
        let gate = ConnGate::new(ConnLimits {
            max_concurrent: 0,
            max_per_minute: 0,
        });
        let mut permits = Vec::new();
        for _ in 0..100 {
            match gate.admit(ip(7)) {
                Admission::Granted(p) => permits.push(p),
                Admission::TooMany => panic!("disabled gate must admit"),
            }
        }
    }

    #[test]
    fn sweep_keeps_active_entries() {
        let gate = ConnGate::new(ConnLimits::default());
        let _held = match gate.admit(ip(1)) {
            Admission::Granted(p) => p,
            Admission::TooMany => panic!(),
        };
        // Force a sweep by inflating the table.
        {
            let mut map = gate.inner.lock().unwrap();
            let old = Instant::now() - Duration::from_secs(600);
            for i in 0..(SWEEP_THRESHOLD + 8) {
                map.insert(
                    IpAddr::from([10, 0, (i / 256) as u8, (i % 256) as u8]),
                    IpEntry {
                        active: 0,
                        window_start: old,
                        in_window: 1,
                        last_seen: old,
                    },
                );
            }
        }
        // Next admit sweeps the stale bulk but keeps the active entry.
        assert!(matches!(gate.admit(ip(2)), Admission::Granted(_)));
        let map = gate.inner.lock().unwrap();
        assert!(map.len() < 100, "stale entries swept, len={}", map.len());
        assert!(map.contains_key(&ip(1)), "active entry survives the sweep");
    }
}

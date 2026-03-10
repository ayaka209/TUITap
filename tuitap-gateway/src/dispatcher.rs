use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::{Duration, Instant},
};

/// Event-driven dispatcher that suppresses duplicate and too-frequent pushes.
///
/// A snapshot is eligible for dispatch only when **both** conditions hold:
///
/// 1. The content has changed since the last dispatched snapshot.
/// 2. At least `cooldown` time has elapsed since the last dispatch.
///
/// The cooldown prevents flooding channels during rapid terminal activity
/// (e.g. while a progress bar is updating).
#[derive(Debug)]
pub struct Dispatcher {
    last_hash: Option<u64>,
    last_sent: Option<Instant>,
    cooldown: Duration,
}

impl Dispatcher {
    /// Create a dispatcher with the given cooldown window.
    pub fn new(cooldown: Duration) -> Self {
        Dispatcher {
            last_hash: None,
            last_sent: None,
            cooldown,
        }
    }

    /// Create a dispatcher with a 500 ms default cooldown.
    pub fn default_cooldown() -> Self {
        Dispatcher::new(Duration::from_millis(500))
    }

    /// Return `true` if `content` should be dispatched now.
    ///
    /// Updates internal state (last hash / last sent timestamp) only when
    /// returning `true`.
    pub fn should_dispatch(&mut self, content: &str) -> bool {
        let hash = hash_str(content);
        let now = Instant::now();

        // Unchanged content → skip
        if self.last_hash == Some(hash) {
            return false;
        }

        // Within cooldown window → skip
        if let Some(last) = self.last_sent {
            if now.duration_since(last) < self.cooldown {
                return false;
            }
        }

        self.last_hash = Some(hash);
        self.last_sent = Some(now);
        true
    }

    /// Force-reset all state (useful for tests or on PTY restart).
    pub fn reset(&mut self) {
        self.last_hash = None;
        self.last_sent = None;
    }

    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }
}

fn hash_str(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_call_dispatches() {
        let mut d = Dispatcher::new(Duration::ZERO);
        assert!(d.should_dispatch("hello"));
    }

    #[test]
    fn test_same_content_no_dispatch() {
        let mut d = Dispatcher::new(Duration::ZERO);
        assert!(d.should_dispatch("hello"));
        assert!(!d.should_dispatch("hello"));
    }

    #[test]
    fn test_changed_content_dispatches() {
        let mut d = Dispatcher::new(Duration::ZERO);
        assert!(d.should_dispatch("hello"));
        assert!(d.should_dispatch("world"));
    }

    #[test]
    fn test_cooldown_blocks_rapid_changes() {
        let mut d = Dispatcher::new(Duration::from_secs(60));
        assert!(d.should_dispatch("a"));
        // Content changed but still within cooldown → blocked
        assert!(!d.should_dispatch("b"));
    }

    #[test]
    fn test_reset_allows_redispatch() {
        let mut d = Dispatcher::new(Duration::ZERO);
        assert!(d.should_dispatch("hello"));
        d.reset();
        assert!(d.should_dispatch("hello"));
    }
}

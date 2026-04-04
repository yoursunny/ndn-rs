//! Exponential backoff with jitter for hello/probe scheduling.
//!
//! Used by neighbor discovery to schedule retransmits and probes without
//! creating correlated bursts across multiple nodes (thundering-herd effect).
//!
//! The algorithm:
//! 1. Start at `initial_interval`.
//! 2. On each failure, double the interval (capped at `max_interval`).
//! 3. Add uniform random jitter of ±`jitter_fraction` of the current interval.
//! 4. On success, reset to `initial_interval`.

use std::time::Duration;

/// Static configuration for a backoff strategy.
///
/// Construct once and pass by reference; per-instance mutable state lives
/// in [`BackoffState`].
#[derive(Clone, Debug)]
pub struct BackoffConfig {
    /// Initial retry interval.
    pub initial_interval: Duration,
    /// Maximum retry interval (backoff ceiling).
    pub max_interval: Duration,
    /// Jitter fraction in [0.0, 1.0].  `0.25` means ±25 % of the current
    /// interval is added as random noise.
    pub jitter_fraction: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(30),
            jitter_fraction: 0.25,
        }
    }
}

impl BackoffConfig {
    /// Backoff profile suitable for link-local neighbor discovery hellos.
    ///
    /// Short initial interval for fast bootstrap; caps at 10 s to avoid
    /// stale neighbor entries persisting too long.
    pub fn for_neighbor_hello() -> Self {
        Self {
            initial_interval: Duration::from_millis(500),
            max_interval: Duration::from_secs(10),
            jitter_fraction: 0.3,
        }
    }

    /// Backoff profile for SWIM indirect probing.
    ///
    /// Shorter intervals to match SWIM's failure-detection SLA.
    pub fn for_swim_probe() -> Self {
        Self {
            initial_interval: Duration::from_millis(200),
            max_interval: Duration::from_secs(5),
            jitter_fraction: 0.2,
        }
    }
}

/// Per-instance mutable backoff state.
///
/// Carries the current interval and a lightweight pseudo-random seed
/// so that no external RNG dependency is needed.
#[derive(Clone, Debug)]
pub struct BackoffState {
    current: Duration,
    /// Xorshift32 seed for jitter; never zero.
    rng: u32,
}

impl BackoffState {
    /// Create fresh state, seeded from the given value.
    ///
    /// Use a per-peer value (e.g. truncated FaceId or timestamp) as the seed
    /// to decorrelate hellos across nodes.
    pub fn new(seed: u32) -> Self {
        Self {
            current: Duration::ZERO,
            rng: if seed == 0 { 0xdeadbeef } else { seed },
        }
    }

    /// Compute the next wait duration and update internal state.
    ///
    /// Call this after a *failed* probe to get the retry delay.
    pub fn next_failure(&mut self, cfg: &BackoffConfig) -> Duration {
        if self.current.is_zero() {
            self.current = cfg.initial_interval;
        } else {
            self.current = (self.current * 2).min(cfg.max_interval);
        }
        self.apply_jitter(cfg)
    }

    /// Reset to the initial interval after a *successful* exchange.
    ///
    /// Clears the current interval so the next [`next_failure`] call
    /// starts fresh from `cfg.initial_interval`.
    pub fn reset(&mut self, _cfg: &BackoffConfig) {
        self.current = Duration::ZERO;
    }

    /// Current interval without modification (peek).
    pub fn current(&self) -> Duration {
        self.current
    }

    fn apply_jitter(&mut self, cfg: &BackoffConfig) -> Duration {
        let base_ms = self.current.as_millis() as u64;
        if base_ms == 0 || cfg.jitter_fraction <= 0.0 {
            return self.current;
        }
        // Xorshift32 for lightweight deterministic noise.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let range_ms = (base_ms as f64 * cfg.jitter_fraction) as u64;
        let jitter_ms = if range_ms > 0 {
            (self.rng as u64 % (2 * range_ms)) as i64 - range_ms as i64
        } else {
            0
        };
        let result_ms = (base_ms as i64 + jitter_ms).max(1) as u64;
        Duration::from_millis(result_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_on_failure() {
        let cfg = BackoffConfig {
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(10),
            jitter_fraction: 0.0, // no jitter for determinism
        };
        let mut state = BackoffState::new(1);
        let d1 = state.next_failure(&cfg);
        let d2 = state.next_failure(&cfg);
        let d3 = state.next_failure(&cfg);
        assert_eq!(d1, Duration::from_millis(100));
        assert_eq!(d2, Duration::from_millis(200));
        assert_eq!(d3, Duration::from_millis(400));
    }

    #[test]
    fn capped_at_max() {
        let cfg = BackoffConfig {
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_millis(300),
            jitter_fraction: 0.0,
        };
        let mut state = BackoffState::new(1);
        for _ in 0..10 {
            state.next_failure(&cfg);
        }
        assert_eq!(state.current(), Duration::from_millis(300));
    }

    #[test]
    fn reset_goes_to_zero() {
        let cfg = BackoffConfig {
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(30),
            jitter_fraction: 0.0, // no jitter for determinism
        };
        let mut state = BackoffState::new(42);
        state.next_failure(&cfg);
        state.next_failure(&cfg);
        state.reset(&cfg);
        // After reset, current is zero; next_failure will start from initial.
        assert_eq!(state.current(), Duration::ZERO);
        let d = state.next_failure(&cfg);
        assert_eq!(d, cfg.initial_interval);
    }

    #[test]
    fn jitter_stays_in_range() {
        let cfg = BackoffConfig {
            initial_interval: Duration::from_millis(1000),
            max_interval: Duration::from_secs(60),
            jitter_fraction: 0.25,
        };
        let mut state = BackoffState::new(999);
        for _ in 0..50 {
            let d = state.next_failure(&cfg);
            // 1000ms ± 25% → [750ms, 1250ms]
            assert!(d >= Duration::from_millis(750), "too low: {d:?}");
            assert!(d <= Duration::from_millis(1250), "too high: {d:?}");
            state.reset(&cfg);
        }
    }
}

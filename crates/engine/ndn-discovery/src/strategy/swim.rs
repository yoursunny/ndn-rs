//! SWIM-style fixed-interval probe scheduler.
//!
//! Unlike [`BackoffScheduler`], `SwimScheduler` does NOT apply exponential
//! back-off — it broadcasts at a constant period T, matching the SWIM paper's
//! requirement that each node probes exactly one random peer per protocol
//! period.
//!
//! **Indirect probing** (K-fanout via `/ndn/local/nd/probe/via/`) is handled
//! by the protocol layer, not here.  This scheduler only decides *when* to
//! send the next broadcast hello.

use std::time::{Duration, Instant};

use crate::config::DiscoveryConfig;
use crate::strategy::{NeighborProbeStrategy, ProbeRequest, TriggerEvent};

// ── SwimScheduler ─────────────────────────────────────────────────────────────

/// Fixed-interval SWIM probe scheduler.
///
/// Broadcasts a hello at every `interval`.  Unlike [`BackoffScheduler`] it
/// never increases its interval after a successful exchange.  Topology changes
/// (`trigger()`) cause an immediate probe on the next tick, after which the
/// regular period resumes.
///
/// [`BackoffScheduler`]: super::BackoffScheduler
pub struct SwimScheduler {
    interval: Duration,
    next_probe_at: Instant,
    pending_immediate: bool,
}

impl SwimScheduler {
    /// Create a scheduler firing every `interval`.
    ///
    /// Sends an immediate probe on the first tick to bootstrap the neighbor
    /// table.
    pub fn new(interval: Duration) -> Self {
        let now = Instant::now();
        Self {
            interval,
            next_probe_at: now + interval,
            pending_immediate: true,
        }
    }

    /// Build from a [`DiscoveryConfig`], using `hello_interval_base` as the
    /// fixed SWIM protocol period T.
    pub fn from_discovery_config(cfg: &DiscoveryConfig) -> Self {
        Self::new(cfg.hello_interval_base)
    }
}

impl NeighborProbeStrategy for SwimScheduler {
    fn on_tick(&mut self, now: Instant) -> Vec<ProbeRequest> {
        if self.pending_immediate || now >= self.next_probe_at {
            self.pending_immediate = false;
            self.next_probe_at = now + self.interval;
            vec![ProbeRequest::Broadcast]
        } else {
            vec![]
        }
    }

    fn on_probe_success(&mut self, _rtt: Duration) {
        // SWIM uses a fixed interval; a successful exchange does not alter the
        // schedule.
    }

    fn on_probe_timeout(&mut self) {
        // Indirect probing on failure is handled by the protocol layer.
        // The scheduler keeps its fixed rate regardless of probe outcomes.
    }

    fn trigger(&mut self, event: TriggerEvent) {
        match event {
            TriggerEvent::FaceUp
            | TriggerEvent::ForwardingFailure
            | TriggerEvent::NeighborStale => {
                self.pending_immediate = true;
            }
            TriggerEvent::PassiveDetection => {
                // Passive MAC overhearing is not a SWIM topology event; ignore.
            }
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_immediately_on_first_tick() {
        let mut s = SwimScheduler::new(Duration::from_secs(5));
        let now = Instant::now();
        let probes = s.on_tick(now);
        assert_eq!(probes, vec![ProbeRequest::Broadcast]);
    }

    #[test]
    fn no_second_fire_before_interval() {
        let mut s = SwimScheduler::new(Duration::from_secs(5));
        let now = Instant::now();
        s.on_tick(now);
        let probes = s.on_tick(now + Duration::from_millis(100));
        assert!(probes.is_empty());
    }

    #[test]
    fn fires_after_interval() {
        let mut s = SwimScheduler::new(Duration::from_secs(5));
        let now = Instant::now();
        s.on_tick(now); // consume initial
        let probes = s.on_tick(now + Duration::from_secs(6));
        assert_eq!(probes, vec![ProbeRequest::Broadcast]);
    }

    #[test]
    fn timeout_does_not_change_interval() {
        let interval = Duration::from_secs(5);
        let mut s = SwimScheduler::new(interval);
        let now = Instant::now();
        s.on_tick(now); // consume initial
        s.on_probe_timeout();
        // Should NOT fire early
        assert!(s.on_tick(now + Duration::from_millis(100)).is_empty());
        // Should fire at the regular interval
        assert_eq!(
            s.on_tick(now + interval + Duration::from_millis(100)),
            vec![ProbeRequest::Broadcast]
        );
    }

    #[test]
    fn trigger_schedules_immediate_probe() {
        let mut s = SwimScheduler::new(Duration::from_secs(60));
        let now = Instant::now();
        s.on_tick(now); // consume initial
        s.trigger(TriggerEvent::NeighborStale);
        let probes = s.on_tick(now + Duration::from_millis(10));
        assert_eq!(probes, vec![ProbeRequest::Broadcast]);
    }

    #[test]
    fn passive_detection_ignored() {
        let mut s = SwimScheduler::new(Duration::from_secs(60));
        let now = Instant::now();
        s.on_tick(now); // consume initial
        s.trigger(TriggerEvent::PassiveDetection);
        // PassiveDetection must NOT trigger an immediate probe
        assert!(s.on_tick(now + Duration::from_millis(1)).is_empty());
    }
}

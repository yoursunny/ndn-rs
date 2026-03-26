use dashmap::DashMap;
use ndn_packet::Name;
use ndn_transport::FaceId;
use std::sync::Arc;

/// Per-prefix flow entry tracking observed throughput and preferred radio face.
#[derive(Clone, Debug)]
pub struct FlowEntry {
    pub prefix:         Arc<Name>,
    /// The radio face that has been giving the best performance.
    pub preferred_face: FaceId,
    /// EWMA bytes/sec observed on this prefix.
    pub observed_tput:  f32,
    /// EWMA RTT in milliseconds.
    pub observed_rtt_ms: f32,
    /// Timestamp of last update (ns since Unix epoch).
    pub last_updated:   u64,
}

/// Maps name prefixes to preferred radio faces based on observed flow performance.
///
/// Acts as a fast path for the `MultiRadioStrategy` — established flows skip the
/// FIB and go directly to the historically best face. Invalidated when radio
/// channel assignments change.
pub struct FlowTable {
    entries: DashMap<Arc<Name>, FlowEntry>,
}

impl FlowTable {
    pub fn new() -> Self {
        Self { entries: DashMap::new() }
    }

    pub fn get(&self, name: &Arc<Name>) -> Option<FlowEntry> {
        self.entries.get(name).map(|r| r.clone())
    }

    pub fn update(&self, entry: FlowEntry) {
        self.entries.insert(Arc::clone(&entry.prefix), entry);
    }

    /// Clear all entries for faces on `iface` — called on channel switch.
    pub fn flush_interface(&self, face_id: FaceId) {
        self.entries.retain(|_, v| v.preferred_face != face_id);
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

impl Default for FlowTable {
    fn default() -> Self { Self::new() }
}

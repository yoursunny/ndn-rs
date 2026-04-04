//! `DiscoveryProtocol` trait and `ProtocolId` type.

use std::time::Instant;

use bytes::Bytes;
use ndn_packet::Name;
use ndn_transport::FaceId;

use crate::DiscoveryContext;

/// Stable identifier for a discovery protocol instance.
///
/// Used to tag FIB entries so they can be bulk-removed when the protocol
/// stops or reconfigures, and to route inbound packets in `CompositeDiscovery`
/// without ambiguity.
///
/// Implementations should use a short, descriptive ASCII string such as
/// `"ether-nd"`, `"swim"`, or `"sd-browser"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProtocolId(pub &'static str);

impl std::fmt::Display for ProtocolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// A pluggable discovery protocol.
///
/// Implementations observe face lifecycle events and inbound packets;
/// they mutate engine state exclusively through the [`DiscoveryContext`]
/// interface, making them decoupled from the engine and independently
/// testable.
///
/// # Namespace isolation
///
/// Each protocol declares which NDN name prefixes it uses via
/// [`claimed_prefixes`].  [`CompositeDiscovery`] checks at construction
/// time that no two protocols claim overlapping prefixes.  All discovery
/// prefixes live under the reserved `/ndn/local/` sub-tree.
///
/// [`CompositeDiscovery`]: crate::CompositeDiscovery
pub trait DiscoveryProtocol: Send + Sync + 'static {
    /// Unique protocol identifier.
    fn protocol_id(&self) -> ProtocolId;

    /// NDN name prefixes this protocol reserves.
    ///
    /// Typically sub-prefixes of `/ndn/local/nd/` (neighbor discovery) or
    /// `/ndn/local/sd/` (service discovery).  Used for namespace conflict
    /// detection and inbound routing in `CompositeDiscovery`.
    fn claimed_prefixes(&self) -> &[Name];

    /// Called when a new face comes up (after `FaceTable::insert`).
    fn on_face_up(&self, face_id: FaceId, ctx: &dyn DiscoveryContext);

    /// Called when a face goes down (before `FaceTable::remove`).
    fn on_face_down(&self, face_id: FaceId, ctx: &dyn DiscoveryContext);

    /// Called for every inbound raw packet before it enters the pipeline.
    ///
    /// Returns `true` if the packet was consumed by this protocol and should
    /// **not** be forwarded through the NDN pipeline.  Return `false` to let
    /// the packet continue normally.
    ///
    /// Discovery protocols intercept packets addressed to their claimed
    /// prefixes (e.g. hello Interests sent to `/ndn/local/nd/hello`).
    fn on_inbound(&self, raw: &Bytes, incoming_face: FaceId, ctx: &dyn DiscoveryContext) -> bool;

    /// Periodic tick, called every ~100 ms by the engine's tick task.
    ///
    /// Use this to send hellos, check timeouts, rotate probes, and update
    /// SWIM gossip state.
    fn on_tick(&self, now: Instant, ctx: &dyn DiscoveryContext);
}

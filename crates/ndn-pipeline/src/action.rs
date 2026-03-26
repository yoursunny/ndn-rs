use bytes::Bytes;
use smallvec::SmallVec;

use ndn_transport::FaceId;

/// Reason a packet was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropReason {
    MalformedPacket,
    UnknownFace,
    LoopDetected,
    Suppressed,
    RateLimited,
    Other,
}

/// Reason for a Nack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NackReason {
    NoRoute,
    Duplicate,
    Congestion,
    NotYet,
}

/// The return value from a pipeline stage.
///
/// Ownership-based: `Continue` returns the context back to the runner.
/// All other variants consume the context, making it a compiler error to
/// use the context after it has been handed off.
pub enum Action {
    /// Pass context to the next stage.
    Continue(super::context::PacketContext),
    /// Forward the packet to the given faces and exit the pipeline.
    Send(super::context::PacketContext, SmallVec<[FaceId; 4]>),
    /// Satisfy pending PIT entries and exit the pipeline.
    Satisfy(super::context::PacketContext),
    /// Drop the packet silently.
    Drop(DropReason),
    /// Send a Nack back to the incoming face.
    Nack(NackReason),
}

/// The forwarding decision returned by a `Strategy`.
pub enum ForwardingAction {
    /// Forward to these faces immediately.
    Forward(SmallVec<[FaceId; 4]>),
    /// Forward to these faces after `delay`.
    ForwardAfter {
        faces: SmallVec<[FaceId; 4]>,
        delay: std::time::Duration,
    },
    /// Send a Nack.
    Nack(NackReason),
    /// Suppress — do not forward (loop or policy decision).
    Suppress,
}

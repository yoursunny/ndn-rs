use tokio::sync::mpsc;
use ndn_pipeline::{Action, DropReason, PacketContext, PipelineStage};

/// A pipeline stage that emits observation events without blocking forwarding.
///
/// Uses `try_send` on a bounded channel — events are dropped when the receiver
/// falls behind rather than slowing the forwarding pipeline.
pub struct FlowObserverStage {
    tx:              mpsc::Sender<FlowEvent>,
    /// Optional sampling rate (0.0–1.0). Events are dropped probabilistically
    /// to limit observer overhead on high-rate testbeds.
    sampling_rate:   f32,
}

/// An observation event emitted for each packet.
#[derive(Debug, Clone)]
pub struct FlowEvent {
    pub arrival_ns:  u64,
    pub face_id:     ndn_transport::FaceId,
    pub name:        Option<std::sync::Arc<ndn_packet::Name>>,
    pub packet_type: PacketType,
    pub cs_hit:      bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Interest,
    Data,
    Nack,
    Unknown,
}

impl FlowObserverStage {
    pub fn new(tx: mpsc::Sender<FlowEvent>) -> Self {
        Self { tx, sampling_rate: 1.0 }
    }

    pub fn with_sampling_rate(mut self, rate: f32) -> Self {
        self.sampling_rate = rate.clamp(0.0, 1.0);
        self
    }
}

impl PipelineStage for FlowObserverStage {
    async fn process(&self, ctx: PacketContext) -> Result<Action, DropReason> {
        let packet_type = match &ctx.packet {
            ndn_pipeline::DecodedPacket::Interest(_) => PacketType::Interest,
            ndn_pipeline::DecodedPacket::Data(_)     => PacketType::Data,
            ndn_pipeline::DecodedPacket::Nack(_)     => PacketType::Nack,
            ndn_pipeline::DecodedPacket::Raw         => PacketType::Unknown,
        };

        let event = FlowEvent {
            arrival_ns:  ctx.arrival,
            face_id:     ctx.face_id,
            name:        ctx.name.clone(),
            packet_type,
            cs_hit:      ctx.cs_hit,
        };

        // try_send is non-blocking — drops the event if the receiver is behind.
        let _ = self.tx.try_send(event);

        Ok(Action::Continue(ctx))
    }
}

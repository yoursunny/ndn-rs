use bytes::Bytes;
use ndn_transport::{Face, FaceError, FaceId, FaceKind};

/// NDN face over a serial port with COBS framing.
///
/// COBS (Consistent Overhead Byte Stuffing) provides reliable frame
/// resynchronisation after line noise — a `0x00` byte never appears in the
/// encoded payload, so re-sync is just a matter of waiting for the next `0x00`.
///
/// Suitable for: UART sensor nodes, LoRa radio modems, RS-485 industrial buses.
pub struct SerialFace {
    id:   FaceId,
    port: String,
    baud: u32,
}

impl SerialFace {
    pub fn new(id: FaceId, port: impl Into<String>, baud: u32) -> Self {
        Self { id, port: port.into(), baud }
    }
}

impl Face for SerialFace {
    fn id(&self) -> FaceId { self.id }
    fn kind(&self) -> FaceKind { FaceKind::Serial }

    async fn recv(&self) -> Result<Bytes, FaceError> {
        Err(FaceError::Closed) // placeholder
    }

    async fn send(&self, _pkt: Bytes) -> Result<(), FaceError> {
        Err(FaceError::Closed) // placeholder
    }
}

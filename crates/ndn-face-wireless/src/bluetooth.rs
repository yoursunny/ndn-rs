use bytes::Bytes;
use ndn_transport::{Face, FaceError, FaceId, FaceKind};

/// NDN face over Bluetooth Classic (RFCOMM).
///
/// On Linux, a paired RFCOMM channel appears as `/dev/rfcommN`.
/// This face reuses the same COBS-framed stream model as `SerialFace`.
/// Throughput up to ~3 Mbps; latency 20–40 ms.
pub struct BluetoothFace {
    id: FaceId,
}

impl BluetoothFace {
    pub fn new(id: FaceId) -> Self { Self { id } }
}

impl Face for BluetoothFace {
    fn id(&self) -> FaceId { self.id }
    fn kind(&self) -> FaceKind { FaceKind::Bluetooth }

    async fn recv(&self) -> Result<Bytes, FaceError> {
        Err(FaceError::Closed) // placeholder
    }

    async fn send(&self, _pkt: Bytes) -> Result<(), FaceError> {
        Err(FaceError::Closed) // placeholder
    }
}

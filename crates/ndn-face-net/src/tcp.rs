use bytes::Bytes;
use tokio::sync::Mutex;
use ndn_transport::{Face, FaceError, FaceId, FaceKind};

/// NDN face over TCP with TLV length-prefix framing.
///
/// Uses `tokio_util::codec` for framing. `send` serialises concurrent callers
/// via an internal `Mutex<WriteHalf>`.
pub struct TcpFace {
    id: FaceId,
    // TODO: add framed reader/writer fields
}

impl Face for TcpFace {
    fn id(&self) -> FaceId { self.id }
    fn kind(&self) -> FaceKind { FaceKind::Tcp }

    async fn recv(&self) -> Result<Bytes, FaceError> {
        Err(FaceError::Closed) // placeholder
    }

    async fn send(&self, _pkt: Bytes) -> Result<(), FaceError> {
        Err(FaceError::Closed) // placeholder
    }
}

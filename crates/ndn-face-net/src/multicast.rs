use bytes::Bytes;
use ndn_transport::{Face, FaceError, FaceId, FaceKind};

/// NDN face over IPv6 link-local multicast (ff02::1:6363).
///
/// Used for local NDN neighbor discovery without AP dependency.
/// Interests are flooded via multicast; Data is returned via unicast `UdpFace`.
pub struct MulticastUdpFace {
    id: FaceId,
    // TODO: bind to ff02::1:6363 on a specific interface
}

impl Face for MulticastUdpFace {
    fn id(&self) -> FaceId { self.id }
    fn kind(&self) -> FaceKind { FaceKind::Udp }

    async fn recv(&self) -> Result<Bytes, FaceError> {
        Err(FaceError::Closed) // placeholder
    }

    async fn send(&self, _pkt: Bytes) -> Result<(), FaceError> {
        Err(FaceError::Closed) // placeholder
    }
}

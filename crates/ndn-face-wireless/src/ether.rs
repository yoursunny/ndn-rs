use bytes::Bytes;
use ndn_packet::Name;
use ndn_transport::{Face, FaceError, FaceId, FaceKind};
use crate::radio::RadioFaceMetadata;

/// MAC address (6 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacAddr([u8; 6]);

impl MacAddr {
    pub const BROADCAST: MacAddr = MacAddr([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);

    pub fn new(bytes: [u8; 6]) -> Self { Self(bytes) }
    pub fn as_bytes(&self) -> &[u8; 6] { &self.0 }
}

/// NDN face over raw Ethernet (AF_PACKET / Ethertype 0x8624).
///
/// The MAC address is an internal implementation detail — above the face layer
/// everything is NDN names. The node name is stable across channel switches
/// and radio changes; only the internal MAC binding needs updating on mobility.
pub struct NamedEtherFace {
    id:          FaceId,
    /// NDN node name of the remote peer.
    pub node_name: Name,
    /// Resolved MAC address of the remote peer.
    peer_mac:    MacAddr,
    /// Local network interface name.
    iface:       String,
    /// Radio metadata for multi-radio strategies.
    pub radio:   RadioFaceMetadata,
}

impl NamedEtherFace {
    pub fn new(
        id:        FaceId,
        node_name: Name,
        peer_mac:  MacAddr,
        iface:     impl Into<String>,
        radio:     RadioFaceMetadata,
    ) -> Self {
        Self { id, node_name, peer_mac, iface: iface.into(), radio }
    }
}

impl Face for NamedEtherFace {
    fn id(&self) -> FaceId { self.id }
    fn kind(&self) -> FaceKind { FaceKind::Ethernet }

    async fn recv(&self) -> Result<Bytes, FaceError> {
        Err(FaceError::Closed) // placeholder — implement with AF_PACKET + PACKET_RX_RING
    }

    async fn send(&self, _pkt: Bytes) -> Result<(), FaceError> {
        // Placeholder — build Ethernet frame with NDN_ETHERTYPE and send via AF_PACKET.
        Err(FaceError::Closed)
    }
}

pub mod ether;
pub mod wfb;
pub mod bluetooth;
pub mod neighbor;
pub mod radio;

pub use ether::NamedEtherFace;
pub use wfb::WfbFace;
pub use bluetooth::BluetoothFace;
pub use neighbor::NeighborDiscovery;
pub use radio::{RadioFaceMetadata, RadioTable};

/// IANA-assigned Ethertype for NDN over Ethernet (IEEE 802.3).
pub const NDN_ETHERTYPE: u16 = 0x8624;

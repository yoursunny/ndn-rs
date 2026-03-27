// NamedEtherFace (AF_PACKET raw sockets), WfbFace (802.11 monitor-mode
// injection), and BluetoothFace (BlueZ RFCOMM) all require Linux kernel APIs
// that do not exist on macOS, Windows, Android, or embedded targets.
// They are compiled only when the host is Linux.
#[cfg(target_os = "linux")]
pub mod ether;

#[cfg(target_os = "linux")]
pub mod wfb;

#[cfg(target_os = "linux")]
pub mod bluetooth;

// NeighborDiscovery depends on MacAddr from the Linux-specific ether module,
// so it is also Linux-only.  RadioTable is a pure data structure with no
// platform dependencies and compiles everywhere.
#[cfg(target_os = "linux")]
pub mod neighbor;
pub mod radio;

#[cfg(target_os = "linux")]
pub use ether::NamedEtherFace;

#[cfg(target_os = "linux")]
pub use wfb::WfbFace;

#[cfg(target_os = "linux")]
pub use bluetooth::BluetoothFace;

#[cfg(target_os = "linux")]
pub use neighbor::NeighborDiscovery;
pub use radio::{RadioFaceMetadata, RadioTable};

/// IANA-assigned Ethertype for NDN over Ethernet (IEEE 802.3).
pub const NDN_ETHERTYPE: u16 = 0x8624;

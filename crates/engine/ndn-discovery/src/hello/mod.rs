//! SWIM/Hello neighbor discovery protocol family.
//!
//! This module contains the shared hello protocol state machine and its
//! link-medium implementations.
//!
//! ## Module layout
//!
//! | Sub-module  | Contents |
//! |-------------|----------|
//! | [`payload`] | `HelloPayload` TLV codec — the wire format for hello Data Content |
//! | [`medium`]  | `LinkMedium` trait — link-specific face creation, signing, address extraction |
//! | [`protocol`]| `HelloProtocol<T>` — generic SWIM/hello/probe state machine |
//! | [`udp`]     | `UdpNeighborDiscovery` — UDP multicast neighbor discovery (requires `udp-hello` feature) |
//! | [`probe`]   | SWIM direct/indirect probe packet builders and parsers |
//!
//! ## Adding a new link-medium
//!
//! To support a new link type (e.g. Ethernet, Bluetooth, LoRa):
//!
//! 1. Create a new file in this module (e.g. `ether.rs`).
//! 2. Implement [`medium::LinkMedium`] for your medium struct — this provides
//!    link-specific face creation, hello Data signing, and address extraction.
//! 3. Define a type alias: `pub type EtherNeighborDiscovery = HelloProtocol<YourMedium>;`
//! 4. Re-export it from this `mod.rs` and from the crate root (`lib.rs`).
//!
//! The generic [`protocol::HelloProtocol<T>`] handles all the shared logic:
//! hello scheduling, SWIM probes, neighbor lifecycle, gossip diffs.  Your
//! medium only implements the link-specific parts.

pub mod medium;
pub mod payload;
pub mod probe;
pub mod protocol;

#[cfg(feature = "udp-hello")]
pub mod udp;

#[cfg(all(feature = "ether-nd", target_os = "linux"))]
pub mod ether;

pub use medium::{HELLO_PREFIX_DEPTH, HELLO_PREFIX_STR, HelloCore, HelloState, LinkMedium};
pub use payload::{
    CAP_CONTENT_STORE, CAP_FRAGMENTATION, CAP_SVS, CAP_VALIDATION, DiffEntry, HelloPayload,
    NeighborDiff, T_ADD_ENTRY, T_CAPABILITIES, T_NEIGHBOR_DIFF, T_NODE_NAME, T_PUBLIC_KEY,
    T_REMOVE_ENTRY, T_SERVED_PREFIX, T_UNICAST_PORT,
};
pub use probe::{
    DirectProbe, IndirectProbe, build_direct_probe, build_indirect_probe,
    build_indirect_probe_encoded, build_probe_ack, is_probe_ack, parse_direct_probe,
    parse_indirect_probe,
};
pub use protocol::HelloProtocol;

#[cfg(feature = "udp-hello")]
pub use udp::UdpNeighborDiscovery;

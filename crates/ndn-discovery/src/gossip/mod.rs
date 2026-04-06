//! Gossip-based discovery supplements.
//!
//! Two independent `DiscoveryProtocol` implementations for network-wide
//! state dissemination:
//!
//! | Module | Protocol | Namespace | Mechanism |
//! |--------|----------|-----------|-----------|
//! | [`epidemic`] | [`EpidemicGossip`] | `/ndn/local/nd/gossip/` | Pull-gossip; Interest-driven neighbor state snapshots |
//! | [`svs_gossip`] | [`SvsServiceDiscovery`] | `/ndn/local/sd/updates/` | SVS sync group for push service-record notifications |
//!
//! Both are optional. Attach them to a [`CompositeDiscovery`] alongside
//! `UdpNeighborDiscovery` or `EtherNeighborDiscovery`:
//!
//! ```ignore
//! let mut composite = CompositeDiscovery::default();
//! composite.add(Box::new(udp_nd)).unwrap();
//! composite.add(Box::new(EpidemicGossip::new(config.clone()))).unwrap();
//! composite.add(Box::new(SvsServiceDiscovery::new(node_name, config))).unwrap();
//! ```
//!
//! [`CompositeDiscovery`]: crate::CompositeDiscovery

pub mod epidemic;
pub mod svs_gossip;

pub use epidemic::EpidemicGossip;
pub use svs_gossip::SvsServiceDiscovery;

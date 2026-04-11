//! NDN routing protocol implementations.
//!
//! This crate provides pluggable routing algorithms for the NDN forwarder:
//!
//! - [`StaticProtocol`]: installs a fixed set of routes at startup, useful for
//!   simple single-hop topologies and testing.
//! - [`DvrProtocol`]: Distance Vector Routing — distributed Bellman-Ford over
//!   NDN link-local multicast. Works alongside neighbor discovery; implements
//!   both [`RoutingProtocol`] (RIB lifecycle) and [`DiscoveryProtocol`] (packet
//!   I/O via the discovery context).
//!
//! # Usage
//!
//! Register protocols with the engine builder:
//!
//! ```rust,ignore
//! use ndn_routing::{StaticProtocol, StaticRoute};
//! use ndn_engine::EngineBuilder;
//!
//! let engine = EngineBuilder::new()
//!     .routing_protocol(StaticProtocol::new(vec![
//!         StaticRoute { prefix: "/ndn/edu/ucla".parse().unwrap(), face_id: FaceId(1), cost: 10 },
//!     ]))
//!     .build().await?;
//! ```

pub mod protocols;

pub use protocols::dvr::{DvrConfig, DvrProtocol};
pub use protocols::r#static::{StaticProtocol, StaticRoute};

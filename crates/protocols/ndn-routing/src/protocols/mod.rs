//! Routing protocol implementations.
//!
//! Each sub-module contains one routing algorithm. All implement
//! [`RoutingProtocol`](ndn_engine::RoutingProtocol); some also implement
//! [`DiscoveryProtocol`](ndn_discovery::DiscoveryProtocol) for protocols that
//! need direct packet I/O (e.g. DVR broadcasts).
//!
//! ## Adding a new routing protocol
//!
//! 1. Create `protocols/your_protocol.rs`.
//! 2. Implement `RoutingProtocol` (and optionally `DiscoveryProtocol`).
//! 3. Add `pub mod your_protocol;` and a `pub use` line here.
//! 4. Re-export from `crate::lib` so downstream crates can access it.
//!
//! See the [routing protocols developer guide](https://ndn-rs.github.io/wiki/guides/implementing-routing-protocol)
//! for details on the dual-protocol pattern and RIB interaction.

pub mod dvr;
pub mod r#static;

pub use dvr::DvrProtocol;
pub use r#static::{StaticProtocol, StaticRoute};

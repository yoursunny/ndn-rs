//! Link-layer (MAC) address — re-exported from `ndn-transport` where it is defined.
//!
//! `MacAddr` lives in `ndn-transport` so that `ndn-faces` can use it without
//! creating a circular dependency (ndn-faces → ndn-discovery → ndn-faces).

pub use ndn_transport::MacAddr;

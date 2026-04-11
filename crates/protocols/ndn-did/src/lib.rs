//! Thin re-export shim — the `ndn-did` crate has been merged into `ndn-security`.
//!
//! All types are now available under `ndn_security::did` or, for convenience,
//! as top-level re-exports from `ndn_security`.
//!
//! This crate is kept for backwards compatibility. New code should use
//! `ndn_security::did` directly.
#![allow(unused_imports)]
pub use ndn_security::did::*;

//! # ndn-research -- Research and measurement extensions
//!
//! Optional instrumentation for multi-radio NDN testbeds. Tracks per-flow
//! statistics and exposes a pipeline observer stage for real-time measurement.
//! On Linux, integrates with nl80211 for Wi-Fi channel management.
//!
//! ## Key types
//!
//! - [`FlowTable`] -- per-name-prefix flow tracking and statistics
//! - [`FlowObserverStage`] -- pipeline stage that records flow observations
//! - [`ChannelManager`] -- Linux-only nl80211 Wi-Fi channel control
//!
//! ## Platform notes
//!
//! `ChannelManager` is only available on `target_os = "linux"`.

#![allow(missing_docs)]

pub mod flow_table;
pub mod observer;

pub use flow_table::FlowTable;
pub use observer::FlowObserverStage;

// ChannelManager uses nl80211 Netlink sockets which are Linux-specific.
// The module is compiled only on Linux so that the rest of the research
// crate remains portable.
#[cfg(target_os = "linux")]
pub mod channel_manager;

#[cfg(target_os = "linux")]
pub use channel_manager::ChannelManager;

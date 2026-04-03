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

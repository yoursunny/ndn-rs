//! Protocol constants and name-construction helpers for NDN-FT v0.1.

use ndn_packet::Name;

/// The protocol component inserted under a node's namespace.
///
/// Full prefix: `/<node>/ndn-ft/v0/`
pub const PROTOCOL_PREFIX: &str = "ndn-ft";
/// Protocol version component.
pub const PROTOCOL_VERSION: &str = "v0";

/// Default segment size in bytes.
pub const DEFAULT_SEGMENT_SIZE: usize = 8192;

/// Build the protocol base prefix for a node.
///
/// Returns `/<node>/ndn-ft/v0`.
pub fn base_prefix(node_prefix: &Name) -> Name {
    node_prefix
        .clone()
        .append(PROTOCOL_PREFIX)
        .append(PROTOCOL_VERSION)
}

/// Name of the catalog endpoint: `/<node>/ndn-ft/v0/catalog`.
pub fn catalog_name(node_prefix: &Name) -> Name {
    base_prefix(node_prefix).append("catalog")
}

/// Name of the notification endpoint: `/<node>/ndn-ft/v0/notify`.
pub fn notify_name(node_prefix: &Name) -> Name {
    base_prefix(node_prefix).append("notify")
}

/// Prefix for a specific file: `/<node>/ndn-ft/v0/file/<file-id>`.
pub fn file_prefix(node_prefix: &Name, file_id: &str) -> Name {
    base_prefix(node_prefix)
        .append("file")
        .append(file_id)
}

/// Name of a file's metadata packet: `/<node>/ndn-ft/v0/file/<file-id>/meta`.
pub fn file_meta_name(node_prefix: &Name, file_id: &str) -> Name {
    file_prefix(node_prefix, file_id).append("meta")
}

/// Name of a file segment: `/<node>/ndn-ft/v0/file/<file-id>/<seg>`.
pub fn file_segment_name(node_prefix: &Name, file_id: &str, seg: usize) -> Name {
    file_prefix(node_prefix, file_id).append(seg.to_string())
}

//! `CompositeDiscovery` — runs multiple protocols simultaneously.
//!
//! Validates at construction time that no two protocols claim overlapping name
//! prefixes, then routes inbound packets to the correct protocol by prefix
//! match.  Face lifecycle hooks are delivered to all protocols in registration
//! order.

use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use ndn_packet::Name;
use ndn_transport::FaceId;

use crate::{DiscoveryContext, DiscoveryProtocol, ProtocolId};

/// Wrapper that runs multiple [`DiscoveryProtocol`] implementations in parallel.
///
/// # Namespace safety
///
/// [`CompositeDiscovery::new`] returns an error if any two protocols claim
/// overlapping name prefixes (one is a prefix of the other).  Each protocol
/// must use a distinct sub-tree of `/ndn/local/`.
///
/// # Inbound routing
///
/// When a raw packet arrives, `CompositeDiscovery` tries to parse its top-level
/// NDN name and routes it to the first protocol whose `claimed_prefixes` contains
/// a matching prefix.  If the name cannot be parsed or no protocol matches, the
/// packet is not consumed (returns `false`).
///
/// # Tick delivery
///
/// All protocols receive every `on_tick` call.  Order is not guaranteed.
pub struct CompositeDiscovery {
    protocols: Vec<Arc<dyn DiscoveryProtocol>>,
}

impl CompositeDiscovery {
    /// Construct a composite from a list of protocols.
    ///
    /// Returns `Err` with a human-readable message if any two protocols claim
    /// overlapping prefixes.
    pub fn new(protocols: Vec<Arc<dyn DiscoveryProtocol>>) -> Result<Self, String> {
        // Collect all (prefix, protocol_id) pairs and check for overlaps.
        let mut all_prefixes: Vec<(Name, ProtocolId)> = Vec::new();
        for proto in &protocols {
            for prefix in proto.claimed_prefixes() {
                // Check against all previously registered prefixes.
                for (existing, existing_id) in &all_prefixes {
                    if prefixes_overlap(existing, prefix) {
                        return Err(format!(
                            "protocol '{}' prefix '{}' overlaps with protocol '{}' prefix '{}'",
                            proto.protocol_id(),
                            prefix,
                            existing_id,
                            existing,
                        ));
                    }
                }
                all_prefixes.push((prefix.clone(), proto.protocol_id()));
            }
        }
        Ok(Self { protocols })
    }

    /// Number of contained protocols.
    pub fn len(&self) -> usize {
        self.protocols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.protocols.is_empty()
    }
}

impl DiscoveryProtocol for CompositeDiscovery {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId("composite")
    }

    fn claimed_prefixes(&self) -> &[Name] {
        // CompositeDiscovery doesn't claim additional prefixes beyond
        // what its children claim — return empty here since children
        // are already registered and checked.
        &[]
    }

    fn on_face_up(&self, face_id: FaceId, ctx: &dyn DiscoveryContext) {
        for proto in &self.protocols {
            proto.on_face_up(face_id, ctx);
        }
    }

    fn on_face_down(&self, face_id: FaceId, ctx: &dyn DiscoveryContext) {
        for proto in &self.protocols {
            proto.on_face_down(face_id, ctx);
        }
    }

    fn on_inbound(&self, raw: &Bytes, incoming_face: FaceId, ctx: &dyn DiscoveryContext) -> bool {
        // Try to parse the packet name for prefix-based routing.
        if let Some(name) = parse_first_name(raw) {
            for proto in &self.protocols {
                for prefix in proto.claimed_prefixes() {
                    if name.has_prefix(prefix) {
                        return proto.on_inbound(raw, incoming_face, ctx);
                    }
                }
            }
            // Name parsed but no protocol claimed it — not consumed.
            return false;
        }

        // Name parse failed — try all protocols in order (fallback).
        for proto in &self.protocols {
            if proto.on_inbound(raw, incoming_face, ctx) {
                return true;
            }
        }
        false
    }

    fn on_tick(&self, now: Instant, ctx: &dyn DiscoveryContext) {
        for proto in &self.protocols {
            proto.on_tick(now, ctx);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns true if `a` is a prefix of `b` or vice-versa (i.e. they overlap
/// in the name tree).
fn prefixes_overlap(a: &Name, b: &Name) -> bool {
    b.has_prefix(a) || a.has_prefix(b)
}

/// Try to parse the first NDN name out of a raw TLV packet.
///
/// NDN packet TLV: Interest (0x05) or Data (0x06), then a Name TLV (0x07)
/// immediately as the first child.  This does a minimal parse — just enough
/// to route the packet; full decode happens in the pipeline.
fn parse_first_name(raw: &Bytes) -> Option<Name> {
    // Require at least a 2-byte TLV header.
    if raw.len() < 4 {
        return None;
    }
    let pkt_type = raw[0];
    if pkt_type != 0x05 && pkt_type != 0x06 {
        return None; // Not an Interest or Data
    }
    // Skip packet type + length (variable-length varint).
    let (_, inner) = skip_tlv_header(raw)?;
    // First child should be a Name TLV (type 0x07).
    if inner.is_empty() || inner[0] != 0x07 {
        return None;
    }
    // inner begins with the Name TLV; skip its type+length to get just the
    // component bytes that Name::decode expects.
    let (_, name_value) = skip_tlv_header(inner)?;
    let name_bytes = bytes::Bytes::copy_from_slice(name_value);
    Name::decode(name_bytes).ok()
}

/// Skip a TLV type+length prefix, returning a slice of the value bytes.
/// Returns `(type, value_bytes)` or `None` on truncation.
fn skip_tlv_header(buf: &[u8]) -> Option<(u8, &[u8])> {
    if buf.is_empty() {
        return None;
    }
    let t = buf[0];
    let (len, hdr_size) = read_varu(buf.get(1..)?)?;
    let end = 1 + hdr_size + len;
    Some((t, buf.get(1 + hdr_size..end)?))
}

/// Read a minimal NDN TLV varint.  Returns `(value, bytes_consumed)`.
fn read_varu(buf: &[u8]) -> Option<(usize, usize)> {
    match buf.first()? {
        b if *b < 253 => Some((*b as usize, 1)),
        253 => {
            let hi = *buf.get(1)? as usize;
            let lo = *buf.get(2)? as usize;
            Some(((hi << 8) | lo, 3))
        }
        254 => {
            let b1 = *buf.get(1)? as usize;
            let b2 = *buf.get(2)? as usize;
            let b3 = *buf.get(3)? as usize;
            let b4 = *buf.get(4)? as usize;
            Some(((b1 << 24) | (b2 << 16) | (b3 << 8) | b4, 5))
        }
        _ => None, // 8-byte form not needed for discovery packets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use crate::{NoDiscovery, ProtocolId};

    fn make_proto_with_prefix(id: &'static str, prefix: &str) -> Arc<dyn DiscoveryProtocol> {
        struct MockProto {
            id: ProtocolId,
            prefixes: Vec<Name>,
        }
        impl DiscoveryProtocol for MockProto {
            fn protocol_id(&self) -> ProtocolId { self.id }
            fn claimed_prefixes(&self) -> &[Name] { &self.prefixes }
            fn on_face_up(&self, _: FaceId, _: &dyn DiscoveryContext) {}
            fn on_face_down(&self, _: FaceId, _: &dyn DiscoveryContext) {}
            fn on_inbound(&self, _: &Bytes, _: FaceId, _: &dyn DiscoveryContext) -> bool { false }
            fn on_tick(&self, _: Instant, _: &dyn DiscoveryContext) {}
        }
        Arc::new(MockProto {
            id: ProtocolId(id),
            prefixes: vec![Name::from_str(prefix).unwrap()],
        })
    }

    #[test]
    fn no_overlap_is_ok() {
        let p1 = make_proto_with_prefix("nd", "/ndn/local/nd");
        let p2 = make_proto_with_prefix("sd", "/ndn/local/sd");
        assert!(CompositeDiscovery::new(vec![p1, p2]).is_ok());
    }

    #[test]
    fn overlap_is_rejected() {
        let p1 = make_proto_with_prefix("nd", "/ndn/local/nd");
        // /ndn/local/nd/hello is a sub-prefix of /ndn/local/nd → overlap
        let p2 = make_proto_with_prefix("nd2", "/ndn/local/nd/hello");
        assert!(CompositeDiscovery::new(vec![p1, p2]).is_err());
    }

    #[test]
    fn empty_composite_works() {
        let c = CompositeDiscovery::new(vec![]).unwrap();
        assert!(c.is_empty());
    }

    #[test]
    fn no_discovery_doesnt_conflict() {
        let nd = Arc::new(NoDiscovery) as Arc<dyn DiscoveryProtocol>;
        let nd2 = Arc::new(NoDiscovery) as Arc<dyn DiscoveryProtocol>;
        // Both claim no prefixes → no conflict.
        assert!(CompositeDiscovery::new(vec![nd, nd2]).is_ok());
    }
}

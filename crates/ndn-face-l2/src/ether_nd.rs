//! `EtherNeighborDiscovery` — NDN neighbor discovery over raw Ethernet.
//!
//! Implements [`DiscoveryProtocol`] using periodic hello Interest broadcasts on
//! a [`MulticastEtherFace`] and unicast [`NamedEtherFace`] creation per peer.
//!
//! # Protocol (doc format)
//!
//! **Hello Interest** (broadcast on multicast face):
//! ```text
//! Name: /ndn/local/nd/hello/<nonce-u32>
//! (no AppParams)
//! ```
//!
//! **Hello Data** (reply sent back on multicast face):
//! ```text
//! Name:    /ndn/local/nd/hello/<nonce-u32>
//! Content: HelloPayload TLV
//!   NODE-NAME     = /ndn/site/mynode
//!   SERVED-PREFIX = ...        (optional, InHello mode)
//!   CAPABILITIES  = [flags]    (optional)
//!   NEIGHBOR-DIFF = [...]      (SWIM gossip piggyback, optional)
//! ```
//!
//! The sender's MAC is extracted from `meta.source` (populated by the engine
//! via `MulticastEtherFace::recv_with_source`), not from the packet payload.
//!
//! On receiving a Hello Interest a node:
//! 1. Reads the sender MAC from `meta.source` (`LinkAddr::Ether`).
//! 2. Triggers `PassiveDetection` on the strategy when the MAC is new.
//! 3. Replies with a Hello Data carrying its own `HelloPayload`.
//!
//! On receiving a Hello Data the sender:
//! 1. Decodes `HelloPayload` from Content.
//! 2. Reads responder MAC from `meta.source`.
//! 3. Creates a [`NamedEtherFace`] to the responder if needed.
//! 4. Updates the neighbor to `Established` and records RTT.
//! 5. Installs FIB routes for `served_prefixes` (if `InHello` mode).
//! 6. Applies any piggybacked `NEIGHBOR-DIFF` entries.

use std::collections::{HashMap, VecDeque};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bytes::Bytes;
use ndn_discovery::{
    DiffEntry, DiscoveryConfig, DiscoveryContext, DiscoveryProfile, DiscoveryProtocol,
    HelloPayload, InboundMeta, LinkAddr, NeighborDiff, NeighborEntry, NeighborState,
    NeighborUpdate, PrefixAnnouncementMode, ProtocolId,
};
use ndn_discovery::probe::{
    build_direct_probe, build_indirect_probe, build_probe_ack,
    is_probe_ack, parse_direct_probe, parse_indirect_probe,
};
use ndn_discovery::scope::{probe_direct, probe_via};
use ndn_discovery::strategy::{
    NeighborProbeStrategy, ProbeRequest, TriggerEvent, build_strategy,
};
use ndn_discovery::wire::{parse_raw_data, parse_raw_interest, write_name_tlv, write_nni};
use ndn_packet::{Name, tlv_type};
use ndn_tlv::TlvWriter;
use ndn_transport::FaceId;
use tracing::{debug, warn};

use crate::af_packet::MacAddr;
use crate::radio::RadioFaceMetadata;
use crate::ether::NamedEtherFace;

/// Hello prefix used by EtherND.
const HELLO_PREFIX_STR: &str = "/ndn/local/nd/hello";
/// Number of name components in the hello prefix.
const HELLO_PREFIX_DEPTH: usize = 4; // /ndn/local/nd/hello
/// Protocol identifier.
const PROTOCOL: ProtocolId = ProtocolId("ether-nd");
/// Max NEIGHBOR-DIFF entries piggybacked per hello.
const MAX_DIFF_ENTRIES: usize = 16;

/// Mutable state protected by a `Mutex` for interior mutability.
struct EtherNdState {
    /// Outstanding hellos: nonce -> send_time.
    pending_probes: HashMap<u32, Instant>,
    /// Recent neighbor additions/removals for SWIM gossip piggyback.
    recent_diffs: VecDeque<DiffEntry>,
    /// SWIM direct probes: nonce -> (sent_at, target_name).
    swim_probes: HashMap<u32, (Instant, Name)>,
    /// Relay state for via-probes: relay_nonce -> (origin_face, original_interest_name).
    relay_probes: HashMap<u32, (FaceId, Name)>,
}

/// NDN neighbor discovery protocol over raw Ethernet.
///
/// Attach one instance per interface + multicast face.  Multiple instances
/// (one per interface) can coexist inside a [`CompositeDiscovery`].
///
/// [`CompositeDiscovery`]: ndn_discovery::CompositeDiscovery
pub struct EtherNeighborDiscovery {
    /// Multicast face used for hello broadcasts.
    multicast_face_id: FaceId,
    /// Network interface name (e.g. "wlan0").
    iface: String,
    /// This node's NDN name.
    node_name: Name,
    /// Our Ethernet MAC address (needed when creating unicast faces).
    local_mac: MacAddr,
    /// Parsed `/ndn/local/nd/hello` prefix.
    hello_prefix: Name,
    /// Claimed prefixes (single element: `hello_prefix`).
    claimed: Vec<Name>,
    /// Monotonically increasing nonce counter.
    nonce_counter: AtomicU32,
    /// Discovery parameters (liveness timeout, miss count, probe timeout).
    config: DiscoveryConfig,
    /// Probe-scheduling strategy (behind a mutex for `&self` trait compat).
    strategy: Mutex<Box<dyn NeighborProbeStrategy>>,
    /// Prefixes this node produces, announced in Hello Data when `InHello` mode.
    served_prefixes: Mutex<Vec<Name>>,
    /// Protected mutable state.
    state: Mutex<EtherNdState>,
}

impl EtherNeighborDiscovery {
    /// Create a new instance with the default LAN profile.
    ///
    /// - `multicast_face_id`: `FaceId` of the [`MulticastEtherFace`] already
    ///   registered with the engine.
    /// - `iface`: network interface name (e.g. `"wlan0"`).
    /// - `node_name`: this node's NDN name.
    /// - `local_mac`: this node's Ethernet MAC address on `iface`.
    pub fn new(
        multicast_face_id: FaceId,
        iface: impl Into<String>,
        node_name: Name,
        local_mac: MacAddr,
    ) -> Self {
        Self::new_with_config(
            multicast_face_id,
            iface,
            node_name,
            local_mac,
            DiscoveryConfig::for_profile(&DiscoveryProfile::Lan),
        )
    }

    /// Create with an explicit [`DiscoveryConfig`].
    pub fn new_with_config(
        multicast_face_id: FaceId,
        iface: impl Into<String>,
        node_name: Name,
        local_mac: MacAddr,
        config: DiscoveryConfig,
    ) -> Self {
        let hello_prefix = Name::from_str(HELLO_PREFIX_STR).expect("static prefix is valid");
        let mut claimed = vec![hello_prefix.clone()];
        if config.swim_indirect_fanout > 0 {
            claimed.push(probe_direct().clone());
            claimed.push(probe_via().clone());
        }
        let strategy = build_strategy(&config);
        Self {
            multicast_face_id,
            iface: iface.into(),
            node_name,
            local_mac,
            hello_prefix,
            claimed,
            nonce_counter: AtomicU32::new(1),
            strategy: Mutex::new(strategy),
            served_prefixes: Mutex::new(Vec::new()),
            config,
            state: Mutex::new(EtherNdState {
                pending_probes: HashMap::new(),
                recent_diffs: VecDeque::new(),
                swim_probes: HashMap::new(),
                relay_probes: HashMap::new(),
            }),
        }
    }

    /// Create with a named deployment profile.
    pub fn from_profile(
        multicast_face_id: FaceId,
        iface: impl Into<String>,
        node_name: Name,
        local_mac: MacAddr,
        profile: &DiscoveryProfile,
    ) -> Self {
        Self::new_with_config(
            multicast_face_id,
            iface,
            node_name,
            local_mac,
            DiscoveryConfig::for_profile(profile),
        )
    }

    /// Set the prefixes this node serves.
    ///
    /// When `config.prefix_announcement == InHello`, these are advertised in
    /// every Hello Data so peers can auto-populate their FIBs.
    pub fn set_served_prefixes(&self, prefixes: Vec<Name>) {
        *self.served_prefixes.lock().unwrap() = prefixes;
    }

    // ── Packet builders ───────────────────────────────────────────────────────

    /// Build a Hello Interest TLV.
    ///
    /// Name: `/ndn/local/nd/hello/<nonce-u32>` — no AppParams.
    fn build_hello_interest(&self, nonce: u32) -> Bytes {
        let mut w = TlvWriter::new();
        w.write_nested(tlv_type::INTEREST, |w: &mut TlvWriter| {
            w.write_nested(tlv_type::NAME, |w: &mut TlvWriter| {
                for comp in self.hello_prefix.components() {
                    w.write_tlv(comp.typ, &comp.value);
                }
                w.write_tlv(tlv_type::NAME_COMPONENT, &nonce.to_be_bytes());
            });
            w.write_tlv(tlv_type::NONCE, &nonce.to_be_bytes());
            // Lifetime = hello_interval_base * 2 (doc §Hello Packet Format)
            let lifetime_ms = self.config.hello_interval_base.as_millis().min(u32::MAX as u128) as u64 * 2;
            write_nni(w, tlv_type::INTEREST_LIFETIME, lifetime_ms);
        });
        w.finish()
    }

    /// Build a Hello Data reply TLV.
    ///
    /// Name: same as the received Interest.
    /// Content: `HelloPayload` including served prefixes (InHello mode) and
    /// piggybacked SWIM diffs.
    fn build_hello_data(&self, interest_name: &Name) -> Bytes {
        let mut payload = HelloPayload::new(self.node_name.clone());

        // InHello: advertise locally served prefixes.
        if self.config.prefix_announcement == PrefixAnnouncementMode::InHello {
            let sp = self.served_prefixes.lock().unwrap();
            payload.served_prefixes = sp.clone();
        }

        // Piggyback recent SWIM gossip diffs.
        {
            let st = self.state.lock().unwrap();
            if !st.recent_diffs.is_empty() {
                payload.neighbor_diffs.push(NeighborDiff {
                    entries: st.recent_diffs.iter().cloned().collect(),
                });
            }
        }

        let content = payload.encode();
        // FreshnessPeriod = hello_interval_base * 2 (doc §Hello Packet Format)
        let freshness_ms = self.config.hello_interval_base.as_millis().min(u32::MAX as u128) as u64 * 2;

        let mut w = TlvWriter::new();
        w.write_nested(tlv_type::DATA, |w: &mut TlvWriter| {
            write_name_tlv(w, interest_name);
            w.write_nested(tlv_type::META_INFO, |w: &mut TlvWriter| {
                write_nni(w, tlv_type::FRESHNESS_PERIOD, freshness_ms);
            });
            w.write_tlv(tlv_type::CONTENT, &content);
            w.write_nested(tlv_type::SIGNATURE_INFO, |w: &mut TlvWriter| {
                w.write_tlv(tlv_type::SIGNATURE_TYPE, &[0u8]);
            });
            w.write_tlv(tlv_type::SIGNATURE_VALUE, &[0u8; 32]);
        });
        w.finish()
    }

    // ── Inbound handlers ──────────────────────────────────────────────────────

    fn handle_hello_interest(
        &self,
        raw: &Bytes,
        _incoming_face: FaceId,
        meta: &InboundMeta,
        ctx: &dyn DiscoveryContext,
    ) -> bool {
        let parsed = match parse_raw_interest(raw) {
            Some(p) => p,
            None => return false,
        };

        let name = &parsed.name;
        if !name.has_prefix(&self.hello_prefix) {
            return false;
        }

        // Validate: prefix (4) + nonce (1) = 5 components minimum.
        if name.components().len() != HELLO_PREFIX_DEPTH + 1 {
            return false;
        }

        // Extract sender MAC from link-layer metadata.
        let sender_mac = match &meta.source {
            Some(LinkAddr::Ether(mac)) => *mac,
            _ => {
                debug!("EtherND: hello Interest has no source MAC in meta — ignoring");
                return true;
            }
        };

        // Trigger PassiveDetection when a previously-unknown MAC sends a hello.
        let is_new = ctx.neighbors().face_for_peer(&sender_mac, &self.iface).is_none();
        if is_new {
            let mut strategy = self.strategy.lock().unwrap();
            strategy.trigger(TriggerEvent::PassiveDetection);
        }

        let reply = self.build_hello_data(name);
        ctx.send_on(self.multicast_face_id, reply);

        debug!("EtherND: received hello Interest from {:?}, sent Data reply", sender_mac);
        true
    }

    fn handle_hello_data(
        &self,
        raw: &Bytes,
        _incoming_face: FaceId,
        meta: &InboundMeta,
        ctx: &dyn DiscoveryContext,
    ) -> bool {
        let parsed = match parse_raw_data(raw) {
            Some(d) => d,
            None => return false,
        };

        let name = &parsed.name;
        if !name.has_prefix(&self.hello_prefix) {
            return false;
        }

        if name.components().len() != HELLO_PREFIX_DEPTH + 1 {
            return false;
        }

        // Extract nonce and look up send time for RTT measurement.
        let nonce_comp = &name.components()[HELLO_PREFIX_DEPTH];
        if nonce_comp.value.len() != 4 {
            return false;
        }
        let nonce = u32::from_be_bytes(nonce_comp.value[..4].try_into().unwrap());
        let send_time = {
            let mut st = self.state.lock().unwrap();
            st.pending_probes.remove(&nonce)
        };

        // Decode HelloPayload from Content.
        let content = match parsed.content {
            Some(c) => c,
            None => {
                debug!("EtherND: hello Data has no content");
                return true;
            }
        };
        let payload = match HelloPayload::decode(&content) {
            Some(p) => p,
            None => {
                debug!("EtherND: could not decode HelloPayload");
                return true;
            }
        };
        let responder_name = payload.node_name.clone();

        // Extract responder MAC from link-layer metadata.
        let responder_mac = match &meta.source {
            Some(LinkAddr::Ether(mac)) => *mac,
            _ => {
                debug!("EtherND: hello Data has no source MAC in meta — ignoring");
                return true;
            }
        };

        let peer_face_id = self.ensure_peer(ctx, &responder_name, responder_mac);

        ctx.update_neighbor(NeighborUpdate::SetState {
            name: responder_name.clone(),
            state: NeighborState::Established { last_seen: Instant::now() },
        });

        if let Some(sent) = send_time {
            let rtt = sent.elapsed();
            let rtt_us = rtt.as_micros().min(u32::MAX as u128) as u32;
            ctx.update_neighbor(NeighborUpdate::UpdateRtt {
                name: responder_name.clone(),
                rtt_us,
            });
            let mut strategy = self.strategy.lock().unwrap();
            strategy.on_probe_success(rtt);
        }

        // InHello: auto-populate FIB with served prefixes.
        if self.config.prefix_announcement == PrefixAnnouncementMode::InHello {
            if let Some(face_id) = peer_face_id {
                for prefix in &payload.served_prefixes {
                    ctx.add_fib_entry(prefix, face_id, 10, PROTOCOL);
                    debug!("EtherND: auto-FIB {:?} -> {:?} via {face_id:?}", prefix, responder_name);
                }
            }
        }

        // Apply SWIM gossip diffs from the peer.
        self.apply_neighbor_diffs(&payload, ctx);

        // Record this new/refreshed neighbor for our own outbound diffs.
        {
            let mut st = self.state.lock().unwrap();
            st.recent_diffs.push_back(DiffEntry::Add(responder_name));
            while st.recent_diffs.len() > MAX_DIFF_ENTRIES {
                st.recent_diffs.pop_front();
            }
        }

        true
    }

    fn handle_direct_probe_interest(&self, raw: &Bytes, incoming_face: FaceId, ctx: &dyn DiscoveryContext) -> bool {
        let probe = match parse_direct_probe(raw) { Some(p) => p, None => return false };
        if probe.target == self.node_name {
            if let Some(parsed) = parse_raw_interest(raw) {
                let ack = build_probe_ack(&parsed.name);
                ctx.send_on(incoming_face, ack);
                debug!("EtherND: probe ACK sent (direct, nonce={:#010x})", probe.nonce);
            }
        }
        true
    }

    fn handle_via_probe_interest(&self, raw: &Bytes, incoming_face: FaceId, ctx: &dyn DiscoveryContext) -> bool {
        let probe = match parse_indirect_probe(raw) { Some(p) => p, None => return false };
        if probe.intermediary != self.node_name { return false; }
        if let Some(entry) = ctx.neighbors().get(&probe.target) {
            if let Some((face_id, _, _)) = entry.faces.first() {
                let relay_nonce = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
                let direct_pkt = build_direct_probe(&probe.target, relay_nonce);
                ctx.send_on(*face_id, direct_pkt);
                if let Some(parsed) = parse_raw_interest(raw) {
                    let mut st = self.state.lock().unwrap();
                    st.relay_probes.insert(relay_nonce, (incoming_face, parsed.name.clone()));
                }
                debug!("EtherND: relaying via-probe to {:?}", probe.target);
                return true;
            }
        }
        debug!("EtherND: via-probe target {:?} unknown, dropping", probe.target);
        true
    }

    fn handle_probe_ack(&self, raw: &Bytes, _incoming_face: FaceId, ctx: &dyn DiscoveryContext) -> Option<bool> {
        let parsed = parse_raw_data(raw)?;
        let name = &parsed.name;
        let comps = name.components();
        let last = comps.last()?;
        if last.value.len() != 4 { return Some(false); }
        let nonce = u32::from_be_bytes(last.value[..4].try_into().ok()?);

        let relay = { let mut st = self.state.lock().unwrap(); st.relay_probes.remove(&nonce) };
        if let Some((origin_face, original_name)) = relay {
            let ack = build_probe_ack(&original_name);
            ctx.send_on(origin_face, ack);
            debug!("EtherND: relayed probe ACK for nonce={nonce:#010x}");
        }

        let swim = { let mut st = self.state.lock().unwrap(); st.swim_probes.remove(&nonce) };
        if let Some((sent, _target)) = swim {
            let rtt = sent.elapsed();
            self.strategy.lock().unwrap().on_probe_success(rtt);
            debug!("EtherND: SWIM direct probe ACK nonce={nonce:#010x} rtt={rtt:?}");
        }
        Some(true)
    }

    /// Apply piggybacked NEIGHBOR-DIFF entries from a received Hello Data.
    ///
    /// - `Add(name)` from a peer means that peer has a neighbor we might not
    ///   know about; schedule a broadcast to discover them.
    /// - `Remove(name)` means the peer lost contact with that node; if we still
    ///   have them as Established, move them to Stale.
    fn apply_neighbor_diffs(&self, payload: &HelloPayload, ctx: &dyn DiscoveryContext) {
        let mut should_broadcast = false;

        for diff in &payload.neighbor_diffs {
            for entry in &diff.entries {
                match entry {
                    DiffEntry::Add(name) => {
                        if ctx.neighbors().get(name).is_none() {
                            // Spec: newly-heard-of neighbor starts in Probing state
                            // (not yet verified via direct hello exchange).
                            ctx.update_neighbor(NeighborUpdate::Upsert(NeighborEntry {
                                node_name: name.clone(),
                                state: NeighborState::Probing { attempts: 0, last_probe: Instant::now() },
                                faces: Vec::new(),
                                rtt_us: None,
                                pending_nonce: None,
                            }));
                            should_broadcast = true;
                            debug!("EtherND: SWIM diff — new peer {name:?} in Probing");
                        }
                    }
                    DiffEntry::Remove(name) => {
                        // Peer lost contact with this node; move to Stale if we know it.
                        if ctx.neighbors().get(name).is_some() {
                            ctx.update_neighbor(NeighborUpdate::SetState {
                                name: name.clone(),
                                state: NeighborState::Stale {
                                    miss_count: 1,
                                    last_seen: Instant::now(),
                                },
                            });
                            debug!("EtherND: SWIM diff — peer reports {name:?} gone, -> Stale");
                        }
                    }
                }
            }
        }

        if should_broadcast {
            let mut strategy = self.strategy.lock().unwrap();
            strategy.trigger(TriggerEvent::ForwardingFailure);
        }
    }

    // ── Peer management ───────────────────────────────────────────────────────

    /// Ensure a unicast face and neighbor entry exist for `peer_name`/`peer_mac`.
    ///
    /// Returns the `FaceId` of the unicast face (existing or newly created).
    fn ensure_peer(
        &self,
        ctx: &dyn DiscoveryContext,
        peer_name: &Name,
        peer_mac: MacAddr,
    ) -> Option<FaceId> {
        let existing = ctx.neighbors().face_for_peer(&peer_mac, &self.iface);

        let face_id = if let Some(fid) = existing {
            fid
        } else {
            let fid = ctx.alloc_face_id();
            match NamedEtherFace::new(
                fid,
                peer_name.clone(),
                peer_mac,
                self.iface.clone(),
                RadioFaceMetadata::default(),
            ) {
                Ok(face) => {
                    let registered = ctx.add_face(std::sync::Arc::new(face));
                    debug!("EtherND: created unicast face {registered:?} -> {peer_name}");
                    registered
                }
                Err(e) => {
                    warn!("EtherND: failed to create unicast face to {peer_name}: {e}");
                    return None;
                }
            }
        };

        if ctx.neighbors().get(peer_name).is_none() {
            ctx.update_neighbor(NeighborUpdate::Upsert(NeighborEntry::new(peer_name.clone())));
        }

        ctx.update_neighbor(NeighborUpdate::AddFace {
            name: peer_name.clone(),
            face_id,
            mac: peer_mac,
            iface: self.iface.clone(),
        });

        ctx.add_fib_entry(peer_name, face_id, 0, PROTOCOL);
        Some(face_id)
    }
}

// ── DiscoveryProtocol impl ────────────────────────────────────────────────────

impl DiscoveryProtocol for EtherNeighborDiscovery {
    fn protocol_id(&self) -> ProtocolId {
        PROTOCOL
    }

    fn claimed_prefixes(&self) -> &[Name] {
        &self.claimed
    }

    fn tick_interval(&self) -> Duration {
        self.config.tick_interval
    }

    fn on_face_up(&self, face_id: FaceId, ctx: &dyn DiscoveryContext) {
        if face_id == self.multicast_face_id {
            // Send an immediate hello to bootstrap the neighbor table.
            let nonce = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
            {
                let mut st = self.state.lock().unwrap();
                st.pending_probes.insert(nonce, Instant::now());
            }
            let pkt = self.build_hello_interest(nonce);
            ctx.send_on(self.multicast_face_id, pkt);
            // Notify strategy so it resets backoff for future ticks.
            let mut strategy = self.strategy.lock().unwrap();
            strategy.trigger(TriggerEvent::FaceUp);
            debug!("EtherND: sent initial hello on face {face_id:?}");
        }
    }

    fn on_face_down(&self, _face_id: FaceId, _ctx: &dyn DiscoveryContext) {}

    fn on_inbound(
        &self,
        raw: &Bytes,
        incoming_face: FaceId,
        meta: &InboundMeta,
        ctx: &dyn DiscoveryContext,
    ) -> bool {
        if raw.is_empty() {
            return false;
        }
        // Bytes arrive LP-unwrapped from the pipeline (TlvDecodeStage strips LP
        // before on_inbound is called). Dispatch directly on the NDN type byte.
        match raw.first() {
            Some(&0x05) => {
                if self.config.swim_indirect_fanout > 0 {
                    if let Some(parsed) = parse_raw_interest(raw) {
                        if parsed.name.has_prefix(probe_via()) {
                            return self.handle_via_probe_interest(raw, incoming_face, ctx);
                        }
                        if parsed.name.has_prefix(probe_direct()) {
                            return self.handle_direct_probe_interest(raw, incoming_face, ctx);
                        }
                    }
                }
                self.handle_hello_interest(raw, incoming_face, meta, ctx)
            }
            Some(&0x06) => {
                if self.config.swim_indirect_fanout > 0 && is_probe_ack(raw) {
                    return self.handle_probe_ack(raw, incoming_face, ctx).unwrap_or(false);
                }
                self.handle_hello_data(raw, incoming_face, meta, ctx)
            }
            _ => false,
        }
    }

    fn on_tick(&self, now: Instant, ctx: &dyn DiscoveryContext) {
        // ── Probe scheduling via strategy ─────────────────────────────────────
        let probes = {
            let mut strategy = self.strategy.lock().unwrap();
            strategy.on_tick(now)
        };
        for probe in probes {
            match probe {
                ProbeRequest::Broadcast => {
                    let nonce = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
                    let pkt = self.build_hello_interest(nonce);
                    ctx.send_on(self.multicast_face_id, pkt);
                    let mut st = self.state.lock().unwrap();
                    st.pending_probes.insert(nonce, now);
                    debug!("EtherND: broadcast hello (nonce={nonce:#010x})");
                }
                ProbeRequest::Unicast(face_id) => {
                    let nonce = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
                    let pkt = self.build_hello_interest(nonce);
                    ctx.send_on(face_id, pkt);
                    let mut st = self.state.lock().unwrap();
                    st.pending_probes.insert(nonce, now);
                    debug!("EtherND: unicast hello on face {face_id:?} (nonce={nonce:#010x})");
                }
            }
        }

        // ── Neighbor state machine ────────────────────────────────────────────
        let liveness_timeout = self.config.liveness_timeout;
        let miss_limit = self.config.liveness_miss_count;
        let gossip_k = self.config.gossip_fanout as usize;
        let all = ctx.neighbors().all();
        for entry in &all {
            match &entry.state {
                NeighborState::Established { last_seen } => {
                    if now.duration_since(*last_seen) > liveness_timeout {
                        ctx.update_neighbor(NeighborUpdate::SetState {
                            name: entry.node_name.clone(),
                            state: NeighborState::Stale {
                                miss_count: 1,
                                last_seen: *last_seen,
                            },
                        });
                        self.strategy.lock().unwrap().trigger(TriggerEvent::NeighborStale);
                        // Send unicast hello directly to the stale neighbor's face.
                        if let Some((face_id, _, _)) = entry.faces.first() {
                            let nonce = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
                            ctx.send_on(*face_id, self.build_hello_interest(nonce));
                            self.state.lock().unwrap().pending_probes.insert(nonce, now);
                        }
                        // Emergency gossip: K unicast hellos to other established peers.
                        if gossip_k > 0 {
                            let stale_name = &entry.node_name;
                            let peers: Vec<FaceId> = all.iter()
                                .filter(|e| e.is_reachable() && &e.node_name != stale_name)
                                .flat_map(|e| e.faces.iter().map(|(fid, _, _)| *fid))
                                .take(gossip_k)
                                .collect();
                            for face_id in peers {
                                let nonce = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
                                ctx.send_on(face_id, self.build_hello_interest(nonce));
                                self.state.lock().unwrap().pending_probes.insert(nonce, now);
                            }
                        }
                    }
                }
                NeighborState::Stale { miss_count, last_seen } => {
                    if u32::from(*miss_count) >= miss_limit {
                        debug!("EtherND: peer {} is Absent", entry.node_name);
                        for (face_id, _, _) in &entry.faces {
                            ctx.remove_fib_entry(&entry.node_name, *face_id, PROTOCOL);
                            ctx.remove_face(*face_id);
                        }
                        // Record departure for SWIM gossip piggyback.
                        {
                            let mut st = self.state.lock().unwrap();
                            st.recent_diffs.push_back(DiffEntry::Remove(entry.node_name.clone()));
                            while st.recent_diffs.len() > MAX_DIFF_ENTRIES {
                                st.recent_diffs.pop_front();
                            }
                        }
                        ctx.update_neighbor(NeighborUpdate::Remove(entry.node_name.clone()));
                    } else if now.duration_since(*last_seen) > liveness_timeout {
                        // Advance last_seen to now so the next miss fires after
                        // another full liveness_timeout, not on the next tick.
                        ctx.update_neighbor(NeighborUpdate::SetState {
                            name: entry.node_name.clone(),
                            state: NeighborState::Stale {
                                miss_count: miss_count + 1,
                                last_seen: now,
                            },
                        });
                    }
                }
                _ => {}
            }
        }

        // ── SWIM direct probes to established neighbors ───────────────────────
        if self.config.swim_indirect_fanout > 0 {
            for entry in all.iter().filter(|e| e.is_reachable()) {
                if let Some((face_id, _, _)) = entry.faces.first() {
                    let nonce = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
                    ctx.send_on(*face_id, build_direct_probe(&entry.node_name, nonce));
                    self.state.lock().unwrap().swim_probes.insert(nonce, (now, entry.node_name.clone()));
                }
            }
        }

        // ── Expire pending probes ─────────────────────────────────────────────
        let probe_timeout = self.config.probe_timeout;
        let mut timed_out = 0u32;
        {
            let mut st = self.state.lock().unwrap();
            st.pending_probes.retain(|_, sent| {
                if now.duration_since(*sent) >= probe_timeout {
                    timed_out += 1;
                    false
                } else {
                    true
                }
            });
        }
        if timed_out > 0 {
            let mut strategy = self.strategy.lock().unwrap();
            for _ in 0..timed_out {
                strategy.on_probe_timeout();
            }
        }

        // ── Expire SWIM direct probes; dispatch indirect probes on failure ─────
        if self.config.swim_indirect_fanout > 0 {
            let k = self.config.swim_indirect_fanout as usize;
            let mut timed_out_swim: Vec<Name> = Vec::new();
            {
                let mut st = self.state.lock().unwrap();
                st.swim_probes.retain(|_, (sent, target)| {
                    if now.duration_since(*sent) >= probe_timeout {
                        timed_out_swim.push(target.clone()); false
                    } else { true }
                });
            }
            for target in timed_out_swim {
                let intermediaries: Vec<_> = ctx.neighbors().all().into_iter()
                    .filter(|e| e.is_reachable() && e.node_name != target)
                    .take(k)
                    .collect();
                for via in intermediaries {
                    let nonce = self.nonce_counter.fetch_add(1, Ordering::Relaxed);
                    if let Some((face_id, _, _)) = via.faces.first() {
                        ctx.send_on(*face_id, build_indirect_probe(&via.node_name, &target, nonce));
                        debug!("EtherND: SWIM indirect probe -> {:?} via {:?}", target, via.node_name);
                    }
                }
                self.strategy.lock().unwrap().on_probe_timeout();
            }
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use ndn_discovery::wire::parse_raw_data;

    fn make_nd() -> EtherNeighborDiscovery {
        EtherNeighborDiscovery::new(
            FaceId(1),
            "eth0",
            Name::from_str("/ndn/test/node").unwrap(),
            MacAddr([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]),
        )
    }

    #[test]
    fn hello_interest_format() {
        let nd = make_nd();
        let nonce: u32 = 0xDEAD_BEEF;
        let pkt = nd.build_hello_interest(nonce);

        let parsed = parse_raw_interest(&pkt).unwrap();
        let comps = parsed.name.components();

        // /ndn/local/nd/hello/<nonce> = 5 components
        assert_eq!(comps.len(), HELLO_PREFIX_DEPTH + 1,
            "unexpected component count: {}", comps.len());

        let last = &comps[HELLO_PREFIX_DEPTH];
        let decoded_nonce = u32::from_be_bytes(last.value[..4].try_into().unwrap());
        assert_eq!(decoded_nonce, nonce);

        // No AppParams in the doc format.
        assert!(parsed.app_params.is_none(), "Interest must have no AppParams");
    }

    #[test]
    fn hello_data_carries_hello_payload() {
        let nd = make_nd();
        let interest_name = Name::from_str("/ndn/local/nd/hello/DEADBEEF").unwrap();
        let pkt = nd.build_hello_data(&interest_name);

        let parsed = parse_raw_data(&pkt).unwrap();
        assert_eq!(parsed.name, interest_name);

        let content = parsed.content.unwrap();
        let payload = HelloPayload::decode(&content).unwrap();
        assert_eq!(payload.node_name, nd.node_name);
    }

    #[test]
    fn in_hello_served_prefixes_encoded() {
        let nd = make_nd();
        nd.set_served_prefixes(vec![
            Name::from_str("/ndn/edu/test").unwrap(),
            Name::from_str("/ndn/edu/test2").unwrap(),
        ]);

        let interest_name = Name::from_str("/ndn/local/nd/hello/DEADBEEF").unwrap();
        // LAN config uses InHello mode by default.
        let pkt = nd.build_hello_data(&interest_name);

        let parsed = parse_raw_data(&pkt).unwrap();
        let payload = HelloPayload::decode(&parsed.content.unwrap()).unwrap();
        assert_eq!(payload.served_prefixes.len(), 2);
        assert_eq!(payload.served_prefixes[0], Name::from_str("/ndn/edu/test").unwrap());
    }

    #[test]
    fn neighbor_diffs_piggybacked() {
        let nd = make_nd();
        {
            let mut st = nd.state.lock().unwrap();
            st.recent_diffs.push_back(DiffEntry::Add(Name::from_str("/ndn/peer/alpha").unwrap()));
        }
        let interest_name = Name::from_str("/ndn/local/nd/hello/1").unwrap();
        let pkt = nd.build_hello_data(&interest_name);
        let parsed = parse_raw_data(&pkt).unwrap();
        let payload = HelloPayload::decode(&parsed.content.unwrap()).unwrap();
        assert_eq!(payload.neighbor_diffs.len(), 1);
        assert_eq!(payload.neighbor_diffs[0].entries.len(), 1);
    }

    #[test]
    fn protocol_id_and_prefix() {
        let nd = make_nd();
        assert_eq!(nd.protocol_id(), PROTOCOL);
        assert_eq!(nd.claimed_prefixes().len(), 1);
        assert_eq!(
            nd.claimed_prefixes()[0],
            Name::from_str(HELLO_PREFIX_STR).unwrap()
        );
    }

    #[test]
    fn from_profile_sets_config() {
        let nd = EtherNeighborDiscovery::from_profile(
            FaceId(1),
            "wlan0",
            Name::from_str("/ndn/test/node").unwrap(),
            MacAddr([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]),
            &DiscoveryProfile::HighMobility,
        );
        // High-mobility profile has a very short base interval.
        use std::time::Duration;
        assert!(nd.config.hello_interval_base < Duration::from_millis(100));
    }
}

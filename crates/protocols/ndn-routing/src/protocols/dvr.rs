//! Distance Vector Routing (DVR) over NDN link-local multicast.
//!
//! # Algorithm
//!
//! Distributed Bellman-Ford with split horizon. Each router maintains a
//! distance table: `prefix → (cost, via_face)`. Every `UPDATE_INTERVAL` the
//! router broadcasts its table to all known neighbor faces. On receiving a
//! broadcast from a neighbor, the router applies Bellman-Ford and updates
//! routes in the RIB.
//!
//! **Split horizon**: when sending an update on face F, routes that were
//! learned *via* face F are omitted. This prevents routing loops on two-node
//! links.
//!
//! # Dual-trait design
//!
//! `DvrProtocol` implements both:
//! - [`DiscoveryProtocol`] — receives packet I/O callbacks (`on_inbound`,
//!   `on_tick`) from the engine's discovery subsystem.
//! - [`RoutingProtocol`] — `start()` stores the [`RoutingHandle`] so that
//!   `on_inbound` can write learned routes into the RIB.
//!
//! The engine registers it twice: once with `EngineBuilder::discovery()` for
//! packet delivery, and once with `EngineBuilder::routing_protocol()` for RIB
//! lifecycle. The two registrations share an `Arc<DvrInner>` so state is
//! consistent.
//!
//! # Wire format
//!
//! DVR updates are sent as NDN Interest packets with AppParams:
//!
//! ```text
//! Interest name: /ndn/local/dvr/adv
//! AppParams = DVR-UPDATE TLV:
//!   NODE-NAME  TLV (0xD1)
//!   (ROUTE TLV (0xD2))*
//!     PREFIX   TLV (0xD3)  — a Name
//!     DVR-COST TLV (0xD4)  — NonNegativeInteger
//! ```
//!
//! Interest packets are used (not Data) because they require no signature.
//! The `/ndn/local/dvr/adv` name is consumed by `on_inbound` and never
//! reaches the NDN forwarding pipeline.
//!
//! # Route lifetime
//!
//! Routes learned from neighbors expire after `ROUTE_TTL` (90 s) if no
//! update is received. The RIB's background expiry task removes them.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use bytes::Bytes;
use dashmap::DashMap;
use ndn_config::control_parameters::route_flags;
use ndn_discovery::{DiscoveryContext, DiscoveryProtocol, InboundMeta, ProtocolId};
use ndn_engine::{RibRoute, RoutingHandle, RoutingProtocol};
use ndn_packet::{Name, NameComponent, tlv_type};
use ndn_tlv::{TlvReader, TlvWriter};
use ndn_transport::FaceId;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace};

// ─── Constants / runtime config ──────────────────────────────────────────────

/// Default broadcast interval (used when no custom config is provided).
pub const DEFAULT_UPDATE_INTERVAL: Duration = Duration::from_secs(30);

/// Default route TTL (should be ≥ 2× update interval).
pub const DEFAULT_ROUTE_TTL: Duration = Duration::from_secs(90);

// Keep the old names as aliases so external code that references them still compiles.
pub const UPDATE_INTERVAL: Duration = DEFAULT_UPDATE_INTERVAL;
pub const ROUTE_TTL: Duration = DEFAULT_ROUTE_TTL;

/// Runtime-mutable configuration for the DVR protocol.
///
/// Wrap in `Arc<RwLock<>>` and share between the protocol instance and the
/// management handler so that parameters can be updated at runtime.
#[derive(Clone, Debug)]
pub struct DvrConfig {
    /// How often to broadcast the routing table to all neighbor faces.
    pub update_interval: Duration,
    /// Routes learned from neighbors expire after this long without a refresh.
    /// Must be ≥ 2× `update_interval` to avoid false expiry between broadcasts.
    pub route_ttl: Duration,
}

impl Default for DvrConfig {
    fn default() -> Self {
        Self {
            update_interval: DEFAULT_UPDATE_INTERVAL,
            route_ttl: DEFAULT_ROUTE_TTL,
        }
    }
}

/// DVR protocol identifier used in `DiscoveryProtocol::protocol_id`.
pub const DVR_PROTOCOL_ID: ProtocolId = ProtocolId("dvr");

/// Origin value for DVR-learned routes.
pub const DVR_ORIGIN: u64 = ndn_config::control_parameters::origin::DVR;

// ─── Wire format TLV types (application-specific range ≥ 0xC0) ──────────────

/// Root TLV for a DVR route advertisement.
const T_DVR_UPDATE: u64 = 0xD0;
/// Sender's NDN node name.
const T_NODE_NAME: u64 = 0xD1;
/// Single route entry.
const T_ROUTE: u64 = 0xD2;
/// Name prefix inside a `ROUTE`.
const T_PREFIX: u64 = 0xD3;
/// Cost inside a `ROUTE`.
const T_DVR_COST: u64 = 0xD4;

// ─── DVR prefix claimed for inbound routing ──────────────────────────────────

fn dvr_adv_prefix() -> Name {
    Name::from_components([
        NameComponent::generic(bytes::Bytes::from_static(b"ndn")),
        NameComponent::generic(bytes::Bytes::from_static(b"local")),
        NameComponent::generic(bytes::Bytes::from_static(b"dvr")),
        NameComponent::generic(bytes::Bytes::from_static(b"adv")),
    ])
}

// ─── Internal state ──────────────────────────────────────────────────────────

/// Entry in the local distance table.
#[derive(Clone, Debug)]
struct DvrEntry {
    /// Accumulated cost to reach this prefix via `via_face`.
    cost: u32,
    /// Face through which this route was learned (None = locally originated).
    via_face: Option<FaceId>,
    /// When this entry expires (refreshed on each received update).
    expires_at: Instant,
}

/// Shared state for a DVR protocol instance.
pub struct DvrInner {
    /// This router's NDN node name, included in updates so neighbors can
    /// identify the sender.
    node_name: Name,
    /// Distance table: `prefix → best entry`.
    table: DashMap<Name, DvrEntry>,
    /// Handles to RIB/FIB/faces, populated by `RoutingProtocol::start`.
    routing: OnceLock<RoutingHandle>,
    /// Prefixes claimed for inbound routing (constructed once, immutable).
    claimed: Vec<Name>,
    /// Time of last broadcast — used to avoid redundant updates on tick.
    last_update: std::sync::Mutex<Option<Instant>>,
    /// Runtime-mutable protocol configuration (intervals, TTL).
    config: Arc<RwLock<DvrConfig>>,
}

impl DvrInner {
    fn new(node_name: Name, config: Arc<RwLock<DvrConfig>>) -> Arc<Self> {
        Arc::new(Self {
            node_name,
            table: DashMap::new(),
            routing: OnceLock::new(),
            claimed: vec![dvr_adv_prefix()],
            last_update: std::sync::Mutex::new(None),
            config,
        })
    }

    // ── Wire encode ──────────────────────────────────────────────────────────

    /// Build a DVR advertisement Interest for face `send_face`.
    ///
    /// Split horizon: routes learned via `send_face` are excluded.
    fn build_adv(&self, send_face: FaceId) -> Bytes {
        let mut params_writer = TlvWriter::new();
        params_writer.write_nested(T_DVR_UPDATE, |w: &mut TlvWriter| {
            // Node name.
            w.write_nested(T_NODE_NAME, |w: &mut TlvWriter| {
                for comp in self.node_name.components() {
                    w.write_tlv(comp.typ, &comp.value);
                }
            });
            // Routes (split horizon: skip routes via this face).
            for entry in self.table.iter() {
                if entry.via_face == Some(send_face) {
                    continue; // split horizon
                }
                w.write_nested(T_ROUTE, |w: &mut TlvWriter| {
                    // PREFIX TLV contains a Name.
                    w.write_nested(T_PREFIX, |w: &mut TlvWriter| {
                        for comp in entry.key().components() {
                            w.write_tlv(comp.typ, &comp.value);
                        }
                    });
                    // DVR-COST as big-endian u32.
                    w.write_tlv(T_DVR_COST, &entry.cost.to_be_bytes());
                });
            }
        });
        let params_bytes = params_writer.finish();

        let adv_prefix = dvr_adv_prefix();
        let mut w = TlvWriter::new();
        w.write_nested(tlv_type::INTEREST, |w: &mut TlvWriter| {
            w.write_nested(tlv_type::NAME, |w: &mut TlvWriter| {
                for comp in adv_prefix.components() {
                    w.write_tlv(comp.typ, &comp.value);
                }
            });
            // Nonce: use low bits of current time for uniqueness.
            let nonce = (Instant::now().elapsed().subsec_nanos()).to_be_bytes();
            w.write_tlv(tlv_type::NONCE, &nonce);
            // AppParams containing the DVR-UPDATE TLV.
            w.write_tlv(tlv_type::APP_PARAMETERS, &params_bytes);
        });
        w.finish()
    }

    // ── Wire decode ──────────────────────────────────────────────────────────

    /// Decode a DVR advertisement from AppParams bytes.
    ///
    /// Returns `None` if the bytes are not a valid DVR-UPDATE TLV.
    fn decode_adv(app_params: &Bytes) -> Option<DvrAdvertisement> {
        let mut r = TlvReader::new(app_params.clone());
        let (typ, val) = r.read_tlv().ok()?;
        if typ != T_DVR_UPDATE {
            return None;
        }
        let mut inner = TlvReader::new(val);
        let mut node_name: Option<Name> = None;
        let mut routes: Vec<(Name, u32)> = Vec::new();

        while !inner.is_empty() {
            let (t, v) = inner.read_tlv().ok()?;
            match t {
                T_NODE_NAME => {
                    node_name = Some(Name::decode(v).ok()?);
                }
                T_ROUTE => {
                    let mut route_r = TlvReader::new(v);
                    let mut prefix: Option<Name> = None;
                    let mut cost: Option<u32> = None;
                    while !route_r.is_empty() {
                        let (rt, rv) = route_r.read_tlv().ok()?;
                        match rt {
                            T_PREFIX => {
                                prefix = Some(Name::decode(rv).ok()?);
                            }
                            T_DVR_COST => {
                                let bytes = rv;
                                cost = Some(match bytes.len() {
                                    1 => bytes[0] as u32,
                                    2 => u16::from_be_bytes([bytes[0], bytes[1]]) as u32,
                                    4 => {
                                        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
                                    }
                                    _ => return None,
                                });
                            }
                            _ => {}
                        }
                    }
                    if let (Some(p), Some(c)) = (prefix, cost) {
                        routes.push((p, c));
                    }
                }
                _ => {}
            }
        }

        Some(DvrAdvertisement {
            node_name: node_name?,
            routes,
        })
    }

    // ── Bellman-Ford update ──────────────────────────────────────────────────

    /// Process a received DVR advertisement from `from_face`.
    ///
    /// Applies Bellman-Ford: for each (prefix, advertised_cost) in the update,
    /// new_cost = advertised_cost + 1 (link cost). If new_cost is better than
    /// our current best or if this is the route we're currently using (so we
    /// track withdrawals), update the table and RIB.
    fn process_adv(&self, adv: DvrAdvertisement, from_face: FaceId) {
        let Some(handle) = self.routing.get() else {
            return; // RoutingProtocol::start not yet called
        };

        let now = Instant::now();
        let route_ttl = self.config.read().unwrap().route_ttl;
        let expires = now + route_ttl;

        // Build a set of prefixes advertised in this update (for withdrawal detection).
        let advertised: HashMap<Name, u32> = adv.routes.into_iter().collect();

        // Update known routes.
        for (prefix, &adv_cost) in &advertised {
            // Cost: neighbour's cost + 1 hop. Cap at u32::MAX to avoid overflow.
            let new_cost = adv_cost.saturating_add(1);

            let mut updated = false;
            self.table
                .entry(prefix.clone())
                .and_modify(|e| {
                    if new_cost < e.cost || e.via_face == Some(from_face) {
                        e.cost = new_cost;
                        e.via_face = Some(from_face);
                        e.expires_at = expires;
                        updated = true;
                    } else if e.via_face == Some(from_face) {
                        // Same face — refresh TTL even if cost unchanged.
                        e.expires_at = expires;
                    }
                })
                .or_insert_with(|| {
                    updated = true;
                    DvrEntry {
                        cost: new_cost,
                        via_face: Some(from_face),
                        expires_at: expires,
                    }
                });

            if updated {
                debug!(
                    prefix = %prefix,
                    cost = new_cost,
                    via = from_face.0,
                    "DVR route updated"
                );
                handle.rib.add(
                    prefix,
                    RibRoute {
                        face_id: from_face,
                        origin: DVR_ORIGIN,
                        cost: new_cost,
                        flags: route_flags::CHILD_INHERIT,
                        expires_at: Some(expires),
                    },
                );
                handle.rib.apply_to_fib(prefix, &handle.fib);
            }
        }
    }

    // ── Broadcast ────────────────────────────────────────────────────────────

    /// Send a DVR advertisement on every known neighbor face.
    fn broadcast(&self, ctx: &dyn DiscoveryContext) {
        let neighbors = ctx.neighbors();
        let all = neighbors.all();
        if all.is_empty() {
            trace!("DVR: no neighbors, skipping broadcast");
            return;
        }
        for neighbor in &all {
            for (face_id, _, _) in &neighbor.faces {
                let pkt = self.build_adv(*face_id);
                ctx.send_on(*face_id, pkt);
                trace!(face = face_id.0, peer = %neighbor.node_name, "DVR adv sent");
            }
        }
        *self.last_update.lock().unwrap() = Some(Instant::now());
    }
}

struct DvrAdvertisement {
    #[allow(dead_code)]
    node_name: Name,
    routes: Vec<(Name, u32)>,
}

// ─── Public handle ────────────────────────────────────────────────────────────

/// Distance Vector Routing protocol.
///
/// Register this with the engine as *both* a discovery protocol (for packet
/// I/O) and a routing protocol (for RIB lifecycle):
///
/// ```rust,ignore
/// use ndn_routing::DvrProtocol;
/// use std::sync::Arc;
///
/// let dvr = DvrProtocol::new(my_node_name.clone());
/// let engine = EngineBuilder::new()
///     .discovery(Arc::clone(&dvr) as Arc<dyn DiscoveryProtocol>)
///     .routing_protocol(Arc::clone(&dvr))
///     .build().await?;
/// ```
///
/// Note: `DvrProtocol` wraps `Arc<DvrInner>` internally, so cloning it is
/// cheap and shares state between the two registrations.
#[derive(Clone)]
pub struct DvrProtocol {
    inner: Arc<DvrInner>,
}

impl DvrProtocol {
    pub fn new(node_name: Name) -> Arc<Self> {
        Self::new_with_config(node_name, Arc::new(RwLock::new(DvrConfig::default())))
    }

    pub fn new_with_config(node_name: Name, config: Arc<RwLock<DvrConfig>>) -> Arc<Self> {
        Arc::new(Self {
            inner: DvrInner::new(node_name, config),
        })
    }

    /// Return a cloneable handle to the shared DVR config for the management handler.
    pub fn config_handle(&self) -> Arc<RwLock<DvrConfig>> {
        Arc::clone(&self.inner.config)
    }
}

// ─── DiscoveryProtocol ────────────────────────────────────────────────────────

impl DiscoveryProtocol for DvrProtocol {
    fn protocol_id(&self) -> ProtocolId {
        DVR_PROTOCOL_ID
    }

    fn claimed_prefixes(&self) -> &[Name] {
        &self.inner.claimed
    }

    fn on_face_up(&self, _face_id: FaceId, _ctx: &dyn DiscoveryContext) {
        // Trigger an immediate broadcast so the new neighbor learns our table.
        // We cannot call broadcast() here because ctx may not have the face yet;
        // the next on_tick will handle it.
    }

    fn on_face_down(&self, face_id: FaceId, _ctx: &dyn DiscoveryContext) {
        // Remove all routes learned via this face.
        let Some(handle) = self.inner.routing.get() else {
            return;
        };
        let mut removed: Vec<Name> = Vec::new();
        self.inner.table.retain(|prefix, entry| {
            if entry.via_face == Some(face_id) {
                removed.push(prefix.clone());
                false
            } else {
                true
            }
        });
        for prefix in &removed {
            handle.rib.remove(prefix, face_id, DVR_ORIGIN);
            handle.rib.apply_to_fib(prefix, &handle.fib);
        }
        if !removed.is_empty() {
            debug!(
                face = face_id.0,
                routes = removed.len(),
                "DVR routes withdrawn (face down)"
            );
        }
    }

    fn on_inbound(
        &self,
        raw: &Bytes,
        incoming_face: FaceId,
        _meta: &InboundMeta,
        _ctx: &dyn DiscoveryContext,
    ) -> bool {
        // Only consume Interest packets targeted at our DVR prefix.
        let Some(interest) = parse_dvr_interest(raw) else {
            return false;
        };

        let Some(adv) = DvrInner::decode_adv(&interest.app_params) else {
            return false;
        };

        trace!(face = incoming_face.0, "DVR adv received");
        self.inner.process_adv(adv, incoming_face);
        true
    }

    fn on_tick(&self, _now: std::time::Instant, ctx: &dyn DiscoveryContext) {
        let update_interval = self.inner.config.read().unwrap().update_interval;
        let should_send = self
            .inner
            .last_update
            .lock()
            .unwrap()
            .map(|t| t.elapsed() >= update_interval)
            .unwrap_or(true);

        if should_send {
            self.inner.broadcast(ctx);
        }
    }

    fn tick_interval(&self) -> Duration {
        // Poll at 1s granularity; actual updates are gated by UPDATE_INTERVAL.
        Duration::from_secs(1)
    }
}

// ─── RoutingProtocol ─────────────────────────────────────────────────────────

impl RoutingProtocol for DvrProtocol {
    fn origin(&self) -> u64 {
        DVR_ORIGIN
    }

    fn start(&self, handle: RoutingHandle, cancel: CancellationToken) -> JoinHandle<()> {
        // Store the handle so on_inbound / on_face_down can write to the RIB.
        // Ignore the error — this may be called twice if the protocol is re-enabled.
        let _ = self.inner.routing.set(handle);
        tokio::spawn(async move {
            cancel.cancelled().await;
        })
    }
}

// ─── Packet parsing helper ────────────────────────────────────────────────────

struct DvrInterest {
    app_params: Bytes,
}

/// Parse a DVR advertisement Interest from raw wire bytes.
///
/// Checks that the Interest name starts with `/ndn/local/dvr/adv` and
/// extracts the AppParams payload.
fn parse_dvr_interest(raw: &Bytes) -> Option<DvrInterest> {
    let mut r = TlvReader::new(raw.clone());
    let (typ, val) = r.read_tlv().ok()?;
    if typ != tlv_type::INTEREST {
        return None;
    }
    let mut inner = TlvReader::new(val);
    let mut name: Option<Name> = None;
    let mut app_params: Option<Bytes> = None;

    while !inner.is_empty() {
        let (t, v) = inner.read_tlv().ok()?;
        match t {
            t if t == tlv_type::NAME => {
                name = Some(Name::decode(v).ok()?);
            }
            t if t == tlv_type::APP_PARAMETERS => {
                app_params = Some(v);
            }
            _ => {}
        }
    }

    let name = name?;
    let app_params = app_params?;

    // Check prefix: must start with /ndn/local/dvr/adv (4 components).
    let adv_prefix = dvr_adv_prefix();
    let adv_comps = adv_prefix.components();
    let name_comps = name.components();
    if name_comps.len() < adv_comps.len() {
        return None;
    }
    for (a, b) in adv_comps.iter().zip(name_comps.iter()) {
        if a.value != b.value {
            return None;
        }
    }

    Some(DvrInterest { app_params })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_name(s: &str) -> Name {
        Name::from_components(
            s.split('/')
                .filter(|c| !c.is_empty())
                .map(|c| NameComponent::generic(bytes::Bytes::copy_from_slice(c.as_bytes()))),
        )
    }

    #[test]
    fn adv_roundtrip_no_routes() {
        let node = make_name("/ndn/test/node");
        let inner = DvrInner::new(node, Arc::new(RwLock::new(DvrConfig::default())));
        let pkt = inner.build_adv(FaceId(1));
        let parsed = parse_dvr_interest(&pkt).expect("should parse as DVR interest");
        let adv = DvrInner::decode_adv(&parsed.app_params).expect("should decode adv");
        assert_eq!(adv.routes.len(), 0);
    }

    #[test]
    fn adv_roundtrip_with_routes() {
        let node = make_name("/ndn/test/node");
        let inner = DvrInner::new(node, Arc::new(RwLock::new(DvrConfig::default())));

        // Inject a route via face 2 to simulate a learned route.
        inner.table.insert(
            make_name("/ndn/edu/ucla"),
            DvrEntry {
                cost: 5,
                via_face: Some(FaceId(2)),
                expires_at: Instant::now() + Duration::from_secs(90),
            },
        );
        // Route via face 1 — should be split-horizon excluded when sending on face 1.
        inner.table.insert(
            make_name("/ndn/edu/mit"),
            DvrEntry {
                cost: 3,
                via_face: Some(FaceId(1)),
                expires_at: Instant::now() + Duration::from_secs(90),
            },
        );

        // Build adv for face 1 — /ndn/edu/mit should be excluded (split horizon).
        let pkt = inner.build_adv(FaceId(1));
        let parsed = parse_dvr_interest(&pkt).expect("should parse");
        let adv = DvrInner::decode_adv(&parsed.app_params).expect("should decode");
        assert_eq!(adv.routes.len(), 1);
        assert_eq!(adv.routes[0].0, make_name("/ndn/edu/ucla"));
        assert_eq!(adv.routes[0].1, 5);
    }

    #[test]
    fn adv_non_dvr_packet_rejected() {
        // Build a random Interest that is not a DVR advertisement.
        let mut w = TlvWriter::new();
        w.write_nested(tlv_type::INTEREST, |w: &mut TlvWriter| {
            w.write_nested(tlv_type::NAME, |w: &mut TlvWriter| {
                w.write_tlv(0x08, b"ndn");
                w.write_tlv(0x08, b"test");
            });
            w.write_tlv(tlv_type::NONCE, &[0, 0, 0, 1]);
        });
        let pkt = w.finish();
        assert!(parse_dvr_interest(&pkt).is_none());
    }
}

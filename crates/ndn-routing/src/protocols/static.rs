//! Static routing protocol — installs pre-configured routes at startup.
//!
//! Each `StaticRoute` entry is inserted into the RIB once, under
//! `origin::STATIC` (255), when the protocol starts. Routes are permanent
//! (no expiry) and are automatically flushed from the RIB when the protocol
//! is disabled via `RoutingManager::disable`.

use ndn_config::control_parameters::{origin, route_flags};
use ndn_engine::{RibRoute, RoutingHandle, RoutingProtocol};
use ndn_packet::Name;
use ndn_transport::FaceId;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// A single statically-configured route.
#[derive(Clone, Debug)]
pub struct StaticRoute {
    /// Name prefix to route.
    pub prefix: Name,
    /// Outgoing face for this prefix.
    pub face_id: FaceId,
    /// Route cost (lower is preferred).
    pub cost: u32,
}

/// Routing protocol that installs a fixed set of routes at startup.
///
/// Useful for:
/// - Single-hop links with known prefixes
/// - Testing and simulation
/// - Hybrid deployments where some routes are known statically
///
/// # Origin
///
/// Uses `origin::STATIC` (255). Per NDN convention, static routes have the
/// highest origin value and therefore lose to dynamically-learned routes
/// from NLSR/DVR when costs are equal.
pub struct StaticProtocol {
    routes: Vec<StaticRoute>,
}

impl StaticProtocol {
    pub fn new(routes: Vec<StaticRoute>) -> Self {
        Self { routes }
    }
}

impl RoutingProtocol for StaticProtocol {
    fn origin(&self) -> u64 {
        origin::STATIC
    }

    fn start(&self, handle: RoutingHandle, cancel: CancellationToken) -> JoinHandle<()> {
        let routes = self.routes.clone();
        tokio::spawn(async move {
            // Install all routes into the RIB.
            for route in &routes {
                handle.rib.add(
                    &route.prefix,
                    RibRoute {
                        face_id: route.face_id,
                        origin: origin::STATIC,
                        cost: route.cost,
                        flags: route_flags::CHILD_INHERIT,
                        expires_at: None,
                    },
                );
                handle.rib.apply_to_fib(&route.prefix, &handle.fib);
                info!(
                    prefix = %route.prefix,
                    face_id = route.face_id.0,
                    cost = route.cost,
                    "static route installed"
                );
            }

            // Hold until cancelled — the RoutingManager flushes our routes on drop.
            cancel.cancelled().await;
        })
    }
}

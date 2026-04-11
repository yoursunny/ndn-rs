//! [`NdnEngine`] — embedded NDN forwarder for Android and iOS.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use boltffi::export;
use tokio::runtime::Runtime;

use ndn_app::Producer;
use ndn_faces::local::InProcHandle;
use ndn_mobile::{MobileEngine, MobileEngineBuilder};
use ndn_packet::Name;

use crate::types::{NdnEngineConfig, NdnError, into_security_profile};

/// Embedded NDN forwarder for mobile apps.
///
/// `NdnEngine` runs the forwarder in-process together with the application,
/// embedding a Tokio runtime and managing all face I/O on background threads.
///
/// # Usage
///
/// Construct once at app startup:
/// ```no_run
/// let engine = NdnEngine::new(NdnEngineConfig { ... });
/// ```
/// Then pass `&engine` to the constructors of
/// [`NdnConsumer`](crate::NdnConsumer), [`NdnProducer`](crate::NdnProducer),
/// and [`NdnSubscriber`](crate::NdnSubscriber).
///
/// # Lifecycle callbacks
///
/// Call [`suspend_network_faces`](Self::suspend_network_faces) /
/// [`resume_network_faces`](Self::resume_network_faces) from the platform
/// activity lifecycle to pause/resume network I/O in the background.
pub struct NdnEngine {
    /// The inner MobileEngine; wrapped in Option so shutdown can consume it.
    pub(crate) inner: Mutex<Option<MobileEngine>>,
    /// Shared Tokio runtime — cloned into all derived consumer/producer objects.
    pub(crate) rt: Arc<Runtime>,
    /// Primary InProcHandle from builder.build(); taken on first consumer::new() call.
    default_handle: Mutex<Option<InProcHandle>>,
}

#[export]
impl NdnEngine {
    /// Build and start the embedded forwarder.
    ///
    /// Blocks until the engine initialises (typically < 100 ms). Call once at
    /// app startup; keep the returned object alive for the app session.
    ///
    /// # Errors
    ///
    /// Returns [`NdnError::Engine`] if the Tokio runtime cannot be created or
    /// the engine fails to start (e.g. multicast interface unreachable).
    pub fn new(config: NdnEngineConfig) -> Result<Self, NdnError> {
        let threads = (config.pipeline_threads as usize).max(1);
        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(threads)
                .enable_all()
                .build()
                .map_err(NdnError::engine)?,
        );

        let (mobile_engine, default_handle) = rt.block_on(build_engine(config))?;

        Ok(Self {
            inner: Mutex::new(Some(mobile_engine)),
            rt,
            default_handle: Mutex::new(Some(default_handle)),
        })
    }

    /// Suspend all network faces (UDP, Bluetooth).
    ///
    /// Stops face recv/send tasks while keeping the in-process engine, FIB,
    /// PIT, CS, and AppFace fully active. Call from the platform lifecycle:
    /// - Android: `Activity.onStop()` / `onPause()`
    /// - iOS: `applicationDidEnterBackground(_:)` / `sceneDidEnterBackground(_:)`
    pub fn suspend_network_faces(&self) {
        let mut guard = self.inner.lock().unwrap();
        if let Some(engine) = guard.as_mut() {
            engine.suspend_network_faces();
        }
    }

    /// Resume network faces after [`suspend_network_faces`](Self::suspend_network_faces).
    ///
    /// Recreates UDP multicast (if configured) and re-enables network I/O.
    /// Call from the platform foreground lifecycle:
    /// - Android: `Activity.onStart()` / `onResume()`
    /// - iOS: `applicationWillEnterForeground(_:)` / `sceneWillEnterForeground(_:)`
    pub fn resume_network_faces(&self) {
        let mut guard = self.inner.lock().unwrap();
        if let Some(engine) = guard.as_mut() {
            self.rt.block_on(engine.resume_network_faces());
        }
    }

    /// Gracefully shut down the engine.
    ///
    /// Drains in-flight packets, stops all background tasks, and frees
    /// resources. The engine is unusable after this call.
    pub fn shutdown(&self) {
        if let Some(engine) = self.inner.lock().unwrap().take() {
            self.rt.block_on(engine.shutdown());
        }
    }
}

// ── Internal helpers (not exported to FFI) ────────────────────────────────────

impl NdnEngine {
    /// Borrow the shared Tokio runtime handle.
    pub(crate) fn rt(&self) -> &Arc<Runtime> {
        &self.rt
    }

    /// Take the primary InProcHandle (first call) or allocate a fresh one.
    pub(crate) fn take_consumer_handle(&self) -> Result<InProcHandle, NdnError> {
        let mut guard = self.default_handle.lock().unwrap();
        if let Some(h) = guard.take() {
            return Ok(h);
        }
        let engine_guard = self.inner.lock().unwrap();
        let engine = engine_guard.as_ref()
            .ok_or_else(|| NdnError::engine("engine is shut down"))?;
        let (_, h) = engine.new_app_handle();
        Ok(h)
    }

    /// Allocate a fresh InProcHandle for a subscriber or additional consumer.
    pub(crate) fn alloc_app_handle(&self) -> Result<InProcHandle, NdnError> {
        let engine_guard = self.inner.lock().unwrap();
        let engine = engine_guard.as_ref()
            .ok_or_else(|| NdnError::engine("engine is shut down"))?;
        let (_, h) = engine.new_app_handle();
        Ok(h)
    }

    /// Register a FIB route and return a ready Producer.
    pub(crate) fn register_producer_internal(&self, name: Name) -> Result<Producer, NdnError> {
        let engine_guard = self.inner.lock().unwrap();
        let engine = engine_guard.as_ref()
            .ok_or_else(|| NdnError::engine("engine is shut down"))?;
        Ok(engine.register_producer(name))
    }
}

// ── Builder helper ────────────────────────────────────────────────────────────

async fn build_engine(config: NdnEngineConfig) -> Result<(MobileEngine, InProcHandle), NdnError> {
    let mut builder: MobileEngineBuilder = MobileEngine::builder()
        .cs_capacity_mb(config.cs_capacity_mb as usize)
        .pipeline_threads(config.pipeline_threads as usize)
        .security_profile(into_security_profile(config.security_profile));

    if let Some(iface_str) = config.multicast_interface {
        let iface: Ipv4Addr = iface_str
            .parse()
            .map_err(|_| NdnError::invalid_addr(&iface_str))?;
        builder = builder.with_udp_multicast(iface);
    }

    for peer_str in config.unicast_peers {
        let addr: SocketAddr = peer_str
            .parse()
            .map_err(|_| NdnError::invalid_addr(&peer_str))?;
        builder = builder.with_unicast_peer(addr);
    }

    if let Some(node_name) = config.node_name {
        let name: Name = node_name
            .parse()
            .map_err(|_| NdnError::invalid_name(&node_name))?;
        builder = builder.with_discovery(name);
    }

    #[cfg(feature = "fjall")]
    if let Some(path) = config.persistent_cs_path {
        builder = builder.with_persistent_cs(path);
    }
    #[cfg(not(feature = "fjall"))]
    if config.persistent_cs_path.is_some() {
        tracing::warn!(
            "NdnEngineConfig.persistent_cs_path set but 'fjall' feature is not \
             enabled; falling back to in-memory cache"
        );
    }

    builder.build().await.map_err(NdnError::engine)
}

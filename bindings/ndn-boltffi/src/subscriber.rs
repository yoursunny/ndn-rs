//! [`NdnSubscriber`] — blocking NDN pub/sub subscriber.

use std::sync::{Arc, Mutex};

use boltffi::export;
use tokio::runtime::Runtime;

use ndn_app::{NdnConnection, Subscriber};
use ndn_packet::Name;

use crate::engine::NdnEngine;
use crate::types::{NdnError, NdnSample};

/// Blocking NDN pub/sub subscriber.
///
/// Joins an NDN sync group (SVS protocol) and delivers [`NdnSample`]s as
/// peers publish new data.  Create via
/// [`NdnEngine::subscribe`](crate::NdnEngine::subscribe).
///
/// Call [`recv`](Self::recv) in a loop on a background thread:
///
/// # Kotlin
///
/// ```kotlin
/// val sub = engine.subscribe("/chat/room1")
/// thread {
///     while (true) {
///         val sample = sub.recv() ?: break
///         runOnUiThread { addMessage(sample.publisher, sample.payload) }
///     }
/// }
/// ```
///
/// # Swift
///
/// ```swift
/// let sub = try engine.subscribe(groupPrefix: "/chat/room1")
/// Task.detached {
///     while let sample = sub.recv() {
///         await MainActor.run { addMessage(sample.publisher, sample.payload) }
///     }
/// }
/// ```
pub struct NdnSubscriber {
    inner: Mutex<Subscriber>,
    rt: Arc<Runtime>,
}

impl NdnSubscriber {
    /// Create from an in-process connection (embedded engine path).
    ///
    /// `Subscriber::from_connection` calls `tokio::spawn` internally, so we
    /// enter the runtime context before constructing it.
    pub(crate) fn from_connection(
        conn: NdnConnection,
        group: Name,
        local_name: Name,
        rt: Arc<Runtime>,
    ) -> Result<Self, NdnError> {
        let _guard = rt.enter();
        let sub = Subscriber::from_connection(conn, group, local_name, Default::default())
            .map_err(NdnError::engine)?;
        Ok(Self {
            inner: Mutex::new(sub),
            rt,
        })
    }
}

#[export]
impl NdnSubscriber {
    /// Join an NDN sync group as a subscriber.
    ///
    /// Uses the SVS protocol to receive publications from all peers in
    /// `group_prefix`. Each subscriber gets its own app face and participates
    /// in sync state exchange independently.
    ///
    /// # Errors
    ///
    /// - [`NdnError::InvalidName`] — `group_prefix` is not a valid NDN URI.
    /// - [`NdnError::Engine`] — engine has been shut down or SVS init failed.
    pub fn new(engine: &NdnEngine, group_prefix: String) -> Result<Self, NdnError> {
        let group: Name = group_prefix
            .parse()
            .map_err(|_| NdnError::invalid_name(&group_prefix))?;
        let local_name = group
            .clone()
            .append(format!("node-{}", std::process::id()).as_bytes());
        let handle = engine.alloc_app_handle()?;
        let conn = NdnConnection::Embedded(handle);
        let rt = Arc::clone(engine.rt());
        NdnSubscriber::from_connection(conn, group, local_name, rt)
    }

    /// Wait for and return the next published sample.
    ///
    /// Blocks until a peer publishes data in the sync group. Returns `None`
    /// when the subscription ends (engine shut down or connection closed).
    /// Does not busy-spin — yields to the OS while waiting.
    pub fn recv(&self) -> Option<NdnSample> {
        let mut inner = self.inner.lock().unwrap();
        self.rt.block_on(inner.recv()).map(NdnSample::from_sample)
    }
}

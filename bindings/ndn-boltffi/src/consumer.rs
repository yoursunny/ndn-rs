//! [`NdnConsumer`] — blocking NDN consumer.

use std::sync::{Arc, Mutex};

use boltffi::export;
use tokio::runtime::Runtime;

use ndn_app::Consumer;
use ndn_faces::local::InProcHandle;

use crate::engine::NdnEngine;
use crate::types::{NdnData, NdnError};

/// Blocking NDN consumer.
///
/// Fetches Data packets by Interest name. All methods block the calling thread
/// for up to the Interest lifetime (~4.5 s). Call them on a background thread:
/// - Kotlin: `withContext(Dispatchers.IO) { consumer.fetch("/ndn/...") }`
/// - Swift: `try await Task.detached { try consumer.fetch(name: "/ndn/...") }.value`
///
/// Each `NdnConsumer` owns an independent app face and PIT context, so multiple
/// consumers can issue concurrent Interests without interfering.
pub struct NdnConsumer {
    // Mutex because Consumer::fetch/get take &mut self.
    inner: Mutex<Consumer>,
    rt: Arc<Runtime>,
}

impl NdnConsumer {
    pub(crate) fn from_handle(handle: InProcHandle, rt: Arc<Runtime>) -> Self {
        Self {
            inner: Mutex::new(Consumer::from_handle(handle)),
            rt,
        }
    }
}

#[export]
impl NdnConsumer {
    /// Create a consumer from a running engine.
    ///
    /// The first call reuses the engine's default app face. Subsequent calls
    /// allocate a new face each time. Pass `&engine` obtained from
    /// [`NdnEngine::new`].
    ///
    /// # Errors
    ///
    /// Returns [`NdnError::Engine`] if the engine has been shut down.
    pub fn new(engine: &NdnEngine) -> Result<Self, NdnError> {
        let handle = engine.take_consumer_handle()?;
        let rt = Arc::clone(engine.rt());
        Ok(NdnConsumer::from_handle(handle, rt))
    }

    /// Fetch a Data packet by name.
    ///
    /// Blocks until the Data arrives or the default timeout (~4.5 s) elapses.
    ///
    /// # Errors
    ///
    /// - [`NdnError::InvalidName`] — `name` is not a valid NDN URI.
    /// - [`NdnError::Timeout`] — no Data arrived before timeout.
    /// - [`NdnError::Nacked`] — forwarder returned a Nack (e.g. `NoRoute`).
    /// - [`NdnError::Engine`] — connection closed or internal error.
    pub fn fetch(&self, name: String) -> Result<NdnData, NdnError> {
        let parsed: ndn_packet::Name = name.parse()
            .map_err(|_| NdnError::invalid_name(&name))?;
        let mut inner = self.inner.lock().unwrap();
        self.rt
            .block_on(inner.fetch(parsed))
            .map(NdnData::from_packet)
            .map_err(|e| NdnError::from_app(e, &name))
    }

    /// Fetch the raw content bytes for a name.
    ///
    /// Like [`fetch`](Self::fetch) but returns only the content payload.
    /// Returns [`NdnError::Engine`] if the Data packet has no content field.
    pub fn get(&self, name: String) -> Result<Vec<u8>, NdnError> {
        let parsed: ndn_packet::Name = name.parse()
            .map_err(|_| NdnError::invalid_name(&name))?;
        let mut inner = self.inner.lock().unwrap();
        self.rt
            .block_on(inner.get(parsed))
            .map(|b| b.to_vec())
            .map_err(|e| NdnError::from_app(e, &name))
    }
}

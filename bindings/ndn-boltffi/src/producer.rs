//! [`NdnProducer`] — blocking NDN producer with a callback interface.

use std::sync::{Arc, Mutex};

use boltffi::export;
use bytes::Bytes;
use tokio::runtime::Runtime;

use ndn_app::Producer;
use ndn_packet::Name;

use crate::engine::NdnEngine;
use crate::types::NdnError;

// ── Callback trait ────────────────────────────────────────────────────────────

/// Callback for responding to incoming Interests.
///
/// Implement this in Kotlin or Swift and pass it to [`NdnProducer::serve`].
///
/// # Kotlin
///
/// ```kotlin
/// producer.serve(object : NdnInterestHandler {
///     override fun handleInterest(name: String): ByteArray? =
///         if (name.endsWith("/temperature")) "23.5C".toByteArray() else null
/// })
/// ```
///
/// # Swift
///
/// ```swift
/// class MySensor: NdnInterestHandler {
///     func handleInterest(name: String) -> Data? {
///         name.hasSuffix("/temperature") ? Data("23.5C".utf8) : nil
///     }
/// }
/// producer.serve(handler: MySensor())
/// ```
#[export]
pub trait NdnInterestHandler: Send + Sync {
    /// Called for each incoming Interest whose name falls under the registered prefix.
    ///
    /// - `name` — full Interest name URI, e.g. `"/ndn/sensor/temperature"`.
    /// - Return `Some(payload)` to respond; `None` drops the Interest silently.
    fn handle_interest(&self, name: String) -> Option<Vec<u8>>;
}

// ── NdnProducer ───────────────────────────────────────────────────────────────

/// Blocking NDN producer.
///
/// Serves Data packets in response to incoming Interests for a registered
/// name prefix. Create via [`NdnEngine::register_producer`](crate::NdnEngine::register_producer).
///
/// [`serve`](Self::serve) blocks until the engine connection closes. Run it on
/// a dedicated background thread:
/// - Kotlin: `Executors.newSingleThreadExecutor().execute { producer.serve(handler) }`
/// - Swift: `Task.detached { try producer.serve(handler: myHandler) }`
pub struct NdnProducer {
    inner: Mutex<Producer>,
    rt: Arc<Runtime>,
}

impl NdnProducer {
    pub(crate) fn new_inner(producer: Producer, rt: Arc<Runtime>) -> Self {
        Self {
            inner: Mutex::new(producer),
            rt,
        }
    }
}

#[export]
impl NdnProducer {
    /// Register a producer for a name prefix on the given engine.
    ///
    /// Interests whose names fall under `prefix` are forwarded to this producer.
    ///
    /// # Errors
    ///
    /// - [`NdnError::InvalidName`] — `prefix` is not a valid NDN URI.
    /// - [`NdnError::Engine`] — engine has been shut down.
    pub fn new(engine: &NdnEngine, prefix: String) -> Result<Self, NdnError> {
        let name: Name = prefix
            .parse()
            .map_err(|_| NdnError::invalid_name(&prefix))?;
        let producer = engine.register_producer_internal(name)?;
        Ok(NdnProducer::new_inner(producer, Arc::clone(engine.rt())))
    }

    /// The NDN name prefix this producer is registered for.
    pub fn prefix(&self) -> String {
        self.inner.lock().unwrap().prefix().to_string()
    }

    /// Run the producer serve loop.
    ///
    /// Calls `handler.handle_interest(name)` for each incoming Interest.
    /// Blocks until the engine connection closes (typically at app shutdown).
    ///
    /// # Errors
    ///
    /// Returns [`NdnError::Engine`] if the connection closes unexpectedly.
    pub fn serve(&self, handler: Box<dyn NdnInterestHandler>) -> Result<(), NdnError> {
        // Arc so the Fn closure can be called repeatedly without consuming handler.
        let handler: Arc<dyn NdnInterestHandler> = handler.into();
        let inner = self.inner.lock().unwrap();
        self.rt
            .block_on(inner.serve(move |interest, responder| {
                let name = interest.name.to_string();
                let result = handler.handle_interest(name).map(Bytes::from);
                async move {
                    if let Some(wire) = result {
                        responder.respond_bytes(wire).await.ok();
                    }
                }
            }))
            .map_err(NdnError::engine)
    }
}

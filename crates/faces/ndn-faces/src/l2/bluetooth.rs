//! BLE face implementing the NDNts `@ndn/web-bluetooth-transport` protocol.
//!
//! # Protocol specification
//!
//! This face implements the same BLE GATT profile as the NDNts
//! `@ndn/web-bluetooth-transport` package and the ESP32 `esp8266ndn`
//! `BleServerTransport`, making it interoperable with web browsers (via the
//! Web Bluetooth API) and ESP32/Arduino devices.
//!
//! ## GATT profile
//!
//! | Role | Detail |
//! |------|--------|
//! | GATT role | **Server** (forwarder acts as peripheral) |
//! | Service UUID | `099577e3-0788-412a-8824-395084d97391` |
//! | TX characteristic (forwarder → client) | `cc5abb89-a541-46d8-a351-2d95a8a1a374` (Notify) |
//! | RX characteristic (client → forwarder) | `972f9527-0d83-4261-b95d-b7b2a9e5007b` (Write Without Response) |
//!
//! ## Framing
//!
//! Each BLE Write/Notify carries exactly one NDN packet. If the packet exceeds
//! the negotiated ATT MTU (typically 23–517 bytes), it is fragmented using the
//! NDNts BLE fragmentation scheme:
//!
//! - Fragmented packets are prefixed with a 1-byte sequence/fragment header.
//! - The high bit (`0x80`) indicates "first fragment"; subsequent fragments
//!   increment a 7-bit counter.
//! - The receiver reassembles on the RX path before passing to the pipeline.
//!
//! ## MTU negotiation
//!
//! On connection, the forwarder requests the maximum ATT MTU (512 bytes) via
//! `exchange_mtu`. The effective payload per fragment is `ATT_MTU - 3` bytes
//! (ATT header overhead).
//!
//! # Implementation status
//!
//! **Stub only** — returns [`FaceError::Closed`] on all operations.
//! Full implementation (GATT server, MTU negotiation, fragmentation) is
//! targeted for **v0.2.0** pending a stable async BLE GATT crate for Linux
//! (candidates: `bluer`, `btleplug` with async GATT support).
//!
//! # References
//!
//! - NDNts source: `packages/web-bluetooth-transport`
//! - ESP32 source: `esp8266ndn` library `BleServerTransport`
//! - NDN BLE spec discussion: <https://github.com/named-data/ndn-cxx/issues/5131>

use bytes::Bytes;
use ndn_transport::{Face, FaceError, FaceId, FaceKind};

// ── GATT UUIDs ───────────────────────────────────────────────────────────────

/// Primary GATT service UUID for the NDN BLE transport.
pub const BLE_SERVICE_UUID: &str = "099577e3-0788-412a-8824-395084d97391";

/// TX characteristic UUID — forwarder notifies client of outgoing NDN packets.
pub const BLE_TX_CHAR_UUID: &str = "cc5abb89-a541-46d8-a351-2d95a8a1a374";

/// RX characteristic UUID — client writes incoming NDN packets to the forwarder.
pub const BLE_RX_CHAR_UUID: &str = "972f9527-0d83-4261-b95d-b7b2a9e5007b";

// ── Face stub ────────────────────────────────────────────────────────────────

/// NDN face over Bluetooth LE using the NDNts `@ndn/web-bluetooth-transport`
/// GATT profile.
///
/// Interoperable with:
/// - Web browsers via the Web Bluetooth API + NDNts
/// - ESP32 devices running `esp8266ndn` `BleServerTransport`
///
/// **This is a stub.** All `Face` operations return [`FaceError::Closed`].
/// Full implementation is targeted for v0.2.0.
pub struct BleFace {
    id: FaceId,
}

impl BleFace {
    /// Create a new stub `BleFace` with the given ID.
    pub fn new(id: FaceId) -> Self {
        Self { id }
    }
}

impl Face for BleFace {
    fn id(&self) -> FaceId {
        self.id
    }

    fn kind(&self) -> FaceKind {
        FaceKind::Bluetooth
    }

    async fn recv(&self) -> Result<Bytes, FaceError> {
        // Stub: BLE GATT implementation pending for v0.2.0.
        std::future::pending::<()>().await;
        Err(FaceError::Closed)
    }

    async fn send(&self, _pkt: Bytes) -> Result<(), FaceError> {
        // Stub: BLE GATT implementation pending for v0.2.0.
        Err(FaceError::Closed)
    }
}

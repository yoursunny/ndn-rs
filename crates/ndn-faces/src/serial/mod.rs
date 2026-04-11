//! # `ndn_faces::serial` — Serial port faces
//!
//! NDN face transport over serial (UART) links, suitable for embedded and IoT
//! deployments. Uses COBS (Consistent Overhead Byte Stuffing) framing to
//! delimit NDN packets on a byte stream.
//!
//! ## Key types
//!
//! - [`SerialFace`] — a `StreamFace` alias for serial port transport
//! - [`serial_face_open()`] — opens a serial port as an NDN face (requires the `serial` feature)
//! - [`cobs::CobsCodec`] — COBS framing codec

#![allow(missing_docs)]

pub mod cobs;
#[allow(clippy::module_inception)]
pub mod serial;

pub use serial::SerialFace;
#[cfg(feature = "serial")]
pub use serial::serial_face_open;

//! # ndn-face-serial — Serial port faces for NDN
//!
//! Provides NDN face transport over serial (UART) links, suitable for
//! embedded and IoT deployments.
//!
//! ## Key types
//!
//! - [`SerialFace`] — a `StreamFace` alias for serial port transport
//! - [`serial_face_open()`] — opens a serial port as an NDN face (requires the `serial` feature)
//! - [`cobs::CobsCodec`] — COBS (Consistent Overhead Byte Stuffing) framing codec
//!
//! ## Features
//!
//! - **`serial`** (default) — enables hardware serial port support via `tokio-serial`.

#![allow(missing_docs)]

pub mod cobs;
pub mod serial;

pub use serial::SerialFace;
#[cfg(feature = "serial")]
pub use serial::serial_face_open;

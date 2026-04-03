use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::trace;

use ndn_transport::{Face, FaceError, FaceId, FaceKind};

use crate::cobs::CobsCodec;

/// NDN face over a serial port with COBS framing.
///
/// COBS (Consistent Overhead Byte Stuffing) provides reliable frame
/// resynchronisation after line noise — a `0x00` byte never appears in the
/// encoded payload, so re-sync is just a matter of waiting for the next `0x00`.
///
/// Suitable for: UART sensor nodes, LoRa radio modems, RS-485 industrial buses.
///
/// The serial stream is split into independent read and write halves, each behind
/// its own `Mutex` — mirroring the `TcpFace` pattern.
#[cfg(feature = "serial")]
pub struct SerialFace {
    id:     FaceId,
    port:   String,
    baud:   u32,
    reader: Mutex<FramedRead<ReadHalf<tokio_serial::SerialStream>, CobsCodec>>,
    writer: Mutex<FramedWrite<WriteHalf<tokio_serial::SerialStream>, CobsCodec>>,
}

#[cfg(feature = "serial")]
impl SerialFace {
    /// Open a serial port and wrap it as an NDN face.
    pub fn open(id: FaceId, port: impl Into<String>, baud: u32) -> std::io::Result<Self> {
        let port = port.into();
        let builder = tokio_serial::new(&port, baud);
        let stream = tokio_serial::SerialStream::open(&builder)?;
        let (r, w) = tokio::io::split(stream);
        Ok(Self {
            id,
            port,
            baud,
            reader: Mutex::new(FramedRead::new(r, CobsCodec::new())),
            writer: Mutex::new(FramedWrite::new(w, CobsCodec::new())),
        })
    }

    pub fn port(&self) -> &str { &self.port }
    pub fn baud(&self) -> u32 { self.baud }
}

#[cfg(feature = "serial")]
impl Face for SerialFace {
    fn id(&self) -> FaceId { self.id }
    fn kind(&self) -> FaceKind { FaceKind::Serial }

    fn remote_uri(&self) -> Option<String> {
        Some(format!("serial://{}", self.port))
    }

    fn local_uri(&self) -> Option<String> {
        Some(format!("serial://{}", self.port))
    }

    async fn recv(&self) -> Result<Bytes, FaceError> {
        let mut reader = self.reader.lock().await;
        let data = reader
            .next()
            .await
            .ok_or(FaceError::Closed)?
            .map_err(FaceError::Io)?;
        trace!(face=%self.id, port=%self.port, len=data.len(), "serial: recv");
        Ok(data)
    }

    async fn send(&self, pkt: Bytes) -> Result<(), FaceError> {
        let wire = ndn_packet::lp::encode_lp_packet(&pkt);
        trace!(face=%self.id, port=%self.port, len=wire.len(), "serial: send");
        let mut writer = self.writer.lock().await;
        writer.send(wire).await.map_err(FaceError::Io)
    }
}

// ─── Fallback when `serial` feature is disabled ────────────────────────────

#[cfg(not(feature = "serial"))]
pub struct SerialFace {
    id:   FaceId,
    port: String,
    baud: u32,
}

#[cfg(not(feature = "serial"))]
impl SerialFace {
    pub fn new(id: FaceId, port: impl Into<String>, baud: u32) -> Self {
        Self { id, port: port.into(), baud }
    }
}

#[cfg(not(feature = "serial"))]
impl Face for SerialFace {
    fn id(&self) -> FaceId { self.id }
    fn kind(&self) -> FaceKind { FaceKind::Serial }

    async fn recv(&self) -> Result<Bytes, FaceError> {
        Err(FaceError::Closed)
    }

    async fn send(&self, _pkt: Bytes) -> Result<(), FaceError> {
        Err(FaceError::Closed)
    }
}

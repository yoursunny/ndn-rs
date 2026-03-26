use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

use ndn_transport::{Face, FaceError, FaceId, FaceKind};

/// NDN face over UDP.
///
/// `send` is `&self`-safe because `UdpSocket::send_to` takes `&self`.
/// `recv` is only called from the face's own task.
pub struct UdpFace {
    id:     FaceId,
    socket: Arc<UdpSocket>,
    peer:   SocketAddr,
}

impl UdpFace {
    pub async fn bind(local: SocketAddr, peer: SocketAddr, id: FaceId) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(local).await?;
        Ok(Self { id, socket: Arc::new(socket), peer })
    }
}

impl Face for UdpFace {
    fn id(&self) -> FaceId { self.id }
    fn kind(&self) -> FaceKind { FaceKind::Udp }

    async fn recv(&self) -> Result<Bytes, FaceError> {
        let mut buf = vec![0u8; 9000];
        let (n, _addr) = self.socket.recv_from(&mut buf).await?;
        buf.truncate(n);
        Ok(Bytes::from(buf))
    }

    async fn send(&self, pkt: Bytes) -> Result<(), FaceError> {
        self.socket.send_to(&pkt, self.peer).await?;
        Ok(())
    }
}

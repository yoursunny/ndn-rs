pub mod multicast;
pub mod reliability;
pub mod tcp;
pub mod udp;

#[cfg(feature = "websocket")]
pub mod websocket;

pub use multicast::MulticastUdpFace;
pub use ndn_packet::fragment::DEFAULT_UDP_MTU;
pub use reliability::{LpReliability, ReliabilityConfig, RtoStrategy};
pub use tcp::{TcpFace, tcp_face_connect, tcp_face_from_stream};
pub use udp::UdpFace;

#[cfg(feature = "websocket")]
pub use websocket::WebSocketFace;

use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicU32, Ordering};

use bytes::Bytes;
use ndn_packet::Name;
use ndn_transport::{Face, FaceError, FaceId, FaceKind};
use tokio::io::unix::AsyncFd;

use crate::radio::RadioFaceMetadata;
use crate::NDN_ETHERTYPE;

/// MAC address (6 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacAddr([u8; 6]);

impl MacAddr {
    pub const BROADCAST: MacAddr = MacAddr([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);

    pub fn new(bytes: [u8; 6]) -> Self { Self(bytes) }
    pub fn as_bytes(&self) -> &[u8; 6] { &self.0 }
}

impl std::fmt::Display for MacAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b = &self.0;
        write!(f, "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
               b[0], b[1], b[2], b[3], b[4], b[5])
    }
}

// ─── AF_PACKET helpers ───────────────────────────────────────────────────────

/// Look up the interface index for `iface` via `SIOCGIFINDEX`.
fn get_ifindex(fd: RawFd, iface: &str) -> std::io::Result<i32> {
    let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
    let name_bytes = iface.as_bytes();
    if name_bytes.len() >= libc::IFNAMSIZ {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "interface name too long",
        ));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(
            name_bytes.as_ptr(),
            ifr.ifr_name.as_mut_ptr() as *mut u8,
            name_bytes.len(),
        );
    }
    if unsafe { libc::ioctl(fd, libc::SIOCGIFINDEX as libc::c_ulong, &mut ifr) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { ifr.ifr_ifru.ifru_ifindex })
}

/// Build a `sockaddr_ll` for `bind` or `sendto`.
fn make_sockaddr_ll(ifindex: i32, dst_mac: &MacAddr, protocol: u16) -> libc::sockaddr_ll {
    let mut addr: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
    addr.sll_family   = libc::AF_PACKET as u16;
    addr.sll_protocol = protocol.to_be();
    addr.sll_ifindex  = ifindex;
    addr.sll_halen    = 6;
    addr.sll_addr[..6].copy_from_slice(dst_mac.as_bytes());
    addr
}

/// Create an `AF_PACKET + SOCK_DGRAM` socket bound to `ifindex`, filtering
/// only frames with ethertype `protocol`.  Returns a non-blocking `OwnedFd`.
fn open_packet_socket(ifindex: i32, protocol: u16) -> std::io::Result<OwnedFd> {
    let fd = unsafe {
        libc::socket(
            libc::AF_PACKET,
            libc::SOCK_DGRAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            (protocol as u16).to_be() as i32,
        )
    };
    if fd == -1 { return Err(std::io::Error::last_os_error()); }
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };

    // Bind to the specific interface so we only receive frames from it.
    let bind_addr = make_sockaddr_ll(ifindex, &MacAddr::new([0; 6]), protocol);
    if unsafe {
        libc::bind(
            owned.as_raw_fd(),
            &bind_addr as *const libc::sockaddr_ll as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
        )
    } == -1 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(owned)
}

// ─── TPACKET_V2 mmap ring buffer ────────────────────────────────────────────

// TPACKET_V2 lives inside libc's `tpacket_versions` enum.
const TPACKET_V2: libc::c_int = 1;

const TPACKET_ALIGNMENT: usize = 16;

const fn tpacket_align(x: usize) -> usize {
    (x + TPACKET_ALIGNMENT - 1) & !(TPACKET_ALIGNMENT - 1)
}

// Ring geometry — each ring gets BLOCK_NR × BLOCK_SIZE bytes.
// 2048-byte frames fit tpacket2_hdr (32 B) + sockaddr_ll (20 B) + 1500 B payload.
const RING_FRAME_SIZE: u32 = 2048;
const RING_BLOCK_SIZE: u32 = 1 << 12; // 4 KiB (one page, 2 frames/block)
const RING_BLOCK_NR:   u32 = 32;      // 32 blocks → 128 KiB per ring
const RING_FRAME_NR:   u32 = (RING_BLOCK_SIZE / RING_FRAME_SIZE) * RING_BLOCK_NR; // 64

/// Byte offset from TX frame start to the packet data payload.
///
/// For TPACKET_V2 with a TX ring the kernel reads data from:
///   `frame + TPACKET_ALIGN(sizeof(tpacket2_hdr)) + sizeof(sockaddr_ll)`
/// The 20-byte `sockaddr_ll` between header and payload can carry a per-frame
/// destination address; we leave it zeroed so the kernel uses the `sendto`
/// address instead.
const TX_DATA_OFFSET: usize =
    tpacket_align(std::mem::size_of::<libc::tpacket2_hdr>())
    + std::mem::size_of::<libc::sockaddr_ll>();

// ─── tp_status atomic access ─────────────────────────────────────────────────

/// Read `tp_status` from a ring frame with Acquire ordering.
///
/// Acquire ensures that all payload bytes written by the kernel (RX) or by a
/// prior `try_push_tx` are visible after we observe the status change.
///
/// # Safety
/// `frame` must point to a valid, 16-byte-aligned tpacket2_hdr in the mmap'd
/// ring.  The first 4 bytes of the frame are `tp_status`.
unsafe fn read_tp_status(frame: *mut u8) -> u32 {
    unsafe { (*AtomicU32::from_ptr(frame as *mut u32)).load(Ordering::Acquire) }
}

/// Write `tp_status` with Release ordering.
///
/// Release ensures the kernel (or the next reader) sees all our prior writes
/// (payload data for TX, or the cleared status for RX) before the status
/// itself changes.
///
/// # Safety
/// Same requirements as [`read_tp_status`].
unsafe fn write_tp_status(frame: *mut u8, val: u32) {
    unsafe { (*AtomicU32::from_ptr(frame as *mut u32)).store(val, Ordering::Release) }
}

// ─── PacketRing ──────────────────────────────────────────────────────────────

/// Mmap'd `PACKET_RX_RING` + `PACKET_TX_RING` for zero-copy packet I/O.
///
/// The kernel fills RX frames and sets `tp_status = TP_STATUS_USER`; userspace
/// reads the payload and releases the frame by writing `TP_STATUS_KERNEL`.
/// For TX, userspace writes the payload, sets `tp_status = TP_STATUS_SEND_REQUEST`,
/// then calls `sendto` to trigger the kernel to transmit all pending frames.
struct PacketRing {
    /// Mmap'd region (RX ring at offset 0, TX ring at `tx_offset`).
    map:         *mut u8,
    map_len:     usize,
    frame_size:  usize,
    rx_frame_nr: u32,
    tx_frame_nr: u32,
    /// Byte offset where the TX ring starts within the mmap region.
    tx_offset:   usize,
    /// Current RX consumer index (single consumer — Face::recv is single-task).
    rx_head:     AtomicU32,
    /// Current TX producer index, protected for concurrent Face::send calls.
    tx_head:     std::sync::Mutex<u32>,
}

// Safety: the mmap'd region is shared with the kernel via MAP_SHARED.
// Synchronisation is through atomic tp_status reads/writes with
// Acquire/Release ordering.  rx_head is single-consumer; tx_head is
// protected by a Mutex.
unsafe impl Send for PacketRing {}
unsafe impl Sync for PacketRing {}

impl PacketRing {
    fn rx_frame(&self, idx: u32) -> *mut u8 {
        unsafe { self.map.add(idx as usize * self.frame_size) }
    }

    fn tx_frame(&self, idx: u32) -> *mut u8 {
        unsafe { self.map.add(self.tx_offset + idx as usize * self.frame_size) }
    }

    /// Try to dequeue one packet from the RX ring.
    ///
    /// Returns `None` if no frame is ready (the caller should poll for
    /// readability via `AsyncFd` and retry).
    fn try_pop_rx(&self) -> Option<Bytes> {
        let idx = self.rx_head.load(Ordering::Relaxed);
        let frame = self.rx_frame(idx);

        // Safety: frame is within the mmap'd region.
        let status = unsafe { read_tp_status(frame) };
        if status & libc::TP_STATUS_USER == 0 {
            return None;
        }

        let hdr = frame as *const libc::tpacket2_hdr;
        let tp_mac    = unsafe { (*hdr).tp_mac } as usize;
        let tp_snaplen = unsafe { (*hdr).tp_snaplen } as usize;

        let data = unsafe {
            std::slice::from_raw_parts(frame.add(tp_mac), tp_snaplen)
        };
        let bytes = Bytes::copy_from_slice(data);

        // Release frame back to the kernel.
        unsafe { write_tp_status(frame, libc::TP_STATUS_KERNEL) };
        self.rx_head.store((idx + 1) % self.rx_frame_nr, Ordering::Relaxed);

        Some(bytes)
    }

    /// Try to enqueue one packet into the TX ring.
    ///
    /// Returns `false` if the ring is full (all frames are still being
    /// transmitted by the kernel).
    fn try_push_tx(&self, data: &[u8]) -> bool {
        let mut head = self.tx_head.lock().unwrap();
        let frame = self.tx_frame(*head);

        // TP_STATUS_AVAILABLE == 0: frame is free for userspace to fill.
        let status = unsafe { read_tp_status(frame) };
        if status != 0 {
            return false;
        }

        // Write payload into the frame.
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                frame.add(TX_DATA_OFFSET),
                data.len(),
            );

            // Set length fields so the kernel knows how much to transmit.
            let hdr = frame as *mut libc::tpacket2_hdr;
            (*hdr).tp_len     = data.len() as u32;
            (*hdr).tp_snaplen = data.len() as u32;
        }

        // Signal the kernel (Release ensures payload writes are visible).
        unsafe { write_tp_status(frame, libc::TP_STATUS_SEND_REQUEST) };

        *head = (*head + 1) % self.tx_frame_nr;
        true
    }
}

impl Drop for PacketRing {
    fn drop(&mut self) {
        if !self.map.is_null() {
            unsafe { libc::munmap(self.map as *mut libc::c_void, self.map_len); }
        }
    }
}

/// Configure TPACKET_V2, create RX + TX rings, and mmap them.
fn setup_packet_ring(fd: RawFd) -> std::io::Result<PacketRing> {
    // 1. Select TPACKET_V2.
    let version: libc::c_int = TPACKET_V2;
    setsockopt_val(fd, libc::SOL_PACKET, libc::PACKET_VERSION, &version)?;

    let req = libc::tpacket_req {
        tp_block_size: RING_BLOCK_SIZE,
        tp_block_nr:   RING_BLOCK_NR,
        tp_frame_size: RING_FRAME_SIZE,
        tp_frame_nr:   RING_FRAME_NR,
    };

    // 2. Configure RX ring, then TX ring (same geometry).
    setsockopt_val(fd, libc::SOL_PACKET, libc::PACKET_RX_RING, &req)?;
    setsockopt_val(fd, libc::SOL_PACKET, libc::PACKET_TX_RING, &req)?;

    // 3. Mmap both rings.  The kernel places the TX ring immediately after
    //    the RX ring in the mapping.
    let rx_ring_size = (RING_BLOCK_SIZE as usize) * (RING_BLOCK_NR as usize);
    let tx_ring_size = rx_ring_size;
    let map_len = rx_ring_size + tx_ring_size;

    let map = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            map_len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        )
    };
    if map == libc::MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }

    Ok(PacketRing {
        map: map as *mut u8,
        map_len,
        frame_size:  RING_FRAME_SIZE as usize,
        rx_frame_nr: RING_FRAME_NR,
        tx_frame_nr: RING_FRAME_NR,
        tx_offset:   rx_ring_size,
        rx_head:     AtomicU32::new(0),
        tx_head:     std::sync::Mutex::new(0),
    })
}

fn setsockopt_val<T>(fd: RawFd, level: libc::c_int, name: libc::c_int, val: &T) -> std::io::Result<()> {
    if unsafe {
        libc::setsockopt(
            fd, level, name,
            val as *const T as *const libc::c_void,
            std::mem::size_of::<T>() as libc::socklen_t,
        )
    } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

// ─── NamedEtherFace ──────────────────────────────────────────────────────────

/// NDN face over raw Ethernet (`AF_PACKET` / Ethertype 0x8624).
///
/// Uses `SOCK_DGRAM` with a TPACKET_V2 mmap'd ring buffer for zero-copy
/// packet I/O.  The kernel strips/builds the Ethernet header automatically:
/// `recv()` returns the NDN TLV payload directly; `send()` accepts the NDN
/// TLV payload and the kernel prepends the Ethernet frame header.
///
/// The MAC address is an internal implementation detail — above the face layer
/// everything is NDN names.  The node name is stable across channel switches
/// and radio changes; only the internal MAC binding needs updating on mobility.
///
/// Requires `CAP_NET_RAW` or root.
pub struct NamedEtherFace {
    id:          FaceId,
    /// NDN node name of the remote peer.
    pub node_name: Name,
    /// Resolved MAC address of the remote peer.
    peer_mac:    MacAddr,
    /// Local network interface name.
    iface:       String,
    /// Interface index (cached from constructor).
    ifindex:     i32,
    /// Radio metadata for multi-radio strategies.
    pub radio:   RadioFaceMetadata,
    /// Non-blocking AF_PACKET socket registered with tokio.
    socket:      AsyncFd<OwnedFd>,
    /// Mmap'd TPACKET_V2 RX + TX ring buffers.
    ring:        PacketRing,
}

impl NamedEtherFace {
    /// Create a new Ethernet face bound to `iface`.
    ///
    /// Opens an `AF_PACKET + SOCK_DGRAM` socket, configures a TPACKET_V2 mmap
    /// ring buffer, binds to the given network interface, and registers the
    /// socket with the tokio reactor.  Requires `CAP_NET_RAW`.
    pub fn new(
        id:        FaceId,
        node_name: Name,
        peer_mac:  MacAddr,
        iface:     impl Into<String>,
        radio:     RadioFaceMetadata,
    ) -> std::io::Result<Self> {
        let iface = iface.into();

        // Temporary socket to resolve the interface index.
        let probe_fd = unsafe {
            libc::socket(libc::AF_PACKET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
                         (NDN_ETHERTYPE as u16).to_be() as i32)
        };
        if probe_fd == -1 { return Err(std::io::Error::last_os_error()); }
        let ifindex = {
            let idx = get_ifindex(probe_fd, &iface);
            unsafe { libc::close(probe_fd); }
            idx?
        };

        let fd = open_packet_socket(ifindex, NDN_ETHERTYPE)?;

        // Set up mmap ring buffers BEFORE registering with AsyncFd.
        let ring = setup_packet_ring(fd.as_raw_fd())?;
        let socket = AsyncFd::new(fd)?;

        Ok(Self { id, node_name, peer_mac, iface, ifindex, radio, socket, ring })
    }

    /// Update the peer MAC address (e.g. after a mobility event).
    pub fn set_peer_mac(&mut self, mac: MacAddr) {
        self.peer_mac = mac;
    }

    /// Current peer MAC address.
    pub fn peer_mac(&self) -> MacAddr {
        self.peer_mac
    }

    /// Interface name this face is bound to.
    pub fn iface(&self) -> &str {
        &self.iface
    }
}

impl Face for NamedEtherFace {
    fn id(&self)   -> FaceId   { self.id }
    fn kind(&self) -> FaceKind { FaceKind::Ethernet }

    /// Receive the next NDN packet from the RX ring.
    ///
    /// Checks the TPACKET_V2 ring for a frame with `TP_STATUS_USER`.  If none
    /// is ready, waits for readability on the socket fd (the kernel wakes epoll
    /// when a new frame lands in the ring).
    async fn recv(&self) -> Result<Bytes, FaceError> {
        loop {
            if let Some(pkt) = self.ring.try_pop_rx() {
                return Ok(pkt);
            }
            let mut guard = self.socket.readable().await?;
            guard.clear_ready();
        }
    }

    /// Send an NDN packet via the TX ring.
    ///
    /// Writes the payload into the next available TX frame, marks it
    /// `TP_STATUS_SEND_REQUEST`, then calls `sendto` to flush all pending
    /// frames to the kernel for transmission.
    async fn send(&self, pkt: Bytes) -> Result<(), FaceError> {
        // Wait for an available TX frame.
        loop {
            if self.ring.try_push_tx(&pkt) {
                break;
            }
            let mut guard = self.socket.writable().await?;
            guard.clear_ready();
        }

        // Flush pending TX frames.  The destination address in sockaddr_ll
        // applies to all frames whose per-frame sockaddr_ll has sll_halen == 0
        // (which is the case since the ring is zero-initialised by mmap).
        let dst = make_sockaddr_ll(self.ifindex, &self.peer_mac, NDN_ETHERTYPE);
        let fd = self.socket.get_ref().as_raw_fd();
        let ret = unsafe {
            libc::sendto(
                fd,
                std::ptr::null(),
                0,
                0,
                &dst as *const libc::sockaddr_ll as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            )
        };
        if ret == -1 {
            let err = std::io::Error::last_os_error();
            // EAGAIN means the NIC queue is full; the frame remains in the
            // ring and will be flushed by the next send.
            if err.kind() != std::io::ErrorKind::WouldBlock {
                return Err(FaceError::Io(err));
            }
        }
        Ok(())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_addr_display() {
        let mac = MacAddr::new([0xaa, 0xbb, 0xcc, 0x01, 0x02, 0x03]);
        assert_eq!(format!("{mac}"), "aa:bb:cc:01:02:03");
    }

    #[test]
    fn mac_addr_broadcast() {
        assert_eq!(MacAddr::BROADCAST.as_bytes(), &[0xff; 6]);
    }

    #[test]
    fn sockaddr_ll_layout() {
        let mac = MacAddr::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        let addr = make_sockaddr_ll(3, &mac, NDN_ETHERTYPE);
        assert_eq!(addr.sll_family, libc::AF_PACKET as u16);
        assert_eq!(addr.sll_ifindex, 3);
        assert_eq!(addr.sll_halen, 6);
        assert_eq!(&addr.sll_addr[..6], mac.as_bytes());
        assert_eq!(addr.sll_protocol, NDN_ETHERTYPE.to_be());
    }

    #[test]
    fn ring_geometry() {
        // Verify the ring constants are consistent.
        assert_eq!(
            RING_FRAME_NR,
            (RING_BLOCK_SIZE / RING_FRAME_SIZE) * RING_BLOCK_NR,
        );
        // Frame must fit the tpacket header + 1500 B payload.
        assert!(RING_FRAME_SIZE as usize >= TX_DATA_OFFSET + 1500);
    }

    #[test]
    fn tx_data_offset_is_correct() {
        // TPACKET_ALIGN(sizeof(tpacket2_hdr)) + sizeof(sockaddr_ll)
        let aligned_hdr = tpacket_align(std::mem::size_of::<libc::tpacket2_hdr>());
        let expected = aligned_hdr + std::mem::size_of::<libc::sockaddr_ll>();
        assert_eq!(TX_DATA_OFFSET, expected);
    }

    /// Opening an AF_PACKET socket without CAP_NET_RAW should fail with EPERM.
    #[tokio::test]
    async fn new_fails_without_cap_net_raw() {
        let name = Name::from_str("/test/node").unwrap();
        let result = NamedEtherFace::new(
            FaceId(1), name, MacAddr::BROADCAST, "lo", RadioFaceMetadata::default(),
        );
        if let Err(e) = result {
            let raw = e.raw_os_error().unwrap_or(0);
            assert!(
                raw == libc::EPERM || raw == libc::EACCES,
                "expected EPERM or EACCES, got: {e}"
            );
        }
    }

    /// Full loopback roundtrip — requires root / CAP_NET_RAW.
    #[tokio::test]
    #[ignore = "requires CAP_NET_RAW"]
    async fn loopback_roundtrip() {
        let name = Name::from_str("/test/node").unwrap();
        let lo_mac = MacAddr::new([0; 6]);
        let face_a = NamedEtherFace::new(
            FaceId(1), name.clone(), lo_mac, "lo", RadioFaceMetadata::default(),
        ).expect("need CAP_NET_RAW");
        let face_b = NamedEtherFace::new(
            FaceId(2), name, lo_mac, "lo", RadioFaceMetadata::default(),
        ).expect("need CAP_NET_RAW");

        let pkt = Bytes::from_static(b"\x05\x03\x01\x02\x03");
        face_a.send(pkt.clone()).await.unwrap();

        let received = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            face_b.recv(),
        ).await.expect("timed out").unwrap();

        assert_eq!(received, pkt);
    }
}

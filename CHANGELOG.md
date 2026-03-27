# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project does not yet follow Semantic Versioning — the codebase is in active
bootstrapping phase and all APIs should be considered unstable.

---

## [Unreleased]

### Added

#### `ndn-store` — Layer 4 data structures

- **`NameTrie::first_descendant(prefix: &Name) -> Option<V>`** — depth-first
  search from a prefix node to find any value stored at or below that position.
  Used by `LruCs` for `CanBePrefix` lookups.

- **`Fib` / `FibEntry` / `FibNexthop`** — Forwarding Information Base wrapping
  `NameTrie<Arc<FibEntry>>`. Exposes `lpm`, `get`, `insert`, `add_nexthop`, and
  `remove`. Uses `u32` for `face_id` (consistent with `PIT`) to avoid a
  same-layer cross-dependency on `ndn-transport`.

- **`StrategyTable<S>`** — generic newtype over `NameTrie<Arc<S>>` for
  longest-prefix strategy dispatch. Decoupled from the concrete `Strategy` trait
  (defined in `ndn-strategy`) so it can live in the foundational store layer.

- **`ShardedCs<C: ContentStore>`** — shards any `ContentStore` across `N`
  instances. Shard selection is by first name component so that related content
  (`/video/seg/1`, `/video/seg/2`) lands in the same shard, preserving LRU
  locality for sequential access patterns. `capacity()` aggregates all shards.

#### `ndn-store` — `LruCs` improvements

- **`CanBePrefix` support** — `LruInner` now carries a
  `prefix_index: NameTrie<Arc<Name>>` secondary index. Every insert adds the
  name to the trie; every eviction (explicit or LRU-triggered) removes it.
  `get()` uses `first_descendant` for `CanBePrefix` interests.

- **Fixed linear-scan evict bug** — `evict()` previously did an O(n) scan to
  reconstruct an `Arc<Name>` key. It now calls `cache.pop(name: &Name)` directly
  via `Arc<Name>: Borrow<Name>`.

- **Fixed entry-count limit** — the `LruCache` cap was `capacity_bytes / 64`
  (minimum 1), which starved small capacities of entries before byte-based
  eviction could kick in. Changed to `capacity_bytes.max(1)` so the byte loop
  is always the sole eviction control.

#### `ndn-transport` — Layer 4 transport utilities

- **`TlvCodec`** — `tokio_util::codec` `Decoder` / `Encoder` for NDN TLV stream
  framing. Handles all `varu64` type and length widths. Used by `TcpFace`,
  `UnixFace`, and `SerialFace` (implemented in Layer 3 face crates).

- **`FacePairTable`** — `DashMap`-backed rx→tx `FaceId` mapping for asymmetric
  wfb-ng links. The dispatch stage calls `get_tx_for_rx(in_face).unwrap_or(in_face)`
  so symmetric faces (no table entry) fall through unchanged.

- **`FaceEvent`** — `Opened(FaceId)` / `Closed(FaceId)` lifecycle events.
  Face tasks emit `Closed` when `recv()` returns `FaceError::Closed`; the face
  manager uses this to clean up PIT `OutRecord` entries.

#### `ndn-face-net` — Layer 3 network faces

- **`UdpFace`** — unicast UDP face. The socket is `connect()`-ed to the peer at
  construction, so the kernel filters inbound datagrams. `from_socket` wraps a
  pre-configured socket; `bind` creates and connects one atomically.

- **`TcpFace`** — TCP face with `TlvCodec` stream framing. Stream is split into
  `FramedRead` / `FramedWrite` halves, each behind a `tokio::sync::Mutex`.
  `from_stream` wraps an accepted stream; `connect` opens a new connection.

- **`MulticastUdpFace`** — NDN IPv4 multicast face. Publishes `NDN_MULTICAST_V4`
  (`224.0.23.170`) and `NDN_PORT` (`6363`) constants. `recv` captures datagrams
  from any sender (neighbor discovery); `send` multicasts to the group. The
  multicast loopback roundtrip test is best-effort and skips gracefully in
  sandboxed CI environments.

#### `ndn-face-local` — Layer 3 local faces

- **`UnixFace`** — Unix domain socket face with `TlvCodec` framing. Same
  `FramedRead`/`FramedWrite` + `Mutex` design as `TcpFace`. `connect` opens a
  new connection; `from_stream` wraps an accepted stream. Carries the socket path
  for diagnostics.

- **`AppFace` / `AppHandle`** — in-process face backed by a pair of
  `tokio::sync::mpsc` channels. `AppFace::new(id, buffer)` returns both halves.
  The pipeline holds `AppFace`; the application holds `AppHandle`. Drop either
  side to signal closure (`FaceError::Closed` / `None`).

### Tests added

| Crate | Module | Count |
|-------|--------|------:|
| `ndn-store` | `trie` | 14 |
| `ndn-store` | `fib` | 8 |
| `ndn-store` | `strategy_table` | 7 |
| `ndn-store` | `lru_cs` | 17 |
| `ndn-store` | `sharded_cs` | 9 |
| `ndn-transport` | `tlv_codec` | 8 |
| `ndn-transport` | `face_pair_table` | 6 |
| `ndn-transport` | `face_event` | 2 |
| `ndn-face-net` | `udp` | 4 |
| `ndn-face-net` | `tcp` | 6 |
| `ndn-face-net` | `multicast` | 4 |
| `ndn-face-local` | `unix` | 5 |
| `ndn-face-local` | `app` | 7 |
| **Total new** | | **97** |

Running total across all foundation crates: **191 tests** (94 layer 5 + 71 layer 4 + 26 layer 3), all passing.

---

## [0.0.2] — Layer 5 tests (89cb5e1)

### Added

- Comprehensive test suites for all layer 5 (foundation) crates.
- `ndn-tlv`: 33 tests covering `read_varu64` / `write_varu64` / `varu64_size`
  roundtrips, all four encoding widths, EOF error cases, `TlvReader` zero-copy
  slice identity, `skip_unknown` critical-bit rule, scoped sub-readers, and
  `TlvWriter` nested encoding with multi-level nesting.
- `ndn-packet`: 61 tests covering `Name` display / prefix matching / hashing,
  `Interest` lazy field decode, `Data` signed region / sig value extraction,
  `MetaInfo` content types / freshness, `SignatureInfo` key locator, and `Nack`
  reason codes.

### Fixed

- `Data::sig_value()` returned the full SignatureValue TLV (type + length +
  value bytes) instead of only the value bytes. Now strips the TLV header using
  `TlvReader::read_tlv` before returning.
- `Nack::decode` passed raw value bytes to `Interest::decode` which expects a
  complete outer INTEREST TLV. Now reconstructs the full wire format via
  `TlvWriter::write_tlv(INTEREST, &v)`.
- Added `#[derive(Debug)]` to `Interest`, `Data`, and `Nack` (required by
  `Result::unwrap_err()` in tests).

---

## [0.0.1] — Initial workspace (1e85c1f / d4e89f1 / 19d6d48)

### Added

- Cargo workspace with `resolver = "2"` and 17 library crates + 3 binary crates
  across 6 dependency layers.
- `ndn-tlv`: `read_varu64`, `write_varu64`, `varu64_size`, `TlvReader`
  (zero-copy `Bytes`-backed), `TlvWriter` (nested encoding with 5-byte length
  placeholder), `TlvError`.
- `ndn-packet`: `Name`, `NameComponent`, `Interest` (lazy `OnceLock` fields),
  `Data` (signed region offsets, lazy content / meta / sig fields), `MetaInfo`,
  `SignatureInfo`, `Nack`, `PacketError`, TLV type constants.
- `ndn-store`: `NameTrie` (per-node `RwLock` LPM), `Pit` / `PitEntry` /
  `PitToken` / `InRecord` / `OutRecord`, `ContentStore` trait, `NullCs`, `LruCs`
  (byte-bounded, MustBeFresh).
- `ndn-transport`: `Face` trait, `FaceId`, `FaceKind`, `FaceError`, `FaceTable`
  (DashMap + `ErasedFace` blanket impl), `RawPacket`.
- Stub `lib.rs` files for all upper-layer crates.
- Design documentation split from `design-session.md` into 12 structured
  reference documents under `docs/`.
- `README.md` project landing page.
- `CLAUDE.md` guidance file for Claude Code.

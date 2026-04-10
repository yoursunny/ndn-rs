//! # ndn-filestore — NDN File Transfer Protocol v0.1
//!
//! A library for hosting and fetching files over Named Data Networking, providing
//! an AirDrop-like experience across all platforms via NDN's content-centric model.
//!
//! ## Protocol Overview
//!
//! Files are hosted under a node's namespace:
//!
//! ```text
//! /<node-prefix>/ndn-ft/v0/catalog              → segmented JSON catalog of hosted files
//! /<node-prefix>/ndn-ft/v0/file/<file-id>/meta  → file metadata (JSON)
//! /<node-prefix>/ndn-ft/v0/file/<file-id>/<seg> → file segment (binary Data)
//! /<node-prefix>/ndn-ft/v0/notify               → incoming transfer offers
//! ```
//!
//! ## Transfer Flow
//!
//! **Sender side:**
//! 1. Register file with [`FileServer`], optionally pre-chunk and sign.
//! 2. Call [`FileServer::offer`] with the target node's NDN prefix.
//! 3. Target receives a notification Interest at its `/notify` prefix.
//! 4. If the target accepts, it fetches metadata then segments.
//!
//! **Receiver side:**
//! 1. Start [`FileClient::listen`] to handle incoming offers.
//! 2. Implement [`OfferHandler`] to accept/reject; accepted offers auto-download.
//! 3. Alternatively, use [`FileClient::browse`] to explore a remote catalog.
//!
//! ## Example
//!
//! ```no_run
//! # use ndn_filestore::{FileServer, FileClient, HostOpts, TransferConfig};
//! # use ndn_packet::Name;
//! # use std::path::Path;
//! # async fn example() -> anyhow::Result<()> {
//! // Host a file.
//! let mut server = FileServer::connect("/tmp/ndn.sock", "/alice/node1").await?;
//! let id = server.host(Path::new("/home/alice/photo.jpg"), HostOpts::default()).await?;
//!
//! // Offer it to Bob.
//! let target: Name = "/bob/node1".parse()?;
//! let accepted = server.offer(&target, &id, None).await?;
//! if accepted { println!("Bob accepted the file!"); }
//!
//! // On Bob's side — listen for offers and auto-accept.
//! let mut client = FileClient::connect("/tmp/ndn.sock", "/bob/node1").await?;
//! client.listen(Path::new("/home/bob/Downloads"), |offer| {
//!     println!("Incoming file: {} ({} bytes)", offer.name, offer.size);
//!     true // accept
//! }).await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

pub mod client;
pub mod protocol;
pub mod server;
pub mod types;

pub use client::FileClient;
pub use server::FileServer;
pub use types::{
    EncryptionMode, FileId, FileMetadata, FileOffer, HostOpts, SigningMode, TransferConfig,
};

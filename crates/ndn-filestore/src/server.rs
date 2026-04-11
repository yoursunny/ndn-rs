//! Server side of the NDN File Transfer Protocol.
//!
//! [`FileServer`] hosts files and sends transfer offers to remote nodes.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use bytes::Bytes;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use ndn_ipc::ForwarderClient;
use ndn_packet::{Interest, Name};
use ndn_packet::encode::{DataBuilder, InterestBuilder};

use crate::protocol::{self, DEFAULT_SEGMENT_SIZE};
use crate::types::{FileId, FileMetadata, FileOffer, HostOpts, OfferResponse, SigningMode};

// ── Hosted file entry ────────────────────────────────────────────────────────

struct HostedFile {
    meta: FileMetadata,
    /// Optionally pre-built segment wires (None = build on demand).
    segments: Option<Vec<Bytes>>,
    /// Raw content (used for on-demand chunking).
    content: Option<Bytes>,
    /// Segment size for on-demand chunking.
    segment_size: usize,
    freshness_ms: u64,
}

impl HostedFile {
    /// Return the wire for segment `idx`, building on demand if needed.
    fn segment_wire(&self, idx: usize) -> Option<Bytes> {
        let meta = &self.meta;
        let last = meta.segments as usize - 1;

        if let Some(ref pre) = self.segments {
            return pre.get(idx).cloned();
        }

        // On-demand chunking.
        let content = self.content.as_ref()?;
        let start = idx * self.segment_size;
        if start >= content.len() {
            return None;
        }
        let end = (start + self.segment_size).min(content.len());
        let chunk = content.slice(start..end);

        let seg_name = protocol::file_segment_name(
            &meta.sender_prefix.parse().ok()?,
            &meta.id,
            idx,
        );
        let mut builder = DataBuilder::new(seg_name, &chunk)
            .final_block_id_seg(last);
        if self.freshness_ms > 0 {
            builder = builder.freshness(Duration::from_millis(self.freshness_ms));
        }
        Some(builder.build())
    }
}

// ── FileServer ───────────────────────────────────────────────────────────────

/// Hosts files and sends transfer offers to remote nodes.
pub struct FileServer {
    client: ForwarderClient,
    node_prefix: Name,
    files: Arc<RwLock<HashMap<FileId, HostedFile>>>,
}

impl FileServer {
    /// Connect to a running `ndn-router` and start hosting.
    pub async fn connect(socket: impl AsRef<std::path::Path>, node_prefix: &str) -> Result<Self> {
        let client = ForwarderClient::connect(socket).await?;
        let node_prefix: Name = node_prefix.parse().map_err(|e| anyhow::anyhow!("{e}"))?;

        // Register the protocol prefix.
        let base = protocol::base_prefix(&node_prefix);
        client.register_prefix(&base).await?;
        info!("ndn-filestore: registered {base}");

        Ok(Self {
            client,
            node_prefix,
            files: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Add a file to the content store and begin serving it.
    ///
    /// Returns the [`FileId`] (SHA-256 hex of the raw content).
    pub async fn host(&self, path: &Path, opts: HostOpts) -> Result<FileId> {
        let content = tokio::fs::read(path)
            .await
            .with_context(|| format!("reading {}", path.display()))?;

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Compute SHA-256 and file ID.
        let sha256_bytes = Sha256::digest(&content);
        let sha256_hex = hex::encode(sha256_bytes);
        let file_id = format!("sha256-{}", &sha256_hex[..16]);

        let segment_size = if opts.segment_size > 0 { opts.segment_size } else { DEFAULT_SEGMENT_SIZE };
        let segments_count = (content.len() + segment_size - 1) / segment_size;

        let mime = mime_guess::from_path(path).first().map(|m| m.to_string());

        let meta = FileMetadata {
            id: file_id.clone(),
            name: file_name,
            size: content.len() as u64,
            segments: segments_count as u32,
            segment_size: segment_size as u32,
            sha256: sha256_hex,
            mime,
            sender_prefix: self.node_prefix.to_string(),
            ts: unix_now(),
            signing: opts.signing,
            encryption: opts.encryption,
        };

        let content = Bytes::from(content);

        let hosted = if opts.pre_chunk {
            // Pre-build all segment wires.
            let last = segments_count - 1;
            let node_pfx = &self.node_prefix;
            let fid = &file_id;
            let fw = opts.freshness_ms;
            let wires: Vec<Bytes> = (0..segments_count)
                .map(|i| {
                    let start = i * segment_size;
                    let end = (start + segment_size).min(content.len());
                    let chunk = content.slice(start..end);
                    let seg_name = protocol::file_segment_name(node_pfx, fid, i);
                    let mut b = DataBuilder::new(seg_name, &chunk).final_block_id_seg(last);
                    if fw > 0 { b = b.freshness(Duration::from_millis(fw)); }
                    b.build()
                })
                .collect();
            HostedFile {
                meta: meta.clone(),
                segments: Some(wires),
                content: None,
                segment_size,
                freshness_ms: opts.freshness_ms,
            }
        } else {
            HostedFile {
                meta: meta.clone(),
                segments: None,
                content: Some(content),
                segment_size,
                freshness_ms: opts.freshness_ms,
            }
        };

        self.files.write().await.insert(file_id.clone(), hosted);
        info!("ndn-filestore: hosting {} ({} segs)", meta.name, segments_count);

        Ok(file_id)
    }

    /// Remove a hosted file.
    pub async fn remove(&self, id: &FileId) -> bool {
        self.files.write().await.remove(id).is_some()
    }

    /// List all currently hosted files.
    pub async fn list(&self) -> Vec<FileMetadata> {
        self.files
            .read()
            .await
            .values()
            .map(|f| f.meta.clone())
            .collect()
    }

    /// Send a transfer offer to a remote node and wait for acceptance.
    ///
    /// Returns `true` if the receiver accepted, `false` if rejected.
    pub async fn offer(
        &self,
        target_prefix: &Name,
        file_id: &FileId,
        timeout_ms: Option<u64>,
    ) -> Result<bool> {
        let files = self.files.read().await;
        let hosted = files
            .get(file_id)
            .ok_or_else(|| anyhow::anyhow!("file {file_id} not hosted"))?;

        let offer = FileOffer {
            version: 1,
            meta: hosted.meta.clone(),
        };
        let offer_json = serde_json::to_vec(&offer)?;

        let notify_name = protocol::notify_name(target_prefix);
        let timeout = Duration::from_millis(timeout_ms.unwrap_or(30_000));

        info!("ndn-filestore: offering {} to {target_prefix}", hosted.meta.name);

        let wire = InterestBuilder::new(notify_name)
            .app_parameters(offer_json)
            .lifetime(timeout)
            .build();

        self.client.send(wire).await?;

        match tokio::time::timeout(
            timeout + Duration::from_millis(500),
            self.client.recv(),
        )
        .await
        {
            Ok(Some(raw)) => {
                if let Ok(data) = ndn_packet::Data::decode(raw) {
                    if let Some(content) = data.content() {
                        if let Ok(resp) = serde_json::from_slice::<OfferResponse>(content) {
                            if resp.accept {
                                info!("ndn-filestore: offer accepted by {target_prefix}");
                            } else {
                                info!(
                                    "ndn-filestore: offer rejected by {target_prefix}: {:?}",
                                    resp.reason
                                );
                            }
                            return Ok(resp.accept);
                        }
                    }
                }
                warn!("ndn-filestore: malformed acceptance response");
                Ok(false)
            }
            Ok(None) => anyhow::bail!("connection closed while waiting for acceptance"),
            Err(_) => {
                warn!("ndn-filestore: offer to {target_prefix} timed out");
                Ok(false)
            }
        }
    }

    /// Run the serve loop, handling Interests for hosted files.
    ///
    /// This blocks until the connection is closed. Spawn it in a background task
    /// if you need to call other methods concurrently:
    ///
    /// ```no_run
    /// # use ndn_filestore::FileServer;
    /// # async fn example(server: FileServer) {
    /// tokio::spawn(async move { server.serve().await });
    /// # }
    /// ```
    pub async fn serve(self) -> Result<()> {
        info!("ndn-filestore: serving on {}", self.node_prefix);

        loop {
            let raw = match self.client.recv().await {
                Some(b) => b,
                None => {
                    info!("ndn-filestore: connection closed");
                    break;
                }
            };

            let interest = match Interest::decode(raw) {
                Ok(i) => i,
                Err(_) => continue,
            };

            if let Err(e) = self.handle_interest(&interest).await {
                warn!("ndn-filestore: error handling {}: {e}", interest.name);
            }
        }

        Ok(())
    }

    async fn handle_interest(&self, interest: &Interest) -> Result<()> {
        let base = protocol::base_prefix(&self.node_prefix);
        let base_len = base.components().len();
        let components: Vec<_> = interest.name.components().into_iter().collect();

        if components.len() <= base_len {
            return Ok(());
        }

        let sub: Vec<&str> = components[base_len..]
            .iter()
            .map(|c| std::str::from_utf8(&c.value).unwrap_or("?"))
            .collect();

        match sub.as_slice() {
            // /<node>/ndn-ft/v0/catalog/<seg>
            ["catalog", seg_str] => {
                let seg: usize = seg_str.parse().unwrap_or(0);
                self.serve_catalog(interest, seg).await?;
            }

            // /<node>/ndn-ft/v0/file/<id>/meta
            ["file", file_id, "meta"] => {
                self.serve_meta(interest, file_id).await?;
            }

            // /<node>/ndn-ft/v0/file/<id>/<seg>
            ["file", file_id, seg_str] => {
                let seg: usize = seg_str.parse().unwrap_or(0);
                self.serve_segment(interest, file_id, seg).await?;
            }

            _ => {
                debug!("ndn-filestore: unhandled Interest: {}", interest.name);
            }
        }

        Ok(())
    }

    async fn serve_catalog(&self, interest: &Interest, seg: usize) -> Result<()> {
        let files = self.files.read().await;
        let catalog: Vec<&FileMetadata> = files.values().map(|f| &f.meta).collect();
        let json = serde_json::to_vec(&catalog)?;

        // Simple single-segment catalog (extend to chunked for large catalogs).
        if seg == 0 {
            let data = DataBuilder::new((*interest.name).clone(), &json)
                .final_block_id_seg(0)
                .freshness(Duration::from_secs(30))
                .build();
            self.client.send(data).await?;
        }
        Ok(())
    }

    async fn serve_meta(&self, interest: &Interest, file_id: &str) -> Result<()> {
        let files = self.files.read().await;
        if let Some(hosted) = files.get(file_id) {
            let json = serde_json::to_vec(&hosted.meta)?;
            let data = DataBuilder::new((*interest.name).clone(), &json)
                .freshness(Duration::from_secs(300))
                .build();
            self.client.send(data).await?;
        }
        Ok(())
    }

    async fn serve_segment(&self, interest: &Interest, file_id: &str, seg: usize) -> Result<()> {
        let files = self.files.read().await;
        if let Some(hosted) = files.get(file_id) {
            if let Some(wire) = hosted.segment_wire(seg) {
                self.client.send(wire).await?;
            }
        }
        Ok(())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Tiny hex encoder (avoids pulling in the hex crate just for this).
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}

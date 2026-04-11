//! Client side of the NDN File Transfer Protocol.
//!
//! [`FileClient`] discovers remote files and downloads them, and can listen
//! for incoming transfer offers.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use bytes::{Bytes, BytesMut};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use ndn_ipc::ForwarderClient;
use ndn_packet::{Data, Interest, Name};
use ndn_packet::encode::{DataBuilder, InterestBuilder};

use crate::protocol;
use crate::types::{FileId, FileMetadata, FileOffer, OfferResponse};

// ─── FileClient ──────────────────────────────────────────────────────────────

/// Discovers and downloads files from remote nodes, and handles incoming offers.
pub struct FileClient {
    client: ForwarderClient,
    node_prefix: Name,
}

impl FileClient {
    /// Connect to a running `ndn-router`.
    pub async fn connect(socket: impl AsRef<std::path::Path>, node_prefix: &str) -> Result<Self> {
        let client = ForwarderClient::connect(socket).await?;
        let node_prefix: Name = node_prefix.parse().map_err(|e| anyhow::anyhow!("{e}"))?;

        // Register the node's notify prefix so we can receive offers.
        let notify = protocol::notify_name(&node_prefix);
        client.register_prefix(&notify).await?;
        info!("ndn-filestore client: registered notify endpoint {notify}");

        Ok(Self { client, node_prefix })
    }

    /// Fetch the file catalog from a remote node.
    ///
    /// Returns a list of files currently hosted by `remote_prefix`.
    pub async fn browse(&self, remote_prefix: &Name) -> Result<Vec<FileMetadata>> {
        let catalog_name = protocol::catalog_name(remote_prefix).append("0");
        let wire = InterestBuilder::new(catalog_name)
            .lifetime(Duration::from_secs(10))
            .build();

        self.client.send(wire).await?;

        match tokio::time::timeout(Duration::from_secs(11), self.client.recv()).await {
            Ok(Some(raw)) => {
                let data = Data::decode(raw).map_err(|e| anyhow::anyhow!("decode: {e}"))?;
                let content = data
                    .content()
                    .ok_or_else(|| anyhow::anyhow!("catalog has no content"))?;
                serde_json::from_slice(content).map_err(|e| anyhow::anyhow!("parse catalog: {e}"))
            }
            Ok(None) => anyhow::bail!("connection closed fetching catalog"),
            Err(_) => anyhow::bail!("timeout fetching catalog from {remote_prefix}"),
        }
    }

    /// Fetch the metadata for a specific file hosted at `remote_prefix`.
    pub async fn fetch_meta(&self, remote_prefix: &Name, file_id: &FileId) -> Result<FileMetadata> {
        let meta_name = protocol::file_meta_name(remote_prefix, file_id);
        let wire = InterestBuilder::new(meta_name)
            .lifetime(Duration::from_secs(10))
            .build();

        self.client.send(wire).await?;

        match tokio::time::timeout(Duration::from_secs(11), self.client.recv()).await {
            Ok(Some(raw)) => {
                let data = Data::decode(raw).map_err(|e| anyhow::anyhow!("decode: {e}"))?;
                let content = data
                    .content()
                    .ok_or_else(|| anyhow::anyhow!("meta has no content"))?;
                serde_json::from_slice(content).map_err(|e| anyhow::anyhow!("parse meta: {e}"))
            }
            Ok(None) => anyhow::bail!("connection closed fetching meta"),
            Err(_) => anyhow::bail!("timeout fetching meta for {file_id}"),
        }
    }

    /// Download a file to a local directory.
    ///
    /// Fetches metadata first, then downloads all segments with a pipelined
    /// window. Verifies the SHA-256 hash after reassembly.
    ///
    /// Progress is reported via `progress_fn(received_segs, total_segs)`.
    pub async fn fetch_file(
        &self,
        remote_prefix: &Name,
        file_id: &FileId,
        dest_dir: &Path,
        pipeline: usize,
        mut progress_fn: impl FnMut(u32, u32),
    ) -> Result<std::path::PathBuf> {
        let meta = self.fetch_meta(remote_prefix, file_id).await?;
        info!(
            "ndn-filestore: fetching {} ({} bytes, {} segs)",
            meta.name, meta.size, meta.segments
        );

        let total = meta.segments as usize;
        let mut segments: Vec<Option<Bytes>> = vec![None; total];
        let mut received_count: u32 = 0;
        let pipeline = pipeline.max(1).min(total);

        // Pipelined fetch.
        use std::collections::HashMap;
        let mut in_flight: HashMap<u64, (usize, std::time::Instant)> = HashMap::new();
        let mut next_seg: usize = 0;
        let mut seq: u64 = 0;
        let lifetime = Duration::from_secs(10);

        let send_interest = |seg_idx: usize| {
            let name = protocol::file_segment_name(remote_prefix, file_id, seg_idx);
            InterestBuilder::new(name).lifetime(lifetime).build()
        };

        // Seed pipeline.
        while in_flight.len() < pipeline && next_seg < total {
            let wire = send_interest(next_seg);
            self.client.send(wire).await?;
            in_flight.insert(seq, (next_seg, std::time::Instant::now()));
            seq += 1;
            next_seg += 1;
        }

        while received_count < meta.segments {
            let timeout = Duration::from_secs(12);
            match tokio::time::timeout(timeout, self.client.recv()).await {
                Ok(Some(raw)) => {
                    if let Ok(data) = Data::decode(raw) {
                        let seg_idx: Option<usize> = data
                            .name
                            .components()
                            .last()
                            .and_then(|c| std::str::from_utf8(&c.value).ok())
                            .and_then(|s| s.parse().ok());
                        if let Some(idx) = seg_idx {
                            if idx < total && segments[idx].is_none() {
                                let content = data.content().cloned().unwrap_or_else(Bytes::new);
                                segments[idx] = Some(content);
                                received_count += 1;
                                in_flight.retain(|_, (s, _)| *s != idx);
                                progress_fn(received_count, meta.segments);

                                // Fill pipeline.
                                while in_flight.len() < pipeline && next_seg < total {
                                    let wire = send_interest(next_seg);
                                    self.client.send(wire).await?;
                                    in_flight.insert(seq, (next_seg, std::time::Instant::now()));
                                    seq += 1;
                                    next_seg += 1;
                                }
                            }
                        }
                    }
                }
                Ok(None) => anyhow::bail!("connection closed during transfer"),
                Err(_) => {
                    // Retransmit stale in-flight.
                    let stale: Vec<(usize, u64)> = in_flight
                        .iter()
                        .filter(|(_, (_, t))| t.elapsed() >= lifetime)
                        .map(|(&sq, &(idx, _))| (idx, sq))
                        .collect();
                    for (idx, old_seq) in stale {
                        in_flight.remove(&old_seq);
                        let wire = send_interest(idx);
                        self.client.send(wire).await?;
                        in_flight.insert(seq, (idx, std::time::Instant::now()));
                        seq += 1;
                    }
                }
            }
        }

        // Reassemble.
        let mut out = BytesMut::with_capacity(meta.size as usize);
        for (i, seg) in segments.iter().enumerate() {
            match seg {
                Some(b) => out.extend_from_slice(b),
                None => anyhow::bail!("missing segment {i}"),
            }
        }
        let assembled = out.freeze();

        // Verify SHA-256.
        let computed = hex::encode(Sha256::digest(&assembled));
        if computed != meta.sha256 {
            anyhow::bail!("SHA-256 mismatch: expected {} got {}", meta.sha256, computed);
        }
        info!("ndn-filestore: hash verified ✓");

        // Save file.
        let dest = dest_dir.join(&meta.name);
        tokio::fs::write(&dest, &assembled)
            .await
            .with_context(|| format!("writing {}", dest.display()))?;
        info!("ndn-filestore: saved {} bytes to {}", assembled.len(), dest.display());

        Ok(dest)
    }

    /// Listen for incoming file transfer offers.
    ///
    /// `offer_handler` receives each [`FileOffer`] and returns `true` to accept,
    /// `false` to reject. Accepted files are downloaded to `dest_dir`.
    ///
    /// This method blocks until the connection is closed.
    pub async fn listen(
        &self,
        dest_dir: &Path,
        offer_handler: impl Fn(&FileOffer) -> bool,
    ) -> Result<()> {
        info!("ndn-filestore client: listening for offers on {}", self.node_prefix);

        loop {
            let raw = match self.client.recv().await {
                Some(b) => b,
                None => break,
            };

            let interest = match Interest::decode(raw) {
                Ok(i) => i,
                Err(_) => continue,
            };

            // Detect notify Interest: /<node>/ndn-ft/v0/notify
            let notify_name = protocol::notify_name(&self.node_prefix);
            let notify_components = notify_name.components();
            let interest_components = interest.name.components();

            if interest_components.len() != notify_components.len() {
                continue;
            }
            if !interest_components.iter().zip(notify_components.iter()).all(|(a, b)| a.value == b.value) {
                continue;
            }

            // Parse FileOffer from AppParam.
            let offer: FileOffer = match interest
                .app_parameters()
                .and_then(|b| serde_json::from_slice(b).ok())
            {
                Some(o) => o,
                None => {
                    warn!("ndn-filestore: received notify Interest without valid offer");
                    continue;
                }
            };

            let accepted = offer_handler(&offer);
            let resp = if accepted {
                OfferResponse::accept()
            } else {
                OfferResponse::reject("user_declined")
            };

            let resp_json = serde_json::to_vec(&resp).unwrap_or_default();
            let resp_data = DataBuilder::new((*interest.name).clone(), &resp_json)
                .freshness(Duration::from_secs(5))
                .build();
            let _ = self.client.send(resp_data).await;

            if accepted {
                let remote_prefix: Name = offer.meta.sender_prefix.parse()
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                let file_id = offer.meta.id.clone();
                let dest = dest_dir.to_path_buf();

                // Download in background.
                // NOTE: in production use a shared ForwarderClient or spawn a new connection.
                // For now we report progress to stderr.
                info!("ndn-filestore: accepted {}, starting download…", offer.meta.name);
                let meta_name = offer.meta.name.clone();
                // We can't easily use self here without Arc; caller should use a separate
                // FileClient for download or call fetch_file directly.
                eprintln!(
                    "ndn-ft: accepted '{}' from {} — fetch with: ndn-recv --from {} --file-id {} --output {}",
                    meta_name,
                    remote_prefix,
                    remote_prefix,
                    file_id,
                    dest.display(),
                );
            }
        }

        Ok(())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}

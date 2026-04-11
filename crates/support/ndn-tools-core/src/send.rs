//! Embeddable NDN file-send logic.
//!
//! Hosts files over NDN via the `ndn-filestore` crate and optionally offers
//! them to a remote node.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::mpsc;

use ndn_filestore::{FileServer, HostOpts, SigningMode};

use crate::common::ToolEvent;

// ── Parameter type ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SendParams {
    pub face_socket: String,
    /// This node's NDN prefix (e.g. `/alice/node1`).
    pub node: String,
    /// Offer the file(s) to this remote node prefix. `None` → host only.
    pub to: Option<String>,
    /// Files to host. Must not be empty.
    pub files: Vec<PathBuf>,
    pub sign: bool,
    pub hmac: bool,
    /// Pre-chunk and sign all segments at startup.
    pub pre_chunk: bool,
    /// Segment size in bytes (0 = library default).
    pub segment_size: usize,
    /// Data freshness in milliseconds.
    pub freshness_ms: u64,
    /// Offer acceptance timeout in seconds.
    pub offer_timeout_secs: u64,
}

// ── Run ───────────────────────────────────────────────────────────────────────

pub async fn run_send(params: SendParams, tx: mpsc::Sender<ToolEvent>) -> Result<()> {
    let signing = if params.sign      { SigningMode::Ed25519 }
                  else if params.hmac { SigningMode::Hmac    }
                  else                { SigningMode::None    };

    let opts = HostOpts {
        signing,
        segment_size: params.segment_size,
        freshness_ms: params.freshness_ms,
        pre_chunk: params.pre_chunk,
        ..Default::default()
    };

    let server = FileServer::connect(&params.face_socket, &params.node).await?;

    if params.files.is_empty() {
        anyhow::bail!("no files specified");
    }

    let mut file_ids: Vec<(String, String)> = Vec::new();
    for path in &params.files {
        let id   = server.host(path, opts.clone()).await?;
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let _ = tx.send(ToolEvent::info(format!(
            "ndn-send: hosted '{}' → {}/{}", name, params.node, id
        ))).await;
        file_ids.push((id, name));
    }

    if let Some(ref target) = params.to {
        let target: ndn_packet::Name = target.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
        for (id, name) in &file_ids {
            let _ = tx.send(ToolEvent::info(format!("ndn-send: offering '{name}' to {target}…"))).await;
            let timeout_ms = params.offer_timeout_secs * 1000;
            match server.offer(&target, id, Some(timeout_ms)).await? {
                true  => { let _ = tx.send(ToolEvent::info(format!("ndn-send: '{name}' accepted"))).await; }
                false => { let _ = tx.send(ToolEvent::warn(format!("ndn-send: '{name}' rejected or timed out"))).await; }
            }
        }
        return Ok(());
    }

    let _ = tx.send(ToolEvent::info(format!(
        "ndn-send: serving {} file(s) under {}  (will stop when task is cancelled)",
        file_ids.len(), params.node
    ))).await;
    let _ = tx.send(ToolEvent::info(format!(
        "  remote nodes can browse with: ndn-recv --from {} --browse", params.node
    ))).await;

    // serve() runs until the connection drops or the task is aborted.
    server.serve().await?;
    Ok(())
}

/// Minimal convenience type so callers can specify a serve duration.
pub async fn run_send_with_timeout(
    params: SendParams,
    serve_secs: u64,
    tx: mpsc::Sender<ToolEvent>,
) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(serve_secs), run_send(params, tx))
        .await
        .ok()
        .unwrap_or(Ok(()))
}

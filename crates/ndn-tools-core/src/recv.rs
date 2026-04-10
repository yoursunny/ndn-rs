//! Embeddable NDN file-recv logic.
//!
//! Browse remote catalogs, download files, or listen for incoming offers
//! via the `ndn-filestore` crate.

use std::path::PathBuf;

use anyhow::Result;
use tokio::sync::mpsc;

use ndn_filestore::{FileClient, FileOffer};

use crate::common::{ToolData, ToolEvent};

// ── Parameter type ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RecvParams {
    pub face_socket: String,
    /// This node's NDN prefix (required for listen mode).
    pub node: Option<String>,
    /// Remote node prefix to browse or download from.
    pub from: Option<String>,
    /// Browse the remote catalog.
    pub browse: bool,
    /// Download a specific file by ID.
    pub file_id: Option<String>,
    /// Listen for incoming transfer offers.
    pub listen: bool,
    /// Auto-accept all offers (no interactive prompt).
    pub auto_accept: bool,
    /// Output directory for downloaded files.
    pub output: PathBuf,
    /// Parallel pipeline depth for segmented download.
    pub pipeline: usize,
}

// ── Run ───────────────────────────────────────────────────────────────────────

pub async fn run_recv(params: RecvParams, tx: mpsc::Sender<ToolEvent>) -> Result<()> {
    tokio::fs::create_dir_all(&params.output).await?;

    // ── Browse ───────────────────────────────────────────────────────────────
    if params.browse {
        let from = params.from.as_ref()
            .ok_or_else(|| anyhow::anyhow!("'from' is required for browse mode"))?;
        let from_prefix: ndn_packet::Name = from.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
        let node = params.node.as_deref().unwrap_or("/ndn-recv/temp");
        let client = FileClient::connect(&params.face_socket, node).await?;

        let _ = tx.send(ToolEvent::info(format!("ndn-recv: browsing {from_prefix}…"))).await;
        let catalog = client.browse(&from_prefix).await?;

        if catalog.is_empty() {
            let _ = tx.send(ToolEvent::info("ndn-recv: no files available")).await;
        } else {
            let _ = tx.send(ToolEvent::info(format!("ndn-recv: {} file(s) available:", catalog.len()))).await;
            for (i, meta) in catalog.iter().enumerate() {
                let size_str = fmt_bytes(meta.size);
                let mime     = meta.mime.as_deref().unwrap_or("?");
                let _ = tx.send(ToolEvent::info(format!(
                    "  [{i}] {} ({size_str})  id={}  mime={mime}  sign={}",
                    meta.name, meta.id, meta.signing
                ))).await;
            }
            let _ = tx.send(ToolEvent::info(format!(
                "\n  download:  ndn-recv --from {from} --file-id <id> --output {}",
                params.output.display()
            ))).await;
        }
        return Ok(());
    }

    // ── Download specific file ────────────────────────────────────────────────
    if let Some(ref file_id) = params.file_id {
        let from = params.from.as_ref()
            .ok_or_else(|| anyhow::anyhow!("'from' is required to download a file"))?;
        let from_prefix: ndn_packet::Name = from.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
        let node   = params.node.as_deref().unwrap_or("/ndn-recv/temp");
        let client = FileClient::connect(&params.face_socket, node).await?;

        let _ = tx.send(ToolEvent::info(format!("ndn-recv: downloading {file_id} from {from}…"))).await;
        let t0  = std::time::Instant::now();
        let tx2 = tx.clone();

        let dest = client
            .fetch_file(
                &from_prefix, file_id, &params.output, params.pipeline,
                move |recv, total| {
                    let _ = tx2.try_send(
                        ToolEvent::info(format!("ndn-recv: {recv}/{total} segments"))
                            .with_data(ToolData::TransferProgress {
                                bytes_done:  recv as u64,
                                bytes_total: Some(total as u64),
                            })
                    );
                },
            )
            .await?;

        let elapsed = t0.elapsed();
        let _ = tx.send(ToolEvent::summary(format!(
            "ndn-recv: saved to {}  ({:.2}s)",
            dest.display(), elapsed.as_secs_f64()
        ))).await;
        return Ok(());
    }

    // ── Listen mode ──────────────────────────────────────────────────────────
    if params.listen {
        let node = params.node.as_ref()
            .ok_or_else(|| anyhow::anyhow!("'node' is required for listen mode"))?;
        let client      = FileClient::connect(&params.face_socket, node).await?;
        let auto_accept = params.auto_accept;
        let output      = params.output.clone();
        let tx2         = tx.clone();

        let _ = tx.send(ToolEvent::info(format!("ndn-recv: listening for offers on {node}…"))).await;
        if auto_accept {
            let _ = tx.send(ToolEvent::info("ndn-recv: auto-accept enabled")).await;
        }

        client
            .listen(&output, move |offer: &FileOffer| {
                let meta = &offer.meta;
                let _ = tx2.try_send(ToolEvent::info(format!(
                    "ndn-recv: incoming offer: '{}' ({})  from {}",
                    meta.name, fmt_bytes(meta.size), meta.sender_prefix,
                )));
                if auto_accept {
                    let _ = tx2.try_send(ToolEvent::info("ndn-recv: auto-accepting"));
                    return true;
                }
                // In embedded mode always auto-accept; interactive prompt lives in the binary.
                false
            })
            .await?;
        return Ok(());
    }

    let _ = tx.send(ToolEvent::warn("ndn-recv: specify browse=true, a file_id, or listen=true")).await;
    Ok(())
}

fn fmt_bytes(n: u64) -> String {
    if n >= 1_073_741_824 { format!("{:.2} GB", n as f64 / 1_073_741_824.0) }
    else if n >= 1_048_576 { format!("{:.2} MB", n as f64 / 1_048_576.0) }
    else if n >= 1024      { format!("{:.2} KB", n as f64 / 1024.0) }
    else                   { format!("{n} B") }
}

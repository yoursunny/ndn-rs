//! `ndn-send` — host a file over NDN and optionally offer it to a remote node.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;

use ndn_tools_core::common::{EventLevel, ToolEvent};
use ndn_tools_core::send::{SendParams, run_send};

#[derive(Parser)]
#[command(name = "ndn-send", about = "Host a file over NDN and optionally offer it to a remote node")]
struct Cli {
    #[arg(long)] node: String,
    #[arg(long)] to: Option<String>,
    file: Option<PathBuf>,
    #[arg(long)] dir: Option<PathBuf>,
    #[arg(long)] sign: bool,
    #[arg(long)] hmac: bool,
    #[arg(long)] pre_chunk: bool,
    #[arg(long, default_value_t = 0)] segment_size: usize,
    #[arg(long, default_value_t = 60_000)] freshness: u64,
    #[arg(long, default_value_t = 60)]    offer_timeout: u64,
    #[arg(long, default_value_t = ndn_config::ManagementConfig::default().face_socket)]
    face_socket: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let mut files: Vec<PathBuf> = Vec::new();
    if let Some(ref dir) = cli.dir {
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                files.push(entry.path());
            }
        }
        eprintln!("ndn-send: hosting {} files from {}", files.len(), dir.display());
    }
    if let Some(ref f) = cli.file { files.push(f.clone()); }
    if files.is_empty() {
        anyhow::bail!("no file specified — provide a file path or --dir");
    }

    let (tx, mut rx) = mpsc::channel::<ToolEvent>(256);
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev.level {
                EventLevel::Error | EventLevel::Warn => eprintln!("{}", ev.text),
                _ => { if !ev.text.is_empty() { eprintln!("{}", ev.text); } }
            }
        }
    });

    run_send(SendParams {
        face_socket: cli.face_socket,
        node: cli.node,
        to: cli.to,
        files,
        sign: cli.sign,
        hmac: cli.hmac,
        pre_chunk: cli.pre_chunk,
        segment_size: cli.segment_size,
        freshness_ms: cli.freshness,
        offer_timeout_secs: cli.offer_timeout,
    }, tx).await
}

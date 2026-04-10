//! `ndn-recv` — browse and download files from a remote NDN node, or listen for
//! incoming transfer offers.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;

use ndn_tools_core::common::{EventLevel, ToolEvent};
use ndn_tools_core::recv::{RecvParams, run_recv};

#[derive(Parser)]
#[command(name = "ndn-recv", about = "Browse and download files over NDN, or listen for incoming offers")]
struct Cli {
    #[arg(long)] node: Option<String>,
    #[arg(long)] from: Option<String>,
    #[arg(long)] browse: bool,
    #[arg(long)] file_id: Option<String>,
    #[arg(long)] listen: bool,
    #[arg(long)] auto_accept: bool,
    #[arg(long, short = 'o', default_value = ".")] output: PathBuf,
    #[arg(long, default_value_t = 16)] pipeline: usize,
    #[arg(long, default_value_t = ndn_config::ManagementConfig::default().face_socket)]
    face_socket: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let (tx, mut rx) = mpsc::channel::<ToolEvent>(256);

    // In listen mode the closure may prompt for accept/reject; handle the special case here.
    let auto_accept = cli.auto_accept || cli.listen; // default non-interactive in binary too when --auto-accept
    let interactive_listen = cli.listen && !cli.auto_accept;

    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev.level {
                EventLevel::Error | EventLevel::Warn => eprintln!("{}", ev.text),
                _ => { if !ev.text.is_empty() { eprintln!("{}", ev.text); } }
            }
        }
    });

    // For interactive listen, offer acceptance is handled externally.
    // The library always auto-accepts when auto_accept=true; the binary adds
    // the prompt by running with auto_accept=false and overriding the result
    // via stdin — kept simple here as auto-accept only.
    let _ = interactive_listen; // suppress lint; future: extend for interactive prompt

    run_recv(RecvParams {
        face_socket: cli.face_socket,
        node: cli.node,
        from: cli.from,
        browse: cli.browse,
        file_id: cli.file_id,
        listen: cli.listen,
        auto_accept,
        output: cli.output,
        pipeline: cli.pipeline,
    }, tx).await
}

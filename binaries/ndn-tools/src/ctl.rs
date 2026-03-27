/// ndn-ctl — send a single management command to a running ndn-router and
/// print the response.
///
/// The transport is selected at **compile time** via the `iceoryx2-mgmt` Cargo
/// feature, mirroring the feature of the same name on `ndn-router`:
///
/// | Build                           | Transport                      |
/// |---------------------------------|--------------------------------|
/// | (default, Unix targets)         | Unix domain socket             |
/// | `--features iceoryx2-mgmt`      | iceoryx2 shared-memory RPC     |
///
/// # Examples
///
/// ```sh
/// # Query engine statistics:
/// ndn-ctl get-stats
///
/// # Add a FIB route:
/// ndn-ctl add-route /ndn --face 1 --cost 10
///
/// # Remove a FIB route:
/// ndn-ctl remove-route /ndn --face 1
///
/// # List registered faces:
/// ndn-ctl list-faces
///
/// # List FIB routes:
/// ndn-ctl list-routes
///
/// # Graceful shutdown:
/// ndn-ctl shutdown
///
/// # Custom Unix socket path:
/// ndn-ctl --socket /var/run/ndn/router.sock get-stats
///
/// # Custom iceoryx2 service name (iceoryx2-mgmt build):
/// ndn-ctl --service ndn/router/mgmt get-stats
/// ```
use clap::{Parser, Subcommand};
use ndn_config::{ManagementRequest, ManagementResponse};

// ─── iceoryx2 wire types ──────────────────────────────────────────────────────
//
// Must match `MgmtReq` / `MgmtResp` in `ndn-router/src/mgmt_ipc.rs` exactly.
// Both are `#[repr(C)]` structs with a single `data: [u8; 4096]` field so the
// layout is guaranteed identical regardless of which crate defines them.

#[cfg(feature = "iceoryx2-mgmt")]
mod ipc_wire {
    use iceoryx2::prelude::ZeroCopySend;

    #[derive(Debug, Clone, Copy, ZeroCopySend)]
    #[repr(C)]
    pub struct MgmtReq {
        pub data: [u8; 4096],
    }

    impl Default for MgmtReq {
        fn default() -> Self { Self { data: [0u8; 4096] } }
    }

    #[derive(Debug, Clone, Copy, ZeroCopySend)]
    #[repr(C)]
    pub struct MgmtResp {
        pub data: [u8; 4096],
    }

    impl Default for MgmtResp {
        fn default() -> Self { Self { data: [0u8; 4096] } }
    }
}

// ─── CLI definition ───────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name    = "ndn-ctl",
    about   = "Send a management command to a running ndn-router",
    version
)]
struct Cli {
    /// Unix socket path (Unix-socket transport only).
    ///
    /// May also be set via $NDN_MGMT_SOCK.  Ignored when the binary is
    /// built with --features iceoryx2-mgmt.
    #[arg(long, env = "NDN_MGMT_SOCK", default_value = "/tmp/ndn-router.sock")]
    socket: String,

    /// iceoryx2 service name (iceoryx2-mgmt transport only).
    ///
    /// May also be set via $NDN_MGMT_SERVICE.  Ignored when the binary is
    /// built without --features iceoryx2-mgmt.
    #[arg(long, env = "NDN_MGMT_SERVICE", default_value = "ndn/router/mgmt")]
    service: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add (or update) a FIB route.
    AddRoute {
        /// NDN name prefix (e.g. /ndn/test).
        prefix: String,

        /// Face ID to forward matching Interests on.
        #[arg(long)]
        face: u32,

        /// Routing cost; lower values are preferred (default: 10).
        #[arg(long, default_value = "10")]
        cost: u32,
    },

    /// Remove a FIB route.
    RemoveRoute {
        /// NDN name prefix.
        prefix: String,

        /// Face ID to remove the nexthop for.
        #[arg(long)]
        face: u32,
    },

    /// List all FIB routes.
    ListRoutes,

    /// List all registered face IDs.
    ListFaces,

    /// Display engine statistics (PIT size, etc.).
    GetStats,

    /// Request a graceful shutdown of the router.
    Shutdown,
}

// ─── Entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let req = build_request(&cli.command);

    #[cfg(feature = "iceoryx2-mgmt")]
    {
        let service = cli.service.clone();
        let resp = tokio::task::spawn_blocking(move || send_ipc(&service, req))
            .await??;
        print_response(resp);
        return Ok(());
    }

    #[cfg(all(unix, not(feature = "iceoryx2-mgmt")))]
    {
        let resp = send_unix(&cli.socket, &req).await?;
        print_response(resp);
        return Ok(());
    }

    #[cfg(all(not(unix), not(feature = "iceoryx2-mgmt")))]
    {
        anyhow::bail!(
            "No management transport available on this platform.\n\
             Rebuild with `--features iceoryx2-mgmt` for cross-platform support."
        );
    }
}

// ─── Request builder ──────────────────────────────────────────────────────────

fn build_request(cmd: &Command) -> ManagementRequest {
    match cmd {
        Command::AddRoute { prefix, face, cost } => ManagementRequest::AddRoute {
            prefix: prefix.clone(),
            face:   *face,
            cost:   *cost,
        },
        Command::RemoveRoute { prefix, face } => ManagementRequest::RemoveRoute {
            prefix: prefix.clone(),
            face:   *face,
        },
        Command::ListRoutes  => ManagementRequest::ListRoutes,
        Command::ListFaces   => ManagementRequest::ListFaces,
        Command::GetStats    => ManagementRequest::GetStats,
        Command::Shutdown    => ManagementRequest::Shutdown,
    }
}

// ─── Response printer ─────────────────────────────────────────────────────────

fn print_response(resp: ManagementResponse) {
    match resp {
        ManagementResponse::Ok => {
            println!("ok");
        }
        ManagementResponse::OkData { data } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&data)
                    .unwrap_or_else(|_| data.to_string())
            );
        }
        ManagementResponse::Error { message } => {
            eprintln!("error: {message}");
            std::process::exit(1);
        }
    }
}

// ─── Unix socket transport ────────────────────────────────────────────────────

#[cfg(all(unix, not(feature = "iceoryx2-mgmt")))]
async fn send_unix(
    socket_path: &str,
    req: &ManagementRequest,
) -> anyhow::Result<ManagementResponse> {
    use anyhow::Context as _;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let stream = UnixStream::connect(socket_path).await.with_context(|| {
        format!("Could not connect to '{socket_path}'. Is ndn-router running?")
    })?;

    let (reader, mut writer) = stream.into_split();

    // Encode the request as a newline-delimited JSON line.
    let mut json = serde_json::to_string(req)?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;

    // Read exactly one response line.
    let mut lines = BufReader::new(reader).lines();
    let line = lines.next_line().await?.ok_or_else(|| {
        anyhow::anyhow!("Connection closed before a response was received.")
    })?;

    serde_json::from_str::<ManagementResponse>(&line)
        .with_context(|| format!("Unparseable response: {line}"))
}

// ─── iceoryx2 shared-memory transport ────────────────────────────────────────

/// Maximum number of 5 ms ticks to wait for a response before timing out.
///
/// 1000 ticks × 5 ms = 5 seconds.
#[cfg(feature = "iceoryx2-mgmt")]
const MAX_TICKS: u32 = 1000;

/// Send `req` over an iceoryx2 request-response service and return the response.
///
/// Blocking — intended to be called inside `tokio::task::spawn_blocking`.
#[cfg(feature = "iceoryx2-mgmt")]
fn send_ipc(
    service_name: &str,
    req: ManagementRequest,
) -> anyhow::Result<ManagementResponse> {
    use ipc_wire::{MgmtReq, MgmtResp};
    use iceoryx2::node::NodeWaitFailure;
    use iceoryx2::prelude::*;
    use std::time::Duration;

    let node = NodeBuilder::new()
        .create::<ipc::Service>()
        .map_err(|e| anyhow::anyhow!("Failed to create iceoryx2 node: {e}"))?;

    let svc_name: ServiceName = service_name
        .try_into()
        .map_err(|e| anyhow::anyhow!("Invalid service name '{service_name}': {e}"))?;

    let service = node
        .service_builder(&svc_name)
        .request_response::<MgmtReq, MgmtResp>()
        .open_or_create()
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to open service '{service_name}': {e}\n\
                 Is ndn-router running with --features iceoryx2-mgmt?"
            )
        })?;

    let client = service
        .client_builder()
        .create()
        .map_err(|e| anyhow::anyhow!("Failed to create client: {e}"))?;

    // Encode the request into the wire buffer.
    let json  = serde_json::to_string(&req)?;
    let bytes = json.as_bytes();
    let mut wire_req = MgmtReq::default();
    let len = bytes.len().min(wire_req.data.len() - 1); // leave one null terminator byte
    wire_req.data[..len].copy_from_slice(&bytes[..len]);

    let loan = client
        .loan_uninit()
        .map_err(|e| anyhow::anyhow!("Failed to loan request buffer: {e}"))?;

    let pending = loan
        .write_payload(wire_req)
        .send()
        .map_err(|e| anyhow::anyhow!("Failed to send request: {e}"))?;

    // Poll for a response, using node.wait() to avoid a busy-spin.
    for _ in 0..MAX_TICKS {
        match node.wait(Duration::from_millis(5)) {
            Ok(()) | Err(NodeWaitFailure::Interrupt) => {}
            Err(NodeWaitFailure::TerminationRequest) => {
                anyhow::bail!("iceoryx2 termination request received while waiting for response");
            }
            #[allow(unreachable_patterns)]
            Err(e) => anyhow::bail!("iceoryx2 wait error: {e}"),
        }

        match pending.receive() {
            Ok(Some(response)) => {
                let end = response.data.iter().position(|&b| b == 0).unwrap_or(4096);
                let text = std::str::from_utf8(&response.data[..end])
                    .map_err(|_| anyhow::anyhow!("Response payload is not valid UTF-8"))?;
                return serde_json::from_str::<ManagementResponse>(text)
                    .map_err(|e| anyhow::anyhow!("Unparseable response JSON: {e}"));
            }
            Ok(None)   => continue, // response not yet available; try next tick
            Err(e)     => anyhow::bail!("Failed to receive response: {e}"),
        }
    }

    anyhow::bail!(
        "Timed out waiting for response from '{service_name}' after {}ms",
        MAX_TICKS * 5
    )
}

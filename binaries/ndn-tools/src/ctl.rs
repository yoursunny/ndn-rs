/// ndn-ctl — send a management command to a running ndn-router.
///
/// By default, commands are expressed as NDN Interest packets sent over the
/// router's face socket (`/tmp/ndn-faces.sock`).  The response is a Data
/// packet whose Content carries the JSON `ManagementResponse`.
///
/// An optional `--bypass` flag falls back to the legacy transport: raw JSON
/// over a Unix socket, or iceoryx2 shared-memory RPC when built with
/// `--features iceoryx2-mgmt`.
///
/// # NDN protocol
///
/// - **Interest name**: `/localhost/ndn-ctl/<verb>`
///   (e.g. `/localhost/ndn-ctl/get-stats`)
/// - **ApplicationParameters** (TLV 0x24): JSON-encoded `ManagementRequest`
/// - **Data name**: same as the Interest name
/// - **Content**: JSON-encoded `ManagementResponse`
///
/// # Examples
///
/// ```sh
/// ndn-ctl get-stats
/// ndn-ctl add-route /ndn --face 1 --cost 10
/// ndn-ctl list-faces
/// ndn-ctl shutdown
///
/// # Custom face socket (NDN transport):
/// ndn-ctl --face-socket /var/run/ndn/faces.sock get-stats
///
/// # Bypass: Unix socket JSON (non-iceoryx2 builds):
/// ndn-ctl --bypass --socket /tmp/ndn-router.sock get-stats
///
/// # Bypass: iceoryx2 (iceoryx2-mgmt builds):
/// ndn-ctl --bypass --service ndn/router/mgmt get-stats
/// ```
use bytes::Bytes;
use clap::{Parser, Subcommand};
use ndn_config::{ManagementRequest, ManagementResponse};
use ndn_packet::{Name, NameComponent};
use ndn_packet::encode::encode_interest;

// iceoryx2 wire types — only needed for the bypass iceoryx2 transport.
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
    /// Use the legacy bypass transport instead of NDN Interest/Data.
    ///
    /// With `--bypass`, raw JSON is sent over a Unix socket (or iceoryx2 when
    /// built with --features iceoryx2-mgmt).
    #[arg(long)]
    bypass: bool,

    /// NDN face socket path (NDN transport).
    ///
    /// May also be set via $NDN_FACE_SOCK.
    #[arg(long, env = "NDN_FACE_SOCK", default_value = "/tmp/ndn-faces.sock")]
    face_socket: String,

    /// Unix socket path (bypass transport only).
    ///
    /// May also be set via $NDN_MGMT_SOCK.
    #[arg(long, env = "NDN_MGMT_SOCK", default_value = "/tmp/ndn-router.sock")]
    socket: String,

    /// iceoryx2 service name (bypass + iceoryx2-mgmt feature only).
    ///
    /// May also be set via $NDN_MGMT_SERVICE.
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
        /// Face ID.
        #[arg(long)]
        face: u32,
        /// Routing cost; lower is preferred (default: 10).
        #[arg(long, default_value = "10")]
        cost: u32,
    },
    /// Remove a FIB route.
    RemoveRoute {
        /// NDN name prefix.
        prefix: String,
        /// Face ID.
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

    if cli.bypass {
        run_bypass(&cli, req).await
    } else {
        run_ndn(&cli.face_socket, req).await
    }
}

// ─── NDN transport (primary) ──────────────────────────────────────────────────

/// Send `req` as an NDN Interest over the face socket and return the response.
#[cfg(unix)]
async fn run_ndn(face_socket: &str, req: ManagementRequest) -> anyhow::Result<()> {
    use anyhow::Context as _;
    use ndn_face_local::UnixFace;
    use ndn_packet::Data;
    use ndn_transport::{Face, FaceId};

    let face = UnixFace::connect(FaceId(0), face_socket)
        .await
        .with_context(|| {
            format!("Cannot connect to '{face_socket}'. Is ndn-router running?")
        })?;

    // Build Interest name: /localhost/ndn-ctl/<verb>
    let name = mgmt_name(&req);

    // Encode command as ApplicationParameters JSON.
    let json  = serde_json::to_string(&req)?;
    let interest_bytes = encode_interest(&name, Some(json.as_bytes()));

    face.send(interest_bytes).await
        .map_err(|e| anyhow::anyhow!("Failed to send Interest: {e}"))?;

    // Wait up to 5 s for the Data response.
    let data_bytes = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        face.recv(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Timed out waiting for response from '{face_socket}'"))?
    .map_err(|e| anyhow::anyhow!("Failed to receive response: {e}"))?;

    let data = Data::decode(data_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to decode Data response: {e}"))?;

    let content = data.content()
        .ok_or_else(|| anyhow::anyhow!("Data response has no Content field"))?;

    let resp_json = std::str::from_utf8(content)
        .map_err(|_| anyhow::anyhow!("Response Content is not valid UTF-8"))?;

    let resp = serde_json::from_str::<ManagementResponse>(resp_json)
        .map_err(|e| anyhow::anyhow!("Cannot parse response JSON: {e}"))?;

    print_response(resp);
    Ok(())
}

#[cfg(not(unix))]
async fn run_ndn(_face_socket: &str, _req: ManagementRequest) -> anyhow::Result<()> {
    anyhow::bail!(
        "NDN management transport requires Unix domain sockets (unix target).\n\
         Use `--bypass` with the `iceoryx2-mgmt` feature for cross-platform support."
    )
}

// ─── Bypass transport (legacy) ────────────────────────────────────────────────

async fn run_bypass(cli: &Cli, req: ManagementRequest) -> anyhow::Result<()> {
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
        let _ = (cli, req);
        anyhow::bail!(
            "No bypass transport available on this platform.\n\
             Rebuild with `--features iceoryx2-mgmt` for cross-platform bypass support."
        );
    }
}

// ─── Unix socket bypass ───────────────────────────────────────────────────────

#[cfg(all(unix, not(feature = "iceoryx2-mgmt")))]
async fn send_unix(
    socket_path: &str,
    req: &ManagementRequest,
) -> anyhow::Result<ManagementResponse> {
    use anyhow::Context as _;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let stream = UnixStream::connect(socket_path).await.with_context(|| {
        format!("Could not connect to '{socket_path}'. Is ndn-router running with bypass transport?")
    })?;

    let (reader, mut writer) = stream.into_split();
    let mut json = serde_json::to_string(req)?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;

    let mut lines = BufReader::new(reader).lines();
    let line = lines.next_line().await?.ok_or_else(|| {
        anyhow::anyhow!("Connection closed before a response was received.")
    })?;

    serde_json::from_str::<ManagementResponse>(&line)
        .with_context(|| format!("Unparseable response: {line}"))
}

// ─── iceoryx2 bypass ─────────────────────────────────────────────────────────

#[cfg(feature = "iceoryx2-mgmt")]
const MAX_TICKS: u32 = 1000;

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
        .map_err(|e| anyhow::anyhow!("Invalid service name: {e}"))?;

    let service = node
        .service_builder(&svc_name)
        .request_response::<MgmtReq, MgmtResp>()
        .open_or_create()
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to open service '{service_name}': {e}\n\
                 Is ndn-router running with --features iceoryx2-mgmt and bypass transport?"
            )
        })?;

    let client = service
        .client_builder()
        .create()
        .map_err(|e| anyhow::anyhow!("Failed to create client: {e}"))?;

    let json  = serde_json::to_string(&req)?;
    let bytes = json.as_bytes();
    let mut wire_req = MgmtReq::default();
    let len = bytes.len().min(wire_req.data.len() - 1);
    wire_req.data[..len].copy_from_slice(&bytes[..len]);

    let loan = client
        .loan_uninit()
        .map_err(|e| anyhow::anyhow!("Failed to loan request buffer: {e}"))?;

    let pending = loan
        .write_payload(wire_req)
        .send()
        .map_err(|e| anyhow::anyhow!("Failed to send request: {e}"))?;

    for _ in 0..MAX_TICKS {
        match node.wait(Duration::from_millis(5)) {
            Ok(()) | Err(NodeWaitFailure::Interrupt) => {}
            Err(NodeWaitFailure::TerminationRequest) => {
                anyhow::bail!("iceoryx2 termination during wait");
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
            Ok(None)   => continue,
            Err(e)     => anyhow::bail!("Failed to receive response: {e}"),
        }
    }
    anyhow::bail!("Timed out waiting for response from '{service_name}'")
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

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

/// Build the Interest name for a management request:
/// `/localhost/ndn-ctl/<verb>`.
fn mgmt_name(req: &ManagementRequest) -> Name {
    let verb: &[u8] = match req {
        ManagementRequest::AddRoute    { .. } => b"add-route",
        ManagementRequest::RemoveRoute { .. } => b"remove-route",
        ManagementRequest::ListRoutes          => b"list-routes",
        ManagementRequest::ListFaces           => b"list-faces",
        ManagementRequest::GetStats            => b"get-stats",
        ManagementRequest::Shutdown            => b"shutdown",
    };
    Name::from_components([
        NameComponent::generic(Bytes::from_static(b"localhost")),
        NameComponent::generic(Bytes::from_static(b"ndn-ctl")),
        NameComponent::generic(Bytes::copy_from_slice(verb)),
    ])
}

fn print_response(resp: ManagementResponse) {
    match resp {
        ManagementResponse::Ok => println!("ok"),
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

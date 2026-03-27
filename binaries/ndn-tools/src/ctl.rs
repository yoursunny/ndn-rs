/// ndn-ctl — send a management command to a running ndn-router.
///
/// By default, commands are expressed as NDN Interest packets sent over the
/// router's face socket (`/tmp/ndn-faces.sock`).  The response is a Data
/// packet whose Content carries the JSON `ManagementResponse`.
///
/// An optional `--bypass` flag falls back to the legacy transport: raw JSON
/// over a Unix socket.
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
/// # Bypass: Unix socket JSON:
/// ndn-ctl --bypass --socket /tmp/ndn-router.sock get-stats
/// ```
use bytes::Bytes;
use clap::{Parser, Subcommand};
use ndn_config::{ManagementRequest, ManagementResponse};
use ndn_packet::{Name, NameComponent};
use ndn_packet::encode::encode_interest;

// ─── CLI definition ───────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name    = "ndn-ctl",
    about   = "Send a management command to a running ndn-router",
    version
)]
struct Cli {
    /// Use the legacy bypass transport (raw JSON over Unix socket).
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
    anyhow::bail!("NDN management transport requires Unix domain sockets")
}

// ─── Bypass transport (legacy) ────────────────────────────────────────────────

#[cfg(unix)]
async fn run_bypass(cli: &Cli, req: ManagementRequest) -> anyhow::Result<()> {
    let resp = send_unix(&cli.socket, &req).await?;
    print_response(resp);
    Ok(())
}

#[cfg(not(unix))]
async fn run_bypass(_cli: &Cli, _req: ManagementRequest) -> anyhow::Result<()> {
    anyhow::bail!("Bypass transport requires Unix domain sockets")
}

// ─── Unix socket bypass ───────────────────────────────────────────────────────

#[cfg(unix)]
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

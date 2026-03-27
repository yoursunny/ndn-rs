/// NDN-native management server.
///
/// Implements the management plane over real NDN Interest/Data packets so that
/// management traffic inherits the forwarder's security and routing primitives.
///
/// # Architecture
///
/// ```text
///  ndn-ctl ──UnixFace──► face listener ──► FaceTable
///                                               │
///                              pipeline (decode → CS → PIT → strategy)
///                                               │  FIB: /localhost/ndn-ctl → mgmt AppFace
///                                               ▼
///                                        mgmt AppFace  ◄──► AppHandle
///                                                              │
///                                                    run_ndn_mgmt_handler()
///                                                              │ encode Data response
///                                               AppHandle::send(data_bytes)
///                                                              │
///                              pipeline (PIT match → satisfy → UnixFace back to ndn-ctl)
/// ```
///
/// # Protocol
///
/// Management Interests use the name prefix `/localhost/ndn-ctl` registered
/// in the FIB.  The specific command is the next component
/// (e.g. `/localhost/ndn-ctl/get-stats`).  Command arguments are carried in
/// the `ApplicationParameters` TLV (type 0x24) as a JSON-encoded
/// `ManagementRequest`.
///
/// The response is a Data packet with the same name as the Interest and a
/// JSON-encoded `ManagementResponse` in its `Content` field.
///
/// # Face listener
///
/// `run_face_listener` accepts Unix domain socket connections and registers
/// each as a dynamic `UnixFace` with the engine via `ForwarderEngine::add_face`.
/// Once registered, the face participates in forwarding immediately.
use std::path::Path;

use bytes::Bytes;
use ndn_face_local::UnixFace;
use ndn_packet::{Interest, Name, NameComponent, encode::{encode_data_unsigned}};
use ndn_face_local::AppHandle;
use ndn_config::{ManagementResponse, ManagementServer};
use ndn_engine::ForwarderEngine;
use tokio_util::sync::CancellationToken;

use crate::handle_request;

// ─── Management prefix ────────────────────────────────────────────────────────

/// Build the `/localhost/ndn-ctl` name prefix registered in the FIB.
pub fn mgmt_prefix() -> Name {
    Name::from_components([
        NameComponent::generic(Bytes::from_static(b"localhost")),
        NameComponent::generic(Bytes::from_static(b"ndn-ctl")),
    ])
}

// ─── Face listener ────────────────────────────────────────────────────────────

/// Accept NDN face connections on `path` and register each as a dynamic face.
///
/// Each accepted `UnixStream` becomes a `UnixFace` that is inserted into the
/// engine's `FaceTable` and immediately gets a packet-reader task.  The face
/// participates in forwarding like any other face: Interests it sends are
/// routed by the FIB; Data packets satisfying its PIT entries are sent back
/// to it.
///
/// **Blocking until `cancel` fires.**
pub async fn run_face_listener(
    path:   &Path,
    engine: ForwarderEngine,
    cancel: CancellationToken,
) {
    // Remove any stale socket file from a previous run.
    let _ = std::fs::remove_file(path);

    let listener = match tokio::net::UnixListener::bind(path) {
        Ok(l)  => l,
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "face-listener: bind failed");
            return;
        }
    };

    tracing::info!(path = %path.display(), "NDN face listener ready");

    loop {
        let (stream, _addr) = tokio::select! {
            _ = cancel.cancelled() => break,
            r = listener.accept() => match r {
                Ok(s)  => s,
                Err(e) => {
                    tracing::warn!(error = %e, "face-listener: accept error");
                    continue;
                }
            },
        };

        let face_id = engine.faces().alloc_id();
        let face    = UnixFace::from_stream(face_id, stream, path);
        tracing::debug!(face = %face_id, "face-listener: accepted connection");
        engine.add_face(face, cancel.clone());
    }

    let _ = std::fs::remove_file(path);
    tracing::info!("NDN face listener stopped");
}

// ─── Management handler ───────────────────────────────────────────────────────

/// Read Interests from the management `AppHandle`, dispatch to `handle_request`,
/// and write Data responses back.
///
/// Runs until `cancel` fires or the channel closes.
pub async fn run_ndn_mgmt_handler(
    mut handle: AppHandle,
    engine:     ForwarderEngine,
    cancel:     CancellationToken,
) {
    loop {
        let raw = tokio::select! {
            _ = cancel.cancelled() => break,
            r = handle.recv() => match r {
                Some(b) => b,
                None    => break,   // AppFace dropped (engine shut down)
            },
        };

        // Decode the inbound Interest.
        let interest = match Interest::decode(raw) {
            Ok(i)  => i,
            Err(e) => {
                tracing::warn!(error = %e, "ndn-mgmt: malformed Interest; skipping");
                continue;
            }
        };

        // Extract JSON command from ApplicationParameters.
        let json = match interest.app_parameters() {
            Some(p) => match std::str::from_utf8(p) {
                Ok(s)  => s.to_owned(),
                Err(_) => {
                    tracing::warn!("ndn-mgmt: ApplicationParameters is not valid UTF-8");
                    send_error(&mut handle, &interest.name, "invalid UTF-8 in request").await;
                    continue;
                }
            },
            None => {
                tracing::warn!(
                    name = %format_name(&interest.name),
                    "ndn-mgmt: Interest has no ApplicationParameters"
                );
                send_error(&mut handle, &interest.name, "missing ApplicationParameters").await;
                continue;
            }
        };

        // Dispatch to the synchronous management handler.
        let resp = match ManagementServer::decode_request(&json) {
            Ok(req)  => handle_request(req, &engine, &cancel),
            Err(msg) => ManagementResponse::Error { message: msg },
        };

        // Encode the response as a Data packet and send it back.
        let resp_json = ManagementServer::encode_response(&resp);
        let data      = encode_data_unsigned(&interest.name, resp_json.as_bytes());

        if let Err(e) = handle.send(data).await {
            tracing::warn!(error = %e, "ndn-mgmt: failed to send Data response");
        }
    }

    tracing::info!("NDN management handler stopped");
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

async fn send_error(handle: &mut AppHandle, name: &Name, message: &str) {
    let resp = ManagementResponse::Error { message: message.to_owned() };
    let json = ManagementServer::encode_response(&resp);
    let data = encode_data_unsigned(name, json.as_bytes());
    let _ = handle.send(data).await;
}

fn format_name(name: &Name) -> String {
    let mut s = String::new();
    for comp in name.components() {
        s.push('/');
        for &b in comp.value.iter() {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' {
                s.push(b as char);
            } else {
                s.push_str(&format!("%{b:02X}"));
            }
        }
    }
    if s.is_empty() { s.push('/'); }
    s
}

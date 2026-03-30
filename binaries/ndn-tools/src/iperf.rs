//! `ndn-iperf` — NDN bandwidth measurement tool.
//!
//! Embeds a forwarding engine with a producer and consumer `AppFace` pair,
//! drives a sliding-window Interest/Data exchange, and reports sustained
//! throughput and latency.
//!
//! Usage:
//! ```text
//! ndn-iperf [--duration SECS] [--size BYTES] [--window N] [--prefix NAME]
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use bytes::Bytes;
use clap::Parser;

use ndn_engine::{EngineBuilder, EngineConfig};
use ndn_face_local::{AppFace, AppHandle};
use ndn_packet::encode::{encode_data_unsigned, encode_interest};
use ndn_packet::{Data, Interest, Name, NameComponent};
use ndn_transport::FaceId;

// ─── CLI ─────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "ndn-iperf", about = "NDN bandwidth measurement tool")]
struct Cli {
    /// Test duration in seconds.
    #[arg(long, default_value_t = 10)]
    duration: u64,

    /// Data payload size in bytes.
    #[arg(long, default_value_t = 8192)]
    size: usize,

    /// Sliding window size (max outstanding Interests).
    #[arg(long, default_value_t = 64)]
    window: usize,

    /// Name prefix.
    #[arg(long, default_value = "/iperf")]
    prefix: String,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn parse_name(s: &str) -> Name {
    let components: Vec<NameComponent> = s
        .split('/')
        .filter(|c| !c.is_empty())
        .map(|c| NameComponent::generic(Bytes::copy_from_slice(c.as_bytes())))
        .collect();
    if components.is_empty() {
        Name::root()
    } else {
        Name::from_components(components)
    }
}

fn build_name(prefix: &Name, seq: u64) -> Name {
    let seq_comp = NameComponent::generic(Bytes::copy_from_slice(
        format!("{seq}").as_bytes(),
    ));
    Name::from_components(
        prefix.components().iter().cloned().chain([seq_comp]),
    )
}

/// Extract the sequence number from the last name component of a Data packet.
fn extract_seq(raw: &Bytes) -> Option<u64> {
    let data = Data::decode(raw.clone()).ok()?;
    let last = data.name.components().last()?;
    let s = std::str::from_utf8(&last.value).ok()?;
    s.parse().ok()
}

// ─── Producer ────────────────────────────────────────────────────────────────

async fn run_producer(mut handle: AppHandle, payload: Arc<Vec<u8>>) {
    loop {
        let raw = match handle.recv().await {
            Some(b) => b,
            None => break,
        };
        let interest = match Interest::decode(raw) {
            Ok(i) => i,
            Err(_) => continue,
        };
        let data = encode_data_unsigned(&interest.name, &payload);
        if handle.send(data).await.is_err() {
            break;
        }
    }
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let prefix = parse_name(&cli.prefix);

    println!(
        "ndn-iperf: duration={}s payload={}B window={}",
        cli.duration, cli.size, cli.window,
    );

    // ── Build engine ─────────────────────────────────────────────────────────

    let buf_size = cli.window * 4;
    let consumer_id = FaceId(1);
    let producer_id = FaceId(2);

    let (consumer_face, mut consumer_handle) = AppFace::new(consumer_id, buf_size);
    let (producer_face, producer_handle) = AppFace::new(producer_id, buf_size);

    let (engine, shutdown) = EngineBuilder::new(EngineConfig {
        pipeline_channel_cap: buf_size,
        ..Default::default()
    })
    .face(consumer_face)
    .face(producer_face)
    .build()
    .await?;

    engine.fib().add_nexthop(&prefix, producer_id, 0);

    // ── Spawn producer ───────────────────────────────────────────────────────

    let payload = Arc::new(vec![0xAAu8; cli.size]);
    let producer_task = tokio::spawn(run_producer(producer_handle, payload));

    // ── Sliding-window consumer ──────────────────────────────────────────────

    let deadline = Instant::now() + Duration::from_secs(cli.duration);
    let mut timestamps: HashMap<u64, Instant> = HashMap::new();
    let mut next_seq: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut received: u64 = 0;
    let mut rtts: Vec<u64> = Vec::new();

    // Fill the window.
    let initial_window = cli.window as u64;
    for _ in 0..initial_window {
        let name = build_name(&prefix, next_seq);
        let wire = encode_interest(&name, None);
        timestamps.insert(next_seq, Instant::now());
        consumer_handle.send(wire).await?;
        next_seq += 1;
    }

    let sent = loop {
        match tokio::time::timeout(Duration::from_secs(4), consumer_handle.recv()).await {
            Ok(Some(data_bytes)) => {
                total_bytes += data_bytes.len() as u64;
                received += 1;

                if let Some(seq) = extract_seq(&data_bytes) {
                    if let Some(t0) = timestamps.remove(&seq) {
                        rtts.push(t0.elapsed().as_micros() as u64);
                    }
                }

                // Send next Interest if within deadline.
                if Instant::now() < deadline {
                    let name = build_name(&prefix, next_seq);
                    let wire = encode_interest(&name, None);
                    timestamps.insert(next_seq, Instant::now());
                    consumer_handle.send(wire).await?;
                    next_seq += 1;
                } else if timestamps.is_empty() {
                    break next_seq;
                }
            }
            Ok(None) => break next_seq, // channel closed
            Err(_) => {
                // Timeout — if past deadline, stop.
                if Instant::now() >= deadline {
                    break next_seq;
                }
            }
        }
    };

    let elapsed = Duration::from_secs(cli.duration);

    // ── Report ───────────────────────────────────────────────────────────────

    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
    let mbps = total_bytes as f64 * 8.0 / elapsed.as_secs_f64() / 1_000_000.0;
    let avg_rtt = if !rtts.is_empty() {
        rtts.iter().sum::<u64>() / rtts.len() as u64
    } else {
        0
    };

    println!("  transferred: {total_mb:.2} MB ({total_bytes} bytes)");
    println!("  throughput:  {mbps:.2} Mbps");
    println!("  packets:     {sent} sent, {received} received");
    println!("  avg RTT:     {avg_rtt} us");

    if !rtts.is_empty() {
        rtts.sort_unstable();
        let n = rtts.len();
        let p50 = rtts[n / 2];
        let p95 = rtts[(n * 95) / 100];
        let p99 = rtts[(n * 99) / 100];
        println!("  RTT detail:  p50={p50}us p95={p95}us p99={p99}us");
    }

    // ── Shutdown ─────────────────────────────────────────────────────────────

    drop(consumer_handle);
    producer_task.abort();
    let _ = producer_task.await;
    shutdown.shutdown().await;

    Ok(())
}

//! `ndn-bench` — throughput and latency benchmarking for the NDN forwarder.
//!
//! Drives controlled Interest/Data exchanges against a running engine and
//! reports per-packet latency percentiles and aggregate throughput.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("ndn-bench: not yet implemented");
    // TODO: embed engine with AppFace, drive Interest/Data load, report metrics
    Ok(())
}

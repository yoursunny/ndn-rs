//! `ndn-ping` — measure round-trip time to a named prefix.
//!
//! Usage: ndn-ping /name/prefix

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let prefix = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: ndn-ping <prefix>");
        std::process::exit(1);
    });
    println!("ndn-ping: pinging {prefix}");
    // TODO: express Interests with nonce and measure RTT
    Ok(())
}

//! `ndn-peek` — fetch a single named Data packet and print its content.
//!
//! Usage: ndn-peek /name/of/data

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let name = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: ndn-peek <name>");
        std::process::exit(1);
    });
    println!("ndn-peek: fetching {name}");
    // TODO: connect to local forwarder and express Interest
    Ok(())
}

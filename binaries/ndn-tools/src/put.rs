//! `ndn-put` — publish a file as named Data.
//!
//! Usage: ndn-put /name/of/data <file>

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: ndn-put <name> <file>");
        std::process::exit(1);
    }
    println!("ndn-put: publishing {} from {}", args[1], args[2]);
    // TODO: connect to local forwarder and register prefix handler
    Ok(())
}

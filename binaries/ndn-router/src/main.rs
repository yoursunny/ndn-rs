use anyhow::Result;
use tracing_subscriber::EnvFilter;

use ndn_engine::{EngineBuilder, EngineConfig};
use ndn_strategy::BestRouteStrategy;
use ndn_store::LruCs;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .with_thread_ids(true)
        .init();

    tracing::info!("ndn-router starting");

    let config = EngineConfig::default();
    let (_engine, shutdown) = EngineBuilder::new(config)
        .build()
        .await?;

    tracing::info!("engine running — press Ctrl-C to stop");

    tokio::signal::ctrl_c().await?;

    tracing::info!("shutting down");
    shutdown.shutdown().await;
    Ok(())
}

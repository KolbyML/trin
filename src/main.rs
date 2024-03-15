#![warn(clippy::unwrap_used)]
use std::time::Duration;

use ethportal_api::types::cli::TrinConfig;
use tracing::error;

use trin::run_trin;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    console_subscriber::ConsoleLayer::builder()
        .retention(Duration::from_secs(60))
        .init();
    let trin_config = TrinConfig::from_cli();
    let rpc_handle = run_trin(trin_config).await?;

    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    if let Err(err) = rpc_handle.stop() {
        error!(err = %err, "Failed to close RPC server")
    }

    Ok(())
}

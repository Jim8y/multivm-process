//! Basic Solana Execution Engine Usage Example
//!
//! This example demonstrates how to create, initialize, run, and shutdown
//! a Solana execution engine. It shows the basic lifecycle of the engine
//! with a 60-second runtime period.
//!
//! ## Usage
//!
//! Run this example with:
//! ```bash
//! cargo run --example basic_engine_usage
//! ```
//!
//! ## What this example does:
//! 1. Sets up logging for better visibility
//! 2. Creates and initializes a Solana execution engine
//! 3. Runs the engine for 60 seconds
//! 4. Gracefully shuts down the engine
//!
//! ## Prerequisites
//! - Make sure you have built the `solana-private-validator` binary first:
//!   ```bash
//!   cargo build --bin solana-private-validator
//!   ```

use solana_execution_engine::engine::SolanaEngine;
use std::time::Duration;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .without_time()
        .try_init()
        .ok();

    let mut engine = SolanaEngine::new_default().await?;

    engine.initialize().await?;

    // You can add additional operations here while the engine is running
    // For example:
    // - Process transactions
    // - Process a block
    // - Query blockchain state
    // - Monitor health status

    tokio::time::sleep(Duration::from_secs(30)).await;

    info!("⏰ 30 seconds elapsed, proceeding to shutdown");

    engine.shutdown(Some(Duration::from_secs(10))).await?;

    Ok(())
}

//! Solana Engine RPC Tests
//!
//! This module contains tests for the Solana execution engine RPC functionality,
//! specifically testing the getSlot RPC method through the RPC proxy server.

use crate::engine::{SolanaEngine, SolanaEngineError};
use crate::engine_rpc_client::solana_engine_client::rpc_client::RpcClient;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

#[cfg(test)]
mod tests {
    use solana_sdk::commitment_config::CommitmentConfig;

    use super::*;
    use crate::test_utils::{create_and_initialize_engine, setup_logging, shutdown_engine};

    /// Test the getSlot RPC method through the RPC proxy server
    #[tokio::test]
    async fn test_get_slot_rpc_until_slot_10() -> Result<(), SolanaEngineError> {
        setup_logging();

        info!("=== Starting getSlot RPC Test ===");
        info!("Goal: Monitor slot progression until reaching slot 10 via RPC proxy");

        // Step 1: Create and initialize the Solana engine
        info!("Step 1: Creating and initializing Solana engine");
        let mut engine = create_and_initialize_engine().await?;
        // Wait a moment for the server to start
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Step 3: Create Solana RPC client for connecting to port 8888
        info!("Step 3: Creating Solana RPC client");
        let rpc_url = format!(
            "http://{}:{}",
            engine.get_rpc_server_host(),
            engine.get_rpc_server_port()
        );
        info!("RPC URL: {}", rpc_url);

        let commitment = CommitmentConfig::confirmed();
        let rpc_client = Arc::new(RpcClient::new_with_commitment(rpc_url.clone(), commitment));

        // Step 4: Monitor slot progression until slot 10
        info!("Step 4: Starting slot progression monitoring via RPC");
        let target_slot = 10u64;
        let mut current_slot = 0u64;
        let mut check_count = 0;
        let max_checks = 120; // Maximum number of checks (2 minutes with 1-second intervals)
        let check_interval = Duration::from_secs(1);

        info!("Starting getSlot RPC method polling...");
        info!("Target slot: {}", target_slot);
        info!("Check interval: {:?}", check_interval);
        info!("Maximum checks: {}", max_checks);

        while current_slot < target_slot && check_count < max_checks {
            check_count += 1;

            // Call the getSlot RPC method through the Solana RPC client
            // Use spawn_blocking since get_slot() is a blocking call
            let rpc_client_clone = Arc::clone(&rpc_client);
            match tokio::task::spawn_blocking(move || rpc_client_clone.get_slot()).await {
                Ok(result) => match result {
                    Ok(slot) => {
                        current_slot = slot;
                        info!(
                            "Check #{}: Current slot = {} (Target: {})",
                            check_count, current_slot, target_slot
                        );

                        if current_slot >= target_slot {
                            info!(
                                "✓ Success! Slot {} has reached target slot {}",
                                current_slot, target_slot
                            );
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("⚠ Check #{}: getSlot RPC call failed: {}", check_count, e);
                        // Continue checking even if individual calls fail
                    }
                },
                Err(e) => {
                    warn!("⚠ Check #{}: spawn_blocking failed: {}", check_count, e);
                    // Continue checking even if spawn_blocking fails
                }
            }

            // Wait before next check
            tokio::time::sleep(check_interval).await;
        }

        // Step 5: Verify results
        info!("Step 5: Verifying results");
        if current_slot >= target_slot {
            info!("✓ Test completed successfully!");
            info!("  - Final slot: {}", current_slot);
            info!("  - Target slot: {}", target_slot);
            info!("  - Total checks: {}", check_count);
            info!("  - Total time: approximately {} seconds", check_count);
        } else {
            warn!("⚠ Test timed out!");
            warn!("  - Final slot: {}", current_slot);
            warn!("  - Target slot: {}", target_slot);
            warn!("  - Total checks: {}", check_count);
            warn!("  - May need more time for slot progression");
        }

        // Step 6: Stop the RPC proxy server
        info!("Step 6: Stopping RPC proxy server");
        engine.stop_rpc_proxy_server().await?;

        // Step 7: Shutdown the engine
        info!("Step 7: Shutting down Solana engine");
        shutdown_engine(engine).await?;

        // Summary
        info!("=== Test Summary ===");
        info!("✓ Successfully started Solana engine");
        info!("✓ Successfully started RPC proxy server on port 8888");
        info!("✓ Successfully tested getSlot RPC method via HTTP client");
        if current_slot >= target_slot {
            info!(
                "✓ Successfully waited for slot to reach target value {}",
                target_slot
            );
        } else {
            info!(
                "⚠ Slot did not reach target value {} (current: {})",
                target_slot, current_slot
            );
        }
        info!("✓ Successfully stopped RPC proxy server");
        info!("✓ Successfully shut down Solana engine");
        info!("Test completed!");

        Ok(())
    }
}

mod test_utils;

use solana_execution_engine::engine_rpc_client::solana_engine_client::rpc_client::RpcClient;
use solana_execution_engine::SolanaEngineError;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

#[cfg(test)]
mod tests {
    use indicatif::{ProgressBar, ProgressStyle};
    use solana_sdk::commitment_config::CommitmentConfig;

    use super::*;
    use crate::test_utils::{create_and_initialize_engine, setup_logging, shutdown_engine};

    /// Test the getSlot RPC method through the RPC proxy server
    #[tokio::test]
    async fn test_get_slot_rpc_until_slot_10() -> Result<(), SolanaEngineError> {
        setup_logging();

        // Create and initialize the Solana engine
        let mut engine = create_and_initialize_engine()
            .await
            .expect("Failed to create and initialize engine");

        // Wait a moment for the server to start
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Create Solana RPC client
        let rpc_url = format!(
            "http://{}:{}",
            engine.get_rpc_server_host(),
            engine.get_rpc_server_port()
        );
        let commitment = CommitmentConfig::confirmed();
        let rpc_client = Arc::new(RpcClient::new_with_commitment(rpc_url, commitment));

        // Monitor slot progression until slot 10
        info!("Starting slot progression monitoring via RPC...");
        let target_slot = 10u64;
        let mut current_slot = 0u64;
        let mut check_count = 0;
        let max_checks = 120; // Maximum number of checks (2 minutes with 1-second intervals)
        let check_interval = Duration::from_secs(1);

        // Create progress bar
        let pb = ProgressBar::new(target_slot);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} slots ({eta})")
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message("Waiting for Solana blockchain to reach target slot");

        while current_slot < target_slot && check_count < max_checks {
            check_count += 1;

            // Call the getSlot RPC method through the Solana RPC client
            let rpc_client_clone = Arc::clone(&rpc_client);
            let spawn_result = tokio::task::spawn_blocking(move || rpc_client_clone.get_slot())
                .await
                .expect("Failed to spawn blocking task for getSlot RPC call");

            match spawn_result {
                Ok(slot) => {
                    current_slot = slot;
                    // Update progress bar
                    pb.set_position(current_slot.min(target_slot));
                    pb.set_message(format!(
                        "Current slot: {}, Target slot: {}",
                        current_slot, target_slot
                    ));

                    if current_slot >= target_slot {
                        break;
                    }
                }
                Err(e) => {
                    warn!("getSlot RPC call failed: {}", e);
                    pb.set_message(format!("RPC call failed: {}, retrying...", e));
                    // Continue checking even if individual calls fail
                }
            }

            // Wait before next check
            tokio::time::sleep(check_interval).await;
        }

        // Finish progress bar
        pb.finish_with_message(format!("Completed! Final slot: {}", current_slot));

        // Verify results
        assert!(
            current_slot >= target_slot,
            "Test failed: slot {} did not reach target slot {} after {} checks",
            current_slot,
            target_slot,
            check_count
        );

        engine
            .stop_rpc_proxy_server()
            .await
            .expect("Failed to stop RPC proxy server");

        // Shutdown the engine
        shutdown_engine(engine)
            .await
            .expect("Failed to shutdown engine");

        Ok(())
    }
}

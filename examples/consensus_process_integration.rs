//! Example: Malachite Consensus with Process Coordination
//!
//! This example demonstrates how the Malachite BFT consensus engine coordinates
//! with external Reth and Solana processes for transaction execution.
//!
//! Key Architecture Points:
//! - MultiVM uses Malachite for consensus on transaction ordering
//! - MultiVM does NOT execute transactions itself
//! - Execution is delegated to external Reth (Ethereum) and Solana processes
//! - The consensus layer ensures all nodes agree on transaction order
//! - The process coordinator handles actual execution through IPC

use multivm_common::error::MultivmResult;
use multivm_consensus::{
    block::{EvmTransaction, MultiVMBlock, SvmTransaction},
    malachite::ValidatorInfo,
    process_integration::{
        ProcessConsensusConfig, ProcessConsensusCoordinator, ProcessExecutionConfig,
    },
    MalachiteConfig,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

#[tokio::main]
async fn main() -> MultivmResult<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== MultiVM Consensus Process Integration Demo ===");
    info!("Architecture: Malachite Consensus → Process Coordinator → External Processes");

    // Configure Malachite consensus
    let consensus_config = MalachiteConfig {
        node_id: "demo-node-1".to_string(),
        network_config: Default::default(),
        consensus_params: Default::default(),
        validators: vec![
            ValidatorInfo {
                public_key: "demo-node-1".to_string(),
                voting_power: 100,
            },
            ValidatorInfo {
                public_key: "demo-node-2".to_string(),
                voting_power: 100,
            },
            ValidatorInfo {
                public_key: "demo-node-3".to_string(),
                voting_power: 100,
            },
        ],
    };

    // Configure process execution
    let execution_config = ProcessExecutionConfig {
        reth_endpoint: "127.0.0.1:8545".to_string(),
        solana_endpoint: "127.0.0.1:8899".to_string(),
        execution_timeout_ms: 30000,
        max_retries: 3,
    };

    // Create process consensus configuration
    let config = ProcessConsensusConfig {
        consensus_config,
        execution_config,
        enable_batching: false,
        max_batch_size: 10,
        batch_timeout_ms: 1000,
    };

    // Create and start the coordinator
    info!("Creating process consensus coordinator...");
    let mut coordinator = ProcessConsensusCoordinator::new(config).await?;

    info!("Starting consensus coordinator...");
    coordinator.start().await?;

    // Simulate block production
    info!("\n=== Starting Block Production and Execution ===");

    for height in 1..=5 {
        info!("\n--- Block {} ---", height);

        // Create a block with mixed transactions
        let block = create_demo_block(height);
        info!(
            "Created block with {} transactions",
            block.transaction_count()
        );

        // Submit block for consensus
        info!("Submitting block to Malachite consensus...");
        coordinator.submit_block(block).await?;

        // Wait for consensus and execution
        sleep(Duration::from_secs(2)).await;

        // Display metrics
        let metrics = coordinator.get_metrics().await;
        info!("Consensus Metrics:");
        info!("  - Blocks received: {}", metrics.blocks_received);
        info!("  - Blocks committed: {}", metrics.blocks_committed);
        info!("  - Blocks executed: {}", metrics.blocks_executed);
        info!("  - Reth transactions: {}", metrics.reth_transactions);
        info!("  - Solana transactions: {}", metrics.solana_transactions);

        // Display execution state
        let exec_state = coordinator.get_execution_state().await;
        info!("Execution State:");
        info!(
            "  - Last executed height: {}",
            exec_state.last_executed_height
        );

        if let Some(last_record) = exec_state.execution_history.last() {
            info!("  - Last block stats:");
            info!(
                "    - Total transactions: {}",
                last_record.total_transactions
            );
            info!("    - Successful: {}", last_record.successful);
            info!("    - Failed: {}", last_record.failed);
            info!("    - Duration: {}ms", last_record.duration_ms);
        }
    }

    // Final statistics
    sleep(Duration::from_secs(2)).await;
    info!("\n=== Final Statistics ===");

    let final_metrics = coordinator.get_metrics().await;
    let final_state = coordinator.get_execution_state().await;
    let stats = final_state.get_stats();

    info!("Consensus Performance:");
    info!("  - Total blocks: {}", final_metrics.blocks_committed);
    info!(
        "  - Execution success rate: {:.2}%",
        stats.success_rate * 100.0
    );
    info!(
        "  - Average block execution time: {}ms",
        stats.avg_block_execution_time_ms
    );

    info!("\n=== Architecture Summary ===");
    info!("1. Malachite BFT ensures consensus on block ordering");
    info!("2. Committed blocks are sent to the Process Coordinator");
    info!("3. Process Coordinator routes transactions to:");
    info!("   - Reth process for Ethereum transactions");
    info!("   - Solana process for Solana transactions");
    info!("4. MultiVM NEVER executes transactions directly");
    info!("5. All execution happens in external processes");

    Ok(())
}

/// Create a demo block with mixed transaction types
fn create_demo_block(height: u64) -> MultiVMBlock {
    let mut block = MultiVMBlock::new(
        height,
        if height == 1 {
            "0x0".to_string()
        } else {
            format!("0x{:x}", height - 1)
        },
        "demo-node-1".to_string(),
        vec![],
    );

    // Add Ethereum transactions (will be routed to Reth)
    for i in 0..3 {
        let tx = EvmTransaction::new(
            format!("0x{:040x}", height * 1000 + i), // from address
            Some(format!("0x{:040x}", i + 1)),       // to address
            1000 + i * 100,                          // value
            21000,                                   // gas limit
            20,                                      // gas price
            format!("eth_tx_{}_{}", height, i).into_bytes(), // data
            i,                                       // nonce
        );
        block.add_evm_transaction(tx);
    }

    // Add Solana transactions (will be routed to Solana)
    for i in 0..2 {
        let tx = SvmTransaction::new(
            vec![format!("sig_{}_{}", height, i)],           // signatures
            format!("sol_tx_{}_{}", height, i).into_bytes(), // data
            vec![
                format!("account_{}_{}_1", height, i),
                format!("account_{}_{}_2", height, i),
            ], // accounts
        );
        block.add_svm_transaction(tx);
    }

    block
}

// Example output:
// ```
// === MultiVM Consensus Process Integration Demo ===
// Architecture: Malachite Consensus → Process Coordinator → External Processes
// Creating process consensus coordinator...
// Starting consensus coordinator...
//
// === Starting Block Production and Execution ===
//
// --- Block 1 ---
// Created block with 5 transactions
// Submitting block to Malachite consensus...
// [MOCK] Sending transaction to Reth process: tx_65746831
// [MOCK] Sending transaction to Solana process: tx_736f6c31
// Consensus Metrics:
//   - Blocks received: 1
//   - Blocks committed: 1
//   - Blocks executed: 1
//   - Reth transactions: 3
//   - Solana transactions: 2
// Execution State:
//   - Last executed height: 1
//   - Last block stats:
//     - Total transactions: 5
//     - Successful: 5
//     - Failed: 0
//     - Duration: 350ms
//
// === Architecture Summary ===
// 1. Malachite BFT ensures consensus on block ordering
// 2. Committed blocks are sent to the Process Coordinator
// 3. Process Coordinator routes transactions to:
//    - Reth process for Ethereum transactions
//    - Solana process for Solana transactions
// 4. MultiVM NEVER executes transactions directly
// 5. All execution happens in external processes
// ```

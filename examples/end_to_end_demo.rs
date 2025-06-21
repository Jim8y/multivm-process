//! End-to-End MultiVM System Demo
//!
//! This example demonstrates the complete MultiVM system with:
//! - Malachite consensus
//! - Block routing and decomposition
//! - Account mapping with cryptographic verification
//! - Secure IPC communication
//! - Real Solana and Reth execution engines

use multivm_account_mapping::{
    AccountAddress, BindingProof, EthereumAddress, ProofType, SolanaAddress, SpecialTransaction,
};
use multivm_consensus::{
    EvmSignature, EvmTransaction, MalachiteConfig, MultiVMBlock, SvmTransaction,
};
use multivm_process_manager::{CoordinatorConfig, MultivmCoordinator};
use std::time::{Duration, SystemTime};
use tokio::time::sleep;
use tracing::{error, info};

/// Demo configuration
struct DemoConfig {
    /// Run duration
    duration: Duration,
    /// Block generation interval
    block_interval: Duration,
    /// Number of test transactions per block
    transactions_per_block: usize,
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(60),
            block_interval: Duration::from_secs(5),
            transactions_per_block: 10,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("multivm=debug,end_to_end_demo=info")
        .init();

    info!("🚀 Starting MultiVM End-to-End Demo");

    let demo_config = DemoConfig::default();

    // Run the demo
    match run_demo(demo_config).await {
        Ok(_) => {
            info!("✅ Demo completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("❌ Demo failed: {}", e);
            Err(e)
        }
    }
}

async fn run_demo(config: DemoConfig) -> Result<(), Box<dyn std::error::Error>> {
    info!("📋 Demo Configuration:");
    info!("  Duration: {:?}", config.duration);
    info!("  Block interval: {:?}", config.block_interval);
    info!(
        "  Transactions per block: {}",
        config.transactions_per_block
    );

    // Step 1: Initialize the MultiVM Coordinator
    info!("🔧 Initializing MultiVM Coordinator...");
    let coordinator_config = CoordinatorConfig {
        consensus: MalachiteConfig::default(),
        health_check_interval: Duration::from_secs(10),
        block_timeout: Duration::from_secs(30),
        max_concurrent_blocks: 5,
        enable_recovery: true,
    };

    let mut coordinator = MultivmCoordinator::new(coordinator_config).await?;

    // Step 2: Start the system
    info!("▶️  Starting MultiVM system...");
    coordinator.start().await?;

    // Step 3: Wait for system initialization
    info!("⏳ Waiting for system initialization...");
    sleep(Duration::from_secs(3)).await;

    // Step 4: Check system health
    info!("🏥 Checking system health...");
    let health_status = coordinator.get_health_status().await?;
    info!(
        "System health: healthy={}, coordinator={}, processes={}, consensus={}",
        health_status.is_healthy,
        health_status.coordinator_running,
        health_status.processes_healthy,
        health_status.consensus_healthy
    );

    // Step 5: Create sample accounts for testing
    info!("👥 Creating sample accounts...");
    let accounts = create_sample_accounts().await?;
    info!("Created {} sample accounts", accounts.len());

    // Step 6: Generate and process sample blocks
    info!("🧱 Starting block generation and processing...");
    let start_time = std::time::Instant::now();
    let mut block_height = 1u64;

    while start_time.elapsed() < config.duration {
        // Generate a sample block
        let block =
            create_sample_block(block_height, &accounts, config.transactions_per_block).await?;

        info!(
            "📦 Submitting block {} with {} transactions",
            block_height,
            block.transaction_count()
        );

        // Submit block for processing
        if let Err(e) = coordinator.submit_block(block).await {
            error!("Failed to submit block {}: {}", block_height, e);
        } else {
            info!("✅ Block {} submitted successfully", block_height);
        }

        block_height += 1;

        // Wait for next block
        sleep(config.block_interval).await;

        // Periodically check system health
        if block_height % 5 == 0 {
            let health = coordinator.get_health_status().await?;
            info!(
                "📊 System metrics: blocks_processed={}, errors={}, recovery_count={}",
                health.system_metrics.total_transactions_processed,
                health.system_metrics.error_count,
                health.system_metrics.recovery_count
            );
        }
    }

    // Step 7: Final system state
    info!("📈 Final system state:");
    let final_state = coordinator.get_state().await;
    info!("  Blocks processed: {}", final_state.blocks_processed);
    info!("  Last block height: {}", final_state.last_block_height);
    info!("  Active processes: {:?}", final_state.active_processes);

    let final_health = coordinator.get_health_status().await?;
    info!(
        "  Total transactions: {}",
        final_health.system_metrics.total_transactions_processed
    );
    info!(
        "  Total errors: {}",
        final_health.system_metrics.error_count
    );
    info!(
        "  Average consensus latency: {}ms",
        final_health.system_metrics.consensus_latency_ms
    );

    // Step 8: Shutdown the system
    info!("🛑 Shutting down MultiVM system...");
    coordinator.stop().await?;

    info!("🎉 Demo completed successfully!");
    Ok(())
}

/// Create sample accounts for testing
async fn create_sample_accounts() -> Result<Vec<AccountAddress>, Box<dyn std::error::Error>> {
    let mut accounts = Vec::new();

    // Create Solana accounts
    for i in 0..5 {
        let mut addr = [0u8; 32];
        addr[0] = i + 1; // Simple address generation
        accounts.push(AccountAddress::Solana(SolanaAddress(addr)));
    }

    // Create Ethereum accounts
    for i in 0..5 {
        let mut addr = [0u8; 20];
        addr[0] = i + 1; // Simple address generation
        accounts.push(AccountAddress::Ethereum(EthereumAddress(addr)));
    }

    Ok(accounts)
}

/// Create a sample block with various transaction types
async fn create_sample_block(
    height: u64,
    accounts: &[AccountAddress],
    tx_count: usize,
) -> Result<MultiVMBlock, Box<dyn std::error::Error>> {
    use uuid::Uuid;

    // Create sample SVM transactions
    let mut svm_transactions = Vec::new();
    for i in 0..tx_count / 2 {
        svm_transactions.push(SvmTransaction {
            id: Uuid::new_v4(),
            signatures: vec![format!("sig_{}", i)],
            data: format!("svm_tx_{}_{}", height, i).into_bytes(),
            accounts: vec![
                format!("svm_account_{}", i % accounts.len()),
                format!("svm_account_{}", (i + 1) % accounts.len()),
            ],
            recent_blockhash: format!("block_hash_{}", height),
            fee: 5000,
            metadata: serde_json::json!({
                "tx_type": "transfer",
                "amount": 1000 + (i as u64 * 100)
            }),
        });
    }

    // Create sample EVM transactions
    let mut evm_transactions = Vec::new();
    for i in 0..tx_count / 2 {
        evm_transactions.push(EvmTransaction {
            id: Uuid::new_v4(),
            hash: format!(
                "0x{}",
                hex::encode(
                    blake3::hash(&format!("evm_tx_{}_{}", height, i).as_bytes()).as_bytes()
                )
            ),
            from: format!("0x{:040x}", i),
            to: Some(format!("0x{:040x}", i + 1)),
            value: 1000000000000000000u64 + (i as u64 * 1000000000000000u64), // 1 ETH + variation
            gas_price: 20000000000,                                           // 20 gwei
            gas_limit: 21000,
            nonce: i as u64,
            data: Vec::new(),
            signature: EvmSignature {
                v: 27,
                r: "0x1".to_string(),
                s: "0x1".to_string(),
            },
            metadata: serde_json::json!({
                "tx_type": "transfer"
            }),
        });
    }

    // Create sample special transactions
    let mut multivm_transactions = Vec::new();
    if accounts.len() >= 2 {
        let timestamp = SystemTime::now();
        let proof = BindingProof {
            account: accounts[1].clone(),
            proof_type: ProofType::Signature {
                message: format!("bind_{}_{}", height, 1).into_bytes(),
                signature: vec![1u8; 64], // Mock signature
            },
            proof_data: Vec::new(),
            timestamp,
        };

        multivm_transactions.push(SpecialTransaction::AccountBinding {
            source_account: accounts[0].clone(),
            target_account: accounts[1].clone(),
            proof,
            metadata: None,
        });
    }

    // Create the block
    let mut block = MultiVMBlock::new(
        height,
        format!("previous_hash_{}", height - 1),
        format!("node_0"),
        vec![],
    );

    // Add transactions
    for tx in svm_transactions {
        block.add_svm_transaction(tx);
    }
    for tx in evm_transactions {
        block.add_evm_transaction(tx);
    }
    for tx in multivm_transactions {
        block.add_multivm_transaction(tx);
    }

    // Finalize the block
    block.finalize();

    Ok(block)
}

/// Run basic functionality tests
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sample_account_creation() {
        let accounts = create_sample_accounts().await.unwrap();
        assert_eq!(accounts.len(), 10);

        // Check that we have both types of accounts
        let solana_count = accounts
            .iter()
            .filter(|a| matches!(a, AccountAddress::Solana(_)))
            .count();
        let ethereum_count = accounts
            .iter()
            .filter(|a| matches!(a, AccountAddress::Ethereum(_)))
            .count();

        assert_eq!(solana_count, 5);
        assert_eq!(ethereum_count, 5);
    }

    #[tokio::test]
    async fn test_sample_block_creation() {
        let accounts = create_sample_accounts().await.unwrap();
        let block = create_sample_block(1, &accounts, 4).await.unwrap();

        assert_eq!(block.header.height, 1);
        assert!(block.transaction_count() > 0);
        assert!(!block.header.transactions_root.is_empty());
        assert!(!block.header.previous_hash.is_empty());
    }

    #[tokio::test]
    async fn test_coordinator_initialization() {
        let config = CoordinatorConfig::default();
        let coordinator = MultivmCoordinator::new(config).await;
        assert!(coordinator.is_ok());
    }
}

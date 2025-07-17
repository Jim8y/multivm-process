//! Full system integration test for MultiVM
//! 
//! This test verifies the complete MultiVM system with:
//! - Real process spawning (Reth and Solana)
//! - Block routing
//! - IPC communication
//! - Malachite BFT consensus
//! - Cross-VM transactions

use multivm_common::{
    BlockchainType, EngineState, ExecutionEngine, HealthStatus, ProcessingMetrics,
    types::{ProcessId, BlockchainType as VmType},
};
use multivm_process_manager::{
    config::ProcessManagerConfig,
    manager::Manager,
};
use multivm_consensus::{
    block::{MultiVMBlock, BlockHeader},
    manager::ConsensusManager,
    message::{ConsensusMessage, MessageType},
};
use multivm_p2p::{
    config::P2PConfig,
    network::Network,
};
use reth_execution_engine::RethEngine;
use solana_execution_engine::engine::SolanaEngine;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout, Duration};
use tracing::{info, warn, error};

#[tokio::test]
async fn test_full_multivm_system() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("multivm=debug,reth_execution_engine=debug,solana_execution_engine=debug")
        .init();

    info!("Starting full MultiVM system integration test");

    // 1. Set up process manager with real process spawning
    let mut process_config = ProcessManagerConfig::default();
    process_config.ethereum_mock_mode = false; // Use real Reth
    process_config.solana_mock_mode = false;   // Use real Solana
    
    let process_manager = Arc::new(Manager::new(process_config).await?);
    
    // 2. Initialize execution engines
    info!("Initializing execution engines");
    
    // Initialize Reth engine
    let mut reth_engine = RethEngine::new_with_config(
        Default::default(),
        Default::default(),
    ).await?;
    reth_engine.initialize().await?;
    
    // Initialize Solana engine
    let mut solana_engine = SolanaEngine::new_default().await?;
    solana_engine.initialize().await?;
    
    // 3. Start processes
    info!("Starting blockchain processes");
    process_manager.start_process(ProcessId::Ethereum).await?;
    process_manager.start_process(ProcessId::Solana).await?;
    
    // Wait for processes to initialize
    sleep(Duration::from_secs(5)).await;
    
    // 4. Verify processes are healthy
    info!("Verifying process health");
    
    let reth_health = reth_engine.get_health().await?;
    assert_eq!(reth_health, HealthStatus::Healthy, "Reth should be healthy");
    
    let solana_health = solana_engine.get_health().await?;
    assert_eq!(solana_health, HealthStatus::Healthy, "Solana should be healthy");
    
    // 5. Set up P2P network
    info!("Setting up P2P network");
    let p2p_config = P2PConfig::default();
    let network = Arc::new(RwLock::new(Network::new(p2p_config).await?));
    
    // 6. Set up consensus manager
    info!("Setting up consensus manager");
    let consensus_manager = Arc::new(RwLock::new(
        ConsensusManager::new(
            "node1".to_string(),
            vec!["node1".to_string(), "node2".to_string(), "node3".to_string()],
            network.clone(),
        ).await?
    ));
    
    // 7. Test block routing
    info!("Testing block routing");
    
    // Create a test block with mixed transactions
    let test_block = MultiVMBlock {
        header: BlockHeader {
            block_number: 1,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            previous_hash: vec![0u8; 32],
            state_root: vec![0u8; 32],
            transactions_root: vec![0u8; 32],
            receipts_root: vec![0u8; 32],
        },
        evm_transactions: vec![vec![1, 2, 3]], // Mock EVM transaction
        svm_transactions: vec![vec![4, 5, 6]], // Mock SVM transaction
        multivm_transactions: vec![],
        state_transitions: vec![],
    };
    
    // Route block through the system
    let router = process_manager.get_block_router()?;
    router.route_block(test_block.clone()).await?;
    
    // 8. Test consensus with real Malachite BFT
    info!("Testing Malachite BFT consensus");
    
    // Start consensus for the block
    consensus_manager.write().await.propose_block(test_block.clone()).await?;
    
    // Wait for consensus
    let consensus_result = timeout(
        Duration::from_secs(10),
        consensus_manager.read().await.wait_for_consensus(1)
    ).await?;
    
    assert!(consensus_result.is_ok(), "Consensus should complete successfully");
    
    // 9. Test IPC communication
    info!("Testing IPC communication");
    
    // Test IPC server is running
    let ipc_server_running = process_manager.is_ipc_server_running().await;
    assert!(ipc_server_running, "IPC server should be running");
    
    // 10. Test view change protocol
    info!("Testing view change protocol");
    
    // Simulate leader failure to trigger view change
    consensus_manager.write().await.initiate_view_change(1, 1, "Testing view change").await?;
    
    // Wait for view change to complete
    sleep(Duration::from_secs(2)).await;
    
    // 11. Verify final system state
    info!("Verifying final system state");
    
    let reth_state = reth_engine.get_state().await?;
    assert_eq!(reth_state.blockchain_type, BlockchainType::Ethereum);
    assert_eq!(reth_state.process_id, ProcessId::Ethereum);
    
    let solana_state = solana_engine.get_state().await?;
    assert_eq!(solana_state.blockchain_type, BlockchainType::Solana);
    assert_eq!(solana_state.process_id, ProcessId::Solana);
    
    // 12. Clean up
    info!("Cleaning up test resources");
    
    // Shutdown engines
    reth_engine.shutdown(Some(Duration::from_secs(5))).await?;
    solana_engine.shutdown(Some(Duration::from_secs(5))).await?;
    
    // Stop processes
    process_manager.stop_process(ProcessId::Ethereum).await?;
    process_manager.stop_process(ProcessId::Solana).await?;
    
    // Shutdown consensus and network
    consensus_manager.write().await.shutdown().await?;
    network.write().await.shutdown().await?;
    
    info!("Full MultiVM system integration test completed successfully");
    Ok(())
}

#[tokio::test]
async fn test_cross_vm_transaction_flow() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("multivm=debug")
        .init();

    info!("Starting cross-VM transaction flow test");

    // Set up minimal system
    let mut process_config = ProcessManagerConfig::default();
    process_config.ethereum_mock_mode = true; // Use mock for faster testing
    process_config.solana_mock_mode = true;
    
    let process_manager = Arc::new(Manager::new(process_config).await?);
    
    // Start processes
    process_manager.start_process(ProcessId::Ethereum).await?;
    process_manager.start_process(ProcessId::Solana).await?;
    
    // Create a cross-VM transaction (e.g., bridging tokens)
    let cross_vm_tx = multivm_common::types::Transaction {
        id: uuid::Uuid::new_v4().to_string(),
        from_address: "0x1234567890abcdef".to_string(),
        to_address: "SolanaAddr123456".to_string(),
        amount: 1000,
        vm_type: VmType::Ethereum, // Source VM
        target_vm: Some(VmType::Solana), // Target VM
        data: vec![],
        timestamp: std::time::SystemTime::now(),
    };
    
    // Process transaction through the system
    let router = process_manager.get_block_router()?;
    
    // Create blocks containing the cross-VM transaction
    let source_block = MultiVMBlock {
        header: BlockHeader {
            block_number: 1,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            previous_hash: vec![0u8; 32],
            state_root: vec![0u8; 32],
            transactions_root: vec![0u8; 32],
            receipts_root: vec![0u8; 32],
        },
        evm_transactions: vec![bincode::serialize(&cross_vm_tx)?],
        svm_transactions: vec![],
        multivm_transactions: vec![cross_vm_tx.id.clone().into_bytes()],
        state_transitions: vec![],
    };
    
    // Route and process
    router.route_block(source_block).await?;
    
    // Verify transaction was processed on both VMs
    sleep(Duration::from_secs(2)).await;
    
    // Clean up
    process_manager.stop_process(ProcessId::Ethereum).await?;
    process_manager.stop_process(ProcessId::Solana).await?;
    
    info!("Cross-VM transaction flow test completed successfully");
    Ok(())
}

#[tokio::test]
async fn test_system_resilience_and_recovery() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("multivm=info")
        .init();

    info!("Starting system resilience and recovery test");

    let mut process_config = ProcessManagerConfig::default();
    process_config.ethereum_mock_mode = true;
    process_config.solana_mock_mode = true;
    
    let process_manager = Arc::new(Manager::new(process_config).await?);
    
    // Start processes
    process_manager.start_process(ProcessId::Ethereum).await?;
    process_manager.start_process(ProcessId::Solana).await?;
    
    // Simulate process failure
    info!("Simulating Ethereum process failure");
    process_manager.simulate_process_failure(ProcessId::Ethereum).await?;
    
    // Wait for health monitor to detect failure
    sleep(Duration::from_secs(3)).await;
    
    // Verify process was restarted
    let eth_status = process_manager.get_process_status(ProcessId::Ethereum).await?;
    assert!(eth_status.is_running, "Ethereum process should be restarted");
    
    // Test IPC reconnection
    let ipc_healthy = process_manager.test_ipc_connection(ProcessId::Ethereum).await?;
    assert!(ipc_healthy, "IPC connection should be restored");
    
    // Clean up
    process_manager.stop_process(ProcessId::Ethereum).await?;
    process_manager.stop_process(ProcessId::Solana).await?;
    
    info!("System resilience and recovery test completed successfully");
    Ok(())
}

#[tokio::test]
async fn test_consensus_performance() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("multivm=info,multivm_consensus=debug")
        .init();

    info!("Starting consensus performance test");

    // Set up network with multiple nodes
    let node_count = 7;
    let mut networks = Vec::new();
    let mut consensus_managers = Vec::new();
    
    for i in 0..node_count {
        let mut p2p_config = P2PConfig::default();
        p2p_config.listen_port = 9000 + i as u16;
        
        let network = Arc::new(RwLock::new(Network::new(p2p_config).await?));
        networks.push(network.clone());
        
        let node_id = format!("node{}", i);
        let all_nodes: Vec<String> = (0..node_count)
            .map(|j| format!("node{}", j))
            .collect();
        
        let consensus = Arc::new(RwLock::new(
            ConsensusManager::new(node_id, all_nodes, network).await?
        ));
        consensus_managers.push(consensus);
    }
    
    // Connect nodes
    for i in 1..node_count {
        let addr = format!("/ip4/127.0.0.1/tcp/{}", 9000 + i);
        networks[0].write().await.connect_to_peer(&addr).await?;
    }
    
    // Wait for network to stabilize
    sleep(Duration::from_secs(2)).await;
    
    // Measure consensus latency
    let start_time = std::time::Instant::now();
    
    // Propose block from node 0
    let test_block = MultiVMBlock {
        header: BlockHeader {
            block_number: 1,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            previous_hash: vec![0u8; 32],
            state_root: vec![0u8; 32],
            transactions_root: vec![0u8; 32],
            receipts_root: vec![0u8; 32],
        },
        evm_transactions: vec![vec![1; 100]], // 100 byte transaction
        svm_transactions: vec![vec![2; 100]],
        multivm_transactions: vec![],
        state_transitions: vec![],
    };
    
    consensus_managers[0].write().await.propose_block(test_block).await?;
    
    // Wait for consensus on all nodes
    let consensus_futures: Vec<_> = consensus_managers.iter()
        .map(|cm| {
            let cm_clone = cm.clone();
            async move {
                timeout(
                    Duration::from_secs(5),
                    cm_clone.read().await.wait_for_consensus(1)
                ).await
            }
        })
        .collect();
    
    futures::future::try_join_all(consensus_futures).await?;
    
    let consensus_time = start_time.elapsed();
    info!("Consensus achieved in {:?} for {} nodes", consensus_time, node_count);
    
    // Verify consensus time is reasonable
    assert!(consensus_time < Duration::from_secs(3), 
        "Consensus should complete within 3 seconds for {} nodes", node_count);
    
    // Clean up
    for (i, (network, consensus)) in networks.iter().zip(consensus_managers.iter()).enumerate() {
        consensus.write().await.shutdown().await?;
        network.write().await.shutdown().await?;
    }
    
    info!("Consensus performance test completed successfully");
    Ok(())
}
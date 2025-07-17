//! Component-level integration tests for MultiVM subsystems
//! 
//! Tests individual components with their real implementations

use multivm_common::{
    BlockchainType, ProcessId, HealthStatus,
    types::{Transaction, VmType},
};
use multivm_process_manager::{
    block_router::BlockRouter,
    config::ProcessManagerConfig,
    ipc::server::IpcServer,
    manager::Manager,
    process::{ProcessHandle, ProcessSpawner},
};
use multivm_consensus::{
    block::{MultiVMBlock, BlockHeader},
    manager::ConsensusManager,
    message::{ConsensusMessage, MessageType, Vote},
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::info;

mod process_manager_tests {
    use super::*;

    #[tokio::test]
    async fn test_real_process_spawning() -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::fmt()
            .with_env_filter("multivm_process_manager=debug")
            .init();

        info!("Testing real process spawning");

        // Test Reth process spawning
        let reth_config = multivm_process_manager::config::ProcessConfig {
            process_id: ProcessId::Ethereum,
            executable_path: "reth".to_string(), // Assumes reth is in PATH
            args: vec![
                "node".to_string(),
                "--dev".to_string(),
                "--dev.block-time".to_string(),
                "1s".to_string(),
            ],
            env_vars: std::collections::HashMap::new(),
            working_dir: None,
            ipc_path: "/tmp/test_reth.ipc".to_string(),
        };

        let spawner = ProcessSpawner::new();
        let handle = spawner.spawn_process(reth_config).await?;
        
        // Verify process is running
        assert!(handle.is_running().await, "Reth process should be running");
        
        // Test graceful shutdown
        handle.shutdown(Some(Duration::from_secs(5))).await?;
        assert!(!handle.is_running().await, "Reth process should be stopped");

        info!("Real process spawning test completed");
        Ok(())
    }

    #[tokio::test]
    async fn test_block_router_functionality() -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::fmt()
            .with_env_filter("multivm_process_manager=debug")
            .init();

        info!("Testing block router functionality");

        let config = ProcessManagerConfig::default();
        let manager = Arc::new(Manager::new(config).await?);
        
        // Start mock processes for testing
        manager.start_process(ProcessId::Ethereum).await?;
        manager.start_process(ProcessId::Solana).await?;
        
        let router = manager.get_block_router()?;
        
        // Create test block with mixed transactions
        let test_block = MultiVMBlock {
            header: BlockHeader {
                block_number: 1,
                timestamp: 1234567890,
                previous_hash: vec![0u8; 32],
                state_root: vec![0u8; 32],
                transactions_root: vec![0u8; 32],
                receipts_root: vec![0u8; 32],
            },
            evm_transactions: vec![
                vec![1, 2, 3], // Mock EVM tx
                vec![4, 5, 6],
            ],
            svm_transactions: vec![
                vec![7, 8, 9], // Mock SVM tx
            ],
            multivm_transactions: vec![],
            state_transitions: vec![],
        };
        
        // Route block
        router.route_block(test_block.clone()).await?;
        
        // Verify routing metrics
        let metrics = router.get_metrics().await;
        assert_eq!(metrics.total_blocks_routed, 1);
        assert_eq!(metrics.evm_transactions_routed, 2);
        assert_eq!(metrics.svm_transactions_routed, 1);
        
        // Clean up
        manager.stop_process(ProcessId::Ethereum).await?;
        manager.stop_process(ProcessId::Solana).await?;
        
        info!("Block router test completed");
        Ok(())
    }

    #[tokio::test]
    async fn test_ipc_server_implementation() -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::fmt()
            .with_env_filter("multivm_process_manager=debug")
            .init();

        info!("Testing IPC server implementation");

        let server_config = multivm_process_manager::ipc::IpcServerConfig {
            socket_path: "/tmp/test_ipc_server.sock".to_string(),
            max_connections: 10,
            buffer_size: 8192,
        };
        
        let server = IpcServer::new(server_config);
        let server_handle = server.start().await?;
        
        // Test client connection
        use tokio::net::UnixStream;
        let client = UnixStream::connect("/tmp/test_ipc_server.sock").await?;
        
        // Send test message
        use tokio::io::{AsyncWriteExt, AsyncReadExt};
        let test_msg = b"PING";
        client.writable().await?;
        let mut writer = tokio::io::BufWriter::new(&client);
        writer.write_all(test_msg).await?;
        writer.flush().await?;
        
        // Wait for response
        let mut buffer = vec![0u8; 1024];
        let mut reader = tokio::io::BufReader::new(&client);
        let n = reader.read(&mut buffer).await?;
        
        // Verify we got a response
        assert!(n > 0, "Should receive response from IPC server");
        
        // Shutdown server
        server_handle.shutdown().await?;
        
        info!("IPC server test completed");
        Ok(())
    }
}

mod consensus_tests {
    use super::*;

    #[tokio::test]
    async fn test_malachite_bft_consensus() -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::fmt()
            .with_env_filter("multivm_consensus=debug")
            .init();

        info!("Testing Malachite BFT consensus implementation");

        // Create network
        let network = Arc::new(RwLock::new(
            multivm_p2p::network::Network::new(Default::default()).await?
        ));
        
        // Create consensus manager
        let consensus = ConsensusManager::new(
            "test_node".to_string(),
            vec!["test_node".to_string(), "node2".to_string(), "node3".to_string()],
            network.clone(),
        ).await?;
        
        // Create test block
        let block = MultiVMBlock {
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
            evm_transactions: vec![],
            svm_transactions: vec![],
            multivm_transactions: vec![],
            state_transitions: vec![],
        };
        
        // Test proposing block
        consensus.propose_block(block.clone()).await?;
        
        // Test vote validation
        let vote = Vote {
            validator_id: "test_node".to_string(),
            block_hash: vec![1, 2, 3],
            round: 1,
            view: 0,
            signature: vec![],
            timestamp: std::time::SystemTime::now(),
        };
        
        let vote_json = serde_json::to_value(&vote)?;
        let is_valid = consensus.validate_vote_comprehensive(
            &vote_json,
            "test_node",
            1,
            0
        ).await?;
        
        assert!(is_valid, "Vote should be valid");
        
        // Test message handling
        let message = ConsensusMessage {
            msg_type: MessageType::Propose,
            sender: "test_node".to_string(),
            round: 1,
            view: 0,
            content: serde_json::to_value(&block)?,
            signature: vec![],
        };
        
        consensus.handle_message(message).await?;
        
        // Shutdown
        consensus.shutdown().await?;
        network.write().await.shutdown().await?;
        
        info!("Malachite BFT consensus test completed");
        Ok(())
    }

    #[tokio::test]
    async fn test_view_change_protocol() -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::fmt()
            .with_env_filter("multivm_consensus=debug")
            .init();

        info!("Testing view change protocol");

        let network = Arc::new(RwLock::new(
            multivm_p2p::network::Network::new(Default::default()).await?
        ));
        
        let mut consensus = ConsensusManager::new(
            "node1".to_string(),
            vec!["node1".to_string(), "node2".to_string(), "node3".to_string()],
            network.clone(),
        ).await?;
        
        // Test initiating view change
        consensus.initiate_view_change(1, 1, "Leader timeout").await?;
        
        // Verify view change message was created
        let current_view = consensus.get_current_view().await;
        assert_eq!(current_view, 1, "View should be incremented");
        
        // Test processing view change message
        let view_change_msg = ConsensusMessage {
            msg_type: MessageType::ViewChange,
            sender: "node2".to_string(),
            round: 1,
            view: 1,
            content: serde_json::json!({
                "reason": "Leader timeout",
                "last_prepared_round": 0,
                "last_prepared_value": null,
            }),
            signature: vec![],
        };
        
        consensus.handle_message(view_change_msg).await?;
        
        // Shutdown
        consensus.shutdown().await?;
        network.write().await.shutdown().await?;
        
        info!("View change protocol test completed");
        Ok(())
    }

    #[tokio::test]
    async fn test_byzantine_fault_tolerance() -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::fmt()
            .with_env_filter("multivm_consensus=debug")
            .init();

        info!("Testing Byzantine fault tolerance");

        let network = Arc::new(RwLock::new(
            multivm_p2p::network::Network::new(Default::default()).await?
        ));
        
        let consensus = ConsensusManager::new(
            "honest_node".to_string(),
            vec!["honest_node".to_string(), "node2".to_string(), "node3".to_string(), "byzantine_node".to_string()],
            network.clone(),
        ).await?;
        
        // Test double voting detection
        let vote1 = Vote {
            validator_id: "byzantine_node".to_string(),
            block_hash: vec![1, 2, 3],
            round: 1,
            view: 0,
            signature: vec![],
            timestamp: std::time::SystemTime::now(),
        };
        
        let vote2 = Vote {
            validator_id: "byzantine_node".to_string(),
            block_hash: vec![4, 5, 6], // Different block hash!
            round: 1,
            view: 0,
            signature: vec![],
            timestamp: std::time::SystemTime::now(),
        };
        
        // Process first vote
        let msg1 = ConsensusMessage {
            msg_type: MessageType::Vote,
            sender: "byzantine_node".to_string(),
            round: 1,
            view: 0,
            content: serde_json::to_value(&vote1)?,
            signature: vec![],
        };
        consensus.handle_message(msg1).await?;
        
        // Try to process second vote (should be rejected)
        let msg2 = ConsensusMessage {
            msg_type: MessageType::Vote,
            sender: "byzantine_node".to_string(),
            round: 1,
            view: 0,
            content: serde_json::to_value(&vote2)?,
            signature: vec![],
        };
        
        let result = consensus.handle_message(msg2).await;
        assert!(result.is_err(), "Double vote should be rejected");
        
        // Shutdown
        consensus.shutdown().await?;
        network.write().await.shutdown().await?;
        
        info!("Byzantine fault tolerance test completed");
        Ok(())
    }
}

mod execution_engine_tests {
    use super::*;
    use reth_execution_engine::RethEngine;
    use solana_execution_engine::engine::SolanaEngine;

    #[tokio::test]
    async fn test_solana_engine_integration() -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::fmt()
            .with_env_filter("solana_execution_engine=debug")
            .init();

        info!("Testing Solana execution engine integration");

        // Create and initialize engine
        let mut engine = SolanaEngine::new_default().await?;
        
        // Skip initialization if solana-private-validator is not available
        match engine.initialize().await {
            Ok(_) => {
                // Test health check
                let health = engine.get_health().await?;
                assert_eq!(health, HealthStatus::Healthy);
                
                // Test state retrieval
                let state = engine.get_state().await?;
                assert_eq!(state.blockchain_type, BlockchainType::Solana);
                assert_eq!(state.process_id, ProcessId::Solana);
                
                // Test block processing
                let block = solana_execution_engine::engine::SolanaBlockData {
                    slot: 1,
                    block_hash: [0u8; 32],
                    transactions: vec![],
                    block_time: Some(1234567890),
                    parent_slot: 0,
                    previous_blockhash: [0u8; 32],
                };
                
                let result = engine.process_block(block).await?;
                assert_eq!(result.slot, 1);
                
                // Shutdown
                engine.shutdown(Some(Duration::from_secs(5))).await?;
            }
            Err(e) => {
                info!("Skipping Solana engine test: {}", e);
            }
        }
        
        info!("Solana execution engine test completed");
        Ok(())
    }

    #[tokio::test]
    async fn test_reth_engine_integration() -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::fmt()
            .with_env_filter("reth_execution_engine=debug")
            .init();

        info!("Testing Reth execution engine integration");

        // Create and initialize engine
        let mut engine = RethEngine::new_with_config(
            Default::default(),
            Default::default(),
        ).await?;
        
        // Skip initialization if reth is not available
        match engine.initialize().await {
            Ok(_) => {
                // Test health check
                let health = engine.get_health().await?;
                assert_eq!(health, HealthStatus::Healthy);
                
                // Test state retrieval
                let state = engine.get_state().await?;
                assert_eq!(state.blockchain_type, BlockchainType::Ethereum);
                assert_eq!(state.process_id, ProcessId::Ethereum);
                
                // Shutdown
                engine.shutdown(Some(Duration::from_secs(5))).await?;
            }
            Err(e) => {
                info!("Skipping Reth engine test: {}", e);
            }
        }
        
        info!("Reth execution engine test completed");
        Ok(())
    }
}
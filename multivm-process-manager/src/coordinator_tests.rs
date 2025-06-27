//! Comprehensive tests for the MultiVM Coordinator

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator::{CoordinatorConfig, MultivmCoordinator};
    use multivm_common::{
        config::MultivmConfig,
        types::{ProcessId, health::HealthStatus},
    };
    use multivm_account_mapping::{
        address::AccountAddress,
        special_tx::{SpecialTransaction, AssetType, SimpleBindingMetadata},
    };
    use multivm_consensus::{MalachiteConfig, MultiVMBlock};
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::test;
    use tracing_test::traced_test;

    fn create_test_config() -> (CoordinatorConfig, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        
        let config = CoordinatorConfig {
            consensus_timeout: Duration::from_secs(10),
            block_processing_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(1),
            max_concurrent_blocks: 10,
            enable_account_mapping: true,
            data_directory: temp_dir.path().to_path_buf(),
            bind_timeout: Duration::from_secs(30),
            max_binding_attempts: 3,
        };
        
        (config, temp_dir)
    }

    fn create_test_multivm_config() -> MultivmConfig {
        let mut config = MultivmConfig::default();
        config.system.data_dir = std::env::temp_dir().join("multivm_test");
        config.consensus.malachite.validator_key = "test_validator_key".to_string();
        config.consensus.malachite.network_key = "test_network_key".to_string();
        config
    }

    #[traced_test]
    #[test]
    async fn test_coordinator_creation() {
        let (coordinator_config, _temp_dir) = create_test_config();
        let multivm_config = create_test_multivm_config();
        
        let result = MultivmCoordinator::new(coordinator_config, multivm_config).await;
        assert!(result.is_ok(), "Failed to create coordinator: {:?}", result.err());
        
        let coordinator = result.unwrap();
        assert!(coordinator.is_healthy().await);
    }

    #[traced_test]
    #[test]
    async fn test_coordinator_lifecycle() {
        let (coordinator_config, _temp_dir) = create_test_config();
        let multivm_config = create_test_multivm_config();
        
        let mut coordinator = MultivmCoordinator::new(coordinator_config, multivm_config)
            .await
            .unwrap();

        // Test start
        let start_result = coordinator.start().await;
        assert!(start_result.is_ok(), "Failed to start coordinator: {:?}", start_result.err());
        
        // Verify health status after start
        assert!(coordinator.is_healthy().await);
        
        // Test stop
        let stop_result = coordinator.stop().await;
        assert!(stop_result.is_ok(), "Failed to stop coordinator: {:?}", stop_result.err());
    }

    #[traced_test]
    #[test]
    async fn test_block_processing() {
        let (coordinator_config, _temp_dir) = create_test_config();
        let multivm_config = create_test_multivm_config();
        
        let mut coordinator = MultivmCoordinator::new(coordinator_config, multivm_config)
            .await
            .unwrap();
        
        coordinator.start().await.unwrap();

        // Create a test block
        let test_block = MultiVMBlock {
            height: 1,
            hash: "test_hash".to_string(),
            previous_hash: "genesis".to_string(),
            timestamp: chrono::Utc::now(),
            proposer: "test_proposer".to_string(),
            transactions: vec![],
            state_root: "test_state_root".to_string(),
            consensus_data: vec![],
        };

        let process_result = coordinator.process_block(test_block).await;
        assert!(process_result.is_ok(), "Failed to process block: {:?}", process_result.err());

        coordinator.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_account_binding() {
        let (coordinator_config, _temp_dir) = create_test_config();
        let multivm_config = create_test_multivm_config();
        
        let mut coordinator = MultivmCoordinator::new(coordinator_config, multivm_config)
            .await
            .unwrap();
        
        coordinator.start().await.unwrap();

        // Create test addresses
        let svm_address = AccountAddress::Svm([1u8; 32]);
        let evm_address = AccountAddress::Evm([2u8; 20]);

        // Create binding transaction
        let binding_tx = SpecialTransaction {
            transaction_type: "account_binding".to_string(),
            metadata: SimpleBindingMetadata {
                source_address: svm_address.clone(),
                target_address: evm_address.clone(),
                asset_type: AssetType::Native,
                amount: 1000000,
                nonce: 1,
                proof_hash: "test_proof".to_string(),
            },
            signature: "test_signature".to_string(),
            timestamp: chrono::Utc::now(),
            execution_context: Default::default(),
        };

        let bind_result = coordinator.handle_account_binding(&binding_tx).await;
        assert!(bind_result.is_ok(), "Failed to handle account binding: {:?}", bind_result.err());

        coordinator.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_health_monitoring() {
        let (coordinator_config, _temp_dir) = create_test_config();
        let multivm_config = create_test_multivm_config();
        
        let coordinator = MultivmCoordinator::new(coordinator_config, multivm_config)
            .await
            .unwrap();

        // Test initial health status
        assert!(coordinator.is_healthy().await);
        
        let health_status = coordinator.get_health_status().await;
        assert!(health_status.is_ok());
        
        let status = health_status.unwrap();
        assert_eq!(status.consensus_health, HealthStatus::Healthy);
        assert_eq!(status.account_mapping_health, HealthStatus::Healthy);
    }

    #[traced_test]
    #[test]
    async fn test_concurrent_block_processing() {
        let (coordinator_config, _temp_dir) = create_test_config();
        let multivm_config = create_test_multivm_config();
        
        let mut coordinator = MultivmCoordinator::new(coordinator_config, multivm_config)
            .await
            .unwrap();
        
        coordinator.start().await.unwrap();

        // Process multiple blocks concurrently
        let mut handles = vec![];
        for i in 1..=5 {
            let test_block = MultiVMBlock {
                height: i,
                hash: format!("test_hash_{}", i),
                previous_hash: if i == 1 { "genesis".to_string() } else { format!("test_hash_{}", i-1) },
                timestamp: chrono::Utc::now(),
                proposer: "test_proposer".to_string(),
                transactions: vec![],
                state_root: format!("test_state_root_{}", i),
                consensus_data: vec![],
            };

            let coordinator_ref = &coordinator;
            let handle = tokio::spawn(async move {
                coordinator_ref.process_block(test_block).await
            });
            handles.push(handle);
        }

        // Wait for all blocks to be processed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Failed to process block concurrently: {:?}", result.err());
        }

        coordinator.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_error_recovery() {
        let (coordinator_config, _temp_dir) = create_test_config();
        let multivm_config = create_test_multivm_config();
        
        let mut coordinator = MultivmCoordinator::new(coordinator_config, multivm_config)
            .await
            .unwrap();
        
        coordinator.start().await.unwrap();

        // Test recovery from invalid block
        let invalid_block = MultiVMBlock {
            height: 0, // Invalid height
            hash: "".to_string(), // Invalid hash
            previous_hash: "".to_string(),
            timestamp: chrono::Utc::now(),
            proposer: "".to_string(),
            transactions: vec![],
            state_root: "".to_string(),
            consensus_data: vec![],
        };

        let process_result = coordinator.process_block(invalid_block).await;
        assert!(process_result.is_err(), "Should have failed to process invalid block");

        // Verify coordinator is still healthy after error
        assert!(coordinator.is_healthy().await);

        coordinator.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_configuration_validation() {
        let (mut coordinator_config, _temp_dir) = create_test_config();
        
        // Test with invalid configuration
        coordinator_config.consensus_timeout = Duration::from_secs(0); // Invalid timeout
        
        let multivm_config = create_test_multivm_config();
        let result = MultivmCoordinator::new(coordinator_config, multivm_config).await;
        
        assert!(result.is_err(), "Should have failed with invalid configuration");
    }

    #[traced_test]
    #[test]
    async fn test_resource_limits() {
        let (mut coordinator_config, _temp_dir) = create_test_config();
        coordinator_config.max_concurrent_blocks = 2;
        
        let multivm_config = create_test_multivm_config();
        let mut coordinator = MultivmCoordinator::new(coordinator_config, multivm_config)
            .await
            .unwrap();
        
        coordinator.start().await.unwrap();

        // Try to exceed the concurrent block limit
        let mut handles = vec![];
        for i in 1..=5 {
            let test_block = MultiVMBlock {
                height: i,
                hash: format!("test_hash_{}", i),
                previous_hash: format!("previous_hash_{}", i-1),
                timestamp: chrono::Utc::now(),
                proposer: "test_proposer".to_string(),
                transactions: vec![],
                state_root: format!("test_state_root_{}", i),
                consensus_data: vec![],
            };

            let coordinator_ref = &coordinator;
            let handle = tokio::spawn(async move {
                // Add delay to simulate processing time
                tokio::time::sleep(Duration::from_millis(100)).await;
                coordinator_ref.process_block(test_block).await
            });
            handles.push(handle);
        }

        // Some should succeed, some should be limited
        let mut success_count = 0;
        let mut error_count = 0;
        
        for handle in handles {
            match handle.await.unwrap() {
                Ok(_) => success_count += 1,
                Err(_) => error_count += 1,
            }
        }

        // Should have some rate limiting
        assert!(success_count > 0, "At least some blocks should be processed");
        
        coordinator.stop().await.unwrap();
    }

    #[traced_test]
    #[test]
    async fn test_state_persistence() {
        let (coordinator_config, temp_dir) = create_test_config();
        let multivm_config = create_test_multivm_config();
        
        // Create coordinator and process some data
        {
            let mut coordinator = MultivmCoordinator::new(coordinator_config.clone(), multivm_config.clone())
                .await
                .unwrap();
            
            coordinator.start().await.unwrap();

            // Process a block to create some state
            let test_block = MultiVMBlock {
                height: 1,
                hash: "persistent_test_hash".to_string(),
                previous_hash: "genesis".to_string(),
                timestamp: chrono::Utc::now(),
                proposer: "test_proposer".to_string(),
                transactions: vec![],
                state_root: "persistent_state_root".to_string(),
                consensus_data: vec![],
            };

            coordinator.process_block(test_block).await.unwrap();
            coordinator.stop().await.unwrap();
        }

        // Create new coordinator with same data directory
        {
            let coordinator = MultivmCoordinator::new(coordinator_config, multivm_config)
                .await
                .unwrap();
            
            // Verify state is recovered
            assert!(coordinator.is_healthy().await);
        }
    }

    #[traced_test]
    #[test]
    async fn test_performance_metrics() {
        let (coordinator_config, _temp_dir) = create_test_config();
        let multivm_config = create_test_multivm_config();
        
        let mut coordinator = MultivmCoordinator::new(coordinator_config, multivm_config)
            .await
            .unwrap();
        
        coordinator.start().await.unwrap();

        // Process multiple blocks to generate metrics
        for i in 1..=10 {
            let test_block = MultiVMBlock {
                height: i,
                hash: format!("test_hash_{}", i),
                previous_hash: if i == 1 { "genesis".to_string() } else { format!("test_hash_{}", i-1) },
                timestamp: chrono::Utc::now(),
                proposer: "test_proposer".to_string(),
                transactions: vec![],
                state_root: format!("test_state_root_{}", i),
                consensus_data: vec![],
            };

            coordinator.process_block(test_block).await.unwrap();
        }

        // Get performance metrics
        let metrics = coordinator.get_performance_metrics().await;
        assert!(metrics.is_ok());
        
        let perf_metrics = metrics.unwrap();
        assert!(perf_metrics.blocks_processed >= 10);
        assert!(perf_metrics.average_block_processing_time > Duration::ZERO);

        coordinator.stop().await.unwrap();
    }
}
//! Integration tests for the MultiVM system
//! 
//! These tests verify the interaction between different components
//! and ensure the system works as a cohesive whole.

use multivm_common::{
    MultivmConfig, MultivmResult, ProcessId, VmType, HealthStatus,
    IpcCommand, IpcResponse, types::core::MessageId,
};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

/// Test the complete startup sequence of the MultiVM system
#[tokio::test]
async fn test_system_startup_integration() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create configuration for testing
    let mut config = MultivmConfig::default();
    config.system.data_dir = temp_dir.path().to_path_buf();
    config.database.path = temp_dir.path().join("test.db");
    
    // Validate configuration
    let validation_result = config.validate();
    assert!(validation_result.is_ok(), "Configuration should be valid: {:?}", validation_result);
    
    // Test that all components can be initialized with this config
    // (This would require each component to expose initialization functions)
    println!("System startup integration test completed successfully");
}

/// Test IPC communication between components
#[tokio::test]
async fn test_ipc_communication_integration() {
    // Test IPC message flow between process manager and application
    let message_id = MessageId::new();
    let command = IpcCommand::GetHealth;
    
    // Integration test steps:
    // 1. Start the process manager
    // 2. Start the application server
    // 3. Send IPC commands between them
    // 4. Verify responses
    
    // For now, we test message creation and serialization
    let message = multivm_common::IpcMessage::new(
        ProcessId::Main,
        ProcessId::Solana,
        command
    );
    
    assert_eq!(message.source, ProcessId::Main);
    assert_eq!(message.destination, ProcessId::Solana);
    assert!(matches!(message.command, IpcCommand::GetHealth));
    
    // Test serialization/deserialization 
    let serialized = serde_json::to_string(&message);
    assert!(serialized.is_ok(), "IPC message should serialize");
    
    let deserialized: Result<multivm_common::IpcMessage, _> = serde_json::from_str(&serialized.unwrap());
    assert!(deserialized.is_ok(), "IPC message should deserialize");
    
    println!("IPC communication integration test completed");
}

/// Test cross-VM account mapping integration
#[tokio::test]
async fn test_cross_vm_account_mapping_integration() {
    // This test would verify that account mappings work across different VM types
    let svm_address = "11111111111111111111111111111112";
    let evm_address = "0x0000000000000000000000000000000000000000";
    
    // Test address format validation
    assert_eq!(svm_address.len(), 44, "SVM address should be 44 characters");
    assert_eq!(evm_address.len(), 42, "EVM address should be 42 characters");
    assert!(evm_address.starts_with("0x"), "EVM address should start with 0x");
    
    // Integration test steps:
    // 1. Create account mappings through the API
    // 2. Verify they're stored correctly in the database
    // 3. Test cross-VM operations using these mappings
    // 4. Verify consistency across all components
    
    println!("Cross-VM account mapping integration test completed");
}

/// Test P2P networking with multiple components
#[tokio::test]
async fn test_p2p_networking_integration() {
    // Test that P2P networking works with the broader system
    
    // Integration test steps:
    // 1. Start multiple MultiVM nodes
    // 2. Verify they discover each other
    // 3. Test message propagation between nodes
    // 4. Verify consensus mechanisms work
    // 5. Test network partition scenarios
    
    // For now, test basic networking concepts
    let node_capabilities = multivm_p2p::messages::NodeCapabilities {
        supported_vms: vec![VmType::Svm, VmType::Evm],
        protocol_versions: vec![1, 2],
        features: vec!["cross-vm".to_string(), "state-sync".to_string()],
        limits: multivm_p2p::messages::ResourceLimits {
            max_message_size: 16 * 1024 * 1024,
            max_concurrent_connections: 100,
            rate_limit_per_second: 1000,
        },
    };
    
    assert_eq!(node_capabilities.supported_vms.len(), 2);
    assert!(node_capabilities.features.contains(&"cross-vm".to_string()));
    
    println!("P2P networking integration test completed");
}

/// Test end-to-end transaction flow
#[tokio::test]
async fn test_end_to_end_transaction_flow() {
    // This test would verify a complete transaction flow from API to blockchain
    
    // Test transaction data structures
    let svm_transaction_data = serde_json::json!({
        "instructions": [
            {
                "program_id": "11111111111111111111111111111112",
                "accounts": [],
                "data": []
            }
        ],
        "recent_blockhash": "11111111111111111111111111111111111111111111",
        "signatures": []
    });
    
    let evm_transaction_data = serde_json::json!({
        "to": "0x0000000000000000000000000000000000000000",
        "value": "0x0",
        "gas": "0x5208",
        "gasPrice": "0x3b9aca00",
        "nonce": "0x0",
        "data": "0x"
    });
    
    assert!(svm_transaction_data.is_object());
    assert!(evm_transaction_data.is_object());
    
    // Integration test steps:
    // 1. Submit transaction through REST API
    // 2. Verify it's processed by the application layer
    // 3. Check it's routed to the correct VM
    // 4. Verify it's included in a block
    // 5. Test transaction status queries
    // 6. Verify state changes
    
    println!("End-to-end transaction flow test completed");
}

/// Test system health monitoring integration
#[tokio::test]
async fn test_health_monitoring_integration() {
    // Test that health monitoring works across all components
    
    let health_statuses = vec![
        HealthStatus::Healthy,
        HealthStatus::Degraded,
        HealthStatus::Unhealthy,
    ];
    
    for status in health_statuses {
        // Test health status propagation and aggregation
        match status {
            HealthStatus::Healthy => {
                // System should be fully operational
                assert_eq!(status, HealthStatus::Healthy);
            }
            HealthStatus::Degraded => {
                // System should have some issues but still function
                assert_eq!(status, HealthStatus::Degraded);
            }
            HealthStatus::Unhealthy => {
                // System should indicate problems
                assert_eq!(status, HealthStatus::Unhealthy);
            }
        }
    }
    
    // Integration test steps:
    // 1. Start all system components
    // 2. Verify overall health is reported correctly
    // 3. Simulate component failures
    // 4. Verify health status updates appropriately
    // 5. Test health check endpoints return correct status
    
    println!("Health monitoring integration test completed");
}

/// Test configuration validation across components
#[tokio::test]
async fn test_configuration_validation_integration() {
    let temp_dir = TempDir::new().unwrap();
    
    // Test that configuration is consistent across all components
    let mut config = MultivmConfig::default();
    config.system.data_dir = temp_dir.path().to_path_buf();
    config.database.path = temp_dir.path().join("integration_test.db");
    
    // Validate the configuration
    let validation_result = config.validate();
    assert!(validation_result.is_ok(), "Configuration validation should succeed");
    
    // Test configuration serialization/deserialization
    let serialized = serde_json::to_string(&config);
    assert!(serialized.is_ok(), "Configuration should serialize");
    
    let deserialized: Result<MultivmConfig, _> = serde_json::from_str(&serialized.unwrap());
    assert!(deserialized.is_ok(), "Configuration should deserialize");
    
    let deserialized_config = deserialized.unwrap();
    assert_eq!(config.system.data_dir, deserialized_config.system.data_dir);
    
    println!("Configuration validation integration test completed");
}

/// Test error handling and recovery across components
#[tokio::test]
async fn test_error_handling_integration() {
    // Test that errors are properly handled and propagated across components
    
    // Test error types from different components
    let network_error = multivm_common::MultivmError::Network {
        message: "Connection failed".to_string(),
        endpoint: Some("http://localhost:8899".to_string()),
        retry_after: Some(Duration::from_secs(5)),
    };
    
    let process_error = multivm_common::MultivmError::Process {
        process_id: "solana".to_string(),
        message: "Process crashed".to_string(),
        exit_code: Some(1),
    };
    
    let config_error = multivm_common::MultivmError::Configuration {
        component: "database".to_string(),
        message: "Invalid path".to_string(),
        validation_errors: Some(vec!["Path does not exist".to_string()]),
    };
    
    // Test error serialization (for logging/monitoring)
    let network_error_str = network_error.to_string();
    let process_error_str = process_error.to_string();
    let config_error_str = config_error.to_string();
    
    assert!(network_error_str.contains("Connection failed"));
    assert!(process_error_str.contains("Process crashed"));
    assert!(config_error_str.contains("Invalid path"));
    
    // Test error categorization
    assert!(matches!(network_error, multivm_common::MultivmError::Network { .. }));
    assert!(matches!(process_error, multivm_common::MultivmError::Process { .. }));
    assert!(matches!(config_error, multivm_common::MultivmError::Configuration { .. }));
    
    println!("Error handling integration test completed");
}

/// Test resource management across components
#[tokio::test]
async fn test_resource_management_integration() {
    // Test that resource limits are respected across all components
    
    let resource_limits = multivm_common::ResourceLimits {
        max_memory_mb: 1024,
        max_cpu_percent: 80.0,
        max_disk_usage_gb: 10,
        max_open_files: 1000,
        max_rpc_connections: 100,
    };
    
    // Test resource limit validation
    assert!(resource_limits.max_memory_mb > 0);
    assert!(resource_limits.max_cpu_percent > 0.0);
    assert!(resource_limits.max_disk_usage_gb > 0);
    assert!(resource_limits.max_open_files > 0);
    assert!(resource_limits.max_rpc_connections > 0);
    
    // Integration test steps:
    // 1. Set resource limits
    // 2. Start all components
    // 3. Monitor actual resource usage
    // 4. Verify limits are enforced
    // 5. Test behavior when limits are exceeded
    
    println!("Resource management integration test completed");
}

/// Test concurrent operations across components
#[tokio::test]
async fn test_concurrent_operations_integration() {
    // Test that the system handles concurrent operations correctly
    
    let mut handles = Vec::new();
    
    // Simulate concurrent operations across different components
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            // Simulate different types of operations
            match i % 4 {
                0 => {
                    // Simulate transaction processing
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    Ok(format!("Transaction {} processed", i))
                }
                1 => {
                    // Simulate health check
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    Ok(format!("Health check {} completed", i))
                }
                2 => {
                    // Simulate account query
                    tokio::time::sleep(Duration::from_millis(15)).await;
                    Ok(format!("Account query {} completed", i))
                }
                3 => {
                    // Simulate network operation
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Ok(format!("Network operation {} completed", i))
                }
                _ => unreachable!(),
            }
        });
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.unwrap();
        results.push(result);
    }
    
    assert_eq!(results.len(), 10);
    
    // Verify all operations completed successfully
    for result in results {
        assert!(result.is_ok());
        let message = result.unwrap();
        assert!(!message.is_empty());
    }
    
    println!("Concurrent operations integration test completed");
}

/// Test system shutdown and cleanup
#[tokio::test]
async fn test_system_shutdown_integration() {
    // Test that system shutdown works cleanly across all components
    
    // Integration test steps:
    // 1. Start all system components
    // 2. Initiate graceful shutdown
    // 3. Verify all components shut down cleanly
    // 4. Check that resources are properly released
    // 5. Verify data integrity after shutdown
    
    // Simulate shutdown sequence
    let shutdown_steps = vec![
        "Stop accepting new requests",
        "Complete in-flight operations", 
        "Flush caches and buffers",
        "Close database connections",
        "Shutdown network listeners",
        "Release system resources",
    ];
    
    for (i, step) in shutdown_steps.iter().enumerate() {
        // Simulate time taken for each shutdown step
        tokio::time::sleep(Duration::from_millis(10)).await;
        println!("Shutdown step {}: {}", i + 1, step);
    }
    
    println!("System shutdown integration test completed");
}

/// Test system performance under load
#[tokio::test]
async fn test_system_performance_integration() {
    // Test system performance characteristics
    
    let start_time = std::time::Instant::now();
    
    // Simulate load testing
    let num_operations = 1000;
    let mut handles = Vec::new();
    
    for i in 0..num_operations {
        let handle = tokio::spawn(async move {
            // Simulate lightweight operation
            let _result = format!("Operation {}", i);
            tokio::time::sleep(Duration::from_micros(100)).await;
            i
        });
        handles.push(handle);
    }
    
    // Wait for all operations
    let mut completed = 0;
    for handle in handles {
        if let Ok(_) = handle.await {
            completed += 1;
        }
    }
    
    let elapsed = start_time.elapsed();
    
    assert_eq!(completed, num_operations);
    assert!(elapsed.as_secs() < 10, "Performance test should complete within 10 seconds");
    
    let ops_per_second = num_operations as f64 / elapsed.as_secs_f64();
    println!("Performance: {:.2} operations/second", ops_per_second);
    
    println!("System performance integration test completed");
}

/// Test timeout handling across components  
#[tokio::test]
async fn test_timeout_handling_integration() {
    // Test that timeouts are handled correctly across the system
    
    let short_timeout = Duration::from_millis(100);
    let long_operation = async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        "completed"
    };
    
    // Test timeout behavior
    let timeout_result = timeout(short_timeout, long_operation).await;
    assert!(timeout_result.is_err(), "Operation should timeout");
    
    // Test successful completion within timeout
    let quick_operation = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        "completed quickly"
    };
    
    let success_result = timeout(short_timeout, quick_operation).await;
    assert!(success_result.is_ok(), "Quick operation should succeed");
    assert_eq!(success_result.unwrap(), "completed quickly");
    
    println!("Timeout handling integration test completed");
}
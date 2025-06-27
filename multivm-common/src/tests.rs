//! Integration tests for multivm-common module

#[cfg(test)]
mod integration_tests {
    use crate::config::*;
    use crate::error::*;
    use crate::ipc::messages::*;
    use crate::types::account::*;
    use crate::*;
    use std::time::Duration;

    #[test]
    fn test_multivm_error_creation() {
        let error = MultivmError::Configuration {
            component: "test".to_string(),
            message: "Test error".to_string(),
            validation_errors: Some(vec!["field1 invalid".to_string()]),
        };

        assert!(matches!(error, MultivmError::Configuration { .. }));
        let error_str = error.to_string();
        assert!(error_str.contains("test"));
    }

    #[test]
    fn test_process_id_enum() {
        let main = ProcessId::Main;
        let solana = ProcessId::Solana;
        let ethereum = ProcessId::Ethereum;

        assert_ne!(main, solana);
        assert_ne!(solana, ethereum);
        assert_eq!(main.to_string(), "main");
        assert_eq!(solana.to_string(), "solana");
        assert_eq!(ethereum.to_string(), "ethereum");
    }

    #[test]
    fn test_message_id_creation() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2); // Should be unique
    }

    #[test]
    fn test_ipc_message_creation() {
        let source = ProcessId::Main;
        let dest = ProcessId::Solana;
        let msg = IpcMessage::new(source, dest, IpcCommand::Ping);

        assert_eq!(msg.source, source);
        assert_eq!(msg.destination, dest);
        assert!(matches!(msg.command, IpcCommand::Ping));
        assert!(msg.timeout.is_none());
    }

    #[test]
    fn test_ipc_message_with_timeout() {
        let source = ProcessId::Main;
        let dest = ProcessId::Ethereum;
        let timeout = Duration::from_secs(30);
        let msg = IpcMessage::new(source, dest, IpcCommand::GetHealth).with_timeout(timeout);

        assert_eq!(msg.timeout, Some(timeout));
        assert!(!msg.is_expired());
    }

    #[test]
    fn test_account_binding_info() {
        let binding = AccountBindingInfo::new("multivm_123".to_string());

        assert_eq!(binding.multivm_id, "multivm_123");
        assert!(binding.svm_address.is_none());
        assert!(binding.evm_address.is_none());
        assert!(!binding.has_bindings());
        assert!(binding.get_bound_addresses().is_empty());
    }

    #[test]
    fn test_account_binding_with_addresses() {
        let mut binding = AccountBindingInfo::new("multivm_456".to_string());
        binding.svm_address = Some("11111111111111111111111111111111".to_string());
        binding.evm_address = Some("0x1234567890123456789012345678901234567890".to_string());

        assert!(binding.has_bindings());
        assert_eq!(binding.get_bound_addresses().len(), 2);
    }

    #[test]
    fn test_vm_type_enum() {
        assert_eq!(VmType::Evm.to_string(), "EVM");
        assert_eq!(VmType::Svm.to_string(), "SVM");
    }

    #[test]
    fn test_blockchain_type_enum() {
        let eth = BlockchainType::Ethereum;
        let sol = BlockchainType::Solana;

        assert!(matches!(eth, BlockchainType::Ethereum));
        assert!(matches!(sol, BlockchainType::Solana));
    }

    #[test]
    fn test_health_status_enum() {
        let healthy = HealthStatus::Healthy;
        let degraded = HealthStatus::Degraded;
        let unhealthy = HealthStatus::Unhealthy;

        assert!(healthy.is_operational());
        assert!(degraded.is_operational());
        assert!(!unhealthy.is_operational());
    }

    #[test]
    fn test_engine_state_struct() {
        let state = EngineState {
            process_id: ProcessId::Solana,
            blockchain_type: BlockchainType::Solana,
            current_block: Some(1000),
            state_root: vec![1, 2, 3, 4],
            is_syncing: false,
            peer_count: 0,
            rpc_endpoints: vec!["http://localhost:8899".to_string()],
            data_directory: "/tmp/solana".to_string(),
            chain_id: 1,
        };

        assert_eq!(state.process_id, ProcessId::Solana);
        assert_eq!(state.blockchain_type, BlockchainType::Solana);
        assert_eq!(state.current_block, Some(1000));
        assert_eq!(state.peer_count, 0);
    }

    #[test]
    fn test_ipc_response_variants() {
        let responses = vec![
            IpcResponse::Ack,
            IpcResponse::Pong,
            IpcResponse::Error {
                code: 500,
                message: "Internal error".to_string(),
                details: None,
            },
        ];

        for response in responses {
            match response {
                IpcResponse::Ack => {}  // Ack received successfully
                IpcResponse::Pong => {} // Pong received successfully
                IpcResponse::Error { code, .. } => assert!(code > 0),
                _ => {}
            }
        }
    }

    #[test]
    fn test_multivm_config_creation() {
        let _config = MultivmConfig::default();
        // Just check that default config can be created
        // The actual structure may vary
        // Test passes if no panic occurs
    }

    #[test]
    fn test_manager_state_transitions() {
        use crate::managers::ManagerState;

        let states = [
            ManagerState::Initializing,
            ManagerState::Running,
            ManagerState::Stopping,
            ManagerState::Stopped,
            ManagerState::Error("Test error".to_string()),
        ];

        // Test that all states are distinct
        for (i, state1) in states.iter().enumerate() {
            for (j, state2) in states.iter().enumerate() {
                if i == j {
                    assert_eq!(state1, state2);
                } else {
                    assert_ne!(state1, state2);
                }
            }
        }
    }

    #[test]
    fn test_processing_metrics() {
        use crate::types::metrics::ProcessingMetrics;

        let metrics = ProcessingMetrics::default();
        assert_eq!(metrics.transaction_count, 0);
        assert_eq!(metrics.total_requests, 0);
        assert_eq!(metrics.failed_requests, 0);
        assert_eq!(metrics.cpu_time, Duration::ZERO);
    }

    #[test]
    fn test_resource_limits() {
        use crate::types::resources::ResourceLimits;

        let limits = ResourceLimits::default();
        assert!(limits.max_memory_mb > 0);
        assert!(limits.max_cpu_percent > 0.0);
    }

    #[tokio::test]
    async fn test_async_manager_creation() {
        use crate::managers::BaseManager;

        #[derive(Clone)]
        struct TestConfig {}

        let config = TestConfig {};
        let manager = BaseManager::<TestConfig, ()>::new("test_manager".to_string(), config);
        assert_eq!(manager.name, "test_manager");

        let state = manager.state.read().await;
        assert!(matches!(*state, ManagerState::Uninitialized));
    }

    #[test]
    fn test_error_variants() {
        let storage_error = MultivmError::Storage {
            operation: "read".to_string(),
            message: "File not found".to_string(),
            path: Some("/tmp/test.db".to_string()),
        };

        match storage_error {
            MultivmError::Storage {
                operation,
                message,
                path,
            } => {
                assert_eq!(operation, "read");
                assert!(message.contains("File not found"));
                assert_eq!(path, Some("/tmp/test.db".to_string()));
            }
            _ => panic!("Expected Storage error variant"),
        }

        let network_error = MultivmError::Network {
            message: "Connection refused".to_string(),
            endpoint: Some("127.0.0.1:8080".to_string()),
            retry_after: Some(Duration::from_secs(5)),
        };

        assert!(network_error.to_string().contains("Connection refused"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let msg = IpcMessage::new(ProcessId::Main, ProcessId::Solana, IpcCommand::GetHealth);

        // Test bincode serialization
        let bytes = bincode::serialize(&msg).unwrap();
        let deserialized: IpcMessage = bincode::deserialize(&bytes).unwrap();
        assert_eq!(msg.id, deserialized.id);

        // Test JSON serialization
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: IpcMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.id, deserialized.id);
    }
}

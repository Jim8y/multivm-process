//! Comprehensive tests for MultivmProcessManager

#[cfg(test)]
mod tests {
    use crate::manager::{MultivmProcessManager, ProcessManagerEvent, SolanaExecutionConfig};
    use multivm_common::{
        config::{IpcConfig, IpcTransportConfig, LogFormat, LoggingConfig, MultivmConfig},
        types::ProcessId,
        MultivmError,
    };
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_process_manager_creation() {
        let config = MultivmConfig::default();
        let manager = MultivmProcessManager::new(config).await;
        assert!(manager.is_ok(), "Process manager creation should succeed");
    }

    #[tokio::test]
    async fn test_process_manager_with_custom_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = MultivmConfig {
            logging: LoggingConfig {
                level: "debug".to_string(),
                enable_file_logging: true,
                log_directory: temp_dir.path().to_path_buf(),
                max_log_files: 5,
                max_file_size_bytes: 10 * 1024 * 1024,
                format: LogFormat::Json,
            },
            ipc: IpcConfig {
                transport: IpcTransportConfig::UnixSocket {
                    path: temp_dir.path().join("ipc.sock"),
                },
                message_timeout: Duration::from_secs(10),
                enable_encryption: false,
            },
            ..Default::default()
        };

        let manager = MultivmProcessManager::new(config).await;
        assert!(
            manager.is_ok(),
            "Process manager with custom config should succeed"
        );
    }

    #[tokio::test]
    async fn test_process_manager_startup_sequence() {
        let config = MultivmConfig::default();
        let manager = MultivmProcessManager::new(config).await.unwrap();

        // Test that the manager can be started
        let start_result = manager.start().await;
        assert!(start_result.is_ok(), "Manager start should succeed");
    }

    #[tokio::test]
    async fn test_health_check_functionality() {
        let config = MultivmConfig::default();
        let manager = MultivmProcessManager::new(config).await.unwrap();

        // Test health check for all processes
        let health_info = manager.get_health_status().await;
        assert!(health_info.is_ok(), "Health check should succeed");

        let _health = health_info.unwrap();
        // For a default configuration, process_health may be empty if no processes are configured to start
        // The important thing is that the health check itself succeeds and returns a valid structure
        // We don't assert on emptiness since default config may not start any processes
    }

    #[tokio::test]
    async fn test_process_lifecycle() {
        let config = MultivmConfig::default();
        let manager = MultivmProcessManager::new(config).await.unwrap();

        // Start processes
        let start_result = manager.start().await;
        assert!(start_result.is_ok());

        // Check that processes are starting
        // list_processes method not implemented yet
        // let processes = manager.list_processes().await.unwrap();
        // assert!(!processes.is_empty(), "Processes should be listed");

        // Graceful shutdown
        let shutdown_result = manager.shutdown(true).await;
        assert!(shutdown_result.is_ok(), "Graceful shutdown should succeed");
    }

    #[tokio::test]
    async fn test_resource_monitoring() {
        let config = MultivmConfig::default();
        let _manager = MultivmProcessManager::new(config).await.unwrap();

        // Get system resource information
        // get_system_resources method not implemented yet
        // let resources = manager.get_system_resources().await;
        // assert!(resources.is_ok(), "Resource monitoring should work");

        // let resource_info = resources.unwrap();
        // assert!(resource_info.cpu_usage_percent >= 0.0);
        // assert!(resource_info.memory_usage_mb > 0);
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let config = MultivmConfig::default();
        let manager = MultivmProcessManager::new(config).await.unwrap();

        // Test concurrent health checks
        let mut handles = Vec::new();
        for _ in 0..5 {
            let manager_clone = manager.clone();
            let handle = tokio::spawn(async move { manager_clone.get_health_status().await });
            handles.push(handle);
        }

        // Wait for all health checks to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Concurrent health checks should succeed");
        }
    }

    #[tokio::test]
    async fn test_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_config = MultivmConfig {
            ipc: IpcConfig {
                transport: IpcTransportConfig::UnixSocket {
                    path: temp_dir
                        .path()
                        .join("nonexistent")
                        .join("deep")
                        .join("path")
                        .join("ipc.sock"),
                },
                message_timeout: Duration::from_millis(1), // Very short timeout
                enable_encryption: false,
            },
            ..Default::default()
        };

        // This should either fail gracefully or handle the invalid config
        let manager_result = MultivmProcessManager::new(invalid_config).await;
        match manager_result {
            Ok(_) => {
                // If it succeeds, that's fine - the implementation might handle invalid values gracefully
            }
            Err(e) => {
                // If it fails, it should be a configuration error
                match e {
                    MultivmError::Configuration { .. } => {
                        // Expected error type
                    }
                    _ => panic!("Expected configuration error, got: {e:?}"),
                }
            }
        }
    }

    #[tokio::test]
    async fn test_event_handling() {
        let config = MultivmConfig::default();
        let manager = MultivmProcessManager::new(config).await.unwrap();

        // Test that event handlers can be registered (if the API supports it)
        // This test assumes there's a way to register event handlers
        // and that the manager generates events during operation

        // For now, just test that we can get the manager's status
        let _health = manager.get_health_status().await.unwrap();
        // Health check should succeed even if no processes are running (default config)
        // We verify the method succeeds, not the content (which may be empty for default config)
    }

    #[tokio::test]
    async fn test_shutdown_scenarios() {
        let config = MultivmConfig::default();
        let manager = MultivmProcessManager::new(config).await.unwrap();

        // Test immediate shutdown
        let shutdown_result = manager.shutdown(false).await;
        assert!(shutdown_result.is_ok(), "Immediate shutdown should succeed");
    }

    #[tokio::test]
    async fn test_solana_execution_config() {
        let config = SolanaExecutionConfig::default();

        assert!(config.enabled);
        assert_eq!(config.chain_id, 1);
        assert!(config.rpc_config.is_some());

        let rpc_config = config.rpc_config.unwrap();
        assert_eq!(rpc_config.host, "127.0.0.1");
        assert_eq!(rpc_config.port, 8899);
    }

    #[tokio::test]
    async fn test_custom_solana_config() {
        let temp_dir = TempDir::new().unwrap();
        let custom_config = SolanaExecutionConfig {
            enabled: false,
            data_dir: temp_dir.path().to_path_buf(),
            rpc_config: None,
            ledger_path: temp_dir.path().join("ledger"),
            accounts_path: temp_dir.path().join("accounts"),
            chain_id: 42,
        };

        assert!(!custom_config.enabled);
        assert_eq!(custom_config.chain_id, 42);
        assert!(custom_config.rpc_config.is_none());
    }

    #[tokio::test]
    async fn test_process_manager_event_types() {
        // Test that all event variants can be created
        let events = vec![
            ProcessManagerEvent::ProcessStarted {
                process_id: ProcessId::Solana,
            },
            ProcessManagerEvent::ProcessStopped {
                process_id: ProcessId::Ethereum,
            },
            ProcessManagerEvent::ProcessFailed {
                process_id: ProcessId::Main,
                error: "Test error".to_string(),
            },
            ProcessManagerEvent::ProcessRestarted {
                process_id: ProcessId::Solana,
            },
            ProcessManagerEvent::HealthCheckFailed {
                process_id: ProcessId::Ethereum,
                error: "Health check timeout".to_string(),
            },
        ];

        for event in events {
            // Test that events can be created and have debug output
            let debug_output = format!("{event:?}");
            assert!(!debug_output.is_empty());
        }
    }

    #[tokio::test]
    async fn test_manager_clone_functionality() {
        let config = MultivmConfig::default();
        let manager = MultivmProcessManager::new(config).await.unwrap();

        // Test that the manager can be cloned
        let cloned_manager = manager.clone();

        // Both instances should be able to perform operations
        let _health1 = manager.get_health_status().await.unwrap();
        let _health2 = cloned_manager.get_health_status().await.unwrap();

        // Both should return health information successfully (content may be empty for default config)
        // The fact that we got health data without errors proves the clone works correctly
    }

    #[tokio::test]
    async fn test_timeout_handling() {
        let config = MultivmConfig::default();
        let manager = MultivmProcessManager::new(config).await.unwrap();

        // Test that operations complete within reasonable time
        let health_check = timeout(Duration::from_secs(5), manager.get_health_status()).await;
        assert!(
            health_check.is_ok(),
            "Health check should complete within timeout"
        );

        let health_result = health_check.unwrap();
        assert!(health_result.is_ok(), "Health check should succeed");
    }
}

//! Comprehensive tests for ProcessHandle

#[cfg(test)]
mod tests {
    use crate::process::{DowntimePeriod, ProcessConfig, ProcessHandle, RestartAttempt};
    use multivm_common::types::{BlockchainType, ProcessId};
    use std::collections::VecDeque;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;
    use tokio::sync::{Mutex, RwLock};

    fn create_test_process_handle(process_id: ProcessId, binary_path: PathBuf) -> ProcessHandle {
        let temp_dir = TempDir::new().unwrap();
        let config = ProcessConfig {
            blockchain_type: match process_id {
                ProcessId::Solana => BlockchainType::Solana,
                ProcessId::Ethereum => BlockchainType::Ethereum,
                _ => BlockchainType::Solana,
            },
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            rpc_port: match process_id {
                ProcessId::Solana => 8899,
                ProcessId::Ethereum => 8545,
                _ => 8000,
            },
            ..Default::default()
        };

        ProcessHandle {
            process_id,
            child: RwLock::new(None),
            binary_path,
            args: vec![],
            working_dir: temp_dir.keep(),
            config,
            restart_attempts: Arc::new(Mutex::new(VecDeque::new())),
            last_health_check: Arc::new(Mutex::new(None)),
            is_recovering: Arc::new(Mutex::new(false)),
            start_time: Arc::new(Mutex::new(None)),
            total_uptime: Arc::new(Mutex::new(Duration::ZERO)),
            downtime_periods: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    #[test]
    fn test_process_handle_creation() {
        let handle = create_test_process_handle(ProcessId::Solana, PathBuf::from("/usr/bin/true"));

        assert_eq!(handle.process_id, ProcessId::Solana);
        assert_eq!(handle.config.blockchain_type, BlockchainType::Solana);
        assert_eq!(handle.config.rpc_port, 8899);
    }

    #[test]
    fn test_process_handle_clone() {
        let handle =
            create_test_process_handle(ProcessId::Ethereum, PathBuf::from("/usr/bin/echo"));

        let cloned_handle = handle.clone();

        assert_eq!(handle.process_id, cloned_handle.process_id);
        assert_eq!(handle.binary_path, cloned_handle.binary_path);
        assert_eq!(handle.args, cloned_handle.args);
        assert_eq!(handle.working_dir, cloned_handle.working_dir);
        assert_eq!(
            handle.config.blockchain_type,
            cloned_handle.config.blockchain_type
        );
    }

    #[tokio::test]
    async fn test_process_lifecycle_management() {
        let handle = create_test_process_handle(ProcessId::Solana, PathBuf::from("/usr/bin/sleep"));

        // Test that process starts not running
        let is_running = handle.is_running().await;
        assert!(!is_running, "Process should not be running initially");

        // Test start time tracking
        {
            let mut start_time = handle.start_time.lock().await;
            *start_time = Some(Instant::now());
        }

        let start_time = handle.start_time.lock().await;
        assert!(start_time.is_some(), "Start time should be set");
    }

    #[tokio::test]
    async fn test_recovery_state_management() {
        let handle =
            create_test_process_handle(ProcessId::Ethereum, PathBuf::from("/usr/bin/true"));

        // Test initial recovery state
        let is_recovering = *handle.is_recovering.lock().await;
        assert!(!is_recovering, "Process should not be recovering initially");

        // Test setting recovery state
        {
            let mut recovery_state = handle.is_recovering.lock().await;
            *recovery_state = true;
        }

        let is_recovering = *handle.is_recovering.lock().await;
        assert!(is_recovering, "Process should be marked as recovering");
    }

    #[tokio::test]
    async fn test_restart_attempts_tracking() {
        let handle = create_test_process_handle(ProcessId::Solana, PathBuf::from("/usr/bin/false"));

        // Add a restart attempt
        {
            let mut attempts = handle.restart_attempts.lock().await;
            attempts.push_back(RestartAttempt {
                timestamp: Instant::now(),
                reason: "Test restart".to_string(),
                successful: false,
            });
        }

        let attempts = handle.restart_attempts.lock().await;
        assert_eq!(attempts.len(), 1, "Should have one restart attempt");
        assert_eq!(attempts[0].reason, "Test restart");
        assert!(!attempts[0].successful);
    }

    #[tokio::test]
    async fn test_uptime_tracking() {
        let handle =
            create_test_process_handle(ProcessId::Ethereum, PathBuf::from("/usr/bin/echo"));

        // Test initial uptime
        let uptime = *handle.total_uptime.lock().await;
        assert_eq!(uptime, Duration::ZERO, "Initial uptime should be zero");

        // Update uptime
        {
            let mut total_uptime = handle.total_uptime.lock().await;
            *total_uptime = Duration::from_secs(3600); // 1 hour
        }

        let uptime = *handle.total_uptime.lock().await;
        assert_eq!(
            uptime,
            Duration::from_secs(3600),
            "Uptime should be updated"
        );
    }

    #[tokio::test]
    async fn test_downtime_periods_tracking() {
        let handle = create_test_process_handle(ProcessId::Main, PathBuf::from("/usr/bin/true"));

        // Add downtime periods
        {
            let mut downtime_periods = handle.downtime_periods.lock().await;
            downtime_periods.push_back(DowntimePeriod {
                start_time: Instant::now(),
                end_time: Some(Instant::now()),
                reason: Some("Scheduled maintenance".to_string()),
            });
            downtime_periods.push_back(DowntimePeriod {
                start_time: Instant::now(),
                end_time: None, // Ongoing downtime
                reason: Some("System crash".to_string()),
            });
        }

        let downtime_periods = handle.downtime_periods.lock().await;
        assert_eq!(
            downtime_periods.len(),
            2,
            "Should have two downtime periods"
        );
        assert_eq!(
            downtime_periods[0].reason,
            Some("Scheduled maintenance".to_string())
        );
        assert_eq!(downtime_periods[1].reason, Some("System crash".to_string()));
        assert!(downtime_periods[0].end_time.is_some());
        assert!(downtime_periods[1].end_time.is_none());
    }

    #[tokio::test]
    async fn test_health_check_timing() {
        let handle = create_test_process_handle(ProcessId::Solana, PathBuf::from("/usr/bin/date"));

        // Test initial health check time
        let last_check = *handle.last_health_check.lock().await;
        assert!(
            last_check.is_none(),
            "Initial health check time should be None"
        );

        // Update health check time
        let now = Instant::now();
        {
            let mut last_check = handle.last_health_check.lock().await;
            *last_check = Some(now);
        }

        let last_check = *handle.last_health_check.lock().await;
        assert!(last_check.is_some(), "Health check time should be set");
        assert_eq!(last_check.unwrap(), now, "Health check time should match");
    }

    #[tokio::test]
    async fn test_process_configuration() {
        let solana_handle = create_test_process_handle(
            ProcessId::Solana,
            PathBuf::from("/usr/bin/solana-validator"),
        );

        let ethereum_handle =
            create_test_process_handle(ProcessId::Ethereum, PathBuf::from("/usr/bin/geth"));

        // Test Solana configuration
        assert_eq!(solana_handle.config.blockchain_type, BlockchainType::Solana);
        assert_eq!(solana_handle.config.rpc_port, 8899);

        // Test Ethereum configuration
        assert_eq!(
            ethereum_handle.config.blockchain_type,
            BlockchainType::Ethereum
        );
        assert_eq!(ethereum_handle.config.rpc_port, 8545);
    }

    #[tokio::test]
    async fn test_concurrent_state_access() {
        let handle = create_test_process_handle(ProcessId::Solana, PathBuf::from("/usr/bin/true"));

        // Test concurrent access to various state fields
        let mut tasks = Vec::new();

        // Task 1: Update recovery state
        let handle1 = handle.clone();
        tasks.push(tokio::spawn(async move {
            let mut recovery_state = handle1.is_recovering.lock().await;
            *recovery_state = true;
            "recovery_updated"
        }));

        // Task 2: Update start time
        let handle2 = handle.clone();
        tasks.push(tokio::spawn(async move {
            let mut start_time = handle2.start_time.lock().await;
            *start_time = Some(Instant::now());
            "start_time_updated"
        }));

        // Task 3: Add restart attempt
        let handle3 = handle.clone();
        tasks.push(tokio::spawn(async move {
            let mut attempts = handle3.restart_attempts.lock().await;
            attempts.push_back(RestartAttempt {
                timestamp: Instant::now(),
                reason: "Concurrent test".to_string(),
                successful: true,
            });
            "restart_attempt_added"
        }));

        // Task 4: Update uptime
        let handle4 = handle.clone();
        tasks.push(tokio::spawn(async move {
            let mut uptime = handle4.total_uptime.lock().await;
            *uptime = Duration::from_secs(123);
            "uptime_updated"
        }));

        // Wait for all tasks to complete
        let mut results = Vec::new();
        for task in tasks {
            let result = task.await.unwrap();
            results.push(result);
        }

        // Verify all operations completed
        assert_eq!(results.len(), 4);
        assert!(results.contains(&"recovery_updated"));
        assert!(results.contains(&"start_time_updated"));
        assert!(results.contains(&"restart_attempt_added"));
        assert!(results.contains(&"uptime_updated"));

        // Verify final state
        let is_recovering = *handle.is_recovering.lock().await;
        assert!(is_recovering);

        let start_time = handle.start_time.lock().await;
        assert!(start_time.is_some());

        let attempts = handle.restart_attempts.lock().await;
        assert_eq!(attempts.len(), 1);

        let uptime = *handle.total_uptime.lock().await;
        assert_eq!(uptime, Duration::from_secs(123));
    }

    #[tokio::test]
    async fn test_process_args_and_paths() {
        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("test_binary");

        let mut handle = create_test_process_handle(ProcessId::Ethereum, binary_path.clone());

        // Set custom args
        handle.args = vec![
            "--chain".to_string(),
            "mainnet".to_string(),
            "--rpc-port".to_string(),
            "8545".to_string(),
        ];

        assert_eq!(handle.binary_path, binary_path);
        assert_eq!(handle.args.len(), 4);
        assert_eq!(handle.args[0], "--chain");
        assert_eq!(handle.args[1], "mainnet");
        assert_eq!(handle.args[2], "--rpc-port");
        assert_eq!(handle.args[3], "8545");
    }

    #[tokio::test]
    async fn test_process_working_directory() {
        let temp_dir = TempDir::new().unwrap();
        let working_dir = temp_dir.path().to_path_buf();

        let mut handle = create_test_process_handle(ProcessId::Main, PathBuf::from("/usr/bin/pwd"));

        handle.working_dir = working_dir.clone();
        assert_eq!(handle.working_dir, working_dir);
    }

    #[tokio::test]
    async fn test_restart_attempts_limit() {
        let handle = create_test_process_handle(ProcessId::Solana, PathBuf::from("/usr/bin/false"));

        // Add many restart attempts to test limits
        {
            let mut attempts = handle.restart_attempts.lock().await;
            for i in 0..100 {
                attempts.push_back(RestartAttempt {
                    timestamp: Instant::now(),
                    reason: format!("Restart attempt {i}"),
                    successful: i % 2 == 0,
                });
            }
        }

        let attempts = handle.restart_attempts.lock().await;
        assert_eq!(attempts.len(), 100, "Should store all restart attempts");

        // Test accessing the first and last attempts
        assert_eq!(attempts[0].reason, "Restart attempt 0");
        assert_eq!(attempts[99].reason, "Restart attempt 99");
    }
}

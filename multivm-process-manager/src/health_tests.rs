//! Comprehensive tests for HealthMonitor

#[cfg(test)]
mod tests {
    use crate::health::HealthMonitor;
    use crate::process::{ProcessConfig, ProcessHandle};
    use multivm_common::types::{BlockchainType, HealthStatus, ProcessId};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    #[test]
    fn test_health_monitor_creation() {
        let monitor = HealthMonitor::new(Duration::from_millis(100));
        // Just test that it can be created without panicking
        assert_eq!(monitor.check_interval(), Duration::from_millis(100));
    }

    #[test]
    fn test_health_monitor_with_different_intervals() {
        let intervals = vec![
            Duration::from_millis(1),
            Duration::from_millis(100),
            Duration::from_secs(1),
            Duration::from_secs(60),
        ];

        for interval in intervals {
            let monitor = HealthMonitor::new(interval);
            assert_eq!(monitor.check_interval(), interval);
        }
    }

    #[test]
    fn test_health_monitor_clone() {
        let monitor = HealthMonitor::new(Duration::from_millis(500));
        let cloned_monitor = monitor.clone();

        assert_eq!(monitor.check_interval(), cloned_monitor.check_interval());
    }

    #[tokio::test]
    async fn test_health_check_process_lifecycle() {
        let monitor = HealthMonitor::new(Duration::from_millis(100));

        // Create a mock process handle
        let temp_dir = TempDir::new().unwrap();
        let config = ProcessConfig {
            blockchain_type: BlockchainType::Solana,
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            rpc_port: 8899,
            ..Default::default()
        };

        let handle = ProcessHandle {
            process_id: ProcessId::Solana,
            child: tokio::sync::RwLock::new(None),
            binary_path: PathBuf::from("/usr/bin/true"), // Use a command that always succeeds
            args: vec![],
            working_dir: temp_dir.path().to_path_buf(),
            config,
            restart_attempts: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
            last_health_check: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            is_recovering: std::sync::Arc::new(tokio::sync::Mutex::new(false)),
            start_time: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            total_uptime: std::sync::Arc::new(tokio::sync::Mutex::new(Duration::ZERO)),
            downtime_periods: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
        };

        // Test health check on a process that's not running
        let health_result = monitor.check_process_health(&handle).await;
        assert!(health_result.is_ok(), "Health check should return a result");

        let health_info = health_result.unwrap();
        assert_eq!(health_info.process_id, ProcessId::Solana);
        // Since the process isn't actually running, it should be unhealthy
        assert_eq!(health_info.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_concurrent_health_checks() {
        let monitor = HealthMonitor::new(Duration::from_millis(50));

        // Create multiple process handles
        let mut handles = Vec::new();
        for i in 0..5 {
            let temp_dir = TempDir::new().unwrap();
            let config = ProcessConfig {
                blockchain_type: if i % 2 == 0 {
                    BlockchainType::Solana
                } else {
                    BlockchainType::Ethereum
                },
                data_dir: temp_dir.path().to_string_lossy().to_string(),
                rpc_port: 8899 + i,
                ..Default::default()
            };

            let handle = ProcessHandle {
                process_id: if i % 2 == 0 {
                    ProcessId::Solana
                } else {
                    ProcessId::Ethereum
                },
                child: tokio::sync::RwLock::new(None),
                binary_path: PathBuf::from("/usr/bin/true"),
                args: vec![],
                working_dir: temp_dir.path().to_path_buf(),
                config,
                restart_attempts: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::VecDeque::new(),
                )),
                last_health_check: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
                is_recovering: std::sync::Arc::new(tokio::sync::Mutex::new(false)),
                start_time: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
                total_uptime: std::sync::Arc::new(tokio::sync::Mutex::new(Duration::ZERO)),
                downtime_periods: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::VecDeque::new(),
                )),
            };
            handles.push(handle);
        }

        // Run health checks concurrently
        let mut tasks = Vec::new();
        for handle in handles {
            let monitor_clone = monitor.clone();
            let task =
                tokio::spawn(async move { monitor_clone.check_process_health(&handle).await });
            tasks.push(task);
        }

        // Wait for all tasks to complete
        for task in tasks {
            let result = task.await.unwrap();
            assert!(result.is_ok(), "Concurrent health check should succeed");
        }
    }

    #[tokio::test]
    async fn test_health_monitor_stress_test() {
        let monitor = HealthMonitor::new(Duration::from_millis(10));

        // Create a single process handle
        let temp_dir = TempDir::new().unwrap();
        let config = ProcessConfig {
            blockchain_type: BlockchainType::Solana,
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            rpc_port: 8899,
            ..Default::default()
        };

        let handle = ProcessHandle {
            process_id: ProcessId::Solana,
            child: tokio::sync::RwLock::new(None),
            binary_path: PathBuf::from("/usr/bin/true"),
            args: vec![],
            working_dir: temp_dir.path().to_path_buf(),
            config,
            restart_attempts: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
            last_health_check: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            is_recovering: std::sync::Arc::new(tokio::sync::Mutex::new(false)),
            start_time: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            total_uptime: std::sync::Arc::new(tokio::sync::Mutex::new(Duration::ZERO)),
            downtime_periods: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
        };

        // Perform many health checks rapidly
        let handle = Arc::new(handle);
        let mut tasks = Vec::new();
        for _ in 0..100 {
            let monitor_clone = monitor.clone();
            let handle_clone = handle.clone();
            let task =
                tokio::spawn(
                    async move { monitor_clone.check_process_health(&handle_clone).await },
                );
            tasks.push(task);
        }

        let mut success_count = 0;
        for task in tasks {
            if let Ok(Ok(_)) = task.await {
                success_count += 1;
            }
        }

        assert!(
            success_count > 90,
            "Most health checks should succeed even under stress"
        );
    }

    #[test]
    fn test_health_monitor_edge_cases() {
        // Test with very short interval
        let monitor1 = HealthMonitor::new(Duration::from_nanos(1));
        assert_eq!(monitor1.check_interval(), Duration::from_nanos(1));

        // Test with very long interval
        let monitor2 = HealthMonitor::new(Duration::from_secs(3600));
        assert_eq!(monitor2.check_interval(), Duration::from_secs(3600));

        // Test with zero duration
        let monitor3 = HealthMonitor::new(Duration::ZERO);
        assert_eq!(monitor3.check_interval(), Duration::ZERO);
    }

    #[tokio::test]
    async fn test_health_info_fields() {
        let monitor = HealthMonitor::new(Duration::from_millis(100));

        let temp_dir = TempDir::new().unwrap();
        let config = ProcessConfig {
            blockchain_type: BlockchainType::Ethereum,
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            rpc_port: 8545,
            ..Default::default()
        };

        let handle = ProcessHandle {
            process_id: ProcessId::Ethereum,
            child: tokio::sync::RwLock::new(None),
            binary_path: PathBuf::from("/usr/bin/false"), // Command that always fails
            args: vec![],
            working_dir: temp_dir.path().to_path_buf(),
            config,
            restart_attempts: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
            last_health_check: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            is_recovering: std::sync::Arc::new(tokio::sync::Mutex::new(false)),
            start_time: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            total_uptime: std::sync::Arc::new(tokio::sync::Mutex::new(Duration::ZERO)),
            downtime_periods: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
        };

        let health_result = monitor.check_process_health(&handle).await;
        assert!(health_result.is_ok());

        let health_info = health_result.unwrap();

        // Verify all fields are populated
        assert_eq!(health_info.process_id, ProcessId::Ethereum);
        assert_eq!(health_info.status, HealthStatus::Unhealthy);
        assert_eq!(health_info.blocks_processed_total, 0);
        assert_eq!(health_info.uptime, Duration::ZERO);
        assert_eq!(health_info.memory_usage, 0);
        assert_eq!(health_info.cpu_usage_percent, 0.0);
        assert!(!health_info.rpc_active);
        assert!(health_info.errors_count >= 1);
        assert!(health_info.last_error.is_some());

        // Timestamp should be recent
        let now = std::time::SystemTime::now();
        let time_diff = now
            .duration_since(health_info.timestamp)
            .unwrap_or(Duration::ZERO);
        assert!(
            time_diff < Duration::from_secs(1),
            "Timestamp should be recent"
        );
    }

    #[tokio::test]
    async fn test_health_monitor_timing() {
        let interval = Duration::from_millis(50);
        let monitor = HealthMonitor::new(interval);

        // Verify the interval is stored correctly
        assert_eq!(monitor.check_interval(), interval);

        // Test that health checks complete reasonably quickly
        let temp_dir = TempDir::new().unwrap();
        let config = ProcessConfig {
            blockchain_type: BlockchainType::Solana,
            data_dir: temp_dir.path().to_string_lossy().to_string(),
            rpc_port: 8899,
            ..Default::default()
        };

        let handle = ProcessHandle {
            process_id: ProcessId::Solana,
            child: tokio::sync::RwLock::new(None),
            binary_path: PathBuf::from("/usr/bin/true"),
            args: vec![],
            working_dir: temp_dir.path().to_path_buf(),
            config,
            restart_attempts: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
            last_health_check: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            is_recovering: std::sync::Arc::new(tokio::sync::Mutex::new(false)),
            start_time: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            total_uptime: std::sync::Arc::new(tokio::sync::Mutex::new(Duration::ZERO)),
            downtime_periods: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
        };

        let start_time = Instant::now();
        let _health_result = monitor.check_process_health(&handle).await;
        let elapsed = start_time.elapsed();

        // Health check should complete quickly (within 1 second)
        assert!(
            elapsed < Duration::from_secs(1),
            "Health check should be fast"
        );
    }
}

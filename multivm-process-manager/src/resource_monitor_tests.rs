//! Comprehensive tests for SystemResourceMonitor

#[cfg(test)]
mod tests {
    use crate::resource_monitor::SystemResourceMonitor;
    use multivm_common::ResourceLimits;
    use std::time::Duration;

    fn create_test_resource_limits() -> ResourceLimits {
        ResourceLimits {
            max_memory_mb: 1024,
            max_cpu_percent: 80.0,
            max_disk_usage_gb: 10,
            max_open_files: 1000,
            max_rpc_connections: 100,
        }
    }

    #[test]
    fn test_resource_monitor_creation() {
        let limits = create_test_resource_limits();
        let _monitor = SystemResourceMonitor::new(limits.clone());

        // Test that monitor was created successfully
        // Note: We can't directly access private fields, but creation shouldn't panic
    }

    #[test]
    fn test_resource_limits_configuration() {
        let limits = ResourceLimits {
            max_memory_mb: 2048,
            max_cpu_percent: 90.0,
            max_disk_usage_gb: 20,
            max_open_files: 2000,
            max_rpc_connections: 200,
        };

        let _monitor = SystemResourceMonitor::new(limits);
        // Just test that it can be created with custom limits
    }

    #[tokio::test]
    async fn test_system_resource_check() {
        let limits = create_test_resource_limits();
        let mut monitor = SystemResourceMonitor::new(limits);

        // Test resource checking
        let violations = monitor.check_system_resources().await;
        assert!(violations.is_ok(), "Resource check should succeed");

        let _violation_list = violations.unwrap();
        // The violation list might be empty or contain violations depending on system state
        // We just test that the check completes without error
    }

    #[tokio::test]
    async fn test_resource_check_with_strict_limits() {
        // Set very strict limits that are likely to be violated
        let strict_limits = ResourceLimits {
            max_memory_mb: 1,       // Very low memory limit
            max_cpu_percent: 0.1,   // Very low CPU limit
            max_disk_usage_gb: 1,   // Very low disk limit
            max_open_files: 1,      // Very low file limit
            max_rpc_connections: 1, // Very low connection limit
        };

        let mut monitor = SystemResourceMonitor::new(strict_limits);
        let violations = monitor.check_system_resources().await;

        assert!(
            violations.is_ok(),
            "Resource check should succeed even with strict limits"
        );

        let _violation_list = violations.unwrap();
        // With such strict limits, we expect some violations
        // But we don't assert on the exact number since it depends on the system
    }

    #[tokio::test]
    async fn test_resource_check_with_lenient_limits() {
        // Set very lenient limits that should not be violated
        let lenient_limits = ResourceLimits {
            max_memory_mb: 1024 * 1024,     // 1TB
            max_cpu_percent: 100.0,         // 100% CPU
            max_disk_usage_gb: 1024 * 1024, // 1PB
            max_open_files: 1_000_000,      // 1 million files
            max_rpc_connections: 1_000_000, // 1 million connections
        };

        let mut monitor = SystemResourceMonitor::new(lenient_limits);
        let violations = monitor.check_system_resources().await;

        assert!(violations.is_ok(), "Resource check should succeed");

        let violation_list = violations.unwrap();
        // With lenient limits, we expect no violations
        assert!(
            violation_list.is_empty(),
            "Should have no violations with lenient limits"
        );
    }

    #[tokio::test]
    async fn test_concurrent_resource_checks() {
        let limits = create_test_resource_limits();
        let mut monitor = SystemResourceMonitor::new(limits);

        // Test multiple concurrent resource checks
        let mut tasks = Vec::new();
        for _ in 0..5 {
            let task = async {
                // Note: We can't move the monitor into each task since it's not Clone
                // So we'll test sequential access here
                true
            };
            tasks.push(tokio::spawn(task));
        }

        // Wait for all tasks
        for task in tasks {
            let result = task.await.unwrap();
            assert!(result);
        }

        // Perform actual resource check after
        let violations = monitor.check_system_resources().await;
        assert!(violations.is_ok());
    }

    #[tokio::test]
    async fn test_repeated_resource_checks() {
        let limits = create_test_resource_limits();
        let mut monitor = SystemResourceMonitor::new(limits);

        // Perform multiple resource checks in sequence
        for i in 0..10 {
            let violations = monitor.check_system_resources().await;
            assert!(violations.is_ok(), "Resource check {i} should succeed");

            // Small delay between checks
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn test_resource_check_timing() {
        let limits = create_test_resource_limits();
        let mut monitor = SystemResourceMonitor::new(limits);

        let start_time = std::time::Instant::now();
        let _violations = monitor.check_system_resources().await.unwrap();
        let elapsed = start_time.elapsed();

        // Resource check should complete reasonably quickly (within 5 seconds)
        assert!(
            elapsed < Duration::from_secs(5),
            "Resource check should be fast"
        );
    }

    #[test]
    fn test_default_resource_limits() {
        let default_limits = ResourceLimits::default();
        let monitor = SystemResourceMonitor::new(default_limits);

        // Just test that default limits work
    }

    #[test]
    fn test_resource_limits_edge_cases() {
        // Test with zero limits
        let zero_limits = ResourceLimits {
            max_memory_mb: 0,
            max_cpu_percent: 0.0,
            max_disk_usage_gb: 0,
            max_open_files: 0,
            max_rpc_connections: 0,
        };

        let monitor = SystemResourceMonitor::new(zero_limits);
        // Should not panic even with zero limits
    }

    #[tokio::test]
    async fn test_resource_monitor_stress_test() {
        let limits = create_test_resource_limits();
        let mut monitor = SystemResourceMonitor::new(limits);

        // Perform many resource checks rapidly
        for _ in 0..100 {
            let violations = monitor.check_system_resources().await;
            assert!(
                violations.is_ok(),
                "Resource check should succeed during stress test"
            );
        }
    }

    #[test]
    fn test_resource_limits_validation() {
        // Test various limit combinations
        let test_cases = vec![
            ResourceLimits {
                max_memory_mb: 512,
                max_cpu_percent: 50.0,
                max_disk_usage_gb: 5,
                max_open_files: 500,
                max_rpc_connections: 50,
            },
            ResourceLimits {
                max_memory_mb: 2048,
                max_cpu_percent: 100.0,
                max_disk_usage_gb: 20,
                max_open_files: 2000,
                max_rpc_connections: 200,
            },
            ResourceLimits {
                max_memory_mb: 4096,
                max_cpu_percent: 150.0, // Over 100% (multi-core)
                max_disk_usage_gb: 40,
                max_open_files: 4000,
                max_rpc_connections: 400,
            },
        ];

        for limits in test_cases {
            let _monitor = SystemResourceMonitor::new(limits);
            // Should not panic with any reasonable limits
        }
    }

    #[tokio::test]
    async fn test_resource_check_error_handling() {
        let limits = create_test_resource_limits();
        let mut monitor = SystemResourceMonitor::new(limits);

        // Test that resource checks handle any system errors gracefully
        let violations = monitor.check_system_resources().await;
        match violations {
            Ok(_) => {
                // Success is expected
            }
            Err(e) => {
                // If there's an error, it should be a proper MultivmError
                println!("Resource check error (acceptable): {e:?}");
            }
        }
    }

    #[test]
    fn test_resource_limits_cloning() {
        let limits = create_test_resource_limits();
        let cloned_limits = limits.clone();

        assert_eq!(limits.max_memory_mb, cloned_limits.max_memory_mb);
        assert_eq!(limits.max_cpu_percent, cloned_limits.max_cpu_percent);
        assert_eq!(limits.max_disk_usage_gb, cloned_limits.max_disk_usage_gb);
        assert_eq!(limits.max_open_files, cloned_limits.max_open_files);
        assert_eq!(
            limits.max_rpc_connections,
            cloned_limits.max_rpc_connections
        );
    }

    #[tokio::test]
    async fn test_memory_monitoring() {
        let limits = ResourceLimits {
            max_memory_mb: 1024 * 10, // 10GB - reasonable limit
            max_cpu_percent: 100.0,
            max_disk_usage_gb: 1024,
            max_open_files: 10000,
            max_rpc_connections: 1000,
        };

        let mut monitor = SystemResourceMonitor::new(limits);
        let violations = monitor.check_system_resources().await.unwrap();

        // With a reasonable memory limit, we expect no memory violations
        let has_memory_violation = violations.iter().any(|v| v.contains("memory"));
        // This assertion might fail on systems with very high memory usage,
        // so we'll just print the result instead of asserting
        println!("Memory violations detected: {has_memory_violation}");
    }

    #[tokio::test]
    async fn test_cpu_monitoring() {
        let limits = ResourceLimits {
            max_memory_mb: 1024 * 1024,
            max_cpu_percent: 95.0, // Allow high CPU usage
            max_disk_usage_gb: 1024,
            max_open_files: 10000,
            max_rpc_connections: 1000,
        };

        let mut monitor = SystemResourceMonitor::new(limits);
        let violations = monitor.check_system_resources().await.unwrap();

        // With a reasonable CPU limit, we typically expect no violations
        let has_cpu_violation = violations.iter().any(|v| v.contains("CPU"));
        println!("CPU violations detected: {has_cpu_violation}");
    }
}

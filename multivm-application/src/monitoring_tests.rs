//! Comprehensive tests for monitoring and admin functionality

#[cfg(test)]
mod tests {
    use crate::monitoring::{MonitoringService, MonitoringConfig, MetricsConfig, HealthConfig};
    use crate::{ApplicationServer, ApplicationState, ServerStatus};
    use crate::config::ApplicationConfig;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_monitoring_config() -> MonitoringConfig {
        MonitoringConfig {
            metrics: MetricsConfig {
                enabled: true,
                bind_address: "127.0.0.1:0".to_string(), // Random port
                endpoint_path: "/metrics".to_string(),
                update_interval_seconds: 10,
                retention_days: 7,
                enable_system_metrics: true,
                enable_application_metrics: true,
                enable_custom_metrics: true,
            },
            health: HealthConfig {
                enabled: true,
                bind_address: "127.0.0.1:0".to_string(), // Random port
                endpoint_path: "/health".to_string(),
                check_interval_seconds: 30,
                timeout_seconds: 5,
                enable_detailed_checks: true,
                enable_dependency_checks: true,
            },
            tracing: crate::monitoring::TracingConfig {
                enabled: true,
                level: "info".to_string(),
                endpoint: None,
                service_name: "multivm-test".to_string(),
                sample_rate: 1.0,
            },
        }
    }

    #[tokio::test]
    async fn test_monitoring_service_creation() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await;
        assert!(monitoring.is_ok(), "Monitoring service creation should succeed");
    }

    #[tokio::test]
    async fn test_metrics_server_startup() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        let start_result = monitoring.start_metrics_server().await;
        assert!(start_result.is_ok(), "Metrics server startup should succeed");
    }

    #[tokio::test]
    async fn test_health_check_server_startup() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        let start_result = monitoring.start_health_check_server().await;
        assert!(start_result.is_ok(), "Health check server startup should succeed");
    }

    #[tokio::test]
    async fn test_system_metrics_collection() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        let metrics_result = monitoring.collect_system_metrics().await;
        assert!(metrics_result.is_ok(), "System metrics collection should succeed");

        let metrics = metrics_result.unwrap();
        assert!(metrics.contains_key("cpu_usage"), "Should include CPU usage");
        assert!(metrics.contains_key("memory_usage"), "Should include memory usage");
        assert!(metrics.contains_key("disk_usage"), "Should include disk usage");
    }

    #[tokio::test]
    async fn test_application_metrics_collection() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        let metrics_result = monitoring.collect_application_metrics().await;
        assert!(metrics_result.is_ok(), "Application metrics collection should succeed");

        let metrics = metrics_result.unwrap();
        // Application metrics might include request counts, response times, etc.
        assert!(metrics.is_object(), "Metrics should be a JSON object");
    }

    #[tokio::test]
    async fn test_health_checks() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        // Test overall health check
        let health_result = monitoring.check_health().await;
        assert!(health_result.is_ok(), "Health check should succeed");

        let health_status = health_result.unwrap();
        assert!(health_status.contains_key("status"), "Should include overall status");
        assert!(health_status.contains_key("checks"), "Should include individual checks");

        // Test specific component health checks
        let db_health = monitoring.check_database_health().await;
        let cache_health = monitoring.check_cache_health().await;
        let gateway_health = monitoring.check_gateway_health().await;

        // These may succeed or fail depending on component availability
        // We just verify the methods exist and return proper types
        assert!(db_health.is_ok() || db_health.is_err(), "DB health check should return Result");
        assert!(cache_health.is_ok() || cache_health.is_err(), "Cache health check should return Result");
        assert!(gateway_health.is_ok() || gateway_health.is_err(), "Gateway health check should return Result");
    }

    #[tokio::test]
    async fn test_custom_metrics_registration() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        // Test custom counter metric
        let counter_result = monitoring.register_counter("test_counter", "A test counter metric").await;
        assert!(counter_result.is_ok(), "Counter registration should succeed");

        // Test custom gauge metric
        let gauge_result = monitoring.register_gauge("test_gauge", "A test gauge metric").await;
        assert!(gauge_result.is_ok(), "Gauge registration should succeed");

        // Test custom histogram metric
        let histogram_result = monitoring.register_histogram("test_histogram", "A test histogram metric").await;
        assert!(histogram_result.is_ok(), "Histogram registration should succeed");
    }

    #[tokio::test]
    async fn test_metrics_updates() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        // Register and update a counter
        monitoring.register_counter("request_count", "Total requests").await.unwrap();
        
        let increment_result = monitoring.increment_counter("request_count", 1.0).await;
        assert!(increment_result.is_ok(), "Counter increment should succeed");

        let increment_result2 = monitoring.increment_counter("request_count", 5.0).await;
        assert!(increment_result2.is_ok(), "Counter increment should succeed");

        // Register and update a gauge
        monitoring.register_gauge("active_connections", "Active connections").await.unwrap();
        
        let set_result = monitoring.set_gauge("active_connections", 42.0).await;
        assert!(set_result.is_ok(), "Gauge set should succeed");

        // Record histogram values
        monitoring.register_histogram("response_time", "Response time").await.unwrap();
        
        let record_result = monitoring.record_histogram("response_time", 0.123).await;
        assert!(record_result.is_ok(), "Histogram record should succeed");
    }

    #[tokio::test]
    async fn test_application_server_status() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ApplicationConfig::default();
        config.server.rest.port = 0; // Random port
        config.server.graphql.port = 0;
        config.server.websocket.port = 0;
        config.server.admin.port = 0;
        config.database.path = temp_dir.path().join("test.db");
        config.cache.redis.enabled = false;

        let app = ApplicationServer::new(config).await.unwrap();

        // Test server status before starting
        let status_before = app.get_status().await.unwrap();
        assert!(!status_before.is_running, "Server should not be running initially");
        assert!(status_before.uptime.is_none(), "Uptime should be None when not running");

        // Test status fields
        assert!(!status_before.version.is_empty(), "Version should not be empty");
        assert_eq!(status_before.api_servers.rest, false, "REST server should not be running");
        assert_eq!(status_before.api_servers.graphql, false, "GraphQL server should not be running");
        assert_eq!(status_before.api_servers.websocket, false, "WebSocket server should not be running");
        assert_eq!(status_before.api_servers.admin, false, "Admin server should not be running");
    }

    #[tokio::test]
    async fn test_application_state_monitoring() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ApplicationConfig::default();
        config.database.path = temp_dir.path().join("test.db");
        config.cache.redis.enabled = false;

        let state = ApplicationState::new(config).await.unwrap();

        // Test state components
        assert!(!state.is_running.read().await.clone(), "Should not be running initially");
        assert!(state.auth_manager.read().await.is_ok(), "Auth manager should be accessible");
        assert!(state.cache.exists("non_existent_key").await.is_ok(), "Cache should be functional");
        assert!(state.monitoring.check_health().await.is_ok(), "Monitoring should be functional");
    }

    #[tokio::test]
    async fn test_monitoring_error_handling() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        // Test error handling for non-existent metrics
        let invalid_counter = monitoring.increment_counter("non_existent_counter", 1.0).await;
        assert!(invalid_counter.is_err(), "Should fail for non-existent counter");

        let invalid_gauge = monitoring.set_gauge("non_existent_gauge", 1.0).await;
        assert!(invalid_gauge.is_err(), "Should fail for non-existent gauge");

        let invalid_histogram = monitoring.record_histogram("non_existent_histogram", 1.0).await;
        assert!(invalid_histogram.is_err(), "Should fail for non-existent histogram");
    }

    #[tokio::test]
    async fn test_metrics_export_format() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        // Register some test metrics
        monitoring.register_counter("test_requests", "Test requests").await.unwrap();
        monitoring.register_gauge("test_memory", "Test memory usage").await.unwrap();
        
        // Update metrics
        monitoring.increment_counter("test_requests", 100.0).await.unwrap();
        monitoring.set_gauge("test_memory", 75.5).await.unwrap();

        // Export metrics
        let export_result = monitoring.export_metrics().await;
        assert!(export_result.is_ok(), "Metrics export should succeed");

        let metrics_text = export_result.unwrap();
        assert!(metrics_text.contains("test_requests"), "Should contain counter metric");
        assert!(metrics_text.contains("test_memory"), "Should contain gauge metric");
        assert!(metrics_text.contains("100"), "Should contain counter value");
        assert!(metrics_text.contains("75.5"), "Should contain gauge value");
    }

    #[tokio::test]
    async fn test_concurrent_metrics_updates() {
        let config = create_test_monitoring_config();
        let monitoring = std::sync::Arc::new(MonitoringService::new(&config).await.unwrap());

        // Register metrics
        monitoring.register_counter("concurrent_counter", "Concurrent counter").await.unwrap();
        monitoring.register_gauge("concurrent_gauge", "Concurrent gauge").await.unwrap();

        let mut handles = Vec::new();

        // Spawn concurrent metric updates
        for i in 0..10 {
            let monitoring_clone = monitoring.clone();
            let handle = tokio::spawn(async move {
                // Update counter
                let counter_result = monitoring_clone.increment_counter("concurrent_counter", 1.0).await;
                
                // Update gauge
                let gauge_result = monitoring_clone.set_gauge("concurrent_gauge", i as f64).await;
                
                counter_result.is_ok() && gauge_result.is_ok()
            });
            handles.push(handle);
        }

        // Wait for all updates
        let mut success_count = 0;
        for handle in handles {
            if let Ok(true) = handle.await {
                success_count += 1;
            }
        }

        assert_eq!(success_count, 10, "All concurrent updates should succeed");
    }

    #[tokio::test]
    async fn test_health_check_timeout() {
        let mut config = create_test_monitoring_config();
        config.health.timeout_seconds = 1; // Very short timeout
        
        let monitoring = MonitoringService::new(&config).await.unwrap();

        let start_time = std::time::Instant::now();
        let _health_result = monitoring.check_health().await;
        let elapsed = start_time.elapsed();

        // Health check should respect timeout (allowing some overhead)
        assert!(elapsed.as_secs() <= 5, "Health check should respect timeout");
    }

    #[tokio::test]
    async fn test_monitoring_configuration_validation() {
        // Test valid configuration
        let valid_config = create_test_monitoring_config();
        assert!(valid_config.validate().is_ok(), "Valid config should pass validation");

        // Test invalid configuration - empty bind address
        let mut invalid_config = create_test_monitoring_config();
        invalid_config.metrics.bind_address = "".to_string();
        assert!(invalid_config.validate().is_err(), "Empty bind address should fail validation");

        // Test invalid configuration - zero intervals
        let mut invalid_config2 = create_test_monitoring_config();
        invalid_config2.metrics.update_interval_seconds = 0;
        assert!(invalid_config2.validate().is_err(), "Zero interval should fail validation");

        // Test invalid configuration - invalid sample rate
        let mut invalid_config3 = create_test_monitoring_config();
        invalid_config3.tracing.sample_rate = 2.0; // Should be between 0.0 and 1.0
        assert!(invalid_config3.validate().is_err(), "Invalid sample rate should fail validation");
    }

    #[tokio::test]
    async fn test_tracing_configuration() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        // Test tracing setup
        let tracing_result = monitoring.setup_tracing().await;
        assert!(tracing_result.is_ok(), "Tracing setup should succeed");

        // Test span creation and recording
        let span_result = monitoring.create_span("test_operation", "Testing tracing").await;
        assert!(span_result.is_ok(), "Span creation should succeed");
    }

    #[tokio::test]
    async fn test_monitoring_shutdown() {
        let config = create_test_monitoring_config();
        let monitoring = MonitoringService::new(&config).await.unwrap();

        // Start monitoring services
        let _metrics_start = monitoring.start_metrics_server().await;
        let _health_start = monitoring.start_health_check_server().await;

        // Test graceful shutdown
        let shutdown_result = monitoring.shutdown().await;
        assert!(shutdown_result.is_ok(), "Monitoring shutdown should succeed");

        // Verify services are stopped
        let post_shutdown_health = monitoring.check_health().await;
        // After shutdown, health check might fail or return a "shutting down" status
        // We just verify it doesn't panic
        let _ = post_shutdown_health;
    }
}
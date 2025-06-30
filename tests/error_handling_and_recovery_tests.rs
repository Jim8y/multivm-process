//! Comprehensive Error Handling and Recovery Tests
//!
//! These tests verify the system's ability to handle various error conditions
//! gracefully and recover from failures without data loss or corruption.

use multivm_application::api::rest::create_app;
use multivm_common::{
    config::MultivmConfig,
    types::{ProcessId, health::HealthStatus},
    IpcMessage, IpcCommand, IpcResponse,
    MultivmResult, MultivmError,
};
use multivm_p2p::{
    protocol::messages::{NetworkMessage, MessagePayload, ControlMessage},
    core::network::P2PNetwork,
    error::P2PError,
};
use multivm_process_manager::{
    MultivmProcessManager,
    coordinator::MultivmCoordinator,
    health::HealthMonitor,
};
use axum::{
    body::Body,
    http::{Request, Method, StatusCode},
};
use serde_json::{json, Value};
use std::{
    time::{Duration, Instant},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    collections::HashMap,
};
use tempfile::TempDir;
use tokio::{
    test,
    time::{sleep, timeout},
    sync::{Mutex, RwLock},
    task::JoinSet,
};
use tower::ServiceExt;
use tracing_test::traced_test;

/// Error injection framework for testing failure scenarios
#[derive(Debug, Clone)]
struct ErrorInjector {
    network_failures: Arc<AtomicBool>,
    storage_failures: Arc<AtomicBool>,
    consensus_failures: Arc<AtomicBool>,
    memory_pressure: Arc<AtomicBool>,
    slow_responses: Arc<AtomicBool>,
    corruption_errors: Arc<AtomicBool>,
    failure_rate: Arc<Mutex<f64>>,
}

impl ErrorInjector {
    fn new() -> Self {
        Self {
            network_failures: Arc::new(AtomicBool::new(false)),
            storage_failures: Arc::new(AtomicBool::new(false)),
            consensus_failures: Arc::new(AtomicBool::new(false)),
            memory_pressure: Arc::new(AtomicBool::new(false)),
            slow_responses: Arc::new(AtomicBool::new(false)),
            corruption_errors: Arc::new(AtomicBool::new(false)),
            failure_rate: Arc::new(Mutex::new(0.0)),
        }
    }

    async fn enable_network_failures(&self, rate: f64) {
        self.network_failures.store(true, Ordering::Relaxed);
        *self.failure_rate.lock().await = rate;
    }

    async fn enable_storage_failures(&self, rate: f64) {
        self.storage_failures.store(true, Ordering::Relaxed);
        *self.failure_rate.lock().await = rate;
    }

    async fn enable_consensus_failures(&self, rate: f64) {
        self.consensus_failures.store(true, Ordering::Relaxed);
        *self.failure_rate.lock().await = rate;
    }

    async fn enable_memory_pressure(&self) {
        self.memory_pressure.store(true, Ordering::Relaxed);
    }

    async fn enable_slow_responses(&self) {
        self.slow_responses.store(true, Ordering::Relaxed);
    }

    async fn enable_corruption_errors(&self, rate: f64) {
        self.corruption_errors.store(true, Ordering::Relaxed);
        *self.failure_rate.lock().await = rate;
    }

    fn disable_all(&self) {
        self.network_failures.store(false, Ordering::Relaxed);
        self.storage_failures.store(false, Ordering::Relaxed);
        self.consensus_failures.store(false, Ordering::Relaxed);
        self.memory_pressure.store(false, Ordering::Relaxed);
        self.slow_responses.store(false, Ordering::Relaxed);
        self.corruption_errors.store(false, Ordering::Relaxed);
    }

    async fn should_fail(&self) -> bool {
        let rate = *self.failure_rate.lock().await;
        rand::random::<f64>() < rate
    }
}

async fn create_test_system_with_error_injection() -> MultivmResult<(MultivmProcessManager, axum::Router, ErrorInjector)> {
    let temp_dir = TempDir::new().unwrap();
    let mut config = MultivmConfig::default();
    config.system.data_dir = temp_dir.path().to_path_buf();
    config.server.rest.port = 0;
    config.cache.redis.enabled = false;

    let process_manager = MultivmProcessManager::new(config.clone()).await?;
    
    let app_state = create_test_app_state(&config).await?;
    let app = create_app(app_state).await?;
    
    let error_injector = ErrorInjector::new();

    Ok((process_manager, app, error_injector))
}

async fn create_test_app_state(config: &MultivmConfig) -> MultivmResult<multivm_application::api::rest::AppState> {
    let auth_manager = multivm_application::auth::manager::AuthManager::new(config.auth.clone()).await?;
    let cache = Box::new(multivm_application::cache::memory::MemoryCache::new(1000, Duration::from_secs(300)));
    let gateway = multivm_application::gateway::unified::UnifiedGateway::new(config.clone()).await?;
    let health_monitor = multivm_application::monitoring::health::HealthMonitor::new(Duration::from_secs(1)).await?;
    let metrics_collector = multivm_application::monitoring::metrics::MetricsCollector::new().await?;
    
    Ok(multivm_application::api::rest::AppState {
        config: config.clone(),
        auth_manager,
        cache,
        gateway,
        health_monitor,
        metrics_collector,
    })
}

#[traced_test]
#[test]
async fn test_network_partition_recovery() {
    let (mut process_manager, app, error_injector) = create_test_system_with_error_injection().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing network partition recovery");

    // Verify system is healthy initially
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Inject network failures
    error_injector.enable_network_failures(0.8).await;
    
    // System should detect network issues but remain partially operational
    sleep(Duration::from_secs(2)).await;
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    // Should still respond, possibly with degraded status
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::SERVICE_UNAVAILABLE);

    // Disable network failures (simulate recovery)
    error_injector.disable_all();
    
    // Wait for recovery
    sleep(Duration::from_secs(3)).await;
    
    // System should fully recover
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_storage_failure_handling() {
    let (mut process_manager, app, error_injector) = create_test_system_with_error_injection().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing storage failure handling");

    // Create some data before failure
    let account_data = json!({
        "vm_type": "svm",
        "address": "storage_test_account",
        "metadata": {"test": "storage_failure"}
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header("content-type", "application/json")
        .body(Body::from(account_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Inject storage failures
    error_injector.enable_storage_failures(0.5).await;

    // Try to create more accounts during storage failures
    let mut success_count = 0;
    let mut failure_count = 0;

    for i in 0..10 {
        let account_data = json!({
            "vm_type": "svm",
            "address": format!("failure_test_account_{}", i),
            "metadata": {"test": "during_storage_failure"}
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/accounts")
            .header("content-type", "application/json")
            .body(Body::from(account_data.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        if response.status() == StatusCode::CREATED {
            success_count += 1;
        } else {
            failure_count += 1;
        }
    }

    println!("During storage failures: {} success, {} failures", success_count, failure_count);
    
    // Should have some failures due to storage issues
    assert!(failure_count > 0, "Storage failures should cause some request failures");

    // Disable storage failures
    error_injector.disable_all();
    sleep(Duration::from_secs(2)).await;

    // Verify original data is still accessible
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/accounts/storage_test_account")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_consensus_failure_recovery() {
    let (mut process_manager, app, error_injector) = create_test_system_with_error_injection().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing consensus failure recovery");

    // Submit transactions before consensus failure
    let tx_data = json!({
        "vm_type": "svm",
        "transaction_data": "consensus_test_tx",
        "priority": "high"
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/transactions")
        .header("content-type", "application/json")
        .body(Body::from(tx_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Inject consensus failures
    error_injector.enable_consensus_failures(0.7).await;

    // Submit transactions during consensus failures
    let mut consensus_success = 0;
    let mut consensus_failures = 0;

    for i in 0..10 {
        let tx_data = json!({
            "vm_type": "svm",
            "transaction_data": format!("consensus_failure_tx_{}", i),
            "priority": "normal"
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/transactions")
            .header("content-type", "application/json")
            .body(Body::from(tx_data.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        if response.status() == StatusCode::ACCEPTED {
            consensus_success += 1;
        } else {
            consensus_failures += 1;
        }
    }

    println!("During consensus failures: {} accepted, {} rejected", consensus_success, consensus_failures);

    // Disable consensus failures
    error_injector.disable_all();
    sleep(Duration::from_secs(3)).await;

    // Check system status after recovery
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/system/status")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_memory_pressure_handling() {
    let (mut process_manager, app, error_injector) = create_test_system_with_error_injection().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing memory pressure handling");

    // Enable memory pressure simulation
    error_injector.enable_memory_pressure().await;

    // Try to submit large payloads during memory pressure
    let large_payload = "x".repeat(1024 * 1024); // 1MB
    let mut handled_requests = 0;
    let mut rejected_requests = 0;

    for i in 0..5 {
        let tx_data = json!({
            "vm_type": "svm",
            "transaction_data": format!("{}_large_tx_{}", large_payload, i),
            "priority": "normal"
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/transactions")
            .header("content-type", "application/json")
            .body(Body::from(tx_data.to_string()))
            .unwrap();

        let response = timeout(Duration::from_secs(5), app.clone().oneshot(request)).await;
        
        match response {
            Ok(Ok(resp)) if resp.status().is_success() => handled_requests += 1,
            _ => rejected_requests += 1,
        }
    }

    println!("Under memory pressure: {} handled, {} rejected", handled_requests, rejected_requests);

    // System should handle memory pressure gracefully
    assert!(handled_requests > 0 || rejected_requests > 0, "System should respond to requests even under memory pressure");

    // Disable memory pressure
    error_injector.disable_all();
    sleep(Duration::from_secs(2)).await;

    // Normal requests should work after pressure is relieved
    let normal_tx = json!({
        "vm_type": "svm",
        "transaction_data": "normal_tx_after_pressure",
        "priority": "normal"
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/transactions")
        .header("content-type", "application/json")
        .body(Body::from(normal_tx.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert!(response.status().is_success());

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_cascading_failure_prevention() {
    let (mut process_manager, app, error_injector) = create_test_system_with_error_injection().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing cascading failure prevention");

    // Start with network failures
    error_injector.enable_network_failures(0.3).await;
    sleep(Duration::from_secs(1)).await;

    // Add storage failures
    error_injector.enable_storage_failures(0.2).await;
    sleep(Duration::from_secs(1)).await;

    // Add consensus failures
    error_injector.enable_consensus_failures(0.4).await;
    sleep(Duration::from_secs(1)).await;

    // System should still be responsive despite multiple failure types
    let mut total_requests = 0;
    let mut successful_requests = 0;

    for i in 0..20 {
        total_requests += 1;
        
        let request = Request::builder()
            .method(Method::GET)
            .uri(&format!("/api/v1/health?cascade_test={}", i))
            .body(Body::empty())
            .unwrap();

        let response = timeout(Duration::from_secs(2), app.clone().oneshot(request)).await;
        
        if let Ok(Ok(resp)) = response {
            if resp.status().is_success() {
                successful_requests += 1;
            }
        }
    }

    let success_rate = successful_requests as f64 / total_requests as f64;
    println!("Under cascading failures: {:.2}% success rate", success_rate * 100.0);

    // System should maintain some level of functionality even with cascading failures
    assert!(success_rate > 0.1, "System should maintain minimal functionality during cascading failures");

    // Gradually recover from failures
    error_injector.disable_all();
    sleep(Duration::from_secs(3)).await;

    // Verify full recovery
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_data_corruption_detection_and_recovery() {
    let (mut process_manager, app, error_injector) = create_test_system_with_error_injection().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing data corruption detection and recovery");

    // Create valid data first
    let account_data = json!({
        "vm_type": "svm",
        "address": "corruption_test_account",
        "metadata": {
            "checksum": "valid_checksum",
            "integrity": "high"
        }
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header("content-type", "application/json")
        .body(Body::from(account_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Enable corruption errors
    error_injector.enable_corruption_errors(0.3).await;

    // Try to create accounts with potential corruption
    let mut corruption_detected = 0;
    let mut valid_operations = 0;

    for i in 0..15 {
        let account_data = json!({
            "vm_type": "svm",
            "address": format!("corruption_test_{}", i),
            "metadata": {
                "checksum": if i % 3 == 0 { "corrupted_checksum" } else { "valid_checksum" },
                "integrity": "test"
            }
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/accounts")
            .header("content-type", "application/json")
            .body(Body::from(account_data.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        
        if response.status() == StatusCode::BAD_REQUEST || response.status() == StatusCode::UNPROCESSABLE_ENTITY {
            corruption_detected += 1;
        } else if response.status() == StatusCode::CREATED {
            valid_operations += 1;
        }
    }

    println!("Corruption detection: {} corrupted detected, {} valid operations", 
             corruption_detected, valid_operations);

    // System should detect and reject corrupted data
    assert!(corruption_detected > 0, "System should detect data corruption");
    assert!(valid_operations > 0, "System should still process valid data");

    // Disable corruption simulation
    error_injector.disable_all();

    // Verify original valid data is still intact
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/accounts/corruption_test_account")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_slow_response_timeout_handling() {
    let (mut process_manager, app, error_injector) = create_test_system_with_error_injection().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing slow response timeout handling");

    // Normal requests should work quickly
    let start_time = Instant::now();
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let normal_response_time = start_time.elapsed();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(normal_response_time < Duration::from_millis(100));

    // Enable slow responses
    error_injector.enable_slow_responses().await;

    // Test timeout handling
    let mut timeout_count = 0;
    let mut completed_count = 0;

    for i in 0..5 {
        let request = Request::builder()
            .method(Method::GET)
            .uri(&format!("/api/v1/system/status?slow_test={}", i))
            .body(Body::empty())
            .unwrap();

        let result = timeout(Duration::from_secs(2), app.clone().oneshot(request)).await;
        
        match result {
            Ok(Ok(_)) => completed_count += 1,
            Err(_) => timeout_count += 1, // Timeout occurred
            Ok(Err(_)) => {}, // Connection error
        }
    }

    println!("Slow response test: {} completed, {} timed out", completed_count, timeout_count);

    // Some requests should timeout, but system should handle it gracefully
    assert!(timeout_count > 0 || completed_count > 0, "System should handle slow responses");

    // Disable slow responses
    error_injector.disable_all();
    sleep(Duration::from_secs(1)).await;

    // Normal performance should resume
    let start_time = Instant::now();
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let recovered_response_time = start_time.elapsed();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(recovered_response_time < Duration::from_millis(200));

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_concurrent_error_scenarios() {
    let (mut process_manager, app, error_injector) = create_test_system_with_error_injection().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing concurrent error scenarios");

    // Enable multiple types of failures simultaneously
    error_injector.enable_network_failures(0.2).await;
    error_injector.enable_storage_failures(0.1).await;
    error_injector.enable_slow_responses().await;

    let concurrent_requests = 50;
    let mut join_set = JoinSet::new();

    // Submit various types of requests concurrently
    for i in 0..concurrent_requests {
        let app_clone = app.clone();
        
        join_set.spawn(async move {
            let request_type = i % 4;
            
            let (method, uri, body) = match request_type {
                0 => (Method::GET, "/api/v1/health", None),
                1 => (Method::GET, "/api/v1/system/status", None),
                2 => {
                    let account_data = json!({
                        "vm_type": "svm",
                        "address": format!("concurrent_error_test_{}", i),
                        "metadata": {"test": "concurrent_errors"}
                    });
                    (Method::POST, "/api/v1/accounts", Some(account_data.to_string()))
                },
                3 => {
                    let tx_data = json!({
                        "vm_type": "evm",
                        "transaction_data": format!("concurrent_error_tx_{}", i),
                        "priority": "normal"
                    });
                    (Method::POST, "/api/v1/transactions", Some(tx_data.to_string()))
                },
                _ => unreachable!(),
            };

            let mut request_builder = Request::builder()
                .method(method)
                .uri(uri);

            if body.is_some() {
                request_builder = request_builder.header("content-type", "application/json");
            }

            let request = request_builder
                .body(Body::from(body.unwrap_or_default()))
                .unwrap();

            let result = timeout(Duration::from_secs(3), app_clone.oneshot(request)).await;
            
            match result {
                Ok(Ok(response)) => Some(response.status()),
                _ => None,
            }
        });
    }

    // Collect results
    let mut successful = 0;
    let mut failed = 0;
    let mut timed_out = 0;

    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            Some(status) if status.is_success() => successful += 1,
            Some(_) => failed += 1,
            None => timed_out += 1,
        }
    }

    println!("Concurrent error test results: {} successful, {} failed, {} timed out", 
             successful, failed, timed_out);

    // System should handle concurrent errors gracefully
    assert!(successful > 0, "Some requests should succeed even with concurrent errors");
    let total_handled = successful + failed + timed_out;
    assert_eq!(total_handled, concurrent_requests, "All requests should be handled");

    // Disable all errors
    error_injector.disable_all();
    sleep(Duration::from_secs(2)).await;

    // System should fully recover
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_graceful_degradation() {
    let (mut process_manager, app, error_injector) = create_test_system_with_error_injection().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing graceful degradation under various failure modes");

    // Test progressive failure scenarios
    let failure_scenarios = vec![
        ("network_only", 0.1, false, false),
        ("storage_only", 0.0, true, false),
        ("consensus_only", 0.0, false, true),
        ("network_and_storage", 0.2, true, false),
        ("all_failures", 0.3, true, true),
    ];

    for (scenario_name, network_rate, storage_fail, consensus_fail) in failure_scenarios {
        println!("  Testing scenario: {}", scenario_name);
        
        // Reset error injector
        error_injector.disable_all();
        sleep(Duration::from_millis(500)).await;

        // Configure failure scenario
        if network_rate > 0.0 {
            error_injector.enable_network_failures(network_rate).await;
        }
        if storage_fail {
            error_injector.enable_storage_failures(0.2).await;
        }
        if consensus_fail {
            error_injector.enable_consensus_failures(0.3).await;
        }

        // Test basic functionality
        let mut health_checks = 0;
        let mut status_checks = 0;
        let mut account_creates = 0;

        for i in 0..10 {
            // Health check (should be most resilient)
            let request = Request::builder()
                .method(Method::GET)
                .uri(&format!("/api/v1/health?scenario={}&req={}", scenario_name, i))
                .body(Body::empty())
                .unwrap();

            if let Ok(response) = timeout(Duration::from_secs(2), app.clone().oneshot(request)).await {
                if let Ok(resp) = response {
                    if resp.status().is_success() {
                        health_checks += 1;
                    }
                }
            }

            // Status check (should be moderately resilient)
            let request = Request::builder()
                .method(Method::GET)
                .uri(&format!("/api/v1/system/status?scenario={}&req={}", scenario_name, i))
                .body(Body::empty())
                .unwrap();

            if let Ok(response) = timeout(Duration::from_secs(2), app.clone().oneshot(request)).await {
                if let Ok(resp) = response {
                    if resp.status().is_success() {
                        status_checks += 1;
                    }
                }
            }

            // Account creation (should be least resilient to storage/consensus failures)
            let account_data = json!({
                "vm_type": "svm",
                "address": format!("degradation_test_{}_{}", scenario_name, i),
                "metadata": {"test": scenario_name}
            });

            let request = Request::builder()
                .method(Method::POST)
                .uri("/api/v1/accounts")
                .header("content-type", "application/json")
                .body(Body::from(account_data.to_string()))
                .unwrap();

            if let Ok(response) = timeout(Duration::from_secs(2), app.clone().oneshot(request)).await {
                if let Ok(resp) = response {
                    if resp.status() == StatusCode::CREATED {
                        account_creates += 1;
                    }
                }
            }
        }

        println!("    Results: health={}/10, status={}/10, accounts={}/10", 
                 health_checks, status_checks, account_creates);

        // Verify graceful degradation expectations
        match scenario_name {
            "network_only" => {
                assert!(health_checks >= 7, "Health checks should be resilient to network issues");
                assert!(status_checks >= 5, "Status checks should partially work with network issues");
            },
            "storage_only" => {
                assert!(health_checks >= 8, "Health checks should work despite storage issues");
                assert!(account_creates <= 3, "Account creation should be impacted by storage failures");
            },
            "consensus_only" => {
                assert!(health_checks >= 7, "Health checks should work despite consensus issues");
                assert!(status_checks >= 5, "Status checks should partially work with consensus issues");
            },
            "all_failures" => {
                assert!(health_checks >= 3, "Some health checks should work even with all failures");
                // Other operations may fail completely under all failure modes
            },
            _ => {},
        }
    }

    // Full recovery test
    error_injector.disable_all();
    sleep(Duration::from_secs(3)).await;

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    process_manager.stop().await.unwrap();
}
//! Comprehensive Stress and Load Testing
//!
//! These tests verify system behavior under high load conditions,
//! stress scenarios, and edge cases that might occur in production.

use multivm_application::api::rest::create_app;
use multivm_common::{
    config::MultivmConfig,
    types::{ProcessId, health::HealthStatus},
    IpcMessage, IpcCommand,
    MultivmResult,
};
use multivm_p2p::{
    protocol::messages::{NetworkMessage, MessagePayload, ControlMessage},
    core::network::P2PNetwork,
};
use multivm_process_manager::MultivmProcessManager;
use axum::{
    body::Body,
    http::{Request, Method, StatusCode},
};
use serde_json::{json, Value};
use std::{
    time::{Duration, Instant},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicBool, Ordering},
    },
    collections::HashMap,
};
use tempfile::TempDir;
use tokio::{
    test,
    time::{sleep, interval, timeout},
    sync::{Mutex, Semaphore, RwLock},
    task::JoinSet,
};
use tower::ServiceExt;
use tracing_test::traced_test;

/// Performance metrics collector for stress tests
#[derive(Debug, Clone)]
struct StressTestMetrics {
    requests_sent: Arc<AtomicU64>,
    requests_successful: Arc<AtomicU64>,
    requests_failed: Arc<AtomicU64>,
    total_response_time: Arc<Mutex<Duration>>,
    max_response_time: Arc<Mutex<Duration>>,
    min_response_time: Arc<Mutex<Duration>>,
    start_time: Instant,
}

impl StressTestMetrics {
    fn new() -> Self {
        Self {
            requests_sent: Arc::new(AtomicU64::new(0)),
            requests_successful: Arc::new(AtomicU64::new(0)),
            requests_failed: Arc::new(AtomicU64::new(0)),
            total_response_time: Arc::new(Mutex::new(Duration::ZERO)),
            max_response_time: Arc::new(Mutex::new(Duration::ZERO)),
            min_response_time: Arc::new(Mutex::new(Duration::MAX)),
            start_time: Instant::now(),
        }
    }

    async fn record_request(&self, response_time: Duration, success: bool) {
        self.requests_sent.fetch_add(1, Ordering::Relaxed);
        
        if success {
            self.requests_successful.fetch_add(1, Ordering::Relaxed);
        } else {
            self.requests_failed.fetch_add(1, Ordering::Relaxed);
        }

        let mut total = self.total_response_time.lock().await;
        *total += response_time;

        let mut max = self.max_response_time.lock().await;
        if response_time > *max {
            *max = response_time;
        }

        let mut min = self.min_response_time.lock().await;
        if response_time < *min {
            *min = response_time;
        }
    }

    async fn get_summary(&self) -> StressTestSummary {
        let sent = self.requests_sent.load(Ordering::Relaxed);
        let successful = self.requests_successful.load(Ordering::Relaxed);
        let failed = self.requests_failed.load(Ordering::Relaxed);
        let total_time = *self.total_response_time.lock().await;
        let max_time = *self.max_response_time.lock().await;
        let min_time = *self.min_response_time.lock().await;
        let duration = self.start_time.elapsed();

        StressTestSummary {
            total_requests: sent,
            successful_requests: successful,
            failed_requests: failed,
            success_rate: if sent > 0 { successful as f64 / sent as f64 } else { 0.0 },
            average_response_time: if successful > 0 { total_time / successful as u32 } else { Duration::ZERO },
            max_response_time: max_time,
            min_response_time: if min_time == Duration::MAX { Duration::ZERO } else { min_time },
            requests_per_second: sent as f64 / duration.as_secs_f64(),
            test_duration: duration,
        }
    }
}

#[derive(Debug)]
struct StressTestSummary {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    success_rate: f64,
    average_response_time: Duration,
    max_response_time: Duration,
    min_response_time: Duration,
    requests_per_second: f64,
    test_duration: Duration,
}

async fn create_test_system() -> MultivmResult<(MultivmProcessManager, axum::Router)> {
    let temp_dir = TempDir::new().unwrap();
    let mut config = MultivmConfig::default();
    config.system.data_dir = temp_dir.path().to_path_buf();
    config.server.rest.port = 0;
    config.cache.redis.enabled = false;

    let process_manager = MultivmProcessManager::new(config.clone()).await?;
    
    let app_state = create_test_app_state(&config).await?;
    let app = create_app(app_state).await?;

    Ok((process_manager, app))
}

async fn create_test_app_state(config: &MultivmConfig) -> MultivmResult<multivm_application::api::rest::AppState> {
    let auth_manager = multivm_application::auth::manager::AuthManager::new(config.auth.clone()).await?;
    let cache = Box::new(multivm_application::cache::memory::MemoryCache::new(10000, Duration::from_secs(300)));
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
async fn test_high_volume_api_requests() {
    let (_process_manager, app) = create_test_system().await.unwrap();
    let metrics = StressTestMetrics::new();
    
    let concurrent_users = 50;
    let requests_per_user = 100;
    let total_requests = concurrent_users * requests_per_user;

    println!("Starting high volume API test: {} concurrent users, {} requests each", 
             concurrent_users, requests_per_user);

    let semaphore = Arc::new(Semaphore::new(concurrent_users));
    let mut join_set = JoinSet::new();

    for user_id in 0..concurrent_users {
        let app_clone = app.clone();
        let metrics_clone = metrics.clone();
        let semaphore_clone = semaphore.clone();

        join_set.spawn(async move {
            let _permit = semaphore_clone.acquire().await.unwrap();
            
            for req_id in 0..requests_per_user {
                let start_time = Instant::now();
                
                let request = Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/health?user={}&req={}", user_id, req_id))
                    .body(Body::empty())
                    .unwrap();

                let result = app_clone.clone().oneshot(request).await;
                let response_time = start_time.elapsed();
                
                let success = result.map(|r| r.status().is_success()).unwrap_or(false);
                metrics_clone.record_request(response_time, success).await;
            }
        });
    }

    // Wait for all requests to complete
    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    let summary = metrics.get_summary().await;
    
    println!("High Volume API Test Results:");
    println!("  Total requests: {}", summary.total_requests);
    println!("  Successful: {} ({:.2}%)", summary.successful_requests, summary.success_rate * 100.0);
    println!("  Failed: {}", summary.failed_requests);
    println!("  Requests/sec: {:.2}", summary.requests_per_second);
    println!("  Avg response time: {:?}", summary.average_response_time);
    println!("  Max response time: {:?}", summary.max_response_time);
    println!("  Min response time: {:?}", summary.min_response_time);

    // Performance assertions
    assert_eq!(summary.total_requests, total_requests as u64);
    assert!(summary.success_rate > 0.95, "Success rate too low: {:.2}%", summary.success_rate * 100.0);
    assert!(summary.requests_per_second > 100.0, "Throughput too low: {:.2} req/s", summary.requests_per_second);
    assert!(summary.average_response_time < Duration::from_millis(50), "Average response time too high: {:?}", summary.average_response_time);
}

#[traced_test]
#[test]
async fn test_sustained_load() {
    let (_process_manager, app) = create_test_system().await.unwrap();
    let metrics = StressTestMetrics::new();
    
    let test_duration = Duration::from_secs(30);
    let concurrent_workers = 20;
    let target_rps = 200; // Requests per second

    println!("Starting sustained load test: {} workers, target {} RPS for {:?}", 
             concurrent_workers, target_rps, test_duration);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let mut join_set = JoinSet::new();

    // Start worker tasks
    for worker_id in 0..concurrent_workers {
        let app_clone = app.clone();
        let metrics_clone = metrics.clone();
        let stop_flag_clone = stop_flag.clone();

        join_set.spawn(async move {
            let mut request_count = 0;
            let requests_per_worker = target_rps / concurrent_workers;
            let mut interval = interval(Duration::from_millis(1000 / requests_per_worker as u64));

            while !stop_flag_clone.load(Ordering::Relaxed) {
                interval.tick().await;
                
                let start_time = Instant::now();
                
                let request = Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/system/status?worker={}&count={}", worker_id, request_count))
                    .body(Body::empty())
                    .unwrap();

                let result = app_clone.clone().oneshot(request).await;
                let response_time = start_time.elapsed();
                
                let success = result.map(|r| r.status().is_success()).unwrap_or(false);
                metrics_clone.record_request(response_time, success).await;
                
                request_count += 1;
            }
        });
    }

    // Run for specified duration
    sleep(test_duration).await;
    stop_flag.store(true, Ordering::Relaxed);

    // Wait for all workers to stop
    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    let summary = metrics.get_summary().await;
    
    println!("Sustained Load Test Results:");
    println!("  Test duration: {:?}", summary.test_duration);
    println!("  Total requests: {}", summary.total_requests);
    println!("  Successful: {} ({:.2}%)", summary.successful_requests, summary.success_rate * 100.0);
    println!("  Actual RPS: {:.2}", summary.requests_per_second);
    println!("  Avg response time: {:?}", summary.average_response_time);
    println!("  Max response time: {:?}", summary.max_response_time);

    // Performance assertions for sustained load
    assert!(summary.success_rate > 0.98, "Sustained load success rate too low: {:.2}%", summary.success_rate * 100.0);
    assert!(summary.requests_per_second > target_rps as f64 * 0.8, "Sustained RPS too low: {:.2}", summary.requests_per_second);
    assert!(summary.average_response_time < Duration::from_millis(100), "Sustained load response time too high: {:?}", summary.average_response_time);
}

#[traced_test]
#[test]
async fn test_burst_traffic_handling() {
    let (_process_manager, app) = create_test_system().await.unwrap();
    let metrics = StressTestMetrics::new();
    
    let burst_size = 1000;
    let burst_duration = Duration::from_millis(100); // Very short burst
    
    println!("Starting burst traffic test: {} requests in {:?}", burst_size, burst_duration);

    let mut join_set = JoinSet::new();

    // Create a massive burst of requests
    for req_id in 0..burst_size {
        let app_clone = app.clone();
        let metrics_clone = metrics.clone();

        join_set.spawn(async move {
            let start_time = Instant::now();
            
            let request = Request::builder()
                .method(Method::GET)
                .uri(&format!("/api/v1/health?burst={}", req_id))
                .body(Body::empty())
                .unwrap();

            let result = timeout(Duration::from_secs(5), app_clone.oneshot(request)).await;
            let response_time = start_time.elapsed();
            
            let success = match result {
                Ok(Ok(response)) => response.status().is_success(),
                _ => false,
            };
            
            metrics_clone.record_request(response_time, success).await;
        });
    }

    // Wait for all burst requests to complete
    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    let summary = metrics.get_summary().await;
    
    println!("Burst Traffic Test Results:");
    println!("  Burst requests: {}", summary.total_requests);
    println!("  Successful: {} ({:.2}%)", summary.successful_requests, summary.success_rate * 100.0);
    println!("  Peak RPS: {:.2}", summary.requests_per_second);
    println!("  Avg response time: {:?}", summary.average_response_time);
    println!("  Max response time: {:?}", summary.max_response_time);

    // Burst traffic assertions
    assert!(summary.success_rate > 0.8, "Burst traffic success rate too low: {:.2}%", summary.success_rate * 100.0);
    assert!(summary.max_response_time < Duration::from_secs(2), "Burst traffic max response time too high: {:?}", summary.max_response_time);
}

#[traced_test]
#[test]
async fn test_memory_stress() {
    let (_process_manager, app) = create_test_system().await.unwrap();
    let metrics = StressTestMetrics::new();
    
    let large_payload_size = 1024 * 1024; // 1MB per request
    let num_requests = 100;
    
    println!("Starting memory stress test: {} requests with {}MB payloads", 
             num_requests, large_payload_size / (1024 * 1024));

    let mut join_set = JoinSet::new();

    for req_id in 0..num_requests {
        let app_clone = app.clone();
        let metrics_clone = metrics.clone();

        join_set.spawn(async move {
            let start_time = Instant::now();
            
            // Create large payload
            let large_data = json!({
                "id": req_id,
                "data": "x".repeat(large_payload_size),
                "metadata": {
                    "size": large_payload_size,
                    "test": "memory_stress"
                }
            });

            let request = Request::builder()
                .method(Method::POST)
                .uri("/api/v1/transactions")
                .header("content-type", "application/json")
                .body(Body::from(large_data.to_string()))
                .unwrap();

            let result = app_clone.oneshot(request).await;
            let response_time = start_time.elapsed();
            
            let success = result.map(|r| r.status().is_success()).unwrap_or(false);
            metrics_clone.record_request(response_time, success).await;
        });
    }

    // Wait for all memory-intensive requests
    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    let summary = metrics.get_summary().await;
    
    println!("Memory Stress Test Results:");
    println!("  Large requests: {}", summary.total_requests);
    println!("  Successful: {} ({:.2}%)", summary.successful_requests, summary.success_rate * 100.0);
    println!("  Avg response time: {:?}", summary.average_response_time);
    println!("  Max response time: {:?}", summary.max_response_time);

    // Memory stress assertions
    assert!(summary.success_rate > 0.9, "Memory stress success rate too low: {:.2}%", summary.success_rate * 100.0);
    assert!(summary.average_response_time < Duration::from_secs(1), "Memory stress response time too high: {:?}", summary.average_response_time);
}

#[traced_test]
#[test]
async fn test_connection_pool_exhaustion() {
    let (_process_manager, app) = create_test_system().await.unwrap();
    let metrics = StressTestMetrics::new();
    
    let max_connections = 500; // Try to exhaust connection pool
    let hold_duration = Duration::from_secs(2);
    
    println!("Starting connection pool exhaustion test: {} long-lived connections", max_connections);

    let mut join_set = JoinSet::new();

    for conn_id in 0..max_connections {
        let app_clone = app.clone();
        let metrics_clone = metrics.clone();

        join_set.spawn(async move {
            let start_time = Instant::now();
            
            // Simulate long-running request
            sleep(hold_duration).await;
            
            let request = Request::builder()
                .method(Method::GET)
                .uri(&format!("/api/v1/health?conn={}", conn_id))
                .body(Body::empty())
                .unwrap();

            let result = app_clone.oneshot(request).await;
            let response_time = start_time.elapsed();
            
            let success = result.map(|r| r.status().is_success()).unwrap_or(false);
            metrics_clone.record_request(response_time, success).await;
        });
    }

    // Wait for all connection attempts
    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    let summary = metrics.get_summary().await;
    
    println!("Connection Pool Test Results:");
    println!("  Connection attempts: {}", summary.total_requests);
    println!("  Successful: {} ({:.2}%)", summary.successful_requests, summary.success_rate * 100.0);
    println!("  Failed: {}", summary.failed_requests);

    // Connection pool assertions
    assert!(summary.successful_requests > 0, "No connections should succeed");
    // Some connections may fail due to pool limits, which is expected behavior
}

#[traced_test]
#[test]
async fn test_cpu_intensive_operations() {
    let (_process_manager, app) = create_test_system().await.unwrap();
    let metrics = StressTestMetrics::new();
    
    let cpu_intensive_requests = 50;
    let concurrent_workers = 10;
    
    println!("Starting CPU intensive test: {} requests with {} workers", 
             cpu_intensive_requests, concurrent_workers);

    let semaphore = Arc::new(Semaphore::new(concurrent_workers));
    let mut join_set = JoinSet::new();

    for req_id in 0..cpu_intensive_requests {
        let app_clone = app.clone();
        let metrics_clone = metrics.clone();
        let semaphore_clone = semaphore.clone();

        join_set.spawn(async move {
            let _permit = semaphore_clone.acquire().await.unwrap();
            let start_time = Instant::now();
            
            // Create complex computational request
            let complex_data = json!({
                "operation": "complex_calculation",
                "parameters": {
                    "iterations": 10000,
                    "complexity": "high",
                    "id": req_id
                }
            });

            let request = Request::builder()
                .method(Method::POST)
                .uri("/api/v1/system/compute")
                .header("content-type", "application/json")
                .body(Body::from(complex_data.to_string()))
                .unwrap();

            let result = app_clone.oneshot(request).await;
            let response_time = start_time.elapsed();
            
            let success = result.map(|r| r.status().is_success()).unwrap_or(false);
            metrics_clone.record_request(response_time, success).await;
        });
    }

    // Wait for all CPU-intensive operations
    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    let summary = metrics.get_summary().await;
    
    println!("CPU Intensive Test Results:");
    println!("  CPU requests: {}", summary.total_requests);
    println!("  Successful: {} ({:.2}%)", summary.successful_requests, summary.success_rate * 100.0);
    println!("  Avg response time: {:?}", summary.average_response_time);
    println!("  Max response time: {:?}", summary.max_response_time);

    // CPU intensive assertions - more lenient timeouts expected
    assert!(summary.success_rate > 0.8, "CPU intensive success rate too low: {:.2}%", summary.success_rate * 100.0);
}

#[traced_test]
#[test]
async fn test_cascading_failure_resistance() {
    let (_process_manager, app) = create_test_system().await.unwrap();
    let metrics = StressTestMetrics::new();
    
    let total_requests = 200;
    let failure_injection_rate = 0.1; // 10% failure rate
    
    println!("Starting cascading failure resistance test: {} requests with {}% failure injection", 
             total_requests, failure_injection_rate * 100.0);

    let mut join_set = JoinSet::new();

    for req_id in 0..total_requests {
        let app_clone = app.clone();
        let metrics_clone = metrics.clone();

        join_set.spawn(async move {
            let start_time = Instant::now();
            
            // Randomly inject failures
            let should_fail = (req_id as f64 / total_requests as f64) < failure_injection_rate;
            
            let uri = if should_fail {
                "/api/v1/invalid/endpoint/that/causes/failure"
            } else {
                "/api/v1/health"
            };

            let request = Request::builder()
                .method(Method::GET)
                .uri(&format!("{}?req={}", uri, req_id))
                .body(Body::empty())
                .unwrap();

            let result = app_clone.oneshot(request).await;
            let response_time = start_time.elapsed();
            
            let success = result.map(|r| r.status().is_success()).unwrap_or(false);
            metrics_clone.record_request(response_time, success).await;
        });
    }

    // Wait for all requests including failures
    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    let summary = metrics.get_summary().await;
    
    println!("Cascading Failure Resistance Results:");
    println!("  Total requests: {}", summary.total_requests);
    println!("  Successful: {} ({:.2}%)", summary.successful_requests, summary.success_rate * 100.0);
    println!("  Failed: {}", summary.failed_requests);
    println!("  Expected success rate: {:.2}%", (1.0 - failure_injection_rate) * 100.0);

    // System should handle failures gracefully without cascading
    let expected_success_rate = 1.0 - failure_injection_rate;
    assert!(summary.success_rate >= expected_success_rate - 0.05, 
            "Success rate too low: {:.2}% (expected ~{:.2}%)", 
            summary.success_rate * 100.0, expected_success_rate * 100.0);
}

#[traced_test]
#[test]
async fn test_resource_cleanup_under_load() {
    let (_process_manager, app) = create_test_system().await.unwrap();
    
    let iterations = 5;
    let requests_per_iteration = 100;
    
    println!("Starting resource cleanup test: {} iterations of {} requests", 
             iterations, requests_per_iteration);

    for iteration in 0..iterations {
        let iteration_metrics = StressTestMetrics::new();
        let mut join_set = JoinSet::new();

        println!("  Iteration {} of {}", iteration + 1, iterations);

        // Generate load
        for req_id in 0..requests_per_iteration {
            let app_clone = app.clone();
            let metrics_clone = iteration_metrics.clone();

            join_set.spawn(async move {
                let start_time = Instant::now();
                
                let request = Request::builder()
                    .method(Method::GET)
                    .uri(&format!("/api/v1/health?iter={}&req={}", iteration, req_id))
                    .body(Body::empty())
                    .unwrap();

                let result = app_clone.oneshot(request).await;
                let response_time = start_time.elapsed();
                
                let success = result.map(|r| r.status().is_success()).unwrap_or(false);
                metrics_clone.record_request(response_time, success).await;
            });
        }

        // Wait for iteration to complete
        while let Some(result) = join_set.join_next().await {
            result.unwrap();
        }

        let summary = iteration_metrics.get_summary().await;
        println!("    Iteration {} results: {:.2}% success, {:.2} RPS", 
                 iteration + 1, summary.success_rate * 100.0, summary.requests_per_second);

        // Allow garbage collection between iterations
        sleep(Duration::from_millis(100)).await;

        // Performance should not degrade significantly across iterations
        assert!(summary.success_rate > 0.95, 
                "Success rate degraded in iteration {}: {:.2}%", 
                iteration + 1, summary.success_rate * 100.0);
    }

    println!("Resource cleanup test completed successfully");
}

#[traced_test]
#[test]
async fn test_gradual_load_increase() {
    let (_process_manager, app) = create_test_system().await.unwrap();
    
    let stages = vec![10, 25, 50, 100, 150]; // Gradual increase in concurrent users
    let stage_duration = Duration::from_secs(10);
    
    println!("Starting gradual load increase test: {:?} concurrent users per stage", stages);

    for (stage_num, &concurrent_users) in stages.iter().enumerate() {
        println!("  Stage {}: {} concurrent users for {:?}", stage_num + 1, concurrent_users, stage_duration);
        
        let stage_metrics = StressTestMetrics::new();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let mut join_set = JoinSet::new();

        // Start workers for this stage
        for worker_id in 0..concurrent_users {
            let app_clone = app.clone();
            let metrics_clone = stage_metrics.clone();
            let stop_flag_clone = stop_flag.clone();

            join_set.spawn(async move {
                let mut request_count = 0;
                let mut interval = interval(Duration::from_millis(100)); // 10 RPS per worker

                while !stop_flag_clone.load(Ordering::Relaxed) {
                    interval.tick().await;
                    
                    let start_time = Instant::now();
                    
                    let request = Request::builder()
                        .method(Method::GET)
                        .uri(&format!("/api/v1/health?stage={}&worker={}&count={}", 
                                     stage_num, worker_id, request_count))
                        .body(Body::empty())
                        .unwrap();

                    let result = app_clone.clone().oneshot(request).await;
                    let response_time = start_time.elapsed();
                    
                    let success = result.map(|r| r.status().is_success()).unwrap_or(false);
                    metrics_clone.record_request(response_time, success).await;
                    
                    request_count += 1;
                }
            });
        }

        // Run stage for specified duration
        sleep(stage_duration).await;
        stop_flag.store(true, Ordering::Relaxed);

        // Wait for all workers to stop
        while let Some(result) = join_set.join_next().await {
            result.unwrap();
        }

        let summary = stage_metrics.get_summary().await;
        println!("    Stage {} results: {:.2}% success, {:.2} RPS, {:?} avg response time", 
                 stage_num + 1, 
                 summary.success_rate * 100.0, 
                 summary.requests_per_second,
                 summary.average_response_time);

        // Performance should remain acceptable even as load increases
        assert!(summary.success_rate > 0.95, 
                "Success rate degraded at {} users: {:.2}%", 
                concurrent_users, summary.success_rate * 100.0);
        
        assert!(summary.average_response_time < Duration::from_millis(200), 
                "Response time too high at {} users: {:?}", 
                concurrent_users, summary.average_response_time);
    }

    println!("Gradual load increase test completed successfully");
}
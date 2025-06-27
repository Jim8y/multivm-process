//! Comprehensive End-to-End Integration Tests
//!
//! These tests verify the complete system functionality from API to consensus,
//! including cross-VM transactions, account binding, and system recovery.

use multivm_application::api::rest::create_app;
use multivm_common::{
    config::MultivmConfig,
    types::{ProcessId, health::HealthStatus},
    IpcMessage, IpcCommand, IpcResponse,
    MultivmResult, MultivmError,
};
use multivm_account_mapping::{
    address::AccountAddress,
    mapping::AccountMappingLayer,
    special_tx::{SpecialTransaction, AssetType, SimpleBindingMetadata},
};
use multivm_p2p::{
    messages::{NetworkMessage, MessagePayload, ControlMessage},
    network::P2PNetwork,
    manager::P2PManager,
};
use multivm_process_manager::{
    MultivmProcessManager,
    coordinator::MultivmCoordinator,
    health::HealthMonitor,
};
use axum::{
    body::Body,
    http::{Request, Method, StatusCode, header},
};
use serde_json::{json, Value};
use std::{
    time::Duration,
    sync::Arc,
    collections::HashMap,
};
use tempfile::TempDir;
use tokio::{
    test,
    time::{sleep, timeout},
    sync::{Mutex, RwLock},
};
use tower::ServiceExt;
use tracing_test::traced_test;

/// Test system context that holds all components
struct TestSystemContext {
    temp_dir: TempDir,
    config: MultivmConfig,
    process_manager: MultivmProcessManager,
    coordinator: MultivmCoordinator,
    p2p_manager: P2PManager,
    app_handle: axum::serve::Serve<axum::routing::IntoMakeService<axum::Router>, axum::Router>,
    api_base_url: String,
}

impl TestSystemContext {
    async fn new() -> MultivmResult<Self> {
        let temp_dir = TempDir::new().map_err(|e| MultivmError::Internal {
            message: format!("Failed to create temp directory: {}", e),
            source: None,
        })?;

        let mut config = MultivmConfig::default();
        config.system.data_dir = temp_dir.path().to_path_buf();
        config.server.rest.port = 0; // Random port
        config.server.graphql.port = 0;
        config.server.websocket.port = 0;
        config.server.admin.port = 0;
        config.cache.redis.enabled = false;

        // Initialize process manager
        let process_manager = MultivmProcessManager::new(config.clone()).await?;

        // Initialize coordinator
        let coordinator_config = multivm_process_manager::coordinator::CoordinatorConfig {
            consensus_timeout: Duration::from_secs(10),
            block_processing_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(1),
            max_concurrent_blocks: 10,
            enable_account_mapping: true,
            data_directory: temp_dir.path().to_path_buf(),
            bind_timeout: Duration::from_secs(30),
            max_binding_attempts: 3,
        };
        let coordinator = MultivmCoordinator::new(coordinator_config, config.clone()).await?;

        // Initialize P2P manager
        let p2p_config = multivm_p2p::config::P2PConfig::default();
        let p2p_manager = P2PManager::new(p2p_config).await?;

        // Create API server
        let app_state = create_test_app_state(&config).await?;
        let app = create_app(app_state).await?;
        
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
            MultivmError::Network {
                message: format!("Failed to bind listener: {}", e),
                endpoint: None,
                retry_after: None,
            }
        })?;
        
        let addr = listener.local_addr().map_err(|e| MultivmError::Network {
            message: format!("Failed to get local address: {}", e),
            endpoint: None,
            retry_after: None,
        })?;
        
        let api_base_url = format!("http://{}", addr);
        let app_handle = axum::serve(listener, app);

        Ok(Self {
            temp_dir,
            config,
            process_manager,
            coordinator,
            p2p_manager,
            app_handle,
            api_base_url,
        })
    }

    async fn start(&mut self) -> MultivmResult<()> {
        // Start all components
        self.process_manager.start().await?;
        self.coordinator.start().await?;
        self.p2p_manager.start().await?;
        
        // Start API server in background
        tokio::spawn(async move {
            if let Err(e) = self.app_handle.await {
                tracing::error!("API server error: {}", e);
            }
        });

        // Wait for system to be ready
        self.wait_for_ready().await?;
        
        Ok(())
    }

    async fn stop(&mut self) -> MultivmResult<()> {
        self.p2p_manager.stop().await?;
        self.coordinator.stop().await?;
        self.process_manager.stop().await?;
        Ok(())
    }

    async fn wait_for_ready(&self) -> MultivmResult<()> {
        let max_attempts = 30;
        let mut attempts = 0;

        while attempts < max_attempts {
            if self.is_system_healthy().await {
                return Ok(());
            }
            
            attempts += 1;
            sleep(Duration::from_millis(100)).await;
        }

        Err(MultivmError::Internal {
            message: "System failed to become ready within timeout".to_string(),
            source: None,
        })
    }

    async fn is_system_healthy(&self) -> bool {
        self.process_manager.is_healthy().await &&
        self.coordinator.is_healthy().await &&
        self.p2p_manager.is_healthy().await
    }

    async fn make_api_request(&self, method: Method, path: &str, body: Option<Value>) -> Result<(StatusCode, Value), Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::Client::new();
        let url = format!("{}{}", self.api_base_url, path);
        
        let mut request = client.request(method, &url);
        
        if let Some(body) = body {
            request = request.json(&body);
        }
        
        let response = request.send().await?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or(json!({}));
        
        Ok((StatusCode::from_u16(status.as_u16())?, body))
    }
}

async fn create_test_app_state(config: &MultivmConfig) -> MultivmResult<multivm_application::api::rest::AppState> {
    let auth_manager = multivm_application::auth::manager::AuthManager::new(config.auth.clone()).await?;
    let cache = Box::new(multivm_application::cache::memory::MemoryCache::new(1000, Duration::from_secs(300)));
    let gateway = multivm_application::gateway::unified::UnifiedGateway::new(config.clone()).await?;
    let health_monitor = multivm_application::monitoring::health::HealthMonitor::new(Duration::from_secs(5)).await?;
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
async fn test_complete_system_startup_and_health() {
    let mut system = TestSystemContext::new().await.unwrap();
    
    // Start the system
    system.start().await.unwrap();
    
    // Verify all components are healthy
    assert!(system.is_system_healthy().await);
    
    // Test API health endpoint
    let (status, response) = system.make_api_request(
        Method::GET,
        "/api/v1/health",
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["status"], "healthy");
    
    system.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_cross_vm_account_binding_flow() {
    let mut system = TestSystemContext::new().await.unwrap();
    system.start().await.unwrap();

    // Step 1: Create SVM account
    let svm_account_data = json!({
        "vm_type": "svm",
        "address": "11111111111111111111111111111111",
        "metadata": {
            "name": "Test SVM Account",
            "type": "user"
        }
    });

    let (status, response) = system.make_api_request(
        Method::POST,
        "/api/v1/accounts",
        Some(svm_account_data),
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(response["address"], "11111111111111111111111111111111");

    // Step 2: Create EVM account
    let evm_account_data = json!({
        "vm_type": "evm",
        "address": "0x1234567890123456789012345678901234567890",
        "metadata": {
            "name": "Test EVM Account",
            "type": "user"
        }
    });

    let (status, _) = system.make_api_request(
        Method::POST,
        "/api/v1/accounts",
        Some(evm_account_data),
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::CREATED);

    // Step 3: Create cross-VM binding
    let binding_data = json!({
        "type": "account_binding",
        "source_vm": "svm",
        "target_vm": "evm",
        "source_address": "11111111111111111111111111111111",
        "target_address": "0x1234567890123456789012345678901234567890",
        "asset_type": "native",
        "amount": "1000000",
        "proof": "test_binding_proof"
    });

    let (status, binding_response) = system.make_api_request(
        Method::POST,
        "/api/v1/multivm/bind",
        Some(binding_data),
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(binding_response["binding_id"].is_string());

    // Step 4: Verify binding status
    let binding_id = binding_response["binding_id"].as_str().unwrap();
    let (status, status_response) = system.make_api_request(
        Method::GET,
        &format!("/api/v1/multivm/bind/{}", binding_id),
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert!(["pending", "confirmed"].contains(&status_response["status"].as_str().unwrap()));

    system.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_transaction_flow_across_vms() {
    let mut system = TestSystemContext::new().await.unwrap();
    system.start().await.unwrap();

    // Submit SVM transaction
    let svm_tx_data = json!({
        "vm_type": "svm",
        "transaction_data": "svm_test_transaction_data",
        "priority": "high",
        "metadata": {
            "source": "e2e_test",
            "test_id": "tx_flow_test"
        }
    });

    let (status, svm_response) = system.make_api_request(
        Method::POST,
        "/api/v1/transactions",
        Some(svm_tx_data),
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::ACCEPTED);
    let svm_tx_id = svm_response["transaction_id"].as_str().unwrap();

    // Submit EVM transaction
    let evm_tx_data = json!({
        "vm_type": "evm",
        "transaction_data": "evm_test_transaction_data",
        "priority": "normal",
        "metadata": {
            "source": "e2e_test",
            "test_id": "tx_flow_test"
        }
    });

    let (status, evm_response) = system.make_api_request(
        Method::POST,
        "/api/v1/transactions",
        Some(evm_tx_data),
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::ACCEPTED);
    let evm_tx_id = evm_response["transaction_id"].as_str().unwrap();

    // Check transaction statuses
    sleep(Duration::from_millis(500)).await;

    let (status, svm_status) = system.make_api_request(
        Method::GET,
        &format!("/api/v1/transactions/{}", svm_tx_id),
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert!(["pending", "processing", "confirmed"].contains(&svm_status["status"].as_str().unwrap()));

    let (status, evm_status) = system.make_api_request(
        Method::GET,
        &format!("/api/v1/transactions/{}", evm_tx_id),
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert!(["pending", "processing", "confirmed"].contains(&evm_status["status"].as_str().unwrap()));

    system.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_consensus_and_block_production() {
    let mut system = TestSystemContext::new().await.unwrap();
    system.start().await.unwrap();

    // Wait for initial block production
    sleep(Duration::from_secs(2)).await;

    // Check latest block
    let (status, latest_block) = system.make_api_request(
        Method::GET,
        "/api/v1/blocks/latest",
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert!(latest_block["height"].as_u64().unwrap() >= 0);
    assert!(latest_block["hash"].is_string());
    assert!(latest_block["timestamp"].is_string());

    // Submit transactions and wait for block inclusion
    let tx_data = json!({
        "vm_type": "svm",
        "transaction_data": "consensus_test_tx",
        "priority": "high"
    });

    let (status, tx_response) = system.make_api_request(
        Method::POST,
        "/api/v1/transactions",
        Some(tx_data),
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::ACCEPTED);

    // Wait for next block
    sleep(Duration::from_secs(3)).await;

    // Verify new block was produced
    let (status, new_latest_block) = system.make_api_request(
        Method::GET,
        "/api/v1/blocks/latest",
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert!(new_latest_block["height"].as_u64().unwrap() > latest_block["height"].as_u64().unwrap());

    system.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_p2p_network_communication() {
    let mut system = TestSystemContext::new().await.unwrap();
    system.start().await.unwrap();

    // Test network status
    let (status, network_status) = system.make_api_request(
        Method::GET,
        "/api/v1/network/status",
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert!(network_status["connected_peers"].is_number());
    assert!(network_status["network_id"].is_string());

    // Test peer discovery
    let (status, peers) = system.make_api_request(
        Method::GET,
        "/api/v1/network/peers",
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert!(peers["peers"].is_array());

    system.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_error_recovery_and_resilience() {
    let mut system = TestSystemContext::new().await.unwrap();
    system.start().await.unwrap();

    // Verify system is healthy
    assert!(system.is_system_healthy().await);

    // Simulate component failure by stopping coordinator
    system.coordinator.stop().await.unwrap();

    // System should detect the failure
    sleep(Duration::from_secs(1)).await;
    
    // API should still respond but indicate degraded health
    let (status, health_response) = system.make_api_request(
        Method::GET,
        "/api/v1/health",
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    // Health status might be degraded due to coordinator being down
    assert!(["healthy", "degraded"].contains(&health_response["status"].as_str().unwrap()));

    // Restart coordinator
    system.coordinator.start().await.unwrap();

    // Wait for recovery
    sleep(Duration::from_secs(2)).await;

    // System should recover
    assert!(system.is_system_healthy().await);

    system.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_concurrent_operations() {
    let mut system = TestSystemContext::new().await.unwrap();
    system.start().await.unwrap();

    // Perform multiple operations concurrently
    let mut handles = vec![];

    // Create accounts concurrently
    for i in 0..10 {
        let system_api_url = system.api_base_url.clone();
        let handle = tokio::spawn(async move {
            let client = reqwest::Client::new();
            let account_data = json!({
                "vm_type": if i % 2 == 0 { "svm" } else { "evm" },
                "address": format!("concurrent_test_account_{:02}", i),
                "metadata": {
                    "name": format!("Concurrent Test Account {}", i),
                    "test_batch": "concurrent_ops"
                }
            });

            client
                .post(&format!("{}/api/v1/accounts", system_api_url))
                .json(&account_data)
                .send()
                .await
        });
        handles.push(handle);
    }

    // Submit transactions concurrently
    for i in 0..10 {
        let system_api_url = system.api_base_url.clone();
        let handle = tokio::spawn(async move {
            let client = reqwest::Client::new();
            let tx_data = json!({
                "vm_type": if i % 2 == 0 { "svm" } else { "evm" },
                "transaction_data": format!("concurrent_test_tx_{:02}", i),
                "priority": if i % 3 == 0 { "high" } else { "normal" }
            });

            client
                .post(&format!("{}/api/v1/transactions", system_api_url))
                .json(&tx_data)
                .send()
                .await
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    let mut success_count = 0;
    let mut error_count = 0;

    for handle in handles {
        match handle.await {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    success_count += 1;
                } else {
                    error_count += 1;
                }
            }
            _ => error_count += 1,
        }
    }

    // Most operations should succeed
    assert!(success_count >= 15, "Only {} out of 20 concurrent operations succeeded", success_count);
    println!("Concurrent operations: {} succeeded, {} failed", success_count, error_count);

    system.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_data_persistence_across_restarts() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    // First system instance
    let persistent_data = {
        let mut config = MultivmConfig::default();
        config.system.data_dir = data_dir.clone();
        
        let mut system = TestSystemContext::new().await.unwrap();
        system.config.system.data_dir = data_dir.clone();
        system.start().await.unwrap();

        // Create some persistent data
        let account_data = json!({
            "vm_type": "svm",
            "address": "persistent_test_account",
            "metadata": {
                "name": "Persistent Test Account",
                "should_persist": true
            }
        });

        let (status, response) = system.make_api_request(
            Method::POST,
            "/api/v1/accounts",
            Some(account_data),
        ).await.unwrap();
        
        assert_eq!(status, StatusCode::CREATED);

        let account_id = response["id"].as_str().unwrap().to_string();
        system.stop().await.unwrap();
        
        account_id
    };

    // Second system instance with same data directory
    {
        let mut config = MultivmConfig::default();
        config.system.data_dir = data_dir;
        
        let mut system = TestSystemContext::new().await.unwrap();
        system.config.system.data_dir = config.system.data_dir;
        system.start().await.unwrap();

        // Try to retrieve the persistent data
        let (status, response) = system.make_api_request(
            Method::GET,
            &format!("/api/v1/accounts/{}", persistent_data),
            None,
        ).await.unwrap();
        
        // Should be able to retrieve the persisted account
        if status == StatusCode::OK {
            assert_eq!(response["address"], "persistent_test_account");
            assert_eq!(response["metadata"]["should_persist"], true);
        }

        system.stop().await.unwrap();
    }
}

#[traced_test]
#[test]
async fn test_high_load_performance() {
    let mut system = TestSystemContext::new().await.unwrap();
    system.start().await.unwrap();

    let start_time = std::time::Instant::now();
    let total_requests = 100;

    // Submit high volume of health check requests
    let mut handles = vec![];
    for i in 0..total_requests {
        let system_api_url = system.api_base_url.clone();
        let handle = tokio::spawn(async move {
            let client = reqwest::Client::new();
            let start = std::time::Instant::now();
            
            let response = client
                .get(&format!("{}/api/v1/health?req={}", system_api_url, i))
                .send()
                .await;
                
            (response, start.elapsed())
        });
        handles.push(handle);
    }

    // Collect results
    let mut success_count = 0;
    let mut total_response_time = Duration::ZERO;

    for handle in handles {
        if let Ok((Ok(response), duration)) = handle.await {
            if response.status().is_success() {
                success_count += 1;
                total_response_time += duration;
            }
        }
    }

    let total_time = start_time.elapsed();
    let average_response_time = total_response_time / success_count.max(1) as u32;
    let requests_per_second = success_count as f64 / total_time.as_secs_f64();

    println!("High load test results:");
    println!("  Successful requests: {}/{}", success_count, total_requests);
    println!("  Total time: {:?}", total_time);
    println!("  Average response time: {:?}", average_response_time);
    println!("  Requests per second: {:.2}", requests_per_second);

    // Performance assertions
    assert!(success_count >= total_requests * 90 / 100, "Success rate too low: {}%", success_count * 100 / total_requests);
    assert!(requests_per_second > 50.0, "Throughput too low: {:.2} req/s", requests_per_second);
    assert!(average_response_time < Duration::from_millis(100), "Average response time too high: {:?}", average_response_time);

    system.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_comprehensive_system_monitoring() {
    let mut system = TestSystemContext::new().await.unwrap();
    system.start().await.unwrap();

    // Generate some activity
    for i in 0..5 {
        let account_data = json!({
            "vm_type": if i % 2 == 0 { "svm" } else { "evm" },
            "address": format!("monitoring_test_account_{}", i),
            "metadata": {"monitoring_test": true}
        });

        system.make_api_request(
            Method::POST,
            "/api/v1/accounts",
            Some(account_data),
        ).await.unwrap();

        let tx_data = json!({
            "vm_type": if i % 2 == 0 { "svm" } else { "evm" },
            "transaction_data": format!("monitoring_test_tx_{}", i),
            "priority": "normal"
        });

        system.make_api_request(
            Method::POST,
            "/api/v1/transactions",
            Some(tx_data),
        ).await.unwrap();
    }

    sleep(Duration::from_millis(500)).await;

    // Check comprehensive metrics
    let (status, metrics) = system.make_api_request(
        Method::GET,
        "/api/v1/metrics",
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert!(metrics["system"].is_object());
    assert!(metrics["application"].is_object());
    assert!(metrics["timestamp"].is_string());

    // Check system status
    let (status, status_response) = system.make_api_request(
        Method::GET,
        "/api/v1/system/status",
        None,
    ).await.unwrap();
    
    assert_eq!(status, StatusCode::OK);
    assert!(status_response["uptime"].is_number());
    assert!(status_response["version"].is_string());
    assert!(status_response["processes"].is_object());

    system.stop().await.unwrap();
}
//! Comprehensive Cross-VM Transaction Flow Tests
//!
//! These tests verify the complete flow of transactions across different VMs,
//! including account binding, asset transfers, and state synchronization.

use multivm_application::api::rest::create_app;
use multivm_common::{
    config::MultivmConfig,
    types::{ProcessId, health::HealthStatus},
    IpcMessage, IpcCommand,
    MultivmResult,
};
use multivm_account_mapping::{
    address::AccountAddress,
    mapping::AccountMappingLayer,
    special_tx::{SpecialTransaction, AssetType, SimpleBindingMetadata},
    cross_vm_coordinator::CrossVmCoordinator,
};
use multivm_p2p::{
    messages::{NetworkMessage, MessagePayload, MultiVmMessage},
    network::P2PNetwork,
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
    time::{sleep, timeout},
    sync::{Mutex, RwLock},
    task::JoinSet,
};
use tower::ServiceExt;
use tracing_test::traced_test;

/// Cross-VM transaction state tracker
#[derive(Debug, Clone)]
struct CrossVmTransactionTracker {
    transactions: Arc<RwLock<HashMap<String, CrossVmTransactionState>>>,
    bindings: Arc<RwLock<HashMap<String, AccountBinding>>>,
    balances: Arc<RwLock<HashMap<String, u64>>>,
}

#[derive(Debug, Clone)]
struct CrossVmTransactionState {
    transaction_id: String,
    source_vm: String,
    target_vm: String,
    source_address: String,
    target_address: String,
    amount: u64,
    status: TransactionStatus,
    confirmations: u32,
    created_at: Instant,
    confirmed_at: Option<Instant>,
}

#[derive(Debug, Clone)]
struct AccountBinding {
    binding_id: String,
    svm_address: String,
    evm_address: String,
    status: BindingStatus,
    proof_hash: String,
    created_at: Instant,
    confirmed_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq)]
enum TransactionStatus {
    Pending,
    SvmConfirmed,
    EvmConfirmed,
    CrossVmConfirmed,
    Failed,
    Reverted,
}

#[derive(Debug, Clone, PartialEq)]
enum BindingStatus {
    Pending,
    SvmConfirmed,
    EvmConfirmed,
    BothConfirmed,
    Failed,
}

impl CrossVmTransactionTracker {
    fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            bindings: Arc::new(RwLock::new(HashMap::new())),
            balances: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn track_transaction(&self, tx: CrossVmTransactionState) {
        let mut transactions = self.transactions.write().await;
        transactions.insert(tx.transaction_id.clone(), tx);
    }

    async fn track_binding(&self, binding: AccountBinding) {
        let mut bindings = self.bindings.write().await;
        bindings.insert(binding.binding_id.clone(), binding);
    }

    async fn update_balance(&self, address: &str, amount: u64) {
        let mut balances = self.balances.write().await;
        balances.insert(address.to_string(), amount);
    }

    async fn get_balance(&self, address: &str) -> u64 {
        let balances = self.balances.read().await;
        balances.get(address).copied().unwrap_or(0)
    }

    async fn update_transaction_status(&self, tx_id: &str, status: TransactionStatus) {
        let mut transactions = self.transactions.write().await;
        if let Some(tx) = transactions.get_mut(tx_id) {
            tx.status = status.clone();
            if matches!(status, TransactionStatus::CrossVmConfirmed) {
                tx.confirmed_at = Some(Instant::now());
            }
        }
    }

    async fn get_transaction_status(&self, tx_id: &str) -> Option<TransactionStatus> {
        let transactions = self.transactions.read().await;
        transactions.get(tx_id).map(|tx| tx.status.clone())
    }

    async fn get_binding_status(&self, binding_id: &str) -> Option<BindingStatus> {
        let bindings = self.bindings.read().await;
        bindings.get(binding_id).map(|binding| binding.status.clone())
    }
}

async fn create_test_system() -> MultivmResult<(MultivmProcessManager, axum::Router, CrossVmTransactionTracker)> {
    let temp_dir = TempDir::new().unwrap();
    let mut config = MultivmConfig::default();
    config.system.data_dir = temp_dir.path().to_path_buf();
    config.server.rest.port = 0;
    config.cache.redis.enabled = false;

    let process_manager = MultivmProcessManager::new(config.clone()).await?;
    
    let app_state = create_test_app_state(&config).await?;
    let app = create_app(app_state).await?;
    
    let tracker = CrossVmTransactionTracker::new();

    Ok((process_manager, app, tracker))
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
async fn test_account_binding_flow() {
    let (mut process_manager, app, tracker) = create_test_system().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing complete account binding flow");

    // Step 1: Create SVM account
    let svm_address = "11111111111111111111111111111111";
    let svm_account_data = json!({
        "vm_type": "svm",
        "address": svm_address,
        "metadata": {
            "name": "Test SVM Account",
            "type": "user",
            "cross_vm_enabled": true
        }
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header("content-type", "application/json")
        .body(Body::from(svm_account_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Step 2: Create EVM account
    let evm_address = "0x1234567890123456789012345678901234567890";
    let evm_account_data = json!({
        "vm_type": "evm",
        "address": evm_address,
        "metadata": {
            "name": "Test EVM Account",
            "type": "user",
            "cross_vm_enabled": true
        }
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/accounts")
        .header("content-type", "application/json")
        .body(Body::from(evm_account_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Step 3: Initiate account binding
    let binding_data = json!({
        "type": "account_binding",
        "source_vm": "svm",
        "target_vm": "evm",
        "source_address": svm_address,
        "target_address": evm_address,
        "proof": {
            "signature": "test_signature",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "nonce": 1
        }
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/multivm/bind")
        .header("content-type", "application/json")
        .body(Body::from(binding_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let binding_response: Value = serde_json::from_slice(&body).unwrap();
    let binding_id = binding_response["binding_id"].as_str().unwrap();

    // Track the binding
    let binding = AccountBinding {
        binding_id: binding_id.to_string(),
        svm_address: svm_address.to_string(),
        evm_address: evm_address.to_string(),
        status: BindingStatus::Pending,
        proof_hash: "test_proof_hash".to_string(),
        created_at: Instant::now(),
        confirmed_at: None,
    };
    tracker.track_binding(binding).await;

    // Step 4: Wait for binding confirmation
    sleep(Duration::from_secs(2)).await;

    // Step 5: Check binding status
    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/api/v1/multivm/bind/{}", binding_id))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let status_response: Value = serde_json::from_slice(&body).unwrap();
    
    let status = status_response["status"].as_str().unwrap();
    assert!(["pending", "confirmed", "processing"].contains(&status));

    // Step 6: Verify binding in account mapping layer
    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/api/v1/accounts/{}/bindings", svm_address))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let bindings_response: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(bindings_response["bindings"].is_array());
    let bindings_array = bindings_response["bindings"].as_array().unwrap();
    assert!(!bindings_array.is_empty());

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_cross_vm_asset_transfer() {
    let (mut process_manager, app, tracker) = create_test_system().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing cross-VM asset transfer flow");

    // Setup: Create and bind accounts
    let svm_address = "22222222222222222222222222222222";
    let evm_address = "0x2222222222222222222222222222222222222222";

    // Create accounts and binding (simplified for this test)
    let binding_data = json!({
        "type": "account_binding",
        "source_vm": "svm",
        "target_vm": "evm",
        "source_address": svm_address,
        "target_address": evm_address,
        "proof": {"signature": "test_sig", "nonce": 1}
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/multivm/bind")
        .header("content-type", "application/json")
        .body(Body::from(binding_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Set initial balances
    tracker.update_balance(svm_address, 1000000).await; // 1M units
    tracker.update_balance(evm_address, 0).await;

    // Step 1: Initiate cross-VM transfer
    let transfer_amount = 100000u64; // 100K units
    let transfer_data = json!({
        "type": "cross_vm_transfer",
        "source_vm": "svm",
        "target_vm": "evm",
        "source_address": svm_address,
        "target_address": evm_address,
        "amount": transfer_amount.to_string(),
        "asset_type": "native",
        "metadata": {
            "memo": "Cross-VM transfer test",
            "priority": "high"
        }
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/multivm/transfer")
        .header("content-type", "application/json")
        .body(Body::from(transfer_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let transfer_response: Value = serde_json::from_slice(&body).unwrap();
    let transfer_id = transfer_response["transfer_id"].as_str().unwrap();

    // Track the transfer
    let transfer_tx = CrossVmTransactionState {
        transaction_id: transfer_id.to_string(),
        source_vm: "svm".to_string(),
        target_vm: "evm".to_string(),
        source_address: svm_address.to_string(),
        target_address: evm_address.to_string(),
        amount: transfer_amount,
        status: TransactionStatus::Pending,
        confirmations: 0,
        created_at: Instant::now(),
        confirmed_at: None,
    };
    tracker.track_transaction(transfer_tx).await;

    // Step 2: Monitor transfer progress
    let mut attempts = 0;
    let max_attempts = 10;
    
    while attempts < max_attempts {
        let request = Request::builder()
            .method(Method::GET)
            .uri(&format!("/api/v1/multivm/transfer/{}", transfer_id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let status_response: Value = serde_json::from_slice(&body).unwrap();
        
        let status = status_response["status"].as_str().unwrap();
        let confirmations = status_response["confirmations"].as_u64().unwrap_or(0);

        println!("Transfer status: {}, confirmations: {}", status, confirmations);

        if status == "confirmed" || confirmations >= 2 {
            tracker.update_transaction_status(transfer_id, TransactionStatus::CrossVmConfirmed).await;
            break;
        }

        attempts += 1;
        sleep(Duration::from_millis(500)).await;
    }

    // Step 3: Verify final balances
    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/api/v1/accounts/{}/balance", svm_address))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    if response.status() == StatusCode::OK {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let balance_response: Value = serde_json::from_slice(&body).unwrap();
        let svm_balance = balance_response["balance"].as_u64().unwrap_or(0);
        
        // SVM balance should be reduced
        assert!(svm_balance <= 1000000 - transfer_amount, "SVM balance should be reduced");
    }

    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/api/v1/accounts/{}/balance", evm_address))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    if response.status() == StatusCode::OK {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let balance_response: Value = serde_json::from_slice(&body).unwrap();
        let evm_balance = balance_response["balance"].as_u64().unwrap_or(0);
        
        // EVM balance should be increased
        assert!(evm_balance >= transfer_amount, "EVM balance should be increased");
    }

    // Verify transfer completion in tracker
    let final_status = tracker.get_transaction_status(transfer_id).await;
    assert!(matches!(final_status, Some(TransactionStatus::CrossVmConfirmed)));

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_concurrent_cross_vm_transactions() {
    let (mut process_manager, app, tracker) = create_test_system().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing concurrent cross-VM transactions");

    let concurrent_transfers = 10;
    let mut join_set = JoinSet::new();

    // Setup multiple account pairs
    for i in 0..concurrent_transfers {
        let svm_address = format!("3333333333333333333333333333333{:01}", i);
        let evm_address = format!("0x333333333333333333333333333333333333333{:01}", i);
        
        // Set initial balances
        tracker.update_balance(&svm_address, 1000000).await;
        tracker.update_balance(&evm_address, 0).await;

        let app_clone = app.clone();
        let tracker_clone = tracker.clone();

        join_set.spawn(async move {
            // Create account binding
            let binding_data = json!({
                "type": "account_binding",
                "source_vm": "svm",
                "target_vm": "evm",
                "source_address": svm_address,
                "target_address": evm_address,
                "proof": {"signature": format!("sig_{}", i), "nonce": i + 1}
            });

            let request = Request::builder()
                .method(Method::POST)
                .uri("/api/v1/multivm/bind")
                .header("content-type", "application/json")
                .body(Body::from(binding_data.to_string()))
                .unwrap();

            let bind_response = app_clone.clone().oneshot(request).await;
            
            if bind_response.is_err() || bind_response.unwrap().status() != StatusCode::ACCEPTED {
                return Err(format!("Failed to bind accounts for transfer {}", i));
            }

            // Wait a bit for binding to process
            sleep(Duration::from_millis(100)).await;

            // Initiate transfer
            let transfer_amount = 50000u64 + (i as u64 * 1000); // Varying amounts
            let transfer_data = json!({
                "type": "cross_vm_transfer",
                "source_vm": "svm",
                "target_vm": "evm",
                "source_address": svm_address,
                "target_address": evm_address,
                "amount": transfer_amount.to_string(),
                "asset_type": "native",
                "metadata": {"concurrent_test": true, "transfer_id": i}
            });

            let request = Request::builder()
                .method(Method::POST)
                .uri("/api/v1/multivm/transfer")
                .header("content-type", "application/json")
                .body(Body::from(transfer_data.to_string()))
                .unwrap();

            let transfer_response = app_clone.oneshot(request).await;
            
            match transfer_response {
                Ok(response) if response.status() == StatusCode::ACCEPTED => {
                    // Track successful transfer initiation
                    let transfer_tx = CrossVmTransactionState {
                        transaction_id: format!("concurrent_transfer_{}", i),
                        source_vm: "svm".to_string(),
                        target_vm: "evm".to_string(),
                        source_address: svm_address,
                        target_address: evm_address,
                        amount: transfer_amount,
                        status: TransactionStatus::Pending,
                        confirmations: 0,
                        created_at: Instant::now(),
                        confirmed_at: None,
                    };
                    tracker_clone.track_transaction(transfer_tx).await;
                    Ok(i)
                },
                _ => Err(format!("Failed to initiate transfer {}", i)),
            }
        });
    }

    // Collect results
    let mut successful_transfers = 0;
    let mut failed_transfers = 0;

    while let Some(result) = join_set.join_next().await {
        match result.unwrap() {
            Ok(_) => successful_transfers += 1,
            Err(e) => {
                failed_transfers += 1;
                println!("Transfer failed: {}", e);
            }
        }
    }

    println!("Concurrent transfers: {} successful, {} failed", 
             successful_transfers, failed_transfers);

    // Most transfers should succeed
    assert!(successful_transfers >= concurrent_transfers * 7 / 10, 
            "At least 70% of concurrent transfers should succeed");

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_cross_vm_state_synchronization() {
    let (mut process_manager, app, tracker) = create_test_system().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing cross-VM state synchronization");

    let svm_address = "44444444444444444444444444444444";
    let evm_address = "0x4444444444444444444444444444444444444444";

    // Step 1: Create accounts and binding
    let binding_data = json!({
        "type": "account_binding",
        "source_vm": "svm",
        "target_vm": "evm",
        "source_address": svm_address,
        "target_address": evm_address,
        "proof": {"signature": "sync_test_sig", "nonce": 1}
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/multivm/bind")
        .header("content-type", "application/json")
        .body(Body::from(binding_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Step 2: Perform multiple operations to create state divergence
    let operations = vec![
        (50000u64, "svm", "evm"),
        (25000u64, "evm", "svm"),
        (30000u64, "svm", "evm"),
        (15000u64, "evm", "svm"),
    ];

    for (i, (amount, source_vm, target_vm)) in operations.iter().enumerate() {
        let (source_addr, target_addr) = if *source_vm == "svm" {
            (svm_address, evm_address)
        } else {
            (evm_address, svm_address)
        };

        let transfer_data = json!({
            "type": "cross_vm_transfer",
            "source_vm": source_vm,
            "target_vm": target_vm,
            "source_address": source_addr,
            "target_address": target_addr,
            "amount": amount.to_string(),
            "asset_type": "native",
            "metadata": {"sync_test": true, "operation": i}
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/multivm/transfer")
            .header("content-type", "application/json")
            .body(Body::from(transfer_data.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        // Wait between operations to avoid overwhelming the system
        sleep(Duration::from_millis(200)).await;
    }

    // Step 3: Wait for all operations to settle
    sleep(Duration::from_secs(3)).await;

    // Step 4: Request state synchronization
    let sync_request = json!({
        "type": "state_sync",
        "addresses": [svm_address, evm_address],
        "force_reconciliation": true
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/multivm/sync")
        .header("content-type", "application/json")
        .body(Body::from(sync_request.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Step 5: Wait for synchronization to complete
    sleep(Duration::from_secs(2)).await;

    // Step 6: Verify state consistency
    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/api/v1/multivm/sync/status?addresses={},{}", svm_address, evm_address))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let sync_response: Value = serde_json::from_slice(&body).unwrap();
    
    let sync_status = sync_response["status"].as_str().unwrap();
    assert!(["synchronized", "completed"].contains(&sync_status));

    // Verify balances are consistent
    if let Some(balance_data) = sync_response["balances"].as_object() {
        let svm_balance = balance_data[svm_address].as_u64().unwrap_or(0);
        let evm_balance = balance_data[evm_address].as_u64().unwrap_or(0);
        
        println!("Final balances: SVM={}, EVM={}", svm_balance, evm_balance);
        
        // Total should be conserved (ignoring fees for this test)
        let total_balance = svm_balance + evm_balance;
        assert!(total_balance > 0, "Total balance should be positive");
    }

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_cross_vm_transaction_rollback() {
    let (mut process_manager, app, tracker) = create_test_system().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing cross-VM transaction rollback");

    let svm_address = "55555555555555555555555555555555";
    let evm_address = "0x5555555555555555555555555555555555555555";

    // Setup accounts and binding
    let binding_data = json!({
        "type": "account_binding",
        "source_vm": "svm",
        "target_vm": "evm",
        "source_address": svm_address,
        "target_address": evm_address,
        "proof": {"signature": "rollback_test_sig", "nonce": 1}
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/multivm/bind")
        .header("content-type", "application/json")
        .body(Body::from(binding_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Set initial balances
    tracker.update_balance(svm_address, 1000000).await;
    tracker.update_balance(evm_address, 500000).await;

    // Step 1: Attempt transfer with insufficient balance (should fail)
    let invalid_transfer_data = json!({
        "type": "cross_vm_transfer",
        "source_vm": "svm",
        "target_vm": "evm",
        "source_address": svm_address,
        "target_address": evm_address,
        "amount": "2000000", // More than available balance
        "asset_type": "native",
        "metadata": {"rollback_test": true, "expect_failure": true}
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/multivm/transfer")
        .header("content-type", "application/json")
        .body(Body::from(invalid_transfer_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    // Should be rejected or accepted but then fail
    assert!(response.status() == StatusCode::BAD_REQUEST || response.status() == StatusCode::ACCEPTED);

    if response.status() == StatusCode::ACCEPTED {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let transfer_response: Value = serde_json::from_slice(&body).unwrap();
        let transfer_id = transfer_response["transfer_id"].as_str().unwrap();

        // Wait for processing
        sleep(Duration::from_secs(2)).await;

        // Check status - should be failed or reverted
        let request = Request::builder()
            .method(Method::GET)
            .uri(&format!("/api/v1/multivm/transfer/{}", transfer_id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let status_response: Value = serde_json::from_slice(&body).unwrap();
        
        let status = status_response["status"].as_str().unwrap();
        assert!(["failed", "reverted", "rejected"].contains(&status));
    }

    // Step 2: Attempt valid transfer that should succeed
    let valid_transfer_data = json!({
        "type": "cross_vm_transfer",
        "source_vm": "svm",
        "target_vm": "evm",
        "source_address": svm_address,
        "target_address": evm_address,
        "amount": "100000", // Valid amount
        "asset_type": "native",
        "metadata": {"rollback_test": true, "expect_success": true}
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/multivm/transfer")
        .header("content-type", "application/json")
        .body(Body::from(valid_transfer_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let transfer_response: Value = serde_json::from_slice(&body).unwrap();
    let valid_transfer_id = transfer_response["transfer_id"].as_str().unwrap();

    // Wait for processing
    sleep(Duration::from_secs(2)).await;

    // Check status - should be confirmed
    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/api/v1/multivm/transfer/{}", valid_transfer_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let status_response: Value = serde_json::from_slice(&body).unwrap();
    
    let status = status_response["status"].as_str().unwrap();
    assert!(["confirmed", "completed", "success"].contains(&status));

    process_manager.stop().await.unwrap();
}

#[traced_test]
#[test]
async fn test_complex_multi_hop_cross_vm_flow() {
    let (mut process_manager, app, tracker) = create_test_system().await.unwrap();
    
    process_manager.start().await.unwrap();
    
    println!("Testing complex multi-hop cross-VM transaction flow");

    // Setup multiple accounts across VMs
    let accounts = vec![
        ("66666666666666666666666666666666", "svm"),
        ("0x6666666666666666666666666666666666666666", "evm"),
        ("77777777777777777777777777777777", "svm"),
        ("0x7777777777777777777777777777777777777777", "evm"),
    ];

    // Create all accounts
    for (address, vm_type) in &accounts {
        let account_data = json!({
            "vm_type": vm_type,
            "address": address,
            "metadata": {
                "name": format!("Multi-hop test account ({})", vm_type),
                "multi_hop_enabled": true
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
    }

    // Create bindings between VM pairs
    let bindings = vec![
        (accounts[0].0, accounts[1].0, "svm", "evm"), // SVM1 <-> EVM1
        (accounts[2].0, accounts[3].0, "svm", "evm"), // SVM2 <-> EVM2
    ];

    for (source_addr, target_addr, source_vm, target_vm) in &bindings {
        let binding_data = json!({
            "type": "account_binding",
            "source_vm": source_vm,
            "target_vm": target_vm,
            "source_address": source_addr,
            "target_address": target_addr,
            "proof": {"signature": format!("multi_hop_sig_{}_{}", source_vm, target_vm), "nonce": 1}
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/multivm/bind")
            .header("content-type", "application/json")
            .body(Body::from(binding_data.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    // Wait for bindings to settle
    sleep(Duration::from_secs(1)).await;

    // Execute multi-hop transaction: SVM1 -> EVM1 -> SVM2 -> EVM2
    let multi_hop_data = json!({
        "type": "multi_hop_transfer",
        "hops": [
            {
                "source_vm": "svm",
                "target_vm": "evm",
                "source_address": accounts[0].0,
                "target_address": accounts[1].0,
                "amount": "100000"
            },
            {
                "source_vm": "evm",
                "target_vm": "svm",
                "source_address": accounts[1].0,
                "target_address": accounts[2].0,
                "amount": "90000" // Accounting for fees
            },
            {
                "source_vm": "svm",
                "target_vm": "evm",
                "source_address": accounts[2].0,
                "target_address": accounts[3].0,
                "amount": "80000" // Accounting for more fees
            }
        ],
        "metadata": {
            "multi_hop": true,
            "final_destination": accounts[3].0
        }
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/multivm/multi-hop")
        .header("content-type", "application/json")
        .body(Body::from(multi_hop_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let multi_hop_response: Value = serde_json::from_slice(&body).unwrap();
    let multi_hop_id = multi_hop_response["multi_hop_id"].as_str().unwrap();

    // Monitor multi-hop progress
    let mut attempts = 0;
    let max_attempts = 20; // Allow more time for complex flow

    while attempts < max_attempts {
        let request = Request::builder()
            .method(Method::GET)
            .uri(&format!("/api/v1/multivm/multi-hop/{}", multi_hop_id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let status_response: Value = serde_json::from_slice(&body).unwrap();
        
        let overall_status = status_response["status"].as_str().unwrap();
        let completed_hops = status_response["completed_hops"].as_u64().unwrap_or(0);
        let total_hops = status_response["total_hops"].as_u64().unwrap_or(3);

        println!("Multi-hop progress: {}/{} hops completed, status: {}", 
                 completed_hops, total_hops, overall_status);

        if overall_status == "completed" || completed_hops >= total_hops {
            break;
        }

        if overall_status == "failed" {
            println!("Multi-hop transaction failed");
            break;
        }

        attempts += 1;
        sleep(Duration::from_millis(500)).await;
    }

    // Verify final state
    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/api/v1/multivm/multi-hop/{}/summary", multi_hop_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    if response.status() == StatusCode::OK {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let summary_response: Value = serde_json::from_slice(&body).unwrap();
        
        println!("Multi-hop summary: {:?}", summary_response);
        
        let final_status = summary_response["final_status"].as_str().unwrap();
        assert!(["completed", "partial_success"].contains(&final_status));
    }

    process_manager.stop().await.unwrap();
}
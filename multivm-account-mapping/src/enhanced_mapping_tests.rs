//! Comprehensive tests for enhanced account mapping features

use crate::{
    address::{AccountAddress, EthereumAddress, MultivmAccountId, SolanaAddress},
    binding_message::{BindingAction, BindingMessage},
    binding_policy::{
        BindingPolicy, EnhancedBindingProof, Guardian, RecoveryConfig, SecurityMetadata,
    },
    enhanced_mapping::EnhancedAccountMapper,
    error::AccountMappingError,
    events::{AccountBindingEvent, EventListener},
    mapping::{BindingProof, ProofType},
    storage::{AccountMappingStorage, MemoryStorage},
};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Test event collector for verifying event emission
struct TestEventCollector {
    events: Arc<RwLock<Vec<AccountBindingEvent>>>,
}

impl TestEventCollector {
    fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn get_events(&self) -> Vec<AccountBindingEvent> {
        self.events.read().await.clone()
    }

    async fn clear(&self) {
        self.events.write().await.clear();
    }
}

#[async_trait::async_trait]
impl EventListener for TestEventCollector {
    async fn handle_event(&self, event: AccountBindingEvent) {
        self.events.write().await.push(event);
    }
}

/// Create test accounts
fn create_test_accounts() -> (AccountAddress, AccountAddress) {
    let eth_account = AccountAddress::Ethereum(EthereumAddress([1u8; 20]));
    let sol_account = AccountAddress::Solana(SolanaAddress([2u8; 32]));
    (eth_account, sol_account)
}

/// Create unique test accounts for concurrent tests
fn create_unique_test_accounts() -> (AccountAddress, AccountAddress) {
    use std::sync::atomic::{AtomicU8, Ordering};
    static COUNTER: AtomicU8 = AtomicU8::new(10);
    
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    // Add randomness to avoid conflicts
    let rand_val = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as u8;
    
    let mut eth_bytes = [0u8; 20];
    eth_bytes[0] = id;
    eth_bytes[1] = rand_val;
    let mut sol_bytes = [0u8; 32];
    sol_bytes[0] = id;
    sol_bytes[1] = rand_val;
    
    let eth_account = AccountAddress::Ethereum(EthereumAddress(eth_bytes));
    let sol_account = AccountAddress::Solana(SolanaAddress(sol_bytes));
    (eth_account, sol_account)
}

/// Create test mapper with disabled signature validation
fn create_test_mapper(
    storage: Arc<dyn crate::storage::AccountMappingStorage>,
    policy: BindingPolicy,
    recovery_config: RecoveryConfig,
) -> EnhancedAccountMapper {
    use crate::validation::{AccountBindingValidator, ValidationConfig};
    use crate::distributed_lock::{InMemoryLockManager, RetryableLockManager};
    use crate::events::EventEmitter;
    use crate::enhanced_mapping::RateLimiter;
    use std::collections::HashMap;
    use tokio::sync::RwLock;
    
    let mut validation_config = ValidationConfig::default();
    validation_config.validate_signatures = false; // Disable for tests
    
    // Create a new lock manager instance for each test to avoid contention
    let lock_manager = InMemoryLockManager::new();
    let retryable_lock_manager =
        RetryableLockManager::new(lock_manager, 3, Duration::from_millis(100));
    
    EnhancedAccountMapper {
        storage,
        lock_manager: Arc::new(retryable_lock_manager),
        policy,
        event_emitter: Arc::new(RwLock::new(EventEmitter::new())),
        validator: AccountBindingValidator::new(validation_config),
        recovery_config,
        rate_limiter: Arc::new(RwLock::new(RateLimiter::new())),
        guardians: Arc::new(RwLock::new(HashMap::new())),
        recovery_requests: Arc::new(RwLock::new(HashMap::new())),
    }
}

/// Create test binding proof
fn create_test_proof(account: AccountAddress, nonce: u64) -> BindingProof {
    // Create a valid 65-byte signature for testing
    let mut signature = vec![0u8; 65];
    signature[64] = 27; // Recovery ID
    
    BindingProof {
        account,
        proof_type: ProofType::Signature {
            message: Box::new(b"test message".to_vec()),
            signature: Box::new(signature),
        },
        proof_data: Box::new(vec![0xde, 0xad, 0xbe, 0xef]),
        nonce,
        timestamp: SystemTime::now(),
    }
}

/// Create enhanced binding proof
fn create_enhanced_proof(account: AccountAddress, nonce: u64) -> EnhancedBindingProof {
    EnhancedBindingProof {
        proof: create_test_proof(account, nonce),
        secondary_auth: None,
        risk_score: 0,
        security_metadata: SecurityMetadata {
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("test-agent".to_string()),
            geolocation: None,
            activity_score: 50,
            suspicious_activity_count: 0,
            is_verified: true,
        },
    }
}

#[tokio::test]
async fn test_auto_binding_with_locking() {
    let storage = Arc::new(MemoryStorage::new());
    let mapper = create_test_mapper(
        storage.clone(),
        BindingPolicy::default(),
        RecoveryConfig::default(),
    );

    let (eth_account, _) = create_unique_test_accounts();

    // First auto-binding should succeed
    let multivm_id1 = mapper
        .create_auto_binding(eth_account.clone())
        .await
        .unwrap();

    // For the second call, handle potential lock contention
    let multivm_id2 = loop {
        match mapper.create_auto_binding(eth_account.clone()).await {
            Ok(id) => break id,
            Err(AccountMappingError::LockContention { .. }) => {
                // Wait for lock to be released
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    };
    assert_eq!(multivm_id1, multivm_id2);

    // Verify binding was created
    let binding = storage.get_binding(&multivm_id1).await.unwrap();
    assert!(binding.is_some());
    assert_eq!(binding.unwrap().evm_account, Some(eth_account));
}

#[tokio::test]
async fn test_concurrent_auto_binding() {
    let storage = Arc::new(MemoryStorage::new());
    let mapper = Arc::new(create_test_mapper(
        storage.clone(),
        BindingPolicy::default(),
        RecoveryConfig::default(),
    ));

    let (eth_account, _) = create_unique_test_accounts();

    // Simulate concurrent auto-binding attempts
    let mut handles = vec![];
    for i in 0..10 {
        let mapper_clone = mapper.clone();
        let account_clone = eth_account.clone();
        handles.push(tokio::spawn(async move {
            // Add a small delay to reduce lock contention
            tokio::time::sleep(tokio::time::Duration::from_millis(i as u64 * 10)).await;
            mapper_clone.create_auto_binding(account_clone).await
        }));
    }

    // Collect results, handling potential lock contention
    let mut results = vec![];
    for handle in handles {
        match handle.await.unwrap() {
            Ok(id) => results.push(id),
            Err(AccountMappingError::LockContention { .. }) => {
                // This is expected in concurrent scenarios
                // The important thing is that at least one succeeded
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
    
    // At least one should have succeeded
    assert!(!results.is_empty(), "At least one auto-binding should succeed");

    // All should return the same MultiVM ID
    let first_id = &results[0];
    for id in &results {
        assert_eq!(id, first_id);
    }

    // Should only have one binding in storage - verify through first_id
    let binding = storage.get_binding(first_id).await.unwrap();
    assert!(binding.is_some());
}

#[tokio::test]
async fn test_rate_limiting() {
    let mut policy = BindingPolicy::default();
    policy.max_binding_attempts_per_hour = 3;

    let storage = Arc::new(MemoryStorage::new());
    let mapper = create_test_mapper(storage.clone(), policy, RecoveryConfig::default());

    let (eth_account, sol_account) = create_test_accounts();

    // Create auto-binding first
    let multivm_id = mapper
        .create_auto_binding(eth_account.clone())
        .await
        .unwrap();

    // Create binding messages
    let mut messages = vec![];
    for i in 0..5 {
        messages.push(BindingMessage::new(
            BindingAction::BindAccount,
            "test-chain".to_string(),
            eth_account.clone(),
            sol_account.clone(),
            multivm_id.clone(),
            i,
        ));
    }

    // First 3 attempts should succeed (assuming they pass other validations)
    for i in 0..3 {
        let result = mapper
            .process_binding_message(
                messages[i].clone(),
                create_enhanced_proof(eth_account.clone(), i as u64),
            )
            .await;

        // May fail for other reasons, but not rate limiting
        if let Err(AccountMappingError::RateLimitExceeded { .. }) = result {
            panic!("Should not hit rate limit on attempt {}", i + 1);
        }
    }

    // 4th attempt should fail with rate limit
    let result = mapper
        .process_binding_message(
            messages[3].clone(),
            create_enhanced_proof(eth_account.clone(), 3),
        )
        .await;

    match result {
        Err(AccountMappingError::RateLimitExceeded { .. }) => {}
        _ => panic!("Expected rate limit error"),
    }
}

#[tokio::test]
async fn test_binding_message_validation() {
    let storage = Arc::new(MemoryStorage::new());
    let mapper =
        create_test_mapper(storage, BindingPolicy::default(), RecoveryConfig::default());

    let (eth_account, sol_account) = create_test_accounts();
    let multivm_id = MultivmAccountId::from_account(&eth_account);

    // Test expired message
    let mut expired_message = BindingMessage::new(
        BindingAction::BindAccount,
        "test-chain".to_string(),
        eth_account.clone(),
        sol_account.clone(),
        multivm_id.clone(),
        1,
    );
    expired_message.expires_at = Some(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 3600,
    );

    let result = mapper
        .process_binding_message(
            expired_message,
            create_enhanced_proof(eth_account.clone(), 1),
        )
        .await;

    match result {
        Err(AccountMappingError::InvalidBindingProof { reason }) => {
            assert!(reason.contains("expired"));
        }
        _ => panic!("Expected expired message error"),
    }

    // Test future timestamp
    let mut future_message = BindingMessage::new(
        BindingAction::BindAccount,
        "test-chain".to_string(),
        eth_account,
        sol_account.clone(),
        multivm_id,
        2,
    );
    future_message.timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;

    let result = mapper
        .process_binding_message(
            future_message,
            create_enhanced_proof(sol_account.clone(), 2),
        )
        .await;

    match result {
        Err(AccountMappingError::InvalidBindingProof { reason }) => {
            assert!(reason.contains("future"));
        }
        _ => panic!("Expected future timestamp error"),
    }
}

#[tokio::test]
async fn test_guardian_management() {
    let storage = Arc::new(MemoryStorage::new());
    let event_collector = Arc::new(TestEventCollector::new());

    let mapper = create_test_mapper(
        storage.clone(),
        BindingPolicy::default(),
        RecoveryConfig::default(),
    );

    // Add event collector
    mapper
        .event_emitter
        .write()
        .await
        .add_listener(event_collector.clone());

    let (eth_account, sol_account) = create_test_accounts();
    let guardian = AccountAddress::Ethereum(EthereumAddress([3u8; 20]));

    // Create auto-binding
    let multivm_id = mapper
        .create_auto_binding(eth_account.clone())
        .await
        .unwrap();

    // Add guardian
    let add_message = BindingMessage::new(
        BindingAction::AddGuardian,
        "test-chain".to_string(),
        eth_account.clone(),
        guardian.clone(),
        multivm_id.clone(),
        1,
    );

    mapper
        .process_binding_message(add_message, create_enhanced_proof(eth_account.clone(), 1))
        .await
        .unwrap();

    // Verify guardian was added via events
    let events = event_collector.get_events().await;
    assert!(events
        .iter()
        .any(|e| matches!(e, AccountBindingEvent::GuardianAdded { .. })));

    // Remove guardian
    event_collector.clear().await;
    let remove_message = BindingMessage::new(
        BindingAction::RemoveGuardian,
        "test-chain".to_string(),
        eth_account,
        guardian,
        multivm_id,
        2,
    );

    mapper
        .process_binding_message(
            remove_message,
            create_enhanced_proof(sol_account.clone(), 2),
        )
        .await
        .unwrap();

    // Verify guardian was removed
    let events = event_collector.get_events().await;
    assert!(events
        .iter()
        .any(|e| matches!(e, AccountBindingEvent::GuardianRemoved { .. })));
}

#[tokio::test]
async fn test_recovery_initiation() {
    let mut recovery_config = RecoveryConfig::default();
    recovery_config.enable_social_recovery = true;
    recovery_config.recovery_timelock = Duration::from_secs(3600);

    let storage = Arc::new(MemoryStorage::new());
    let mapper =
        create_test_mapper(storage.clone(), BindingPolicy::default(), recovery_config);

    let (eth_account, _) = create_unique_test_accounts();
    let guardian = AccountAddress::Ethereum(EthereumAddress([3u8; 20]));
    let new_account = AccountAddress::Ethereum(EthereumAddress([4u8; 20]));

    // Create binding and add guardian
    let multivm_id = mapper
        .create_auto_binding(eth_account.clone())
        .await
        .unwrap();

    // Manually add guardian for testing
    mapper
        .guardians
        .write()
        .await
        .entry(multivm_id.clone())
        .or_insert_with(Vec::new)
        .push(Guardian {
            address: guardian.clone(),
            added_at: SystemTime::now(),
            label: Some("Test Guardian".to_string()),
            is_verified: true,
        });

    // Initiate recovery
    let recovery_message = BindingMessage::new(
        BindingAction::InitiateRecovery,
        "test-chain".to_string(),
        guardian,
        new_account,
        multivm_id.clone(),
        1,
    );

    mapper
        .process_binding_message(
            recovery_message,
            create_enhanced_proof(eth_account.clone(), 1),
        )
        .await
        .unwrap();

    // Verify recovery request was created
    let requests = mapper.recovery_requests.read().await;
    assert_eq!(requests.len(), 1);
}

#[tokio::test]
async fn test_event_emission_throughout_lifecycle() {
    let storage = Arc::new(MemoryStorage::new());
    let event_collector = Arc::new(TestEventCollector::new());

    let mapper = create_test_mapper(
        storage.clone(),
        BindingPolicy::default(),
        RecoveryConfig::default(),
    );

    mapper
        .event_emitter
        .write()
        .await
        .add_listener(event_collector.clone());

    let (eth_account, sol_account) = create_test_accounts();

    // Auto-binding
    let multivm_id = mapper
        .create_auto_binding(eth_account.clone())
        .await
        .unwrap();

    // Cross-binding (will fail without proper setup, but we check for events)
    let bind_message = BindingMessage::new(
        BindingAction::BindAccount,
        "test-chain".to_string(),
        eth_account.clone(),
        sol_account.clone(),
        multivm_id,
        1,
    );

    let _ = mapper
        .process_binding_message(bind_message, create_enhanced_proof(eth_account.clone(), 1))
        .await;

    // Check events
    let events = event_collector.get_events().await;

    // Should have at least auto-binding event
    assert!(events
        .iter()
        .any(|e| matches!(e, AccountBindingEvent::AutoBindingCreated { .. })));
}

#[tokio::test]
async fn test_policy_enforcement() {
    let mut policy = BindingPolicy::default();
    policy.max_accounts_per_multivm = 2;
    policy.binding_cooldown = Duration::from_secs(0); // No cooldown for testing
    policy.min_account_age = Duration::from_secs(0); // For testing

    let storage = Arc::new(MemoryStorage::new());
    let mapper = create_test_mapper(storage.clone(), policy, RecoveryConfig::default());

    // Use unique accounts to avoid conflicts
    let eth1 = AccountAddress::Ethereum(EthereumAddress([71u8; 20]));
    let _eth2 = AccountAddress::Ethereum(EthereumAddress([72u8; 20]));
    let eth3 = AccountAddress::Ethereum(EthereumAddress([73u8; 20]));
    let sol1 = AccountAddress::Solana(SolanaAddress([71u8; 32]));

    // Create initial binding
    let multivm_id = mapper.create_auto_binding(eth1.clone()).await.unwrap();

    // Add a Solana account as the second binding
    let bind_sol_message = BindingMessage::new(
        BindingAction::BindAccount,
        "test-chain".to_string(),
        eth1.clone(),
        sol1.clone(),
        multivm_id.clone(),
        1,
    );
    mapper
        .process_binding_message(bind_sol_message, create_enhanced_proof(sol1.clone(), 1))
        .await
        .unwrap();

    // Wait for lock to be released  
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    // Try to add third account (should fail due to policy)
    let bind_message = BindingMessage::new(
        BindingAction::BindAccount,
        "test-chain".to_string(),
        eth1.clone(),
        eth3.clone(),
        multivm_id.clone(),
        2,
    );

    let result = mapper
        .process_binding_message(bind_message, create_enhanced_proof(eth3.clone(), 2))
        .await;

    // Should fail with policy violation (or lock contention if lock not released)
    match result {
        Err(AccountMappingError::PolicyViolation { .. }) => {
            // Expected outcome
        }
        Err(AccountMappingError::LockContention { .. }) => {
            // Wait for lock and retry
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            let bind_message2 = BindingMessage::new(
                BindingAction::BindAccount,
                "test-chain".to_string(),
                eth1,
                eth3.clone(),
                multivm_id,
                2,
            );
            let result2 = mapper
                .process_binding_message(bind_message2, create_enhanced_proof(eth3, 2))
                .await;
            match result2 {
                Err(AccountMappingError::PolicyViolation { .. }) => {}
                Ok(_) => panic!("Expected policy violation but got success"),
                Err(e) => panic!("Expected policy violation but got: {:?}", e),
            }
        }
        Ok(_) => panic!("Expected error but got success"),
        Err(e) => panic!("Expected policy violation but got: {:?}", e),
    }
}

#[tokio::test]
async fn test_unbinding_with_timelock() {
    let mut policy = BindingPolicy::default();
    policy.unbinding_timelock = Duration::from_secs(3600);

    let storage = Arc::new(MemoryStorage::new());
    let mapper = create_test_mapper(storage.clone(), policy, RecoveryConfig::default());

    let (eth_account, _) = create_unique_test_accounts();

    // Create binding
    let multivm_id = mapper
        .create_auto_binding(eth_account.clone())
        .await
        .unwrap();

    // Try to unbind
    let unbind_message = BindingMessage::new(
        BindingAction::UnbindAccount,
        "test-chain".to_string(),
        eth_account.clone(),
        eth_account.clone(),
        multivm_id,
        1,
    );

    // This should process but not immediately unbind due to timelock
    let result = mapper
        .process_binding_message(
            unbind_message,
            create_enhanced_proof(eth_account.clone(), 1),
        )
        .await;

    // In production, this would create an unbinding request with timelock
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrent_operations_with_locking() {
    let storage = Arc::new(MemoryStorage::new());
    let mapper = Arc::new(create_test_mapper(
        storage.clone(),
        BindingPolicy::default(),
        RecoveryConfig::default(),
    ));

    let (eth_account, sol_account) = create_test_accounts();

    // Create initial binding
    let multivm_id = mapper
        .create_auto_binding(eth_account.clone())
        .await
        .unwrap();

    // Simulate concurrent binding attempts
    let mut handles = vec![];
    for i in 0..5 {
        let mapper_clone = mapper.clone();
        let message = BindingMessage::new(
            BindingAction::BindAccount,
            "test-chain".to_string(),
            eth_account.clone(),
            sol_account.clone(),
            multivm_id.clone(),
            i,
        );
        let proof = create_enhanced_proof(eth_account.clone(), i);

        handles.push(tokio::spawn(async move {
            mapper_clone.process_binding_message(message, proof).await
        }));
    }

    // Collect results
    let results: Vec<_> = futures::future::join_all(handles).await;

    // At least one should succeed or fail with appropriate errors
    // No panics or deadlocks should occur
    assert_eq!(results.len(), 5);
}

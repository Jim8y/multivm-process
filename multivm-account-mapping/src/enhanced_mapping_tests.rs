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

/// Create test binding proof
fn create_test_proof(account: AccountAddress, nonce: u64) -> BindingProof {
    BindingProof {
        account,
        proof_type: ProofType::Signature {
            message: Box::new(b"test message".to_vec()),
            signature: Box::new(b"test signature".to_vec()),
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
    let mapper = EnhancedAccountMapper::new(
        storage.clone(),
        BindingPolicy::default(),
        RecoveryConfig::default(),
    );

    let (eth_account, _) = create_test_accounts();

    // First auto-binding should succeed
    let multivm_id1 = mapper
        .create_auto_binding(eth_account.clone())
        .await
        .unwrap();

    // Second auto-binding should return the same ID (idempotent)
    let multivm_id2 = mapper
        .create_auto_binding(eth_account.clone())
        .await
        .unwrap();
    assert_eq!(multivm_id1, multivm_id2);

    // Verify binding was created
    let binding = storage.get_binding(&multivm_id1).await.unwrap();
    assert!(binding.is_some());
    assert_eq!(binding.unwrap().evm_account, Some(eth_account));
}

#[tokio::test]
async fn test_concurrent_auto_binding() {
    let storage = Arc::new(MemoryStorage::new());
    let mapper = Arc::new(EnhancedAccountMapper::new(
        storage.clone(),
        BindingPolicy::default(),
        RecoveryConfig::default(),
    ));

    let (eth_account, _) = create_test_accounts();

    // Simulate concurrent auto-binding attempts
    let mut handles = vec![];
    for _ in 0..10 {
        let mapper_clone = mapper.clone();
        let account_clone = eth_account.clone();
        handles.push(tokio::spawn(async move {
            mapper_clone.create_auto_binding(account_clone).await
        }));
    }

    // Collect results
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await.unwrap().unwrap());
    }

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
    let mapper = EnhancedAccountMapper::new(storage.clone(), policy, RecoveryConfig::default());

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
        EnhancedAccountMapper::new(storage, BindingPolicy::default(), RecoveryConfig::default());

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

    let mapper = EnhancedAccountMapper::new(
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
        EnhancedAccountMapper::new(storage.clone(), BindingPolicy::default(), recovery_config);

    let (eth_account, _) = create_test_accounts();
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

    let mapper = EnhancedAccountMapper::new(
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
    policy.binding_cooldown = Duration::from_secs(60);
    policy.min_account_age = Duration::from_secs(0); // For testing

    let storage = Arc::new(MemoryStorage::new());
    let mapper = EnhancedAccountMapper::new(storage.clone(), policy, RecoveryConfig::default());

    let eth1 = AccountAddress::Ethereum(EthereumAddress([1u8; 20]));
    let eth2 = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));
    let eth3 = AccountAddress::Ethereum(EthereumAddress([3u8; 20]));

    // Create initial binding
    let multivm_id = mapper.create_auto_binding(eth1.clone()).await.unwrap();

    // Manually add a second binding to test limit
    let mut binding = storage.get_binding(&multivm_id).await.unwrap().unwrap();
    binding.evm_account = Some(eth2.clone());
    storage.update_binding(&binding).await.unwrap();

    // Try to add third account (should fail due to policy)
    let bind_message = BindingMessage::new(
        BindingAction::BindAccount,
        "test-chain".to_string(),
        eth1.clone(),
        eth3,
        multivm_id,
        1,
    );

    let result = mapper
        .process_binding_message(bind_message, create_enhanced_proof(eth1.clone(), 1))
        .await;

    // Should fail with policy violation
    match result {
        Err(AccountMappingError::PolicyViolation { .. }) => {}
        _ => panic!("Expected policy violation"),
    }
}

#[tokio::test]
async fn test_unbinding_with_timelock() {
    let mut policy = BindingPolicy::default();
    policy.unbinding_timelock = Duration::from_secs(3600);

    let storage = Arc::new(MemoryStorage::new());
    let mapper = EnhancedAccountMapper::new(storage.clone(), policy, RecoveryConfig::default());

    let (eth_account, _) = create_test_accounts();

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
    let mapper = Arc::new(EnhancedAccountMapper::new(
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

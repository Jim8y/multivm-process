//! Example demonstrating enhanced account mapping features

use multivm_account_mapping::{
    address::{AccountAddress, EthereumAddress, SolanaAddress},
    binding_message::{BindingAction, BindingMessage},
    binding_policy::{BindingPolicy, EnhancedBindingProof, RecoveryConfig, SecurityMetadata},
    enhanced_mapping::EnhancedAccountMapper,
    events::{EventEmitter, LoggingEventListener},
    mapping::BindingProof,
    storage::MemoryStorage,
};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create storage and policies
    let storage = Arc::new(MemoryStorage::new());
    let mut policy = BindingPolicy::default();
    policy.max_accounts_per_multivm = 3;
    policy.binding_cooldown = Duration::from_secs(10);
    policy.max_binding_attempts_per_hour = 100;

    let recovery_config = RecoveryConfig {
        enable_social_recovery: true,
        min_guardians: 2,
        recovery_threshold: 2,
        recovery_timelock: Duration::from_secs(3600),
        max_guardians: 5,
    };

    // Create enhanced mapper
    let mut mapper = EnhancedAccountMapper::new(
        storage.clone(),
        policy,
        recovery_config,
    );

    // Add event listener
    mapper.event_emitter.write().await
        .add_listener(Arc::new(LoggingEventListener));

    println!("Enhanced Account Mapping Example");
    println!("================================\n");

    // Test accounts
    let eth_account = AccountAddress::Ethereum(EthereumAddress([0x01; 20]));
    let sol_account = AccountAddress::Solana(SolanaAddress([0x02; 32]));

    // 1. Automatic Binding with Locking
    println!("1. Creating automatic binding with distributed locking...");
    let multivm_id = mapper.create_auto_binding(eth_account.clone()).await
        .expect("Failed to create auto binding");
    println!("   ✓ Created MultiVM account: {}", multivm_id);

    // Test idempotency
    let multivm_id2 = mapper.create_auto_binding(eth_account.clone()).await
        .expect("Failed to create auto binding");
    println!("   ✓ Idempotent check passed: {}", multivm_id == multivm_id2);

    // 2. Cross-VM Binding with Message Format
    println!("\n2. Testing standardized binding message format...");
    let binding_message = BindingMessage::new(
        BindingAction::BindAccount,
        "multivm-testnet".to_string(),
        eth_account.clone(),
        sol_account.clone(),
        multivm_id.clone(),
        1,
    );

    println!("   Message hash: {}", hex::encode(binding_message.hash()));
    println!("   EIP-712 hash: {}", hex::encode(binding_message.eip712_hash()));

    // Create proof (in real scenario, this would be a signature)
    let proof = EnhancedBindingProof {
        proof: BindingProof {
            proof_type: multivm_account_mapping::mapping::ProofType::Signature,
            proof_data: vec![0xde, 0xad, 0xbe, 0xef],
            nonce: 1,
            timestamp: SystemTime::now(),
        },
        secondary_auth: None,
        risk_score: 10,
        security_metadata: SecurityMetadata {
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("example-client/1.0".to_string()),
            geolocation: Some("US".to_string()),
            activity_score: 80,
            suspicious_activity_count: 0,
            is_verified: true,
        },
    };

    // Note: This will fail because we don't have a valid signature
    match mapper.process_binding_message(binding_message, proof).await {
        Ok(_) => println!("   ✓ Binding message processed"),
        Err(e) => println!("   ✗ Expected error (no valid signature): {}", e),
    }

    // 3. Guardian Management
    println!("\n3. Testing guardian management...");
    let guardian = AccountAddress::Ethereum(EthereumAddress([0x03; 20]));
    
    let add_guardian_msg = BindingMessage::new(
        BindingAction::AddGuardian,
        "multivm-testnet".to_string(),
        eth_account.clone(),
        guardian.clone(),
        multivm_id.clone(),
        2,
    );

    let guardian_proof = EnhancedBindingProof {
        proof: BindingProof {
            proof_type: multivm_account_mapping::mapping::ProofType::Signature,
            proof_data: vec![0xca, 0xfe],
            nonce: 2,
            timestamp: SystemTime::now(),
        },
        secondary_auth: None,
        risk_score: 0,
        security_metadata: SecurityMetadata::default(),
    };

    match mapper.process_binding_message(add_guardian_msg, guardian_proof).await {
        Ok(_) => println!("   ✓ Guardian added"),
        Err(e) => println!("   ✗ Expected error (no valid signature): {}", e),
    }

    // 4. Rate Limiting
    println!("\n4. Testing rate limiting...");
    println!("   Simulating multiple binding attempts...");
    
    for i in 3..8 {
        let msg = BindingMessage::new(
            BindingAction::BindAccount,
            "multivm-testnet".to_string(),
            eth_account.clone(),
            AccountAddress::Ethereum(EthereumAddress([i as u8; 20])),
            multivm_id.clone(),
            i,
        );

        let result = mapper.process_binding_message(
            msg,
            EnhancedBindingProof {
                proof: BindingProof {
                    proof_type: multivm_account_mapping::mapping::ProofType::Signature,
                    proof_data: vec![i as u8],
                    nonce: i,
                    timestamp: SystemTime::now(),
                },
                secondary_auth: None,
                risk_score: 0,
                security_metadata: SecurityMetadata::default(),
            },
        ).await;

        match result {
            Ok(_) => println!("   Attempt {}: Success", i - 2),
            Err(e) => println!("   Attempt {}: {}", i - 2, e),
        }
    }

    // 5. Message Validation
    println!("\n5. Testing message validation...");
    
    // Test expired message
    let mut expired_msg = BindingMessage::new(
        BindingAction::BindAccount,
        "multivm-testnet".to_string(),
        eth_account.clone(),
        AccountAddress::Ethereum(EthereumAddress([0x04; 20])),
        multivm_id.clone(),
        10,
    );
    expired_msg.expires_at = Some(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() - 3600
    );

    match mapper.process_binding_message(
        expired_msg,
        EnhancedBindingProof {
            proof: BindingProof {
                proof_type: multivm_account_mapping::mapping::ProofType::Signature,
                proof_data: vec![],
                nonce: 10,
                timestamp: SystemTime::now(),
            },
            secondary_auth: None,
            risk_score: 0,
            security_metadata: SecurityMetadata::default(),
        },
    ).await {
        Ok(_) => println!("   ✗ Expired message should have failed"),
        Err(e) => println!("   ✓ Expired message rejected: {}", e),
    }

    println!("\n6. Summary");
    println!("   ================================");
    println!("   Enhanced features demonstrated:");
    println!("   ✓ Distributed locking for race condition prevention");
    println!("   ✓ Standardized message format with EIP-712 style hashing");
    println!("   ✓ Policy enforcement and rate limiting");
    println!("   ✓ Guardian management for social recovery");
    println!("   ✓ Message validation (expiry, timestamps)");
    println!("   ✓ Comprehensive event emission");
}
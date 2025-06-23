//! P2P Security Integration Tests
//!
//! Tests for authentication, rate limiting, message validation, and firewall features.

use multivm_p2p::{
    config::P2PConfig,
    error::P2PError,
    rate_limiter::{RateLimiter, RateLimiterConfig},
    security::{SecurityManager, SecurityConfig, MessageMetadata},
    secure_network::SecureNetworkManager,
};
use ed25519_dalek::SigningKey;
use libp2p::PeerId;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

#[tokio::test]
async fn test_rate_limiting_protection() {
    let config = RateLimiterConfig {
        per_peer_rate: 2,
        per_peer_burst: 1,
        global_rate: 10,
        global_burst: 5,
        enabled: true,
    };
    
    let limiter = RateLimiter::new(config);
    let peer_id = PeerId::random();
    
    // First request should succeed (within burst)
    assert!(limiter.check_peer_limit(&peer_id).is_ok());
    
    // Second request should succeed (burst limit)
    assert!(limiter.check_peer_limit(&peer_id).is_ok());
    
    // Third request should fail (exceeds rate limit)
    let result = limiter.check_peer_limit(&peer_id);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), P2PError::RateLimitExceeded(_)));
}

#[tokio::test]
async fn test_message_authentication() {
    let config = SecurityConfig {
        enable_auth: true,
        enable_signing: true,
        enable_replay_protection: true,
        max_message_size: 1024,
        message_expiry: Duration::from_secs(300),
        require_trusted_peers: true,
    };
    
    let mut security_manager = SecurityManager::new(config);
    let peer_id = PeerId::random();
    
    // Add self as trusted peer
    let public_key = security_manager.public_key();
    security_manager.add_trusted_peer(peer_id, public_key);
    
    let payload = b"test secure message".to_vec();
    
    // Secure the message
    let secured_message = security_manager
        .secure_message(payload.clone(), peer_id)
        .await
        .unwrap();
    
    // Create metadata
    let metadata = MessageMetadata {
        sender: peer_id,
        size: payload.len(),
        timestamp: SystemTime::now(),
        message_type: "test".to_string(),
    };
    
    // Validate the message
    let decrypted = security_manager
        .validate_message(secured_message, metadata)
        .await
        .unwrap();
    
    assert_eq!(decrypted, payload);
}

#[tokio::test]
async fn test_replay_attack_prevention() {
    let config = SecurityConfig::default();
    let mut security_manager = SecurityManager::new(config);
    let peer_id = PeerId::random();
    
    // Add peer as trusted
    let public_key = security_manager.public_key();
    security_manager.add_trusted_peer(peer_id, public_key);
    
    let payload = b"test message".to_vec();
    
    // Send first message
    let secured_message1 = security_manager
        .secure_message(payload.clone(), peer_id)
        .await
        .unwrap();
    
    let metadata1 = MessageMetadata {
        sender: peer_id,
        size: payload.len(),
        timestamp: SystemTime::now(),
        message_type: "test".to_string(),
    };
    
    // First validation should succeed
    assert!(security_manager
        .validate_message(secured_message1.clone(), metadata1)
        .await
        .is_ok());
    
    // Try to replay the same message
    let metadata2 = MessageMetadata {
        sender: peer_id,
        size: payload.len(),
        timestamp: SystemTime::now(),
        message_type: "test".to_string(),
    };
    
    // Replay should fail
    let result = security_manager
        .validate_message(secured_message1, metadata2)
        .await;
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), P2PError::ReplayAttack(_, _)));
}

#[tokio::test]
async fn test_message_size_protection() {
    let config = SecurityConfig {
        max_message_size: 100,
        ..Default::default()
    };
    
    let mut security_manager = SecurityManager::new(config);
    let peer_id = PeerId::random();
    
    // Create oversized message
    let large_payload = vec![0u8; 200];
    
    // Should reject oversized message
    let result = security_manager
        .secure_message(large_payload, peer_id)
        .await;
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), P2PError::MessageTooLarge(_)));
}

#[tokio::test]
async fn test_message_expiry() {
    let config = SecurityConfig {
        message_expiry: Duration::from_millis(100),
        ..Default::default()
    };
    
    let mut security_manager = SecurityManager::new(config);
    let peer_id = PeerId::random();
    
    // Add peer as trusted
    let public_key = security_manager.public_key();
    security_manager.add_trusted_peer(peer_id, public_key);
    
    let payload = b"test message".to_vec();
    
    // Create message
    let secured_message = security_manager
        .secure_message(payload.clone(), peer_id)
        .await
        .unwrap();
    
    // Wait for message to expire
    sleep(Duration::from_millis(200)).await;
    
    let metadata = MessageMetadata {
        sender: peer_id,
        size: payload.len(),
        timestamp: SystemTime::now(),
        message_type: "test".to_string(),
    };
    
    // Should fail due to expiry
    let result = security_manager
        .validate_message(secured_message, metadata)
        .await;
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), P2PError::MessageExpired(_)));
}

#[tokio::test]
async fn test_untrusted_peer_rejection() {
    let config = SecurityConfig {
        require_trusted_peers: true,
        ..Default::default()
    };
    
    let mut security_manager = SecurityManager::new(config);
    let trusted_peer = PeerId::random();
    let untrusted_peer = PeerId::random();
    
    // Add only one peer as trusted
    let public_key = security_manager.public_key();
    security_manager.add_trusted_peer(trusted_peer, public_key);
    
    let payload = b"test message".to_vec();
    
    // Message from untrusted peer should be rejected
    let secured_message = security_manager
        .secure_message(payload.clone(), untrusted_peer)
        .await
        .unwrap();
    
    let metadata = MessageMetadata {
        sender: untrusted_peer,
        size: payload.len(),
        timestamp: SystemTime::now(),
        message_type: "test".to_string(),
    };
    
    let result = security_manager
        .validate_message(secured_message, metadata)
        .await;
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), P2PError::UnauthorizedPeer(_)));
}

#[tokio::test]
async fn test_secure_network_creation() {
    let config = P2PConfig::default();
    let result = SecureNetworkManager::new(config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrent_rate_limiting() {
    use std::sync::Arc;
    use tokio::sync::Barrier;
    
    let config = RateLimiterConfig {
        per_peer_rate: 5,
        per_peer_burst: 2,
        global_rate: 20,
        global_burst: 10,
        enabled: true,
    };
    
    let limiter = Arc::new(RateLimiter::new(config));
    let peer_id = PeerId::random();
    let barrier = Arc::new(Barrier::new(10));
    
    let mut handles = vec![];
    
    // Spawn 10 concurrent tasks
    for i in 0..10 {
        let limiter_clone = Arc::clone(&limiter);
        let barrier_clone = Arc::clone(&barrier);
        let peer_id_clone = peer_id;
        
        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;
            
            // Each task makes multiple requests
            let mut allowed = 0;
            let mut denied = 0;
            
            for _ in 0..5 {
                match limiter_clone.check_peer_limit(&peer_id_clone) {
                    Ok(_) => allowed += 1,
                    Err(_) => denied += 1,
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            
            (allowed, denied)
        });
        
        handles.push(handle);
    }
    
    // Collect results
    let mut total_allowed = 0;
    let mut total_denied = 0;
    
    for handle in handles {
        let (allowed, denied) = handle.await.unwrap();
        total_allowed += allowed;
        total_denied += denied;
    }
    
    // Should have some requests denied due to rate limiting
    assert!(total_denied > 0, "Rate limiting should have denied some requests");
    assert!(total_allowed > 0, "Some requests should have been allowed");
    
    println!("Rate limiting test: {} allowed, {} denied", total_allowed, total_denied);
}

#[tokio::test]
async fn test_message_integrity() {
    let config = SecurityConfig::default();
    let mut security_manager = SecurityManager::new(config);
    let peer_id = PeerId::random();
    
    // Add peer as trusted
    let public_key = security_manager.public_key();
    security_manager.add_trusted_peer(peer_id, public_key);
    
    let payload = b"important data".to_vec();
    
    // Secure the message
    let mut secured_message = security_manager
        .secure_message(payload.clone(), peer_id)
        .await
        .unwrap();
    
    // Tamper with the message
    secured_message.payload[0] ^= 0xFF; // Flip bits
    
    let metadata = MessageMetadata {
        sender: peer_id,
        size: payload.len(),
        timestamp: SystemTime::now(),
        message_type: "test".to_string(),
    };
    
    // Should detect tampering
    let result = security_manager
        .validate_message(secured_message, metadata)
        .await;
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), P2PError::InvalidMessage(_)));
}
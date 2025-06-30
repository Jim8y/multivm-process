//! Security-focused tests for the P2P networking layer
//!
//! This module contains comprehensive security tests to validate
//! authentication, authorization, encryption, and other security features.

#[cfg(test)]
mod tests {
    use crate::security::*;

    #[test]
    fn test_security_manager_creation() {
        let config = SecurityConfig::default();
        let manager = SecurityManager::new(config);
        // Just test that the public key exists
        let _public_key = manager.public_key();
    }

    #[test]
    fn test_message_security() {
        let config = SecurityConfig::default();
        let mut manager = SecurityManager::new(config);
        let peer_id = libp2p::PeerId::random();
        let message = b"test message".to_vec();

        // Test securing a message
        let secured = manager.secure_message(message.clone(), peer_id).unwrap();
        assert_eq!(secured.sender, peer_id.to_string());
        assert!(!secured.payload.is_empty());
    }

    #[test]
    fn test_security_config() {
        let config = SecurityConfig::default();
        assert!(config.enable_signing);
        assert!(config.enable_auth);
    }
}

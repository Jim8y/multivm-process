//! Production-ready encryption utilities for secure P2P communication

use crate::error::{P2PError, P2PResult};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Key, Nonce
};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Production encryption manager for P2P messages
pub struct ProductionEncryptionManager {
    /// Cached shared secrets for performance
    shared_secrets: HashMap<Vec<u8>, SharedSecret>,
    /// Our static secret for X25519
    static_secret: Option<x25519_dalek::StaticSecret>,
}

impl ProductionEncryptionManager {
    /// Create a new production encryption manager
    pub fn new() -> Self {
        Self {
            shared_secrets: HashMap::new(),
            static_secret: None,
        }
    }
    
    /// Initialize with a static secret for this node
    pub fn with_static_secret(secret: x25519_dalek::StaticSecret) -> Self {
        Self {
            shared_secrets: HashMap::new(),
            static_secret: Some(secret),
        }
    }
    
    /// Generate a new Ed25519 keypair for signing
    pub fn generate_signing_keypair(&self) -> P2PResult<SigningKey> {
        let mut csprng = OsRng;
        Ok(SigningKey::generate(&mut csprng))
    }
    
    /// Generate a new X25519 keypair for encryption
    pub fn generate_encryption_keypair(&self) -> (x25519_dalek::StaticSecret, x25519_dalek::PublicKey) {
        let secret = x25519_dalek::StaticSecret::random_from_rng(OsRng);
        let public = x25519_dalek::PublicKey::from(&secret);
        (secret, public)
    }
    
    /// Encrypt a message using ChaCha20-Poly1305
    pub fn encrypt_message(&self, plaintext: &[u8], recipient_public_key: &[u8]) -> P2PResult<Vec<u8>> {
        // Generate ephemeral keypair for this message
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_public = PublicKey::from(&ephemeral_secret);
        
        // Parse recipient's public key
        let recipient_public = PublicKey::from(
            <[u8; 32]>::try_from(recipient_public_key)
                .map_err(|_| P2PError::security_error("Invalid recipient public key"))?
        );
        
        // Compute shared secret
        let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);
        
        // Derive encryption key from shared secret
        let key = derive_key_from_shared_secret(shared_secret.as_bytes());
        
        // Create cipher
        let cipher = ChaCha20Poly1305::new(&key);
        
        // Generate nonce
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        
        // Encrypt
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| P2PError::security_error(format!("Encryption failed: {}", e)))?;
        
        // Construct encrypted message: [ephemeral_public || nonce || ciphertext]
        let mut encrypted = Vec::with_capacity(32 + 12 + ciphertext.len());
        encrypted.extend_from_slice(ephemeral_public.as_bytes());
        encrypted.extend_from_slice(&nonce);
        encrypted.extend_from_slice(&ciphertext);
        
        Ok(encrypted)
    }
    
    /// Decrypt a message using our private key
    pub fn decrypt_message(&self, ciphertext: &[u8], our_secret_key: &x25519_dalek::StaticSecret) -> P2PResult<Vec<u8>> {
        if ciphertext.len() < 44 { // 32 (public key) + 12 (nonce)
            return Err(P2PError::security_error("Ciphertext too short"));
        }
        
        // Extract components
        let ephemeral_public_bytes = &ciphertext[0..32];
        let nonce_bytes = &ciphertext[32..44];
        let encrypted_data = &ciphertext[44..];
        
        // Parse ephemeral public key
        let ephemeral_public = PublicKey::from(
            <[u8; 32]>::try_from(ephemeral_public_bytes)
                .map_err(|_| P2PError::security_error("Invalid ephemeral public key"))?
        );
        
        // Compute shared secret
        let shared_secret = our_secret_key.diffie_hellman(&ephemeral_public);
        
        // Derive decryption key
        let key = derive_key_from_shared_secret(shared_secret.as_bytes());
        
        // Create cipher
        let cipher = ChaCha20Poly1305::new(&key);
        
        // Parse nonce
        let nonce = Nonce::from_slice(nonce_bytes);
        
        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, encrypted_data)
            .map_err(|e| P2PError::security_error(format!("Decryption failed: {}", e)))?;
        
        Ok(plaintext)
    }
    
    /// Sign a message with Ed25519
    pub fn sign_message(&self, message: &[u8], signing_key: &SigningKey) -> P2PResult<Vec<u8>> {
        let signature = signing_key.sign(message);
        Ok(signature.to_bytes().to_vec())
    }
    
    /// Verify an Ed25519 signature
    pub fn verify_signature(&self, message: &[u8], signature: &[u8], public_key: &VerifyingKey) -> P2PResult<()> {
        let sig = Signature::from_slice(signature)
            .map_err(|e| P2PError::security_error(format!("Invalid signature: {}", e)))?;
            
        public_key.verify(message, &sig)
            .map_err(|e| P2PError::security_error(format!("Signature verification failed: {}", e)))
    }
    
    /// Derive a shared secret using X25519 ECDH
    pub fn derive_shared_secret(
        &mut self, 
        our_secret: &x25519_dalek::StaticSecret, 
        their_public_key: &[u8]
    ) -> P2PResult<SharedSecret> {
        // Check cache first
        if let Some(cached) = self.shared_secrets.get(their_public_key) {
            return Ok(*cached);
        }
        
        // Parse their public key
        let their_public = PublicKey::from(
            <[u8; 32]>::try_from(their_public_key)
                .map_err(|_| P2PError::security_error("Invalid peer public key"))?
        );
        
        // Compute shared secret
        let shared_secret = our_secret.diffie_hellman(&their_public);
        
        // Cache it
        self.shared_secrets.insert(their_public_key.to_vec(), shared_secret);
        
        Ok(shared_secret)
    }
    
    /// Create an encrypted and authenticated message envelope
    pub fn create_secure_envelope(
        &self,
        plaintext: &[u8],
        recipient_public_key: &[u8],
        signing_key: &SigningKey,
    ) -> P2PResult<SecureEnvelope> {
        // Sign the plaintext
        let signature = self.sign_message(plaintext, signing_key)?;
        
        // Create signed payload
        let mut signed_payload = Vec::new();
        signed_payload.extend_from_slice(&(plaintext.len() as u32).to_be_bytes());
        signed_payload.extend_from_slice(plaintext);
        signed_payload.extend_from_slice(&signature);
        
        // Encrypt the signed payload
        let encrypted = self.encrypt_message(&signed_payload, recipient_public_key)?;
        
        Ok(SecureEnvelope {
            encrypted_payload: encrypted,
            sender_public_key: signing_key.verifying_key().to_bytes().to_vec(),
            timestamp: SystemTime::now(),
        })
    }
    
    /// Open and verify a secure envelope
    pub fn open_secure_envelope(
        &self,
        envelope: &SecureEnvelope,
        our_decryption_key: &x25519_dalek::StaticSecret,
    ) -> P2PResult<Vec<u8>> {
        // Check timestamp (prevent replay attacks)
        let age = envelope.timestamp.elapsed()
            .unwrap_or(Duration::from_secs(u64::MAX));
        if age > Duration::from_secs(300) { // 5 minutes
            return Err(P2PError::security_error("Message too old"));
        }
        
        // Decrypt the payload
        let signed_payload = self.decrypt_message(&envelope.encrypted_payload, our_decryption_key)?;
        
        // Extract components
        if signed_payload.len() < 68 { // 4 (length) + 64 (min signature)
            return Err(P2PError::security_error("Invalid signed payload"));
        }
        
        let payload_len = u32::from_be_bytes([
            signed_payload[0], signed_payload[1], 
            signed_payload[2], signed_payload[3]
        ]) as usize;
        
        if signed_payload.len() < 4 + payload_len + 64 {
            return Err(P2PError::security_error("Invalid payload length"));
        }
        
        let plaintext = &signed_payload[4..4 + payload_len];
        let signature = &signed_payload[4 + payload_len..];
        
        // Verify signature
        let sender_public_key = VerifyingKey::from_bytes(
            &<[u8; 32]>::try_from(&envelope.sender_public_key[..])
                .map_err(|_| P2PError::security_error("Invalid sender public key"))?
        ).map_err(|e| P2PError::security_error(format!("Invalid public key: {}", e)))?;
        
        self.verify_signature(plaintext, signature, &sender_public_key)?;
        
        Ok(plaintext.to_vec())
    }
}

/// Derive an encryption key from a shared secret
fn derive_key_from_shared_secret(shared_secret: &[u8]) -> Key {
    let mut hasher = Sha256::new();
    hasher.update(b"multivm-p2p-encryption-key");
    hasher.update(shared_secret);
    let result = hasher.finalize();
    Key::from_slice(&result)
}

/// Secure message envelope with encryption and authentication
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecureEnvelope {
    /// Encrypted and signed payload
    pub encrypted_payload: Vec<u8>,
    /// Sender's Ed25519 public key for signature verification
    pub sender_public_key: Vec<u8>,
    /// Timestamp for replay protection
    pub timestamp: SystemTime,
}

impl Default for ProductionEncryptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encryption_roundtrip() {
        let manager = ProductionEncryptionManager::new();
        
        // Generate keys for sender and recipient
        let (recipient_secret, recipient_public) = manager.generate_encryption_keypair();
        
        let plaintext = b"Hello, secure P2P world!";
        
        // Encrypt
        let encrypted = manager.encrypt_message(plaintext, recipient_public.as_bytes()).unwrap();
        assert!(encrypted.len() > plaintext.len() + 44); // Overhead from public key, nonce, and auth tag
        
        // Decrypt
        let decrypted = manager.decrypt_message(&encrypted, &recipient_secret).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_secure_envelope() {
        let mut manager = ProductionEncryptionManager::new();
        
        // Generate keys
        let signing_key = manager.generate_signing_keypair().unwrap();
        let (recipient_secret, recipient_public) = manager.generate_encryption_keypair();
        
        let message = b"Important cross-chain transaction";
        
        // Create envelope
        let envelope = manager.create_secure_envelope(
            message,
            recipient_public.as_bytes(),
            &signing_key
        ).unwrap();
        
        // Open envelope
        let recovered = manager.open_secure_envelope(&envelope, &recipient_secret).unwrap();
        assert_eq!(recovered, message);
    }
    
    #[test]
    fn test_tamper_detection() {
        let manager = ProductionEncryptionManager::new();
        
        // Generate keys
        let signing_key = manager.generate_signing_keypair().unwrap();
        let (recipient_secret, recipient_public) = manager.generate_encryption_keypair();
        
        let message = b"Do not tamper";
        
        // Create envelope
        let mut envelope = manager.create_secure_envelope(
            message,
            recipient_public.as_bytes(),
            &signing_key
        ).unwrap();
        
        // Tamper with encrypted payload
        envelope.encrypted_payload[50] ^= 0xFF;
        
        // Opening should fail
        let result = manager.open_secure_envelope(&envelope, &recipient_secret);
        assert!(result.is_err());
    }
}
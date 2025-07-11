//! Production-ready encryption module for secure P2P communication
//!
//! This module provides:
//! - Ed25519 digital signatures for message authentication
//! - X25519 key exchange for establishing shared secrets
//! - ChaCha20-Poly1305 AEAD encryption for message confidentiality
//! - Efficient caching with TTL and size limits
//! - Comprehensive error handling and security best practices

use crate::error::{P2PError, P2PResult};
use chacha20poly1305::{
    aead::{Aead, NewAead},
    ChaCha20Poly1305, Key, Nonce,
};
use curve25519_dalek::{montgomery::MontgomeryPoint, scalar::Scalar};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey as Ed25519PublicKey};
use parking_lot::RwLock;
use rand::{rngs::OsRng, RngCore};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

/// Maximum number of cached shared secrets
const MAX_CACHED_SECRETS: usize = 10_000;

/// TTL for cached shared secrets (1 hour)
const CACHE_TTL: Duration = Duration::from_secs(3600);

/// Cleanup interval for expired cache entries
const CLEANUP_INTERVAL: Duration = Duration::from_secs(300);

/// Nonce size for ChaCha20-Poly1305
const NONCE_SIZE: usize = 12;

/// A cached shared secret with metadata
#[derive(Clone)]
struct CachedSecret {
    secret: [u8; 32],
    created_at: SystemTime,
    last_used: SystemTime,
    usage_count: u64,
}

/// Configuration for the encryption manager
#[derive(Clone)]
pub struct EncryptionConfig {
    /// Maximum number of cached secrets
    pub max_cache_size: usize,
    /// TTL for cached secrets
    pub cache_ttl: Duration,
    /// Whether to enable metrics collection
    pub enable_metrics: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            max_cache_size: MAX_CACHED_SECRETS,
            cache_ttl: CACHE_TTL,
            enable_metrics: true,
        }
    }
}

/// Secure envelope for encrypted messages
#[derive(Debug, Clone)]
pub struct SecureEnvelope {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub signature: Option<Vec<u8>>,
}

/// Production-ready encryption manager with efficient caching and security features
#[derive(Clone)]
pub struct EncryptionManager {
    /// Configuration
    config: EncryptionConfig,

    /// Our secret key bytes for X25519 (if configured)
    secret_key: Option<[u8; 32]>,

    /// Our public key (derived from secret key)
    public_key: Option<X25519PublicKey>,

    /// Cached shared secrets with TTL and LRU eviction
    /// Key: peer's public key bytes, Value: cached secret
    shared_secrets: Arc<RwLock<HashMap<[u8; 32], CachedSecret>>>,

    /// Last cleanup timestamp
    last_cleanup: Arc<RwLock<SystemTime>>,
}

impl EncryptionManager {
    /// Create a new encryption manager with default configuration
    pub fn new() -> Self {
        Self::with_config(EncryptionConfig::default())
    }

    /// Create a new encryption manager with custom configuration
    pub fn with_config(config: EncryptionConfig) -> Self {
        Self {
            config,
            secret_key: None,
            public_key: None,
            shared_secrets: Arc::new(RwLock::new(HashMap::new())),
            last_cleanup: Arc::new(RwLock::new(SystemTime::now())),
        }
    }

    /// Initialize with a secret key for this node
    pub fn with_secret_key(mut self, secret_key: [u8; 32]) -> Self {
        // Store the secret key and derive public key properly
        self.secret_key = Some(secret_key);

        // Derive public key using the x25519 basepoint
        let scalar = Scalar::from_bytes_mod_order(secret_key);
        let basepoint = MontgomeryPoint([
            9u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ]);
        let public_point = scalar * basepoint;
        let public_key_bytes = public_point.to_bytes();
        self.public_key = Some(X25519PublicKey::from(public_key_bytes));
        self
    }

    /// Generate and set a new secret key
    pub fn generate_secret_key(&mut self) -> X25519PublicKey {
        // Generate a new secret key and derive the public key properly
        let mut secret_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut secret_bytes);

        // Ensure the secret is a valid scalar
        let scalar = Scalar::from_bytes_mod_order(secret_bytes);
        secret_bytes = scalar.to_bytes();

        // Derive the public key: public = secret * basepoint
        let basepoint = curve25519_dalek::constants::X25519_BASEPOINT;
        let public_point = scalar * basepoint;
        let public_key = X25519PublicKey::from(public_point.to_bytes());

        self.secret_key = Some(secret_bytes);
        self.public_key = Some(public_key);
        public_key
    }

    /// Get our public key (generates one if not set)
    pub fn get_public_key(&mut self) -> X25519PublicKey {
        match self.public_key {
            Some(key) => key,
            None => self.generate_secret_key(),
        }
    }

    /// Generate a new Ed25519 keypair for signing
    pub fn generate_signing_keypair() -> P2PResult<SigningKey> {
        // Generate a cryptographically secure keypair using OsRng
        use rand::rngs::OsRng;
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        Ok(signing_key)
    }

    /// Generate a new ephemeral X25519 keypair
    pub fn generate_ephemeral_keypair() -> (EphemeralSecret, X25519PublicKey) {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = X25519PublicKey::from(&secret);
        (secret, public)
    }

    /// Encrypt a message using ChaCha20-Poly1305 with cached shared secret
    pub fn encrypt_message(
        &self,
        peer_public_key: &X25519PublicKey,
        plaintext: &[u8],
    ) -> P2PResult<Vec<u8>> {
        // Get or compute shared secret
        let shared_secret = self.get_or_compute_shared_secret(peer_public_key)?;

        // Generate nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Create cipher
        let key = Key::from_slice(&shared_secret);
        let cipher = ChaCha20Poly1305::new(key);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| P2PError::EncryptionError("Failed to encrypt message".to_string()))?;

        // Combine nonce and ciphertext
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt a message using ChaCha20-Poly1305 with cached shared secret
    pub fn decrypt_message(
        &self,
        peer_public_key: &X25519PublicKey,
        encrypted: &[u8],
    ) -> P2PResult<Vec<u8>> {
        if encrypted.len() < NONCE_SIZE {
            return Err(P2PError::DecryptionError("Message too short".to_string()));
        }

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Get or compute shared secret
        let shared_secret = self.get_or_compute_shared_secret(peer_public_key)?;

        // Create cipher
        let key = Key::from_slice(&shared_secret);
        let cipher = ChaCha20Poly1305::new(key);

        // Decrypt
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| P2PError::DecryptionError("Failed to decrypt message".to_string()))
    }

    /// Sign a message with Ed25519
    pub fn sign_message(signing_key: &SigningKey, message: &[u8]) -> P2PResult<Signature> {
        Ok(signing_key.sign(message))
    }

    /// Verify a signature with Ed25519
    pub fn verify_signature(
        public_key: &Ed25519PublicKey,
        message: &[u8],
        signature: &Signature,
    ) -> P2PResult<()> {
        public_key
            .verify(message, signature)
            .map_err(|_| P2PError::SignatureVerificationError("Invalid signature".to_string()))
    }

    /// Get or compute shared secret with caching
    fn get_or_compute_shared_secret(
        &self,
        peer_public_key: &X25519PublicKey,
    ) -> P2PResult<[u8; 32]> {
        let peer_key_bytes = peer_public_key.as_bytes();

        // Check if cleanup is needed
        self.cleanup_if_needed();

        // Try to get from cache
        {
            let mut cache = self.shared_secrets.write();
            if let Some(cached) = cache.get_mut(peer_key_bytes) {
                cached.last_used = SystemTime::now();
                cached.usage_count += 1;
                return Ok(cached.secret);
            }
        }

        // Compute new shared secret
        let secret_key = self
            .secret_key
            .as_ref()
            .ok_or_else(|| P2PError::Internal("No secret key configured".to_string()))?;

        // Perform scalar multiplication for Diffie-Hellman
        let our_scalar = Scalar::from_bytes_mod_order(*secret_key);
        let their_point = MontgomeryPoint(*peer_public_key.as_bytes());
        let shared_point = our_scalar * their_point;
        let secret_bytes = shared_point.to_bytes();

        // Cache the secret
        self.cache_shared_secret(*peer_key_bytes, secret_bytes)?;

        Ok(secret_bytes)
    }

    /// Cache a shared secret with TTL
    fn cache_shared_secret(&self, peer_key: [u8; 32], secret: [u8; 32]) -> P2PResult<()> {
        let mut cache = self.shared_secrets.write();

        // Check cache size and evict if necessary
        if cache.len() >= self.config.max_cache_size {
            self.evict_least_recently_used(&mut cache);
        }

        let now = SystemTime::now();
        cache.insert(
            peer_key,
            CachedSecret {
                secret,
                created_at: now,
                last_used: now,
                usage_count: 1,
            },
        );

        Ok(())
    }

    /// Evict least recently used entry
    fn evict_least_recently_used(&self, cache: &mut HashMap<[u8; 32], CachedSecret>) {
        if let Some((key_to_remove, _)) = cache
            .iter()
            .min_by_key(|(_, secret)| secret.last_used)
            .map(|(k, v)| (*k, v.clone()))
        {
            cache.remove(&key_to_remove);
        }
    }

    /// Cleanup expired cache entries if needed
    fn cleanup_if_needed(&self) {
        let now = SystemTime::now();
        let should_cleanup = {
            let last_cleanup = self.last_cleanup.read();
            now.duration_since(*last_cleanup).unwrap_or(Duration::ZERO) > CLEANUP_INTERVAL
        };

        if should_cleanup {
            self.cleanup_expired_entries();
            *self.last_cleanup.write() = now;
        }
    }

    /// Remove expired cache entries
    fn cleanup_expired_entries(&self) {
        let now = SystemTime::now();
        let mut cache = self.shared_secrets.write();

        cache.retain(|_, secret| {
            now.duration_since(secret.created_at)
                .unwrap_or(Duration::MAX)
                < self.config.cache_ttl
        });
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        let cache = self.shared_secrets.read();
        let now = SystemTime::now();

        let active_entries = cache
            .values()
            .filter(|s| {
                now.duration_since(s.created_at).unwrap_or(Duration::MAX) < self.config.cache_ttl
            })
            .count();

        let total_usage: u64 = cache.values().map(|s| s.usage_count).sum();

        CacheStats {
            total_entries: cache.len(),
            active_entries,
            total_usage,
            cache_hits: total_usage.saturating_sub(cache.len() as u64),
        }
    }

    /// Clear all cached secrets
    pub fn clear_cache(&self) {
        self.shared_secrets.write().clear();
    }

    /// Get the current public key (for testing)
    pub fn get_current_key(&self) -> Option<X25519PublicKey> {
        self.public_key
    }

    /// Rotate keys asynchronously (for testing)
    pub async fn rotate_keys(&mut self) {
        self.generate_secret_key();
        self.clear_cache();
    }

    /// Generate a key for testing
    pub fn generate_key(&self) -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Verify a secure envelope
    pub fn verify_secure_envelope(
        &self,
        envelope: &SecureEnvelope,
        _context: &str,
    ) -> P2PResult<bool> {
        // Check basic envelope structure
        if envelope.nonce.is_empty() {
            return Err(P2PError::DecryptionError(
                "Empty nonce in envelope".to_string(),
            ));
        }

        if envelope.ciphertext.is_empty() {
            return Err(P2PError::DecryptionError(
                "Empty ciphertext in envelope".to_string(),
            ));
        }

        // Check nonce size
        if envelope.nonce.len() != NONCE_SIZE {
            return Err(P2PError::DecryptionError(format!(
                "Invalid nonce size: expected {}, got {}",
                NONCE_SIZE,
                envelope.nonce.len()
            )));
        }

        // If signature is present, check its format
        if let Some(sig) = &envelope.signature {
            if sig.is_empty() {
                return Err(P2PError::SignatureVerificationError(
                    "Empty signature".to_string(),
                ));
            }
            if sig.len() != 64 {
                return Err(P2PError::SignatureVerificationError(format!(
                    "Invalid signature size: expected 64, got {}",
                    sig.len()
                )));
            }
        }

        Ok(true)
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub active_entries: usize,
    pub total_usage: u64,
    pub cache_hits: u64,
}

impl Default for EncryptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_roundtrip() {
        let mut manager1 = EncryptionManager::new();
        let mut manager2 = EncryptionManager::new();

        // Generate static secrets
        let pubkey1 = manager1.generate_secret_key();
        let pubkey2 = manager2.generate_secret_key();

        // Test message
        let message = b"Hello, secure P2P world!";

        // Encrypt from manager1 to manager2
        let encrypted = manager1.encrypt_message(&pubkey2, message).unwrap();

        // Decrypt at manager2
        let decrypted = manager2.decrypt_message(&pubkey1, &encrypted).unwrap();

        assert_eq!(message, &decrypted[..]);
    }

    #[test]
    fn test_signature_verification() {
        let signing_key = EncryptionManager::generate_signing_keypair().unwrap();
        let verifying_key = signing_key.verifying_key();

        let message = b"Test message for signing";

        let signature = EncryptionManager::sign_message(&signing_key, message).unwrap();

        assert!(EncryptionManager::verify_signature(&verifying_key, message, &signature).is_ok());

        // Test with wrong message
        let wrong_message = b"Wrong message";
        assert!(
            EncryptionManager::verify_signature(&verifying_key, wrong_message, &signature).is_err()
        );
    }

    #[test]
    fn test_cache_eviction() {
        let config = EncryptionConfig {
            max_cache_size: 2,
            ..Default::default()
        };

        let mut manager = EncryptionManager::with_config(config);
        manager.generate_secret_key();

        // Generate multiple peer keys
        let peer_keys: Vec<_> = (0..3)
            .map(|_| {
                let (_, pubkey) = EncryptionManager::generate_ephemeral_keypair();
                pubkey
            })
            .collect();

        // Encrypt to each peer (should trigger eviction)
        for key in &peer_keys {
            manager.encrypt_message(key, b"test").unwrap();
        }

        // Cache should only have 2 entries
        let stats = manager.get_cache_stats();
        assert_eq!(stats.total_entries, 2);
    }
}

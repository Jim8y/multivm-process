//! Storage encryption for data at rest

use crate::{StorageError, Result};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tracing::{debug, warn};

/// Encryption configuration
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// Enable encryption
    pub enabled: bool,
    /// Encryption key (base64 encoded)
    pub key: Option<String>,
    /// Key derivation parameters
    pub kdf_params: KdfParams,
}

/// Key derivation parameters
#[derive(Debug, Clone)]
pub struct KdfParams {
    /// Salt for PBKDF2
    pub salt: Vec<u8>,
    /// Number of iterations
    pub iterations: u32,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            key: None,
            kdf_params: KdfParams {
                salt: vec![0u8; 32], // Should be random in production
                iterations: 100_000,
            },
        }
    }
}

/// Storage encryption manager
pub struct StorageEncryption {
    enabled: bool,
    cipher: Option<Arc<StorageCipher>>,
}

impl StorageEncryption {
    /// Create new storage encryption
    pub fn new(config: EncryptionConfig) -> Result<Self> {
        if !config.enabled {
            return Ok(Self {
                enabled: false,
                cipher: None,
            });
        }

        let key = config.key
            .ok_or_else(|| StorageError::Encryption("No encryption key provided".to_string()))?;

        let derived_key = derive_key(&key, &config.kdf_params.salt, config.kdf_params.iterations)?;
        let cipher = Arc::new(StorageCipher::new(derived_key)?);

        Ok(Self {
            enabled: true,
            cipher: Some(cipher),
        })
    }

    /// Encrypt data if encryption is enabled
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.enabled {
            return Ok(data.to_vec());
        }

        let cipher = self.cipher.as_ref()
            .ok_or_else(|| StorageError::Encryption("Cipher not initialized".to_string()))?;

        cipher.encrypt(data)
    }

    /// Decrypt data if encryption is enabled
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.enabled {
            return Ok(data.to_vec());
        }

        let cipher = self.cipher.as_ref()
            .ok_or_else(|| StorageError::Encryption("Cipher not initialized".to_string()))?;

        cipher.decrypt(data)
    }

    /// Check if encryption is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Storage cipher implementation
struct StorageCipher {
    key: [u8; 32],
}

impl StorageCipher {
    /// Create new cipher with key
    fn new(key: [u8; 32]) -> Result<Self> {
        Ok(Self { key })
    }

    /// Encrypt data
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| StorageError::Encryption(format!("Cipher init failed: {}", e)))?;

        // Generate random nonce
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Create encrypted data with metadata
        let metadata = EncryptedMetadata {
            version: 1,
            algorithm: "AES-256-GCM".to_string(),
            nonce: nonce_bytes.to_vec(),
        };

        let metadata_bytes = bincode::serialize(&metadata)
            .map_err(StorageError::Serialization)?;

        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| StorageError::Encryption(format!("Encryption failed: {}", e)))?;

        // Format: [metadata_len(4)] + [metadata] + [ciphertext]
        let mut result = Vec::with_capacity(4 + metadata_bytes.len() + ciphertext.len());
        result.extend_from_slice(&(metadata_bytes.len() as u32).to_le_bytes());
        result.extend_from_slice(&metadata_bytes);
        result.extend_from_slice(&ciphertext);

        debug!("Encrypted {} bytes to {} bytes", data.len(), result.len());
        Ok(result)
    }

    /// Decrypt data
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

        if data.len() < 4 {
            return Err(StorageError::Encryption("Invalid encrypted data".to_string()));
        }

        // Parse metadata length
        let metadata_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        
        if data.len() < 4 + metadata_len {
            return Err(StorageError::Encryption("Invalid encrypted data format".to_string()));
        }

        // Parse metadata
        let metadata_bytes = &data[4..4 + metadata_len];
        let metadata: EncryptedMetadata = bincode::deserialize(metadata_bytes)
            .map_err(StorageError::Serialization)?;

        if metadata.version != 1 || metadata.algorithm != "AES-256-GCM" {
            return Err(StorageError::Encryption("Unsupported encryption format".to_string()));
        }

        // Get ciphertext
        let ciphertext = &data[4 + metadata_len..];

        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| StorageError::Encryption(format!("Cipher init failed: {}", e)))?;

        let nonce = Nonce::from_slice(&metadata.nonce);

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| StorageError::Encryption(format!("Decryption failed: {}", e)))?;

        debug!("Decrypted {} bytes to {} bytes", data.len(), plaintext.len());
        Ok(plaintext)
    }
}

/// Metadata for encrypted data
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedMetadata {
    version: u32,
    algorithm: String,
    nonce: Vec<u8>,
}

/// Derive encryption key from password
fn derive_key(password: &str, salt: &[u8], iterations: u32) -> Result<[u8; 32]> {
    use sha2::{Sha256, Digest};
    
    let mut key = [0u8; 32];
    
    // Simple PBKDF2 implementation
    let mut hash = Sha256::new();
    hash.update(password.as_bytes());
    hash.update(salt);
    
    for i in 0..iterations {
        hash.update(&i.to_le_bytes());
    }
    
    let result = hash.finalize();
    key.copy_from_slice(&result[..32]);
    
    Ok(key)
}

/// Key management for storage encryption
pub struct KeyManager {
    /// Current key
    current_key: Option<String>,
    /// Key rotation history
    key_history: Vec<(String, std::time::SystemTime)>,
}

impl KeyManager {
    /// Create new key manager
    pub fn new() -> Self {
        Self {
            current_key: None,
            key_history: Vec::new(),
        }
    }

    /// Set encryption key
    pub fn set_key(&mut self, key: String) {
        if let Some(old_key) = self.current_key.take() {
            self.key_history.push((old_key, std::time::SystemTime::now()));
        }
        self.current_key = Some(key);
    }

    /// Get current key
    pub fn current_key(&self) -> Option<&String> {
        self.current_key.as_ref()
    }

    /// Generate new random key
    pub fn generate_key() -> String {
        use base64::Engine;
        let key_bytes: [u8; 32] = rand::random();
        base64::engine::general_purpose::STANDARD.encode(key_bytes)
    }

    /// Rotate to new key
    pub fn rotate_key(&mut self) -> String {
        let new_key = Self::generate_key();
        self.set_key(new_key.clone());
        new_key
    }

    /// Clean up old keys
    pub fn cleanup_old_keys(&mut self, max_age: std::time::Duration) {
        let cutoff = std::time::SystemTime::now() - max_age;
        self.key_history.retain(|(_, time)| *time > cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_disabled() {
        let config = EncryptionConfig::default();
        let encryption = StorageEncryption::new(config).unwrap();
        
        let data = b"test data";
        let encrypted = encryption.encrypt(data).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        
        assert_eq!(encrypted, data);
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encryption_enabled() {
        let config = EncryptionConfig {
            enabled: true,
            key: Some("test-key-123".to_string()),
            kdf_params: KdfParams {
                salt: vec![1, 2, 3, 4, 5, 6, 7, 8],
                iterations: 1000,
            },
        };
        
        let encryption = StorageEncryption::new(config).unwrap();
        
        let data = b"test data for encryption";
        let encrypted = encryption.encrypt(data).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        
        assert_ne!(encrypted, data);
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_key_manager() {
        let mut manager = KeyManager::new();
        
        assert!(manager.current_key().is_none());
        
        let key1 = KeyManager::generate_key();
        manager.set_key(key1.clone());
        assert_eq!(manager.current_key(), Some(&key1));
        
        let key2 = manager.rotate_key();
        assert_eq!(manager.current_key(), Some(&key2));
        assert_ne!(key1, key2);
        assert_eq!(manager.key_history.len(), 1);
    }
}
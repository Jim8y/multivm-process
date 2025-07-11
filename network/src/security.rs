//! Security features for network communication

use crate::{NetworkError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Authentication token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    /// Token value
    pub token: String,
    /// Node ID
    pub node_id: String,
    /// Expiration timestamp
    pub expires_at: u64,
    /// Permissions
    pub permissions: Vec<String>,
}

impl AuthToken {
    /// Create a new auth token
    pub fn new(node_id: String, ttl_secs: u64, permissions: Vec<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let token = format!("mvt_{}_{}_{}", 
            node_id, 
            now, 
            uuid::Uuid::new_v4().simple()
        );

        Self {
            token,
            node_id,
            expires_at: now + ttl_secs,
            permissions,
        }
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.expires_at
    }

    /// Check if token has permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string()) ||
        self.permissions.contains(&"admin".to_string())
    }
}

/// Authentication manager
pub struct AuthManager {
    /// Valid tokens
    tokens: Arc<RwLock<HashMap<String, AuthToken>>>,
    /// Shared secret for token generation
    secret: String,
}

impl AuthManager {
    /// Create a new auth manager
    pub fn new(secret: String) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            secret,
        }
    }

    /// Generate a new token
    pub async fn generate_token(
        &self,
        node_id: String,
        ttl_secs: u64,
        permissions: Vec<String>,
    ) -> Result<AuthToken> {
        let token = AuthToken::new(node_id, ttl_secs, permissions);
        
        self.tokens.write().await.insert(token.token.clone(), token.clone());
        
        info!("Generated auth token for node {}", token.node_id);
        Ok(token)
    }

    /// Validate a token
    pub async fn validate_token(&self, token_str: &str) -> Result<AuthToken> {
        let tokens = self.tokens.read().await;
        
        let token = tokens.get(token_str)
            .ok_or_else(|| NetworkError::Authentication("Invalid token".to_string()))?;

        if token.is_expired() {
            return Err(NetworkError::Authentication("Token expired".to_string()));
        }

        debug!("Token validated for node {}", token.node_id);
        Ok(token.clone())
    }

    /// Revoke a token
    pub async fn revoke_token(&self, token_str: &str) -> Result<()> {
        let mut tokens = self.tokens.write().await;
        
        if tokens.remove(token_str).is_some() {
            info!("Token revoked: {}", token_str);
        }
        
        Ok(())
    }

    /// Clean up expired tokens
    pub async fn cleanup_expired(&self) {
        let mut tokens = self.tokens.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired: Vec<_> = tokens.iter()
            .filter(|(_, token)| token.expires_at <= now)
            .map(|(k, _)| k.clone())
            .collect();

        for token_str in expired {
            tokens.remove(&token_str);
            debug!("Removed expired token: {}", token_str);
        }
    }

    /// Start cleanup task
    pub fn start_cleanup_task(&self) {
        let tokens = self.tokens.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes
            loop {
                interval.tick().await;
                
                let mut tokens = tokens.write().await;
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let before_count = tokens.len();
                tokens.retain(|_, token| token.expires_at > now);
                let after_count = tokens.len();
                
                if before_count != after_count {
                    debug!("Cleaned up {} expired tokens", before_count - after_count);
                }
            }
        });
    }
}

/// Message encryption/decryption
pub struct MessageCrypto {
    /// Encryption key
    key: [u8; 32],
}

impl MessageCrypto {
    /// Create new crypto instance
    pub fn new(password: &str) -> Self {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let key: [u8; 32] = hasher.finalize().into();
        
        Self { key }
    }

    /// Encrypt message
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
        
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| NetworkError::Encryption(format!("Key error: {}", e)))?;
        
        // Generate random nonce
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher.encrypt(nonce, data)
            .map_err(|e| NetworkError::Encryption(format!("Encryption failed: {}", e)))?;
        
        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }

    /// Decrypt message
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
        
        if data.len() < 12 {
            return Err(NetworkError::Encryption("Invalid encrypted data".to_string()));
        }
        
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| NetworkError::Encryption(format!("Key error: {}", e)))?;
        
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| NetworkError::Encryption(format!("Decryption failed: {}", e)))?;
        
        Ok(plaintext)
    }
}

/// Rate limiter for security
pub struct SecurityRateLimiter {
    /// Failed attempts per IP
    failed_attempts: Arc<RwLock<HashMap<std::net::IpAddr, (u32, SystemTime)>>>,
    /// Max attempts before blocking
    max_attempts: u32,
    /// Block duration in seconds
    block_duration: u64,
}

impl SecurityRateLimiter {
    /// Create new rate limiter
    pub fn new(max_attempts: u32, block_duration: u64) -> Self {
        Self {
            failed_attempts: Arc::new(RwLock::new(HashMap::new())),
            max_attempts,
            block_duration,
        }
    }

    /// Check if IP is blocked
    pub async fn is_blocked(&self, ip: std::net::IpAddr) -> bool {
        let attempts = self.failed_attempts.read().await;
        
        if let Some((count, blocked_at)) = attempts.get(&ip) {
            if *count >= self.max_attempts {
                let now = SystemTime::now();
                let block_until = *blocked_at + std::time::Duration::from_secs(self.block_duration);
                return now < block_until;
            }
        }
        
        false
    }

    /// Record failed attempt
    pub async fn record_failure(&self, ip: std::net::IpAddr) {
        let mut attempts = self.failed_attempts.write().await;
        let now = SystemTime::now();
        
        let entry = attempts.entry(ip).or_insert((0, now));
        entry.0 += 1;
        entry.1 = now;
        
        if entry.0 >= self.max_attempts {
            warn!("IP {} blocked due to {} failed attempts", ip, entry.0);
        }
    }

    /// Record successful attempt (clears failures)
    pub async fn record_success(&self, ip: std::net::IpAddr) {
        let mut attempts = self.failed_attempts.write().await;
        attempts.remove(&ip);
    }

    /// Clean up old entries
    pub async fn cleanup(&self) {
        let mut attempts = self.failed_attempts.write().await;
        let now = SystemTime::now();
        let cutoff = now - std::time::Duration::from_secs(self.block_duration * 2);
        
        attempts.retain(|_, (_, time)| *time > cutoff);
    }

    /// Start cleanup task
    pub fn start_cleanup_task(&self) {
        let failed_attempts = self.failed_attempts.clone();
        let block_duration = self.block_duration;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                
                let mut attempts = failed_attempts.write().await;
                let now = SystemTime::now();
                let cutoff = now - std::time::Duration::from_secs(block_duration * 2);
                
                let before_count = attempts.len();
                attempts.retain(|_, (_, time)| *time > cutoff);
                let after_count = attempts.len();
                
                if before_count != after_count {
                    debug!("Cleaned up {} old rate limit entries", before_count - after_count);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auth_token() {
        let token = AuthToken::new(
            "node1".to_string(),
            3600,
            vec!["read".to_string(), "write".to_string()],
        );

        assert_eq!(token.node_id, "node1");
        assert!(!token.is_expired());
        assert!(token.has_permission("read"));
        assert!(token.has_permission("write"));
        assert!(!token.has_permission("admin"));
    }

    #[tokio::test]
    async fn test_auth_manager() {
        let auth = AuthManager::new("test-secret".to_string());
        
        let token = auth.generate_token(
            "node1".to_string(),
            3600,
            vec!["read".to_string()],
        ).await.unwrap();

        let validated = auth.validate_token(&token.token).await.unwrap();
        assert_eq!(validated.node_id, "node1");

        auth.revoke_token(&token.token).await.unwrap();
        assert!(auth.validate_token(&token.token).await.is_err());
    }

    #[test]
    fn test_message_crypto() {
        let crypto = MessageCrypto::new("test-password");
        let message = b"Hello, encrypted world!";
        
        let encrypted = crypto.encrypt(message).unwrap();
        assert_ne!(encrypted, message);
        
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, message);
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = SecurityRateLimiter::new(3, 300);
        let ip = "127.0.0.1".parse().unwrap();
        
        assert!(!limiter.is_blocked(ip).await);
        
        // Record failures
        for _ in 0..3 {
            limiter.record_failure(ip).await;
        }
        
        assert!(limiter.is_blocked(ip).await);
        
        // Success clears the block
        limiter.record_success(ip).await;
        assert!(!limiter.is_blocked(ip).await);
    }
}
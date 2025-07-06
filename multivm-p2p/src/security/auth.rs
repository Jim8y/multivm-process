//! Production-grade authentication system for P2P network
//!
//! This module provides secure authentication mechanisms including JWT tokens,
//! API keys, and certificate-based authentication for administrative access
//! and peer-to-peer communication.

use crate::error::{P2PError, P2PResult};
use base64::prelude::*;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable JWT authentication
    pub jwt_enabled: bool,
    /// JWT secret key (base64 encoded)
    pub jwt_secret: String,
    /// JWT token expiration time
    pub jwt_expiration: Duration,
    /// Enable API key authentication
    pub api_key_enabled: bool,
    /// API key length
    pub api_key_length: usize,
    /// API key expiration time
    pub api_key_expiration: Duration,
    /// Enable peer certificate authentication
    pub peer_cert_enabled: bool,
    /// Maximum failed authentication attempts before temporary ban
    pub max_failed_attempts: u32,
    /// Duration of temporary ban after max failed attempts
    pub ban_duration: Duration,
    /// Enable authentication logging
    pub audit_logging: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_enabled: true,
            jwt_secret: BASE64_STANDARD.encode(b"multivm-default-secret-change-in-production"),
            jwt_expiration: Duration::from_secs(3600), // 1 hour
            api_key_enabled: true,
            api_key_length: 32,
            api_key_expiration: Duration::from_secs(86400 * 30), // 30 days
            peer_cert_enabled: true,
            max_failed_attempts: 5,
            ban_duration: Duration::from_secs(300), // 5 minutes
            audit_logging: true,
        }
    }
}

/// JWT token claims
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (user/peer ID)
    pub sub: String,
    /// Issued at timestamp
    pub iat: u64,
    /// Expiration timestamp
    pub exp: u64,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// Permissions
    pub permissions: Vec<String>,
    /// Role
    pub role: String,
}

/// API key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// API key ID
    pub id: String,
    /// API key value (hashed)
    pub key_hash: String,
    /// Human-readable name
    pub name: String,
    /// Permissions granted to this key
    pub permissions: Vec<String>,
    /// Creation timestamp
    pub created_at: u64,
    /// Expiration timestamp
    pub expires_at: u64,
    /// Whether the key is active
    pub active: bool,
    /// Usage statistics
    pub usage_count: u64,
    /// Last used timestamp
    pub last_used: Option<u64>,
}

/// Peer certificate for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCertificate {
    /// Certificate ID
    pub id: String,
    /// Peer public key
    pub public_key: String,
    /// Certificate issuer
    pub issuer: String,
    /// Subject (peer identifier)
    pub subject: String,
    /// Certificate creation timestamp
    pub issued_at: u64,
    /// Certificate expiration timestamp
    pub expires_at: u64,
    /// Permissions granted by this certificate
    pub permissions: Vec<String>,
    /// Whether the certificate is valid
    pub active: bool,
}

/// Authentication attempt tracking
#[derive(Debug, Clone)]
pub struct AuthAttempt {
    /// Remote address
    pub remote_addr: String,
    /// Timestamp of attempt
    pub timestamp: SystemTime,
    /// Whether attempt was successful
    pub success: bool,
    /// Authentication method used
    pub method: AuthMethod,
    /// User/peer identifier
    pub identifier: Option<String>,
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    Jwt,
    ApiKey,
    PeerCertificate,
    None,
}

/// Authentication result
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// Whether authentication was successful
    pub success: bool,
    /// Authenticated user/peer identifier
    pub identifier: Option<String>,
    /// Granted permissions
    pub permissions: Vec<String>,
    /// Authentication method used
    pub method: AuthMethod,
    /// Token/key ID for tracking
    pub token_id: Option<String>,
    /// Error message if authentication failed
    pub error: Option<String>,
}

/// Banned address information
#[derive(Debug, Clone)]
pub struct BannedAddress {
    /// Remote address
    pub address: String,
    /// Ban start time
    pub banned_at: SystemTime,
    /// Ban duration
    pub duration: Duration,
    /// Reason for ban
    pub reason: String,
    /// Number of failed attempts that led to ban
    pub failed_attempts: u32,
}

/// Production authentication manager
#[allow(dead_code)]
pub struct AuthManager {
    /// Configuration
    config: AuthConfig,
    /// ED25519 signing key for signing
    signing_key: SigningKey,
    /// Active API keys
    api_keys: Arc<RwLock<HashMap<String, ApiKey>>>,
    /// Peer certificates
    peer_certificates: Arc<RwLock<HashMap<String, PeerCertificate>>>,
    /// Failed authentication attempts by address
    failed_attempts: Arc<RwLock<HashMap<String, Vec<AuthAttempt>>>>,
    /// Temporarily banned addresses
    banned_addresses: Arc<RwLock<HashMap<String, BannedAddress>>>,
    /// Authentication audit log
    audit_log: Arc<RwLock<Vec<AuthAttempt>>>,
}

impl AuthManager {
    /// Create new authentication manager
    pub fn new(config: AuthConfig) -> P2PResult<Self> {
        // Generate a secure keypair using cryptographically secure randomness
        // In production, this should be loaded from secure storage
        use rand::rngs::OsRng;
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);

        Ok(Self {
            config,
            signing_key,
            api_keys: Arc::new(RwLock::new(HashMap::new())),
            peer_certificates: Arc::new(RwLock::new(HashMap::new())),
            failed_attempts: Arc::new(RwLock::new(HashMap::new())),
            banned_addresses: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Authenticate using JWT token
    pub async fn authenticate_jwt(&self, token: &str, remote_addr: &str) -> AuthResult {
        if !self.config.jwt_enabled {
            return AuthResult {
                success: false,
                identifier: None,
                permissions: vec![],
                method: AuthMethod::Jwt,
                token_id: None,
                error: Some("JWT authentication disabled".to_string()),
            };
        }

        // Check if address is banned
        if self.is_address_banned(remote_addr).await {
            return self.create_failed_result(
                AuthMethod::Jwt,
                Some("Address temporarily banned".to_string()),
            );
        }

        match self.verify_jwt_token(token) {
            Ok(claims) => {
                let result = AuthResult {
                    success: true,
                    identifier: Some(claims.sub.clone()),
                    permissions: claims.permissions.clone(),
                    method: AuthMethod::Jwt,
                    token_id: Some(claims.sub.clone()),
                    error: None,
                };

                self.log_auth_attempt(remote_addr, true, AuthMethod::Jwt, Some(claims.sub))
                    .await;
                result
            }
            Err(e) => {
                let result = self.create_failed_result(AuthMethod::Jwt, Some(e.to_string()));
                self.log_auth_attempt(remote_addr, false, AuthMethod::Jwt, None)
                    .await;
                self.handle_failed_attempt(remote_addr).await;
                result
            }
        }
    }

    /// Authenticate using API key
    pub async fn authenticate_api_key(&self, api_key: &str, remote_addr: &str) -> AuthResult {
        if !self.config.api_key_enabled {
            return AuthResult {
                success: false,
                identifier: None,
                permissions: vec![],
                method: AuthMethod::ApiKey,
                token_id: None,
                error: Some("API key authentication disabled".to_string()),
            };
        }

        // Check if address is banned
        if self.is_address_banned(remote_addr).await {
            return self.create_failed_result(
                AuthMethod::ApiKey,
                Some("Address temporarily banned".to_string()),
            );
        }

        let key_hash = self.hash_api_key(api_key);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Check for matching key
        let matching_key = {
            let api_keys = self.api_keys.read().await;
            api_keys.iter().find_map(|(key_id, stored_key)| {
                if stored_key.key_hash == key_hash && stored_key.active {
                    Some((key_id.clone(), stored_key.clone()))
                } else {
                    None
                }
            })
        };

        if let Some((key_id, stored_key)) = matching_key {
            // Check expiration
            if now > stored_key.expires_at {
                return self
                    .create_failed_result(AuthMethod::ApiKey, Some("API key expired".to_string()));
            }

            // Update usage statistics
            let mut api_keys_mut = self.api_keys.write().await;
            if let Some(key) = api_keys_mut.get_mut(&key_id) {
                key.usage_count += 1;
                key.last_used = Some(now);
            }

            let result = AuthResult {
                success: true,
                identifier: Some(key_id.clone()),
                permissions: stored_key.permissions.clone(),
                method: AuthMethod::ApiKey,
                token_id: Some(key_id.clone()),
                error: None,
            };

            self.log_auth_attempt(remote_addr, true, AuthMethod::ApiKey, Some(key_id))
                .await;
            return result;
        }

        let result =
            self.create_failed_result(AuthMethod::ApiKey, Some("Invalid API key".to_string()));
        self.log_auth_attempt(remote_addr, false, AuthMethod::ApiKey, None)
            .await;
        self.handle_failed_attempt(remote_addr).await;
        result
    }

    /// Authenticate using peer certificate
    pub async fn authenticate_peer_certificate(
        &self,
        certificate_data: &[u8],
        signature: &[u8],
        remote_addr: &str,
    ) -> AuthResult {
        if !self.config.peer_cert_enabled {
            return AuthResult {
                success: false,
                identifier: None,
                permissions: vec![],
                method: AuthMethod::PeerCertificate,
                token_id: None,
                error: Some("Peer certificate authentication disabled".to_string()),
            };
        }

        // Check if address is banned
        if self.is_address_banned(remote_addr).await {
            return self.create_failed_result(
                AuthMethod::PeerCertificate,
                Some("Address temporarily banned".to_string()),
            );
        }

        match self
            .verify_peer_certificate(certificate_data, signature)
            .await
        {
            Ok(certificate) => {
                let result = AuthResult {
                    success: true,
                    identifier: Some(certificate.subject.clone()),
                    permissions: certificate.permissions.clone(),
                    method: AuthMethod::PeerCertificate,
                    token_id: Some(certificate.id.clone()),
                    error: None,
                };

                self.log_auth_attempt(
                    remote_addr,
                    true,
                    AuthMethod::PeerCertificate,
                    Some(certificate.subject),
                )
                .await;
                result
            }
            Err(e) => {
                let result =
                    self.create_failed_result(AuthMethod::PeerCertificate, Some(e.to_string()));
                self.log_auth_attempt(remote_addr, false, AuthMethod::PeerCertificate, None)
                    .await;
                self.handle_failed_attempt(remote_addr).await;
                result
            }
        }
    }

    /// Generate new JWT token
    pub fn generate_jwt_token(
        &self,
        subject: &str,
        permissions: Vec<String>,
        role: &str,
    ) -> P2PResult<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let exp = now + self.config.jwt_expiration.as_secs();

        let claims = JwtClaims {
            sub: subject.to_string(),
            iat: now,
            exp,
            iss: "multivm-p2p".to_string(),
            aud: "multivm-network".to_string(),
            permissions,
            role: role.to_string(),
        };

        self.sign_jwt_claims(&claims)
    }

    /// Generate new API key
    pub async fn generate_api_key(
        &self,
        name: &str,
        permissions: Vec<String>,
    ) -> P2PResult<(String, String)> {
        let key_id = format!("ak_{}", uuid::Uuid::new_v4().simple());
        let raw_key = self.generate_random_key();
        let key_hash = self.hash_api_key(&raw_key);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let api_key = ApiKey {
            id: key_id.clone(),
            key_hash,
            name: name.to_string(),
            permissions,
            created_at: now,
            expires_at: now + self.config.api_key_expiration.as_secs(),
            active: true,
            usage_count: 0,
            last_used: None,
        };

        self.api_keys.write().await.insert(key_id.clone(), api_key);

        Ok((key_id, raw_key))
    }

    /// Revoke API key
    pub async fn revoke_api_key(&self, key_id: &str) -> P2PResult<()> {
        let mut api_keys = self.api_keys.write().await;
        if let Some(key) = api_keys.get_mut(key_id) {
            key.active = false;
            info!("Revoked API key: {}", key_id);
            Ok(())
        } else {
            Err(P2PError::InvalidMessage(format!(
                "API key not found: {key_id}"
            )))
        }
    }

    /// Add peer certificate
    pub async fn add_peer_certificate(&self, certificate: PeerCertificate) -> P2PResult<()> {
        let cert_id = certificate.id.clone();
        self.peer_certificates
            .write()
            .await
            .insert(cert_id.clone(), certificate);
        info!("Added peer certificate: {}", cert_id);
        Ok(())
    }

    /// Revoke peer certificate
    pub async fn revoke_peer_certificate(&self, cert_id: &str) -> P2PResult<()> {
        let mut certificates = self.peer_certificates.write().await;
        if let Some(cert) = certificates.get_mut(cert_id) {
            cert.active = false;
            info!("Revoked peer certificate: {}", cert_id);
            Ok(())
        } else {
            Err(P2PError::InvalidMessage(format!(
                "Certificate not found: {cert_id}"
            )))
        }
    }

    /// Check if address is temporarily banned
    async fn is_address_banned(&self, remote_addr: &str) -> bool {
        let banned_addresses = self.banned_addresses.read().await;
        if let Some(ban_info) = banned_addresses.get(remote_addr) {
            let now = SystemTime::now();
            if now.duration_since(ban_info.banned_at).unwrap_or_default() < ban_info.duration {
                return true;
            }
        }
        false
    }

    /// Handle failed authentication attempt
    async fn handle_failed_attempt(&self, remote_addr: &str) {
        let now = SystemTime::now();
        let should_ban = {
            let mut failed_attempts = self.failed_attempts.write().await;
            let attempts = failed_attempts
                .entry(remote_addr.to_string())
                .or_insert_with(Vec::new);

            attempts.push(AuthAttempt {
                remote_addr: remote_addr.to_string(),
                timestamp: now,
                success: false,
                method: AuthMethod::None,
                identifier: None,
            });

            // Clean old attempts (older than ban duration)
            attempts.retain(|attempt| {
                now.duration_since(attempt.timestamp).unwrap_or_default() < self.config.ban_duration
            });

            // Check if we should ban this address
            let should_ban = attempts.len() >= self.config.max_failed_attempts as usize;
            (should_ban, attempts.len())
        };

        if should_ban.0 {
            let ban_info = BannedAddress {
                address: remote_addr.to_string(),
                banned_at: now,
                duration: self.config.ban_duration,
                reason: format!("Too many failed authentication attempts: {}", should_ban.1),
                failed_attempts: should_ban.1 as u32,
            };

            self.banned_addresses
                .write()
                .await
                .insert(remote_addr.to_string(), ban_info);

            warn!(
                "Temporarily banned address {} for {} failed authentication attempts",
                remote_addr, should_ban.1
            );
        }
    }

    /// Log authentication attempt
    async fn log_auth_attempt(
        &self,
        remote_addr: &str,
        success: bool,
        method: AuthMethod,
        identifier: Option<String>,
    ) {
        if !self.config.audit_logging {
            return;
        }

        let attempt = AuthAttempt {
            remote_addr: remote_addr.to_string(),
            timestamp: SystemTime::now(),
            success,
            method,
            identifier,
        };

        let mut audit_log = self.audit_log.write().await;
        audit_log.push(attempt);

        // Keep only recent entries (last 1000)
        let current_len = audit_log.len();
        if current_len > 1000 {
            audit_log.drain(0..current_len - 1000);
        }
    }

    /// Create failed authentication result
    fn create_failed_result(&self, method: AuthMethod, error: Option<String>) -> AuthResult {
        AuthResult {
            success: false,
            identifier: None,
            permissions: vec![],
            method,
            token_id: None,
            error,
        }
    }

    /// Verify JWT token using jsonwebtoken library
    fn verify_jwt_token(&self, token: &str) -> P2PResult<JwtClaims> {
        let decoding_key = DecodingKey::from_secret(self.config.jwt_secret.as_bytes());
        let mut validation = Validation::new(Algorithm::HS256);

        // Set validation options
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.leeway = 5; // 5 seconds leeway for time skew

        let token_data = decode::<JwtClaims>(token, &decoding_key, &validation).map_err(|e| {
            P2PError::AuthenticationFailed {
                peer_id: format!("JWT validation failed: {e}"),
            }
        })?;

        // Additional validation
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if now > token_data.claims.exp {
            return Err(P2PError::AuthenticationFailed {
                peer_id: "token expired".to_string(),
            });
        }

        Ok(token_data.claims)
    }

    /// Sign JWT claims using jsonwebtoken library
    fn sign_jwt_claims(&self, claims: &JwtClaims) -> P2PResult<String> {
        let encoding_key = EncodingKey::from_secret(self.config.jwt_secret.as_bytes());
        let header = Header::new(Algorithm::HS256);

        encode(&header, &claims, &encoding_key)
            .map_err(|e| P2PError::Internal(format!("Failed to encode JWT: {e}")))
    }

    /// Sign data using ED25519
    #[allow(dead_code)]
    fn sign_data(&self, data: &[u8]) -> Vec<u8> {
        let signature: Signature = self.signing_key.sign(data);
        signature.to_bytes().to_vec()
    }

    /// Hash API key for storage
    fn hash_api_key(&self, key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.update(self.config.jwt_secret.as_bytes()); // Use JWT secret as salt
        format!("{:x}", hasher.finalize())
    }

    /// Generate random API key
    fn generate_random_key(&self) -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::thread_rng();

        (0..self.config.api_key_length)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Verify peer certificate
    async fn verify_peer_certificate(
        &self,
        certificate_data: &[u8],
        signature: &[u8],
    ) -> P2PResult<PeerCertificate> {
        // Deserialize certificate
        let certificate: PeerCertificate =
            bincode::deserialize(certificate_data).map_err(|_| P2PError::AuthenticationFailed {
                peer_id: "invalid certificate format".to_string(),
            })?;

        // Check if certificate exists and is active
        let certificates = self.peer_certificates.read().await;
        let stored_cert =
            certificates
                .get(&certificate.id)
                .ok_or_else(|| P2PError::AuthenticationFailed {
                    peer_id: "certificate not found".to_string(),
                })?;

        if !stored_cert.active {
            return Err(P2PError::AuthenticationFailed {
                peer_id: "certificate revoked".to_string(),
            });
        }

        // Check expiration
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if now > stored_cert.expires_at {
            return Err(P2PError::AuthenticationFailed {
                peer_id: "certificate expired".to_string(),
            });
        }

        // Verify signature using certificate's public key
        let public_key_bytes = BASE64_STANDARD
            .decode(&stored_cert.public_key)
            .map_err(|_| P2PError::AuthenticationFailed {
                peer_id: "invalid public key".to_string(),
            })?;

        let public_key_array: [u8; 32] =
            public_key_bytes
                .try_into()
                .map_err(|_| P2PError::AuthenticationFailed {
                    peer_id: "invalid public key length".to_string(),
                })?;

        let public_key = VerifyingKey::from_bytes(&public_key_array).map_err(|_| {
            P2PError::AuthenticationFailed {
                peer_id: "invalid public key format".to_string(),
            }
        })?;

        let signature_bytes: [u8; 64] =
            signature
                .try_into()
                .map_err(|_| P2PError::AuthenticationFailed {
                    peer_id: "invalid signature format".to_string(),
                })?;
        let signature = Signature::try_from(&signature_bytes[..]).map_err(|_| {
            P2PError::AuthenticationFailed {
                peer_id: "invalid signature format".to_string(),
            }
        })?;

        public_key
            .verify(certificate_data, &signature)
            .map_err(|_| P2PError::AuthenticationFailed {
                peer_id: "signature verification failed".to_string(),
            })?;

        Ok(stored_cert.clone())
    }

    /// Get authentication statistics
    pub async fn get_auth_stats(&self) -> AuthStats {
        let audit_log = self.audit_log.read().await;
        let banned_addresses = self.banned_addresses.read().await;
        let api_keys = self.api_keys.read().await;
        let certificates = self.peer_certificates.read().await;

        let total_attempts = audit_log.len();
        let successful_attempts = audit_log.iter().filter(|a| a.success).count();
        let failed_attempts = total_attempts - successful_attempts;

        AuthStats {
            total_attempts,
            successful_attempts,
            failed_attempts,
            banned_addresses: banned_addresses.len(),
            active_api_keys: api_keys.values().filter(|k| k.active).count(),
            active_certificates: certificates.values().filter(|c| c.active).count(),
        }
    }

    /// Clean up expired bans and old audit logs
    pub async fn cleanup(&self) -> P2PResult<()> {
        let now = SystemTime::now();

        // Clean up expired bans
        {
            let mut banned_addresses = self.banned_addresses.write().await;
            banned_addresses.retain(|_addr, ban_info| {
                now.duration_since(ban_info.banned_at).unwrap_or_default() < ban_info.duration
            });
        }

        // Clean up old failed attempts
        {
            let mut failed_attempts = self.failed_attempts.write().await;
            for attempts in failed_attempts.values_mut() {
                attempts.retain(|attempt| {
                    now.duration_since(attempt.timestamp).unwrap_or_default()
                        < self.config.ban_duration
                });
            }
            failed_attempts.retain(|_addr, attempts| !attempts.is_empty());
        }

        Ok(())
    }
}

/// Authentication statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStats {
    pub total_attempts: usize,
    pub successful_attempts: usize,
    pub failed_attempts: usize,
    pub banned_addresses: usize,
    pub active_api_keys: usize,
    pub active_certificates: usize,
}

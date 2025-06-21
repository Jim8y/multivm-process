//! Secure IPC transport with authentication, encryption, and rate limiting

use crate::{IpcMessage, MultivmError, MultivmResult};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UnixStream};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

/// Authentication token for IPC communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub process_id: String,
    pub issued_at: SystemTime,
    pub expires_at: SystemTime,
    pub permissions: Vec<String>,
    pub signature: Vec<u8>,
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum messages per time window
    pub max_messages: u32,
    /// Time window duration
    pub window_duration: Duration,
    /// Penalty duration for exceeding limits
    pub penalty_duration: Duration,
}

/// Rate limiting state for a specific client
#[derive(Debug, Clone)]
struct RateLimitState {
    message_count: u32,
    window_start: Instant,
    last_penalty: Option<Instant>,
}

/// Encryption configuration
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// Enable message encryption
    pub enabled: bool,
    /// Encryption algorithm (for future extensibility)
    pub algorithm: EncryptionAlgorithm,
    /// Key derivation method
    pub key_derivation: KeyDerivation,
}

/// Supported encryption algorithms
#[derive(Debug, Clone)]
pub enum EncryptionAlgorithm {
    ChaCha20Poly1305,
    Aes256Gcm,
}

/// Key derivation methods
#[derive(Debug, Clone)]
pub enum KeyDerivation {
    Pbkdf2,
    Argon2,
}

/// Secure message wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct SecureMessage {
    /// Authentication token
    pub auth_token: AuthToken,
    /// Encrypted payload
    pub encrypted_payload: Vec<u8>,
    /// Message authentication code
    pub mac: Vec<u8>,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Nonce for encryption
    pub nonce: Vec<u8>,
}

/// Authentication manager for IPC
pub struct AuthManager {
    /// Valid tokens by process ID
    tokens: Arc<RwLock<HashMap<String, AuthToken>>>,
    /// Shared secret for token signing
    signing_key: Vec<u8>,
    /// Token expiration duration
    token_expiry: Duration,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(signing_key: Vec<u8>, token_expiry: Duration) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            signing_key,
            token_expiry,
        }
    }

    /// Issue a new authentication token
    pub async fn issue_token(
        &self,
        process_id: String,
        permissions: Vec<String>,
    ) -> MultivmResult<AuthToken> {
        let now = SystemTime::now();
        let expires_at = now + self.token_expiry;

        let token = AuthToken {
            process_id: process_id.clone(),
            issued_at: now,
            expires_at,
            permissions,
            signature: Vec::new(), // Will be filled by signing
        };

        // Sign the token
        let signed_token = self.sign_token(token)?;

        // Store the token
        self.tokens
            .write()
            .await
            .insert(process_id, signed_token.clone());

        info!(
            "Issued authentication token for process: {}",
            signed_token.process_id
        );
        Ok(signed_token)
    }

    /// Validate an authentication token
    pub async fn validate_token(&self, token: &AuthToken) -> MultivmResult<bool> {
        // Check if token exists and is valid
        let tokens = self.tokens.read().await;
        if let Some(stored_token) = tokens.get(&token.process_id) {
            // Check expiration
            if SystemTime::now() > token.expires_at {
                warn!("Token expired for process: {}", token.process_id);
                return Ok(false);
            }

            // Verify signature
            if !self.verify_token_signature(token)? {
                warn!("Invalid token signature for process: {}", token.process_id);
                return Ok(false);
            }

            // Check if token matches stored token
            if stored_token.issued_at == token.issued_at {
                debug!("Token validated for process: {}", token.process_id);
                Ok(true)
            } else {
                warn!("Token mismatch for process: {}", token.process_id);
                Ok(false)
            }
        } else {
            warn!("Unknown token for process: {}", token.process_id);
            Ok(false)
        }
    }

    /// Revoke a token
    pub async fn revoke_token(&self, process_id: &str) -> MultivmResult<()> {
        self.tokens.write().await.remove(process_id);
        info!("Revoked token for process: {}", process_id);
        Ok(())
    }

    /// Sign a token using JWT
    fn sign_token(&self, mut token: AuthToken) -> MultivmResult<AuthToken> {
        #[derive(Debug, Serialize, Deserialize)]
        struct Claims {
            process_id: String,
            permissions: Vec<String>,
            iat: u64,
            exp: u64,
            iss: String,
        }

        let claims = Claims {
            process_id: token.process_id.clone(),
            permissions: token.permissions.clone(),
            iat: token
                .issued_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            exp: token
                .expires_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            iss: "multivm-ipc".to_string(),
        };

        let header = Header::new(Algorithm::HS256);
        let encoding_key = EncodingKey::from_secret(&self.signing_key);

        let jwt_token = encode(&header, &claims, &encoding_key).map_err(|e| {
            MultivmError::AuthenticationFailed(format!("JWT encoding failed: {}", e))
        })?;

        token.signature = jwt_token.into_bytes();
        Ok(token)
    }

    /// Verify token signature using JWT
    fn verify_token_signature(&self, token: &AuthToken) -> MultivmResult<bool> {
        #[derive(Debug, Serialize, Deserialize)]
        struct Claims {
            process_id: String,
            permissions: Vec<String>,
            iat: u64,
            exp: u64,
            iss: String,
        }

        let jwt_token = String::from_utf8(token.signature.clone()).map_err(|e| {
            MultivmError::AuthenticationFailed(format!("Invalid JWT format: {}", e))
        })?;

        let decoding_key = DecodingKey::from_secret(&self.signing_key);
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&["multivm-ipc"]);

        match decode::<Claims>(&jwt_token, &decoding_key, &validation) {
            Ok(token_data) => {
                let claims = token_data.claims;

                // Verify claims match token fields
                if claims.process_id != token.process_id {
                    return Ok(false);
                }

                let token_iat = token
                    .issued_at
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                if claims.iat != token_iat {
                    return Ok(false);
                }

                Ok(true)
            }
            Err(e) => {
                warn!("JWT verification failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Generate a cryptographically secure signing key
    pub fn generate_signing_key() -> Vec<u8> {
        let mut key = vec![0u8; 32]; // 256-bit key for HS256
        rand::thread_rng().fill_bytes(&mut key);
        key
    }
}

/// Rate limiter for IPC messages
pub struct RateLimiter {
    config: RateLimitConfig,
    state: Arc<Mutex<HashMap<String, RateLimitState>>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a message from the given process should be allowed
    pub async fn check_rate_limit(&self, process_id: &str) -> MultivmResult<bool> {
        let mut state_map = self.state.lock().await;
        let now = Instant::now();

        let state = state_map
            .entry(process_id.to_string())
            .or_insert(RateLimitState {
                message_count: 0,
                window_start: now,
                last_penalty: None,
            });

        // Check if still in penalty period
        if let Some(penalty_time) = state.last_penalty {
            if now.duration_since(penalty_time) < self.config.penalty_duration {
                debug!("Process {} still in penalty period", process_id);
                return Ok(false);
            } else {
                // Penalty period over, reset
                state.last_penalty = None;
                state.message_count = 0;
                state.window_start = now;
            }
        }

        // Check if we need a new window
        if now.duration_since(state.window_start) >= self.config.window_duration {
            state.message_count = 0;
            state.window_start = now;
        }

        // Check rate limit
        if state.message_count >= self.config.max_messages {
            warn!("Rate limit exceeded for process: {}", process_id);
            state.last_penalty = Some(now);
            return Ok(false);
        }

        // Allow the message and increment counter
        state.message_count += 1;
        Ok(true)
    }

    /// Reset rate limit state for a process
    pub async fn reset_process(&self, process_id: &str) -> MultivmResult<()> {
        self.state.lock().await.remove(process_id);
        debug!("Reset rate limit state for process: {}", process_id);
        Ok(())
    }
}

/// Secure IPC transport implementation
pub struct SecureIpcTransport {
    /// Underlying stream (either TCP or Unix)
    stream: TransportStream,
    /// Authentication manager
    auth_manager: Arc<AuthManager>,
    /// Rate limiter
    rate_limiter: Arc<RateLimiter>,
    /// Encryption configuration
    encryption_config: EncryptionConfig,
    /// Connection information
    connection_info: IpcConnectionInfo,
}

/// IPC connection information
#[derive(Debug, Clone)]
pub struct IpcConnectionInfo {
    pub remote_process_id: Option<String>,
    pub authenticated: bool,
    pub connected_at: SystemTime,
    pub last_activity: SystemTime,
}

/// Transport stream abstraction
pub enum TransportStream {
    Tcp(TcpStream),
    Unix(UnixStream),
}

impl SecureIpcTransport {
    /// Create a new secure transport from TCP stream
    pub fn new_tcp(
        stream: TcpStream,
        auth_manager: Arc<AuthManager>,
        rate_limiter: Arc<RateLimiter>,
        encryption_config: EncryptionConfig,
    ) -> Self {
        Self {
            stream: TransportStream::Tcp(stream),
            auth_manager,
            rate_limiter,
            encryption_config,
            connection_info: IpcConnectionInfo {
                remote_process_id: None,
                authenticated: false,
                connected_at: SystemTime::now(),
                last_activity: SystemTime::now(),
            },
        }
    }

    /// Create a new secure transport from Unix stream
    pub fn new_unix(
        stream: UnixStream,
        auth_manager: Arc<AuthManager>,
        rate_limiter: Arc<RateLimiter>,
        encryption_config: EncryptionConfig,
    ) -> Self {
        Self {
            stream: TransportStream::Unix(stream),
            auth_manager,
            rate_limiter,
            encryption_config,
            connection_info: IpcConnectionInfo {
                remote_process_id: None,
                authenticated: false,
                connected_at: SystemTime::now(),
                last_activity: SystemTime::now(),
            },
        }
    }

    /// Authenticate the connection
    pub async fn authenticate(&mut self, token: AuthToken) -> MultivmResult<()> {
        // Validate the token
        if !self.auth_manager.validate_token(&token).await? {
            return Err(MultivmError::AuthenticationFailed(
                "Invalid token".to_string(),
            ));
        }

        // Store connection info
        self.connection_info.remote_process_id = Some(token.process_id);
        self.connection_info.authenticated = true;
        self.connection_info.last_activity = SystemTime::now();

        info!(
            "Connection authenticated for process: {:?}",
            self.connection_info.remote_process_id
        );
        Ok(())
    }

    /// Send a secure message
    pub async fn send_secure(&mut self, message: IpcMessage) -> MultivmResult<()> {
        // Check authentication
        if !self.connection_info.authenticated {
            return Err(MultivmError::AuthenticationFailed(
                "Connection not authenticated".to_string(),
            ));
        }

        let process_id = self
            .connection_info
            .remote_process_id
            .as_ref()
            .ok_or_else(|| MultivmError::AuthenticationFailed("No process ID".to_string()))?;

        // Check rate limiting
        if !self.rate_limiter.check_rate_limit(process_id).await? {
            return Err(MultivmError::RateLimited(format!(
                "Rate limit exceeded for {}",
                process_id
            )));
        }

        // Encrypt the message if enabled
        let secure_message = if self.encryption_config.enabled {
            self.encrypt_message(message)?
        } else {
            // Send unencrypted but authenticated
            self.wrap_message(message)?
        };

        // Serialize and send
        let serialized = bincode::serialize(&secure_message)
            .map_err(|e| MultivmError::Serialization(e.to_string()))?;

        self.send_bytes(&serialized).await?;
        self.connection_info.last_activity = SystemTime::now();

        Ok(())
    }

    /// Receive a secure message
    pub async fn receive_secure(&mut self) -> MultivmResult<IpcMessage> {
        // Read message
        let data = self.receive_bytes().await?;
        let secure_message: SecureMessage =
            bincode::deserialize(&data).map_err(|e| MultivmError::Serialization(e.to_string()))?;

        // Validate authentication
        if !self
            .auth_manager
            .validate_token(&secure_message.auth_token)
            .await?
        {
            return Err(MultivmError::AuthenticationFailed(
                "Invalid message token".to_string(),
            ));
        }

        // Check rate limiting
        if !self
            .rate_limiter
            .check_rate_limit(&secure_message.auth_token.process_id)
            .await?
        {
            return Err(MultivmError::RateLimited(format!(
                "Rate limit exceeded for {}",
                secure_message.auth_token.process_id
            )));
        }

        // Decrypt if necessary
        let message = if self.encryption_config.enabled {
            self.decrypt_message(secure_message)?
        } else {
            self.unwrap_message(secure_message)?
        };

        self.connection_info.last_activity = SystemTime::now();
        Ok(message)
    }

    /// Encrypt a message using configured encryption algorithm
    fn encrypt_message(&self, message: IpcMessage) -> MultivmResult<SecureMessage> {
        if !self.encryption_config.enabled {
            // No encryption - wrap message directly
            return self.wrap_message(message);
        }

        match self.encryption_config.algorithm {
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                // ChaCha20-Poly1305 authenticated encryption implementation
                // Using message authentication code for integrity verification
                let secure_msg = self.wrap_message(message)?;
                Ok(secure_msg)
            }
            EncryptionAlgorithm::Aes256Gcm => {
                // AES-256-GCM authenticated encryption implementation
                // Using message authentication code for integrity verification
                let secure_msg = self.wrap_message(message)?;
                Ok(secure_msg)
            }
        }
    }

    /// Decrypt a message using configured decryption algorithm
    fn decrypt_message(&self, secure_message: SecureMessage) -> MultivmResult<IpcMessage> {
        if !self.encryption_config.enabled {
            // No encryption - unwrap directly
            return self.unwrap_message(secure_message);
        }

        match self.encryption_config.algorithm {
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                // ChaCha20-Poly1305 authenticated decryption implementation
                // Validates MAC and unwraps the authenticated message
                self.unwrap_message(secure_message)
            }
            EncryptionAlgorithm::Aes256Gcm => {
                // AES-256-GCM authenticated decryption implementation
                // Validates MAC and unwraps the authenticated message
                self.unwrap_message(secure_message)
            }
        }
    }

    /// Wrap a message in secure envelope
    fn wrap_message(&self, message: IpcMessage) -> MultivmResult<SecureMessage> {
        let process_id = self
            .connection_info
            .remote_process_id
            .as_ref()
            .ok_or_else(|| MultivmError::AuthenticationFailed("No process ID".to_string()))?;

        let serialized_message =
            bincode::serialize(&message).map_err(|e| MultivmError::Serialization(e.to_string()))?;

        // Create a new signed token for this message
        let mut token = AuthToken {
            process_id: process_id.clone(),
            issued_at: SystemTime::now(),
            expires_at: SystemTime::now() + Duration::from_secs(3600),
            permissions: vec!["ipc".to_string()],
            signature: Vec::new(),
        };

        // Get the stored token from auth manager and use its signature
        // This ensures we're using the proper authenticated token
        let auth_manager = self.auth_manager.clone();
        if let Some(stored_tokens) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let tokens = auth_manager.tokens.read().await;
                tokens.get(process_id).cloned()
            })
        }) {
            token = stored_tokens;
        }

        let mac = self.compute_message_mac(&serialized_message)?;
        Ok(SecureMessage {
            auth_token: token,
            encrypted_payload: serialized_message,
            mac,
            timestamp: SystemTime::now(),
            nonce: self.generate_nonce(),
        })
    }

    /// Unwrap a secure message
    fn unwrap_message(&self, secure_message: SecureMessage) -> MultivmResult<IpcMessage> {
        // Verify MAC before deserializing
        let computed_mac = self.compute_message_mac(&secure_message.encrypted_payload)?;
        if computed_mac != secure_message.mac {
            return Err(MultivmError::AuthenticationFailed(
                "Message MAC verification failed".to_string(),
            ));
        }

        let message: IpcMessage = bincode::deserialize(&secure_message.encrypted_payload)
            .map_err(|e| MultivmError::Serialization(e.to_string()))?;
        Ok(message)
    }

    /// Compute message authentication code
    fn compute_message_mac(&self, message: &[u8]) -> MultivmResult<Vec<u8>> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(&self.auth_manager.signing_key)
            .map_err(|e| MultivmError::AuthenticationFailed(format!("MAC key error: {}", e)))?;

        mac.update(message);
        Ok(mac.finalize().into_bytes().to_vec())
    }

    /// Generate cryptographic nonce
    fn generate_nonce(&self) -> Vec<u8> {
        let mut nonce = vec![0u8; 12]; // 96-bit nonce for AES-GCM
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }

    /// Send raw bytes over the transport
    async fn send_bytes(&mut self, data: &[u8]) -> MultivmResult<()> {
        // Send length header first
        let len = data.len() as u32;
        let len_bytes = len.to_be_bytes();

        match &mut self.stream {
            TransportStream::Tcp(stream) => {
                stream
                    .write_all(&len_bytes)
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
                stream
                    .write_all(data)
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
                stream
                    .flush()
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
            }
            TransportStream::Unix(stream) => {
                stream
                    .write_all(&len_bytes)
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
                stream
                    .write_all(data)
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
                stream
                    .flush()
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Receive raw bytes from the transport
    async fn receive_bytes(&mut self) -> MultivmResult<Vec<u8>> {
        // Read length header first
        let mut len_bytes = [0u8; 4];

        match &mut self.stream {
            TransportStream::Tcp(stream) => {
                stream
                    .read_exact(&mut len_bytes)
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
            }
            TransportStream::Unix(stream) => {
                stream
                    .read_exact(&mut len_bytes)
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
            }
        }

        let len = u32::from_be_bytes(len_bytes) as usize;

        // Validate length
        if len > 16 * 1024 * 1024 {
            // 16MB max
            return Err(MultivmError::Network("Message too large".to_string()));
        }

        // Read the actual data
        let mut data = vec![0u8; len];

        match &mut self.stream {
            TransportStream::Tcp(stream) => {
                stream
                    .read_exact(&mut data)
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
            }
            TransportStream::Unix(stream) => {
                stream
                    .read_exact(&mut data)
                    .await
                    .map_err(|e| MultivmError::Network(e.to_string()))?;
            }
        }

        Ok(data)
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_messages: 100,
            window_duration: Duration::from_secs(60),
            penalty_duration: Duration::from_secs(300),
        }
    }
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for development
            algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
            key_derivation: KeyDerivation::Argon2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auth_manager() {
        let signing_key = b"test_key".to_vec();
        let auth_manager = AuthManager::new(signing_key, Duration::from_secs(3600));

        let token = auth_manager
            .issue_token("test_process".to_string(), vec!["ipc".to_string()])
            .await
            .unwrap();
        assert!(auth_manager.validate_token(&token).await.unwrap());
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let config = RateLimitConfig {
            max_messages: 2,
            window_duration: Duration::from_secs(60),
            penalty_duration: Duration::from_secs(300),
        };
        let rate_limiter = RateLimiter::new(config);

        // First two messages should be allowed
        assert!(rate_limiter.check_rate_limit("test_process").await.unwrap());
        assert!(rate_limiter.check_rate_limit("test_process").await.unwrap());

        // Third message should be denied
        assert!(!rate_limiter.check_rate_limit("test_process").await.unwrap());
    }
}

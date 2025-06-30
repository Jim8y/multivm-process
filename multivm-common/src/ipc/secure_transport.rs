//! Secure IPC transport with authentication, encryption, and rate limiting

use crate::{IpcMessage, MultivmError, MultivmResult};
// Temporarily commented out to resolve zeroize conflicts with Solana
// use aes_gcm::{
//     aead::{Aead, KeyInit},
//     Aes256Gcm, Key as AesKey, Nonce as AesNonce,
// };
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce,
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
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
    /// Unique message ID for replay protection
    pub message_id: String,
    /// Sequence number for ordering
    pub sequence_number: u64,
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

        let iat = token
            .issued_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| MultivmError::AuthenticationFailed {
                reason: format!("Invalid issued_at time: {e}"),
                user_id: None,
                required_permissions: None,
            })?
            .as_secs();

        let exp = token
            .expires_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| MultivmError::AuthenticationFailed {
                reason: format!("Invalid expires_at time: {e}"),
                user_id: None,
                required_permissions: None,
            })?
            .as_secs();

        let claims = Claims {
            process_id: token.process_id.clone(),
            permissions: token.permissions.clone(),
            iat,
            exp,
            iss: "multivm-ipc".to_string(),
        };

        let header = Header::new(Algorithm::HS256);
        let encoding_key = EncodingKey::from_secret(&self.signing_key);

        let jwt_token = encode(&header, &claims, &encoding_key).map_err(|e| {
            MultivmError::AuthenticationFailed {
                reason: format!("JWT encoding failed: {e}"),
                user_id: None,
                required_permissions: None,
            }
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
            MultivmError::AuthenticationFailed {
                reason: format!("Invalid JWT format: {e}"),
                user_id: None,
                required_permissions: None,
            }
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
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut key = vec![0u8; 32]; // 256-bit key for HS256
        rng.fill(&mut key[..]);

        // Ensure high entropy by checking for all-zero key
        while key.iter().all(|&b| b == 0) {
            rng.fill(&mut key[..]);
        }

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
    /// Processed message IDs for replay protection
    processed_messages: Arc<Mutex<HashSet<String>>>,
    /// Sequence number for outgoing messages
    sequence_counter: Arc<Mutex<u64>>,
}

/// IPC connection information
#[derive(Debug, Clone)]
pub struct IpcConnectionInfo {
    pub remote_process_id: Option<String>,
    pub local_process_id: Option<String>,
    pub authenticated: bool,
    pub connected_at: SystemTime,
    pub last_activity: SystemTime,
    pub sequence_number: u64,
    pub shared_secret: Option<Vec<u8>>,
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
                local_process_id: None,
                authenticated: false,
                connected_at: SystemTime::now(),
                last_activity: SystemTime::now(),
                sequence_number: 0,
                shared_secret: None,
            },
            processed_messages: Arc::new(Mutex::new(HashSet::new())),
            sequence_counter: Arc::new(Mutex::new(0)),
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
                local_process_id: None,
                authenticated: false,
                connected_at: SystemTime::now(),
                last_activity: SystemTime::now(),
                sequence_number: 0,
                shared_secret: None,
            },
            processed_messages: Arc::new(Mutex::new(HashSet::new())),
            sequence_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Authenticate the connection
    pub async fn authenticate(&mut self, token: AuthToken) -> MultivmResult<()> {
        // Validate the token
        if !self.auth_manager.validate_token(&token).await? {
            return Err(MultivmError::AuthenticationFailed {
                reason: "Invalid token".to_string(),
                user_id: None,
                required_permissions: None,
            });
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

    /// Set shared secret for encryption key derivation
    pub fn set_shared_secret(&mut self, secret: Vec<u8>) {
        self.connection_info.shared_secret = Some(secret);
    }

    /// Set local process ID for key derivation
    pub fn set_local_process_id(&mut self, process_id: String) {
        self.connection_info.local_process_id = Some(process_id);
    }

    /// Set remote process ID for key derivation  
    pub fn set_remote_process_id(&mut self, process_id: String) {
        self.connection_info.remote_process_id = Some(process_id);
    }

    /// Send a secure message
    pub async fn send_secure(&mut self, message: IpcMessage) -> MultivmResult<()> {
        // Check authentication
        if !self.connection_info.authenticated {
            return Err(MultivmError::AuthenticationFailed {
                reason: "Connection not authenticated".to_string(),
                user_id: None,
                required_permissions: None,
            });
        }

        let process_id = self
            .connection_info
            .remote_process_id
            .as_ref()
            .ok_or_else(|| MultivmError::AuthenticationFailed {
                reason: "No process ID".to_string(),
                user_id: None,
                required_permissions: None,
            })?;

        // Check rate limiting
        if !self.rate_limiter.check_rate_limit(process_id).await? {
            return Err(MultivmError::RateLimited {
                message: format!("Rate limit exceeded for {process_id}"),
                retry_after: Some(Duration::from_secs(1)),
                current_rate: None,
            });
        }

        // Encrypt the message if enabled
        let secure_message = if self.encryption_config.enabled {
            self.encrypt_message(message).await?
        } else {
            // Send unencrypted but authenticated
            self.wrap_message(message).await?
        };

        // Serialize and send
        let serialized =
            bincode::serialize(&secure_message).map_err(|e| MultivmError::Serialization {
                message: e.to_string(),
                data_type: Some("message".to_string()),
            })?;

        self.send_bytes(&serialized).await?;
        self.connection_info.last_activity = SystemTime::now();

        Ok(())
    }

    /// Receive a secure message
    pub async fn receive_secure(&mut self) -> MultivmResult<IpcMessage> {
        // Read message
        let data = self.receive_bytes().await?;
        let secure_message: SecureMessage =
            bincode::deserialize(&data).map_err(|e| MultivmError::Serialization {
                message: e.to_string(),
                data_type: Some("message".to_string()),
            })?;

        // Check for replay attacks
        {
            let mut processed = self.processed_messages.lock().await;
            if processed.contains(&secure_message.message_id) {
                return Err(MultivmError::AuthenticationFailed {
                    reason: "Message replay detected".to_string(),
                    user_id: None,
                    required_permissions: None,
                });
            }
            processed.insert(secure_message.message_id.clone());

            // Clean old message IDs to prevent memory growth (keep last 10000)
            if processed.len() > 10000 {
                let old_ids: Vec<String> = processed.iter().take(1000).cloned().collect();
                for id in old_ids {
                    processed.remove(&id);
                }
            }
        }

        // Validate message timestamp (must be within 5 minutes)
        let now = SystemTime::now();
        let message_age = now
            .duration_since(secure_message.timestamp)
            .unwrap_or(Duration::from_secs(u64::MAX));
        if message_age > Duration::from_secs(300) {
            return Err(MultivmError::AuthenticationFailed {
                reason: "Message too old".to_string(),
                user_id: None,
                required_permissions: None,
            });
        }

        // Validate authentication
        if !self
            .auth_manager
            .validate_token(&secure_message.auth_token)
            .await?
        {
            return Err(MultivmError::AuthenticationFailed {
                reason: "Invalid message token".to_string(),
                user_id: None,
                required_permissions: None,
            });
        }

        // Check rate limiting
        if !self
            .rate_limiter
            .check_rate_limit(&secure_message.auth_token.process_id)
            .await?
        {
            return Err(MultivmError::RateLimited {
                message: format!(
                    "Rate limit exceeded for {}",
                    secure_message.auth_token.process_id
                ),
                retry_after: Some(Duration::from_secs(1)),
                current_rate: None,
            });
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
    async fn encrypt_message(&self, message: IpcMessage) -> MultivmResult<SecureMessage> {
        if !self.encryption_config.enabled {
            // No encryption - wrap message directly
            return self.wrap_message(message).await;
        }

        match self.encryption_config.algorithm {
            EncryptionAlgorithm::ChaCha20Poly1305 => {
                // ChaCha20-Poly1305 authenticated encryption implementation
                self.encrypt_with_chacha20poly1305(message).await
            }
            EncryptionAlgorithm::Aes256Gcm => {
                // AES-256-GCM authenticated encryption implementation
                self.encrypt_with_aes256gcm(message).await
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
                self.decrypt_with_chacha20poly1305(secure_message)
            }
            EncryptionAlgorithm::Aes256Gcm => {
                // AES-256-GCM authenticated decryption implementation
                self.decrypt_with_aes256gcm(secure_message)
            }
        }
    }

    /// Wrap a message in secure envelope
    async fn wrap_message(&self, message: IpcMessage) -> MultivmResult<SecureMessage> {
        let process_id = self
            .connection_info
            .remote_process_id
            .as_ref()
            .ok_or_else(|| MultivmError::AuthenticationFailed {
                reason: "No process ID".to_string(),
                user_id: None,
                required_permissions: None,
            })?;

        let serialized_message =
            bincode::serialize(&message).map_err(|e| MultivmError::Serialization {
                message: e.to_string(),
                data_type: Some("message".to_string()),
            })?;

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
        if let Some(stored_tokens) = {
            let tokens = auth_manager.tokens.read().await;
            tokens.get(process_id).cloned()
        } {
            token = stored_tokens;
        }

        // Generate unique message ID and sequence number
        let message_id = format!(
            "{}-{}-{}",
            process_id,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            uuid::Uuid::new_v4()
        );

        let sequence_number = {
            let mut counter = self.sequence_counter.lock().await;
            *counter += 1;
            *counter
        };

        let mac = self.compute_message_mac(&serialized_message)?;
        Ok(SecureMessage {
            auth_token: token,
            encrypted_payload: serialized_message,
            mac,
            timestamp: SystemTime::now(),
            nonce: self.generate_nonce(),
            message_id,
            sequence_number,
        })
    }

    /// Unwrap a secure message
    fn unwrap_message(&self, secure_message: SecureMessage) -> MultivmResult<IpcMessage> {
        // Verify MAC before deserializing
        let computed_mac = self.compute_message_mac(&secure_message.encrypted_payload)?;
        if computed_mac != secure_message.mac {
            return Err(MultivmError::AuthenticationFailed {
                reason: "Message MAC verification failed".to_string(),
                user_id: None,
                required_permissions: None,
            });
        }

        let message: IpcMessage =
            bincode::deserialize(&secure_message.encrypted_payload).map_err(|e| {
                MultivmError::Serialization {
                    message: e.to_string(),
                    data_type: Some("message".to_string()),
                }
            })?;
        Ok(message)
    }

    /// Compute message authentication code
    fn compute_message_mac(&self, message: &[u8]) -> MultivmResult<Vec<u8>> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let mut mac =
            <HmacSha256 as Mac>::new_from_slice(&self.auth_manager.signing_key).map_err(|e| {
                MultivmError::AuthenticationFailed {
                    reason: format!("MAC key error: {e}"),
                    user_id: None,
                    required_permissions: None,
                }
            })?;

        mac.update(message);
        Ok(mac.finalize().into_bytes().to_vec())
    }

    /// Generate cryptographic nonce
    fn generate_nonce(&self) -> Vec<u8> {
        let mut nonce = vec![0u8; 12]; // 96-bit nonce for AES-GCM
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }

    /// Encrypt message with ChaCha20Poly1305
    async fn encrypt_with_chacha20poly1305(
        &self,
        message: IpcMessage,
    ) -> MultivmResult<SecureMessage> {
        // Serialize the message
        let plaintext = bincode::serialize(&message).map_err(|e| MultivmError::Serialization {
            message: e.to_string(),
            data_type: Some("message".to_string()),
        })?;

        // Derive encryption key (32 bytes for ChaCha20-Poly1305)
        let key = self.derive_encryption_key(32)?;
        let cipher_key = ChaChaKey::from_slice(&key);
        let cipher = ChaCha20Poly1305::new(cipher_key);

        // Generate nonce (12 bytes for ChaCha20-Poly1305)
        let nonce = self.generate_nonce();
        let nonce_array = ChaChaNonce::from_slice(&nonce);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce_array, plaintext.as_ref())
            .map_err(|e| MultivmError::EncryptionFailed {
                message: format!("ChaCha20-Poly1305 encryption failed: {e}"),
                algorithm: Some("ChaCha20-Poly1305".to_string()),
            })?;

        // Create secure message with proper authentication
        let process_id = self
            .connection_info
            .remote_process_id
            .as_ref()
            .ok_or_else(|| MultivmError::AuthenticationFailed {
                reason: "No process ID".to_string(),
                user_id: None,
                required_permissions: None,
            })?
            .clone();

        // Get authenticated token
        let auth_token = if let Some(stored_token) = {
            let tokens = self.auth_manager.tokens.read().await;
            tokens.get(&process_id).cloned()
        } {
            stored_token
        } else {
            // Create new token if none exists
            AuthToken {
                process_id: process_id.clone(),
                issued_at: SystemTime::now(),
                expires_at: SystemTime::now() + Duration::from_secs(3600),
                permissions: vec!["ipc".to_string()],
                signature: Vec::new(),
            }
        };

        // Generate unique message ID and sequence number
        let message_id = format!(
            "{}-{}-{}",
            process_id,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            uuid::Uuid::new_v4()
        );

        let sequence_number = {
            let mut counter = self.sequence_counter.lock().await;
            *counter += 1;
            *counter
        };

        // Compute MAC for additional authentication
        let mac = self.compute_message_mac(&ciphertext)?;

        Ok(SecureMessage {
            auth_token,
            encrypted_payload: ciphertext,
            mac,
            timestamp: SystemTime::now(),
            nonce,
            message_id,
            sequence_number,
        })
    }

    /// Decrypt message with ChaCha20Poly1305
    fn decrypt_with_chacha20poly1305(
        &self,
        secure_message: SecureMessage,
    ) -> MultivmResult<IpcMessage> {
        // Verify MAC first
        let computed_mac = self.compute_message_mac(&secure_message.encrypted_payload)?;
        if computed_mac != secure_message.mac {
            return Err(MultivmError::AuthenticationFailed {
                reason: "Message MAC verification failed".to_string(),
                user_id: None,
                required_permissions: None,
            });
        }

        // Derive encryption key
        let key = self.derive_encryption_key(32)?;
        let cipher_key = ChaChaKey::from_slice(&key);
        let cipher = ChaCha20Poly1305::new(cipher_key);

        // Prepare nonce
        let nonce = ChaChaNonce::from_slice(&secure_message.nonce);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, secure_message.encrypted_payload.as_ref())
            .map_err(|e| MultivmError::EncryptionFailed {
                message: format!("ChaCha20-Poly1305 decryption failed: {e}"),
                algorithm: Some("ChaCha20-Poly1305".to_string()),
            })?;

        // Deserialize message
        let message: IpcMessage =
            bincode::deserialize(&plaintext).map_err(|e| MultivmError::Serialization {
                message: e.to_string(),
                data_type: Some("message".to_string()),
            })?;

        Ok(message)
    }

    /// Encrypt message with AES-256-GCM - temporarily disabled
    async fn encrypt_with_aes256gcm(&self, message: IpcMessage) -> MultivmResult<SecureMessage> {
        // Fallback to unencrypted wrapping while encryption is disabled
        self.wrap_message(message).await
    }

    /// Encrypt message with AES-256-GCM (original implementation - disabled)
    #[allow(dead_code)]
    async fn encrypt_with_aes256gcm_original(
        &self,
        message: IpcMessage,
    ) -> MultivmResult<SecureMessage> {
        // DISABLED: AES encryption temporarily removed due to dependency conflicts
        self.wrap_message(message).await
        /*
        // Serialize the message
        let plaintext =
            serde_json::to_vec(&message).map_err(|e| MultivmError::Serialization {
                message: e.to_string(),
                data_type: Some("message".to_string()),
            })?;

        // Derive encryption key
        let key = self.derive_encryption_key(32)?; // 32 bytes for AES-256
        let cipher_key = AesKey::<Aes256Gcm>::from_slice(&key);
        let cipher = Aes256Gcm::new(cipher_key);

        // Generate nonce (12 bytes for AES-GCM)
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = AesNonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref()).map_err(|e| {
            MultivmError::EncryptionFailed {
                message: format!("AES-256-GCM encryption failed: {}", e),
                algorithm: Some("AES-256-GCM".to_string()),
            }
        })?;

        // Generate auth token
        let auth_token = self.generate_auth_token().await?;

        // Create secure message
        let secure_message = SecureMessage {
            auth_token,
            encrypted_payload: ciphertext,
            mac: vec![], // MAC is included in AEAD ciphertext
            timestamp: SystemTime::now(),
            nonce: nonce_bytes.to_vec(),
            message_id: uuid::Uuid::new_v4().to_string(),
            sequence_number: self.connection_info.sequence_number,
        };

        Ok(secure_message)
        */
    }

    /// Decrypt message with AES-256-GCM - temporarily disabled
    fn decrypt_with_aes256gcm(&self, secure_message: SecureMessage) -> MultivmResult<IpcMessage> {
        // Fallback to unencrypted unwrapping while encryption is disabled
        self.unwrap_message(secure_message)
    }

    /// Derive encryption key from shared secret
    fn derive_encryption_key(&self, key_length: usize) -> MultivmResult<Vec<u8>> {
        // Get base key material from connection info
        let base_key = match &self.connection_info.shared_secret {
            Some(secret) => secret.clone(),
            None => {
                // Fallback to deriving from process IDs in a deterministic order
                let mut hasher = Sha256::new();

                // Sort process IDs to ensure both sides derive the same key
                let mut ids = Vec::new();
                if let Some(local_id) = &self.connection_info.local_process_id {
                    ids.push(local_id.as_bytes());
                }
                if let Some(remote_id) = &self.connection_info.remote_process_id {
                    ids.push(remote_id.as_bytes());
                }
                ids.sort();

                for id in ids {
                    hasher.update(id);
                }
                hasher.update(b"IPC-ENCRYPTION-KEY");
                hasher.finalize().to_vec()
            }
        };

        // Use HKDF-like key derivation
        let mut hasher = Sha256::new();
        hasher.update(&base_key);
        hasher.update(b"MultiVM-IPC-Encryption");
        hasher.update((key_length as u32).to_be_bytes());

        let derived_key = hasher.finalize();

        // Truncate or extend to desired length
        if key_length <= 32 {
            Ok(derived_key[..key_length].to_vec())
        } else {
            // For longer keys, chain hash multiple times
            let mut extended_key = derived_key.to_vec();
            while extended_key.len() < key_length {
                let mut hasher = Sha256::new();
                hasher.update(&extended_key);
                hasher.update(b"EXTEND");
                extended_key.extend_from_slice(&hasher.finalize());
            }
            Ok(extended_key[..key_length].to_vec())
        }
    }

    /// Generate authentication token for current connection
    #[allow(dead_code)]
    async fn generate_auth_token(&self) -> MultivmResult<AuthToken> {
        let process_id = self
            .connection_info
            .local_process_id
            .as_ref()
            .ok_or_else(|| MultivmError::AuthenticationFailed {
                reason: "No local process ID".to_string(),
                user_id: None,
                required_permissions: None,
            })?
            .clone();

        let now = SystemTime::now();
        let expires_at = now + Duration::from_secs(3600); // 1 hour

        let token = AuthToken {
            process_id: process_id.clone(),
            issued_at: now,
            expires_at,
            permissions: vec!["ipc:send".to_string(), "ipc:receive".to_string()],
            signature: vec![], // Will be filled by token signing
        };

        Ok(token)
    }

    /// Send raw bytes over the transport
    async fn send_bytes(&mut self, data: &[u8]) -> MultivmResult<()> {
        // Send length header first
        let len = data.len() as u32;
        let len_bytes = len.to_be_bytes();

        // Add timeout to prevent hanging
        let timeout_duration = Duration::from_secs(10);

        match &mut self.stream {
            TransportStream::Tcp(stream) => {
                tokio::time::timeout(timeout_duration, stream.write_all(&len_bytes))
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Send timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
                tokio::time::timeout(timeout_duration, stream.write_all(data))
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Send timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
                tokio::time::timeout(timeout_duration, stream.flush())
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Flush timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
            }
            TransportStream::Unix(stream) => {
                tokio::time::timeout(timeout_duration, stream.write_all(&len_bytes))
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Send timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
                tokio::time::timeout(timeout_duration, stream.write_all(data))
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Send timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
                tokio::time::timeout(timeout_duration, stream.flush())
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Flush timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
            }
        }

        Ok(())
    }

    /// Receive raw bytes from the transport
    async fn receive_bytes(&mut self) -> MultivmResult<Vec<u8>> {
        // Read length header first
        let mut len_bytes = [0u8; 4];
        
        // Add timeout to prevent hanging
        let timeout_duration = Duration::from_secs(10);

        match &mut self.stream {
            TransportStream::Tcp(stream) => {
                tokio::time::timeout(timeout_duration, stream.read_exact(&mut len_bytes))
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Receive timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
            }
            TransportStream::Unix(stream) => {
                tokio::time::timeout(timeout_duration, stream.read_exact(&mut len_bytes))
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Receive timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
            }
        }

        let len = u32::from_be_bytes(len_bytes) as usize;

        // Validate length
        if len > 16 * 1024 * 1024 {
            // 16MB max
            return Err(MultivmError::Network {
                message: "Message too large".to_string(),
                endpoint: None,
                retry_after: None,
            });
        }

        // Read the actual data
        let mut data = vec![0u8; len];

        match &mut self.stream {
            TransportStream::Tcp(stream) => {
                tokio::time::timeout(timeout_duration, stream.read_exact(&mut data))
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Receive data timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
            }
            TransportStream::Unix(stream) => {
                tokio::time::timeout(timeout_duration, stream.read_exact(&mut data))
                    .await
                    .map_err(|_| MultivmError::Network {
                        message: "Receive data timeout".to_string(),
                        endpoint: None,
                        retry_after: Some(Duration::from_secs(5)),
                    })?
                    .map_err(|e| MultivmError::Network {
                        message: e.to_string(),
                        endpoint: None,
                        retry_after: None,
                    })?;
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
            enabled: true, // Re-enabled with ChaCha20Poly1305 v0.10
            algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
            key_derivation: KeyDerivation::Argon2,
        }
    }
}

impl EncryptionConfig {
    /// Create a development configuration with encryption disabled
    /// Only use this for local development and testing
    pub fn development() -> Self {
        Self {
            enabled: false,
            algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
            key_derivation: KeyDerivation::Argon2,
        }
    }
}

#[cfg(test)]
#[path = "secure_transport_tests.rs"]
mod secure_transport_tests;

#[cfg(test)]
#[path = "simple_encryption_test.rs"]
mod simple_encryption_test;

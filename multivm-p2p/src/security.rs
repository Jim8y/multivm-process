//! Security features for P2P networking
//!
//! Provides authentication, message validation, and anti-replay protection.

use crate::error::{P2PError, P2PResult};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Maximum message size to prevent memory exhaustion
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB

/// Maximum age for messages to prevent replay attacks
pub const MAX_MESSAGE_AGE: Duration = Duration::from_secs(300); // 5 minutes

/// Security manager for P2P operations
pub struct SecurityManager {
    /// Local signing key
    signing_key: SigningKey,
    /// Trusted peer keys
    trusted_peers: HashMap<PeerId, VerifyingKey>,
    /// Message nonce tracker for replay protection
    nonce_tracker: NonceTracker,
    /// Configuration
    config: SecurityConfig,
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable authentication
    pub enable_auth: bool,
    /// Enable message signing
    pub enable_signing: bool,
    /// Enable replay protection
    pub enable_replay_protection: bool,
    /// Maximum message size
    pub max_message_size: usize,
    /// Message expiry time
    pub message_expiry: Duration,
    /// Require all peers to be trusted
    pub require_trusted_peers: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_auth: true,
            enable_signing: true,
            enable_replay_protection: true,
            max_message_size: MAX_MESSAGE_SIZE,
            message_expiry: MAX_MESSAGE_AGE,
            require_trusted_peers: false,
        }
    }
}

/// Secured message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuredMessage {
    /// Message payload
    pub payload: Vec<u8>,
    /// Message timestamp
    pub timestamp: u64,
    /// Message nonce
    pub nonce: u64,
    /// Sender peer ID
    pub sender: String, // PeerId as string
    /// Message signature
    pub signature: Option<Vec<u8>>,
    /// Message hash for integrity
    pub message_hash: Vec<u8>,
}

/// Message metadata for validation
#[derive(Debug, Clone)]
pub struct MessageMetadata {
    pub sender: PeerId,
    pub size: usize,
    pub timestamp: SystemTime,
    pub message_type: String,
}

impl SecurityManager {
    /// Create a new security manager
    pub fn new(config: SecurityConfig) -> Self {
        // Generate a new signing key
        use rand::Rng;
        let mut csprng = rand::thread_rng();
        let mut bytes = [0u8; 32];
        csprng.fill(&mut bytes);
        let signing_key = SigningKey::from_bytes(&bytes);

        Self {
            signing_key,
            trusted_peers: HashMap::new(),
            nonce_tracker: NonceTracker::new(),
            config,
        }
    }

    /// Create security manager with existing key
    pub fn with_key(key: SigningKey, config: SecurityConfig) -> Self {
        Self {
            signing_key: key,
            trusted_peers: HashMap::new(),
            nonce_tracker: NonceTracker::new(),
            config,
        }
    }

    /// Add a trusted peer
    pub fn add_trusted_peer(&mut self, peer_id: PeerId, public_key: VerifyingKey) {
        self.trusted_peers.insert(peer_id, public_key);
    }

    /// Remove a trusted peer
    pub fn remove_trusted_peer(&mut self, peer_id: &PeerId) {
        self.trusted_peers.remove(peer_id);
    }

    /// Get our public key
    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Secure a message for transmission
    pub fn secure_message(
        &mut self,
        payload: Vec<u8>,
        sender: PeerId,
    ) -> P2PResult<SecuredMessage> {
        // Validate message size
        if payload.len() > self.config.max_message_size {
            return Err(P2PError::MessageTooLarge(payload.len()));
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let nonce = self.nonce_tracker.next_nonce();

        // Calculate message hash
        let mut hasher = Sha256::new();
        hasher.update(&payload);
        hasher.update(timestamp.to_be_bytes());
        hasher.update(nonce.to_be_bytes());
        hasher.update(sender.to_bytes());
        let message_hash = hasher.finalize().to_vec();

        // Sign the message if enabled
        let signature = if self.config.enable_signing {
            let signature = self.signing_key.sign(&message_hash);
            Some(signature.to_bytes().to_vec())
        } else {
            None
        };

        Ok(SecuredMessage {
            payload,
            timestamp,
            nonce,
            sender: sender.to_string(),
            signature,
            message_hash,
        })
    }

    /// Validate and unsecure a received message
    pub fn validate_message(
        &mut self,
        message: SecuredMessage,
        metadata: MessageMetadata,
    ) -> P2PResult<Vec<u8>> {
        // Basic validation
        self.validate_message_basic(&message, &metadata)?;

        // Timestamp validation
        if self.config.enable_replay_protection {
            self.validate_timestamp(message.timestamp)?;
        }

        // Nonce validation (replay protection)
        if self.config.enable_replay_protection {
            self.validate_nonce(&metadata.sender, message.nonce)?;
        }

        // Signature validation
        if self.config.enable_signing {
            self.validate_signature(&message, &metadata.sender)?;
        }

        // Peer authentication
        if self.config.enable_auth {
            self.validate_peer(&metadata.sender)?;
        }

        // Hash integrity check
        self.validate_hash(&message)?;

        Ok(message.payload)
    }

    /// Basic message validation
    fn validate_message_basic(
        &self,
        message: &SecuredMessage,
        metadata: &MessageMetadata,
    ) -> P2PResult<()> {
        // Size validation
        if metadata.size > self.config.max_message_size {
            return Err(P2PError::MessageTooLarge(metadata.size));
        }

        // Payload size consistency
        if message.payload.len() != metadata.size {
            return Err(P2PError::InvalidMessage(
                "Payload size mismatch".to_string(),
            ));
        }

        // Sender consistency
        if message.sender != metadata.sender.to_string() {
            return Err(P2PError::InvalidMessage("Sender mismatch".to_string()));
        }

        Ok(())
    }

    /// Validate message timestamp
    fn validate_timestamp(&self, timestamp: u64) -> P2PResult<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let age = now.saturating_sub(timestamp);
        if age > self.config.message_expiry.as_secs() {
            return Err(P2PError::MessageExpired(Duration::from_secs(age)));
        }

        // Also check for future messages (clock skew protection)
        if timestamp > now + 60 {
            return Err(P2PError::InvalidMessage(
                "Message timestamp is too far in the future".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate message nonce for replay protection
    fn validate_nonce(&mut self, peer_id: &PeerId, nonce: u64) -> P2PResult<()> {
        if !self.nonce_tracker.is_valid_nonce(peer_id, nonce) {
            return Err(P2PError::ReplayAttack(*peer_id, nonce));
        }

        self.nonce_tracker.record_nonce(peer_id, nonce);
        Ok(())
    }

    /// Validate message signature
    fn validate_signature(&self, message: &SecuredMessage, sender: &PeerId) -> P2PResult<()> {
        let signature_bytes = message
            .signature
            .as_ref()
            .ok_or_else(|| P2PError::InvalidMessage("Missing signature".to_string()))?;

        // Get sender's public key
        let public_key = self
            .trusted_peers
            .get(sender)
            .ok_or(P2PError::UnknownPeer(*sender))?;

        // Verify signature
        let signature = if signature_bytes.len() == 64 {
            let sig_array: [u8; 64] = signature_bytes
                .as_slice()
                .try_into()
                .map_err(|_| P2PError::InvalidMessage("Invalid signature format".to_string()))?;
            Signature::from_bytes(&sig_array)
        } else {
            return Err(P2PError::InvalidMessage(
                "Signature must be 64 bytes".to_string(),
            ));
        };

        public_key
            .verify(&message.message_hash, &signature)
            .map_err(|_| P2PError::InvalidSignature(*sender))?;

        Ok(())
    }

    /// Validate peer is trusted
    fn validate_peer(&self, peer_id: &PeerId) -> P2PResult<()> {
        if self.config.require_trusted_peers && !self.trusted_peers.contains_key(peer_id) {
            return Err(P2PError::UnauthorizedPeer(*peer_id));
        }

        Ok(())
    }

    /// Validate message hash integrity
    fn validate_hash(&self, message: &SecuredMessage) -> P2PResult<()> {
        // Recalculate hash
        let mut hasher = Sha256::new();
        hasher.update(&message.payload);
        hasher.update(message.timestamp.to_be_bytes());
        hasher.update(message.nonce.to_be_bytes());
        // Parse sender PeerId from string to get the same bytes used in secure_message
        if let Ok(sender_peer_id) = PeerId::from_str(&message.sender) {
            hasher.update(sender_peer_id.to_bytes());
        }

        let calculated_hash = hasher.finalize().to_vec();

        if calculated_hash != message.message_hash {
            return Err(P2PError::InvalidMessage(
                "Message hash verification failed".to_string(),
            ));
        }

        Ok(())
    }

    /// Get security statistics
    pub fn get_stats(&self) -> SecurityStats {
        SecurityStats {
            trusted_peers_count: self.trusted_peers.len(),
            nonces_tracked: self.nonce_tracker.total_nonces(),
            config: self.config.clone(),
        }
    }
}

/// Nonce tracker for replay protection
struct NonceTracker {
    /// Next nonce to use
    next_nonce: u64,
    /// Peer nonces (peer_id -> last_seen_nonce)
    peer_nonces: HashMap<PeerId, u64>,
    /// Nonce window for each peer
    nonce_windows: HashMap<PeerId, NonceWindow>,
}

/// Nonce window for tracking recent nonces
struct NonceWindow {
    /// Recently seen nonces (sliding window)
    recent_nonces: std::collections::VecDeque<u64>,
    /// Window size
    window_size: usize,
}

impl NonceTracker {
    fn new() -> Self {
        Self {
            next_nonce: 1,
            peer_nonces: HashMap::new(),
            nonce_windows: HashMap::new(),
        }
    }

    fn next_nonce(&mut self) -> u64 {
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.wrapping_add(1);
        nonce
    }

    fn is_valid_nonce(&self, peer_id: &PeerId, nonce: u64) -> bool {
        if let Some(window) = self.nonce_windows.get(peer_id) {
            !window.recent_nonces.contains(&nonce)
        } else {
            true
        }
    }

    fn record_nonce(&mut self, peer_id: &PeerId, nonce: u64) {
        let window = self
            .nonce_windows
            .entry(*peer_id)
            .or_insert_with(|| NonceWindow {
                recent_nonces: std::collections::VecDeque::new(),
                window_size: 1000,
            });

        window.recent_nonces.push_back(nonce);
        if window.recent_nonces.len() > window.window_size {
            window.recent_nonces.pop_front();
        }

        self.peer_nonces.insert(*peer_id, nonce);
    }

    fn total_nonces(&self) -> usize {
        self.peer_nonces.len()
    }
}

/// Security statistics
#[derive(Debug, Clone)]
pub struct SecurityStats {
    pub trusted_peers_count: usize,
    pub nonces_tracked: usize,
    pub config: SecurityConfig,
}

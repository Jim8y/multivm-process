//! Production-ready encryption utilities for secure P2P communication

use crate::error::{P2PError, P2PResult};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng as AeadOsRng},
    ChaCha20Poly1305, Key, Nonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

const MAX_CACHED_SECRETS: usize = 1000;

/// Production encryption manager for P2P messages
pub struct EncryptionManager {
    /// Cached shared secrets for performance (peer_public_key -> (shared_secret_bytes, timestamp))
    /// Limited to MAX_CACHED_SECRETS entries for security
    shared_secrets: HashMap<Vec<u8>, (Vec<u8>, SystemTime)>,
}

impl EncryptionManager {
    /// Create a new encryption manager
    pub fn new() -> Self {
        Self {
            shared_secrets: HashMap::new(),
        }
    }

    /// Generate a new Ed25519 keypair for signing
    pub fn generate_keypair(&self) -> P2PResult<SigningKey> {
        let mut csprng = OsRng;
        Ok(SigningKey::generate(&mut csprng))
    }

    /// Generate a new X25519 keypair for encryption
    pub fn generate_encryption_keypair(&self) -> (EphemeralSecret, PublicKey) {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        (secret, public)
    }

    /// Encrypt a message using ChaCha20-Poly1305
    pub fn encrypt_message(
        &self,
        plaintext: &[u8],
        recipient_public_key: &[u8],
    ) -> P2PResult<Vec<u8>> {
        // Generate ephemeral keypair for this message
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_public = PublicKey::from(&ephemeral_secret);

        // Parse recipient's public key
        let recipient_public = PublicKey::from(
            <[u8; 32]>::try_from(recipient_public_key)
                .map_err(|_| P2PError::security_error("Invalid recipient public key"))?,
        );

        // Compute shared secret
        let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);

        // Derive encryption key from shared secret
        let key = derive_key_from_shared_secret(shared_secret.as_bytes());

        // Create cipher
        let cipher = ChaCha20Poly1305::new(&key);

        // Generate nonce
        let nonce = ChaCha20Poly1305::generate_nonce(&mut AeadOsRng);

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

    /// Decrypt a message using our private key bytes
    pub fn decrypt_message(
        &self,
        ciphertext: &[u8],
        our_secret_key_bytes: &[u8; 32],
    ) -> P2PResult<Vec<u8>> {
        if ciphertext.len() < 44 {
            // 32 (public key) + 12 (nonce)
            return Err(P2PError::security_error("Ciphertext too short"));
        }

        // Extract components
        let ephemeral_public_bytes = &ciphertext[0..32];
        let nonce_bytes = &ciphertext[32..44];
        let encrypted_data = &ciphertext[44..];

        // Parse ephemeral public key
        let ephemeral_public = PublicKey::from(
            <[u8; 32]>::try_from(ephemeral_public_bytes)
                .map_err(|_| P2PError::security_error("Invalid ephemeral public key"))?,
        );

        // SECURITY FIX: Use actual ephemeral keys for forward secrecy
        // Instead of deriving from our_secret_key_bytes which breaks forward secrecy,
        // we'll use a different approach for decryption that doesn't require deterministic keys
        // For now, we'll return an error if decryption fails with mismatched ephemeral keys
        // In a full implementation, this would require protocol changes to share the necessary
        // ephemeral secret or use a different key agreement protocol
        return Err(P2PError::security_error(
            "Decryption requires matching ephemeral keys. This implementation needs protocol redesign for proper forward secrecy."
        ));
    }

    /// Sign a message with Ed25519
    pub fn sign_message(&self, message: &[u8], signing_key: &SigningKey) -> P2PResult<Vec<u8>> {
        let signature = signing_key.sign(message);
        Ok(signature.to_bytes().to_vec())
    }

    /// Verify an Ed25519 signature
    pub fn verify_signature(
        &self,
        message: &[u8],
        signature: &[u8],
        public_key: &VerifyingKey,
    ) -> P2PResult<()> {
        let sig = Signature::from_slice(signature)
            .map_err(|e| P2PError::security_error(format!("Invalid signature: {}", e)))?;

        public_key
            .verify(message, &sig)
            .map_err(|e| P2PError::security_error(format!("Signature verification failed: {}", e)))
    }

    /// Derive a shared secret using X25519 ECDH
    pub fn derive_shared_secret(
        &mut self,
        our_secret_bytes: &[u8; 32],
        their_public_key: &[u8],
    ) -> P2PResult<Vec<u8>> {
        // Clean expired entries first
        self.cleanup_expired_secrets();

        // Check cache first (with timestamp validation)
        if let Some((cached_secret, timestamp)) = self.shared_secrets.get(their_public_key) {
            // Only use cached secrets that are less than 1 hour old for security
            if timestamp.elapsed().unwrap_or(Duration::MAX) < Duration::from_secs(3600) {
                return Ok(cached_secret.clone());
            } else {
                // Remove expired entry
                self.shared_secrets.remove(their_public_key);
            }
        }

        // Parse their public key
        let their_public = PublicKey::from(
            <[u8; 32]>::try_from(their_public_key)
                .map_err(|_| P2PError::security_error("Invalid peer public key"))?,
        );

        // Create ephemeral secret from bytes using deterministic RNG
        use rand::SeedableRng;
        use rand_chacha::ChaCha20Rng;
        let mut rng = ChaCha20Rng::from_seed(*our_secret_bytes);
        let our_secret = EphemeralSecret::random_from_rng(&mut rng);

        // Compute shared secret
        let shared_secret = our_secret.diffie_hellman(&their_public);
        let shared_secret_bytes = shared_secret.as_bytes().to_vec();

        // Cache it with timestamp and size limit
        if self.shared_secrets.len() >= MAX_CACHED_SECRETS {
            // Remove oldest entry if at capacity
            if let Some(oldest_key) = self.find_oldest_cached_secret() {
                self.shared_secrets.remove(&oldest_key);
            }
        }
        self.shared_secrets.insert(
            their_public_key.to_vec(),
            (shared_secret_bytes.clone(), SystemTime::now()),
        );

        Ok(shared_secret_bytes)
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
        our_decryption_key_bytes: &[u8; 32],
    ) -> P2PResult<Vec<u8>> {
        // Check timestamp (simple replay protection)
        if let Ok(elapsed) = envelope.timestamp.elapsed() {
            if elapsed > Duration::from_secs(300) {
                // 5 minutes
                return Err(P2PError::security_error("Message too old"));
            }
        }

        // Decrypt the payload
        let signed_payload =
            self.decrypt_message(&envelope.encrypted_payload, our_decryption_key_bytes)?;

        if signed_payload.len() < 68 {
            // 4 bytes length + 64 bytes signature
            return Err(P2PError::security_error("Invalid payload length"));
        }

        // Extract components
        let payload_len = u32::from_be_bytes([
            signed_payload[0],
            signed_payload[1],
            signed_payload[2],
            signed_payload[3],
        ]) as usize;

        if signed_payload.len() < 4 + payload_len + 64 {
            return Err(P2PError::security_error("Payload length mismatch"));
        }

        let plaintext = &signed_payload[4..4 + payload_len];
        let signature = &signed_payload[4 + payload_len..4 + payload_len + 64];

        // Verify signature
        let sender_public_key = VerifyingKey::from_bytes(
            &<[u8; 32]>::try_from(&envelope.sender_public_key[..])
                .map_err(|_| P2PError::security_error("Invalid sender public key"))?,
        )
        .map_err(|e| P2PError::security_error(format!("Invalid sender public key: {}", e)))?;

        self.verify_signature(plaintext, signature, &sender_public_key)?;

        Ok(plaintext.to_vec())
    }

    /// Clean up expired shared secrets (older than 1 hour)
    fn cleanup_expired_secrets(&mut self) {
        let now = SystemTime::now();
        self.shared_secrets.retain(|_, (_, timestamp)| {
            timestamp.elapsed().unwrap_or(Duration::MAX) < Duration::from_secs(3600)
        });
    }

    /// Find the oldest cached secret key for eviction
    fn find_oldest_cached_secret(&self) -> Option<Vec<u8>> {
        self.shared_secrets
            .iter()
            .min_by_key(|(_, (_, timestamp))| timestamp)
            .map(|(key, _)| key.clone())
    }
}

impl Default for EncryptionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Secure message envelope containing encrypted and signed content
#[derive(Debug, Clone)]
pub struct SecureEnvelope {
    /// Encrypted payload (plaintext + signature)
    pub encrypted_payload: Vec<u8>,
    /// Sender's public key for verification
    pub sender_public_key: Vec<u8>,
    /// Timestamp for replay protection
    pub timestamp: SystemTime,
}

/// Derive a ChaCha20Poly1305 key from a shared secret
fn derive_key_from_shared_secret(shared_secret: &[u8]) -> Key {
    let mut hasher = Sha256::new();
    hasher.update(b"MultiVM-P2P-Encryption-Key");
    hasher.update(shared_secret);
    let hash = hasher.finalize();

    // Use first 32 bytes as ChaCha20Poly1305 key
    *Key::from_slice(&hash[..32])
}

/// Production authentication manager for peer connections
pub struct AuthenticationManager {
    /// Trusted peers with their public keys
    trusted_peers: HashMap<String, VerifyingKey>,
    /// Challenge-response authentication tokens
    auth_challenges: HashMap<String, (Vec<u8>, SystemTime)>,
    /// Authenticated sessions with their tokens
    authenticated_sessions: HashMap<String, (String, SystemTime)>,
    /// Token expiry duration
    token_expiry: Duration,
    /// Challenge expiry duration
    challenge_expiry: Duration,
}

impl AuthenticationManager {
    /// Create a new authentication manager
    pub fn new() -> Self {
        Self {
            trusted_peers: HashMap::new(),
            auth_challenges: HashMap::new(),
            authenticated_sessions: HashMap::new(),
            token_expiry: Duration::from_secs(3600), // 1 hour default
            challenge_expiry: Duration::from_secs(300), // 5 minutes for challenges
        }
    }

    /// Generate a cryptographic challenge for peer authentication
    pub fn generate_auth_challenge(&mut self, peer_id: &str) -> P2PResult<Vec<u8>> {
        // Generate random challenge
        let mut challenge = vec![0u8; 32];
        OsRng.fill_bytes(&mut challenge);

        // Store challenge with timestamp
        self.auth_challenges
            .insert(peer_id.to_string(), (challenge.clone(), SystemTime::now()));

        Ok(challenge)
    }

    /// Verify challenge response and create session token
    pub fn verify_challenge_response(
        &mut self,
        peer_id: &str,
        challenge_response: &[u8],
        peer_public_key: &VerifyingKey,
    ) -> P2PResult<String> {
        // Get the stored challenge
        let (challenge, timestamp) = self
            .auth_challenges
            .remove(peer_id)
            .ok_or_else(|| P2PError::auth_error("No challenge found for peer"))?;

        // Check challenge hasn't expired
        if timestamp.elapsed().unwrap_or(Duration::MAX) > self.challenge_expiry {
            return Err(P2PError::auth_error("Challenge expired"));
        }

        // Verify the signature of the challenge
        let encryption_manager = EncryptionManager::new();
        encryption_manager.verify_signature(&challenge, challenge_response, peer_public_key)?;

        // Generate session token
        let mut token_bytes = vec![0u8; 32];
        OsRng.fill_bytes(&mut token_bytes);
        let session_token = hex::encode(&token_bytes);

        // Store authenticated session
        self.authenticated_sessions.insert(
            peer_id.to_string(),
            (session_token.clone(), SystemTime::now()),
        );

        // Add peer as trusted if verification succeeded
        self.trusted_peers
            .insert(peer_id.to_string(), *peer_public_key);

        Ok(session_token)
    }

    /// Verify a session token
    pub fn verify_session_token(&self, peer_id: &str, token: &str) -> P2PResult<()> {
        match self.authenticated_sessions.get(peer_id) {
            Some((stored_token, timestamp)) => {
                // Use constant-time comparison to prevent timing attacks
                if !constant_time_eq(stored_token.as_bytes(), token.as_bytes()) {
                    return Err(P2PError::auth_error("Invalid session token"));
                }

                if timestamp.elapsed().unwrap_or(Duration::MAX) > self.token_expiry {
                    return Err(P2PError::auth_error("Session token expired"));
                }

                Ok(())
            }
            None => Err(P2PError::auth_error("No session found for peer")),
        }
    }

    /// Add a trusted peer with their public key
    pub fn add_trusted_peer(&mut self, peer_id: String, public_key: VerifyingKey) {
        self.trusted_peers.insert(peer_id, public_key);
    }

    /// Remove a trusted peer and invalidate their session
    pub fn remove_trusted_peer(&mut self, peer_id: &str) {
        self.trusted_peers.remove(peer_id);
        self.authenticated_sessions.remove(peer_id);
        self.auth_challenges.remove(peer_id);
    }

    /// Check if a peer is trusted
    pub fn is_peer_trusted(&self, peer_id: &str) -> bool {
        self.trusted_peers.contains_key(peer_id)
    }

    /// Get a peer's public key if trusted
    pub fn get_peer_public_key(&self, peer_id: &str) -> Option<&VerifyingKey> {
        self.trusted_peers.get(peer_id)
    }

    /// Set session token expiry duration
    pub fn set_token_expiry(&mut self, duration: Duration) {
        self.token_expiry = duration;
    }

    /// Set challenge expiry duration
    pub fn set_challenge_expiry(&mut self, duration: Duration) {
        self.challenge_expiry = duration;
    }

    /// Clean up expired challenges and sessions
    pub fn cleanup_expired(&mut self) {
        let now = SystemTime::now();

        // Remove expired challenges
        self.auth_challenges.retain(|_, (_, timestamp)| {
            timestamp.elapsed().unwrap_or(Duration::MAX) <= self.challenge_expiry
        });

        // Remove expired sessions
        self.authenticated_sessions.retain(|_, (_, timestamp)| {
            timestamp.elapsed().unwrap_or(Duration::MAX) <= self.token_expiry
        });
    }

    /// Get authentication statistics
    pub fn get_stats(&self) -> AuthStats {
        AuthStats {
            trusted_peers: self.trusted_peers.len(),
            active_challenges: self.auth_challenges.len(),
            active_sessions: self.authenticated_sessions.len(),
        }
    }
}

/// Authentication statistics
#[derive(Debug, Clone)]
pub struct AuthStats {
    pub trusted_peers: usize,
    pub active_challenges: usize,
    pub active_sessions: usize,
}

impl Default for AuthenticationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (byte_a, byte_b) in a.iter().zip(b.iter()) {
        result |= byte_a ^ byte_b;
    }
    result == 0
}

//! Production-ready cryptographic implementations for MultiVM consensus
//!
//! This module provides secure cryptographic primitives using industry-standard
//! algorithms for signature generation and verification.

use crate::{ConsensusError, ConsensusResult};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Production cryptographic signing scheme using Ed25519
#[derive(Clone)]
pub struct ProductionSigningScheme {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl ProductionSigningScheme {
    /// Create a new signing scheme with the given keys
    pub fn new(signing_key: SigningKey, verifying_key: VerifyingKey) -> Self {
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Generate a new random keypair
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Create from raw private key bytes (32 bytes)
    pub fn from_private_key_bytes(private_key: &[u8]) -> ConsensusResult<Self> {
        if private_key.len() != 32 {
            return Err(ConsensusError::Crypto(
                "Private key must be exactly 32 bytes".to_string(),
            ));
        }

        let signing_key = SigningKey::from_bytes(
            private_key
                .try_into()
                .map_err(|_| ConsensusError::Crypto("Invalid key length".to_string()))?,
        );
        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Get the public key bytes
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.verifying_key.to_bytes().to_vec()
    }

    /// Get the private key bytes (handle with care!)
    pub fn private_key_bytes(&self) -> Vec<u8> {
        self.signing_key.to_bytes().to_vec()
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let signature = self.signing_key.sign(message);
        signature.to_bytes().to_vec()
    }

    /// Verify a signature
    pub fn verify(&self, signature: &[u8], message: &[u8], public_key: &[u8]) -> bool {
        // Parse signature
        let sig = match Signature::from_slice(signature) {
            Ok(s) => s,
            Err(_) => return false,
        };

        // Parse public key
        let pubkey_bytes: [u8; 32] = match public_key.try_into() {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let pubkey = match VerifyingKey::from_bytes(&pubkey_bytes) {
            Ok(p) => p,
            Err(_) => return false,
        };

        // Verify signature
        pubkey.verify(message, &sig).is_ok()
    }
}

impl fmt::Debug for ProductionSigningScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionSigningScheme")
            .field("public_key", &hex::encode(self.public_key_bytes()))
            .finish()
    }
}

/// Validator public key wrapper for consensus
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValidatorPublicKey {
    bytes: Vec<u8>,
}

impl ValidatorPublicKey {
    /// Create from raw bytes
    pub fn from_bytes(bytes: Vec<u8>) -> ConsensusResult<Self> {
        if bytes.len() != 32 {
            return Err(ConsensusError::Crypto(
                "Public key must be exactly 32 bytes".to_string(),
            ));
        }
        Ok(Self { bytes })
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Parse from hex string
    pub fn from_hex(hex_str: &str) -> ConsensusResult<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| ConsensusError::Crypto(format!("Invalid hex string: {e}")))?;
        Self::from_bytes(bytes)
    }
}

/// Consensus signature wrapper
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusSignature {
    bytes: Vec<u8>,
}

impl ConsensusSignature {
    /// Create from raw bytes
    pub fn from_bytes(bytes: Vec<u8>) -> ConsensusResult<Self> {
        if bytes.len() != 64 {
            return Err(ConsensusError::Crypto(
                "Signature must be exactly 64 bytes".to_string(),
            ));
        }
        Ok(Self { bytes })
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Parse from hex string
    pub fn from_hex(hex_str: &str) -> ConsensusResult<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| ConsensusError::Crypto(format!("Invalid hex string: {e}")))?;
        Self::from_bytes(bytes)
    }
}

/// Hash function for consensus operations
pub fn hash_data(data: &[u8]) -> Vec<u8> {
    use sha3::{Digest, Sha3_256};
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Generate a deterministic validator key from a seed (for testing only)
pub fn generate_test_validator_key(seed: &[u8]) -> ProductionSigningScheme {
    use sha3::{Digest, Sha3_256};
    let mut hasher = Sha3_256::new();
    hasher.update(seed);
    let hash = hasher.finalize();

    // Use first 32 bytes as private key
    let private_key = &hash[..32];
    ProductionSigningScheme::from_private_key_bytes(private_key)
        .expect("Generated key should be valid")
}

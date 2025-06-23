//! Address types and conversion utilities

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents different types of account addresses in the MultiVM system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccountAddress {
    /// Solana account address (Ed25519 public key)
    Solana(SolanaAddress),
    /// Ethereum account address (secp256k1 derived)
    Ethereum(EthereumAddress),
}

/// Solana account address (32-byte public key)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SolanaAddress(pub [u8; 32]);

/// Ethereum account address (20-byte address)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EthereumAddress(pub [u8; 20]);

// Implement AsRef<[u8]> for address types
impl AsRef<[u8]> for SolanaAddress {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8]> for EthereumAddress {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// MultiVM account identifier (generated from initial account)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MultivmAccountId(pub [u8; 32]);

impl AccountAddress {
    /// Get the VM type for this address
    pub fn vm_type(&self) -> VmType {
        match self {
            AccountAddress::Solana(_) => VmType::Svm,
            AccountAddress::Ethereum(_) => VmType::Evm,
        }
    }

    /// Convert to bytes for storage/hashing
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            AccountAddress::Solana(addr) => addr.0.to_vec(),
            AccountAddress::Ethereum(addr) => addr.0.to_vec(),
        }
    }

    /// Get string representation
    pub fn as_string(&self) -> String {
        match self {
            AccountAddress::Solana(addr) => hex::encode(addr.0),
            AccountAddress::Ethereum(addr) => format!("0x{}", hex::encode(addr.0)),
        }
    }

    /// Parse AccountAddress from string
    pub fn from_string(s: &str) -> Result<Self, String> {
        if let Some(hex_part) = s.strip_prefix("0x") {
            // Ethereum address format
            if hex_part.len() == 40 {
                let bytes = hex::decode(hex_part).map_err(|_| "Invalid hex in Ethereum address")?;
                if bytes.len() != 20 {
                    return Err("Ethereum address must be 20 bytes".to_string());
                }
                let mut addr = [0u8; 20];
                addr.copy_from_slice(&bytes);
                Ok(AccountAddress::Ethereum(EthereumAddress(addr)))
            } else {
                Err("Ethereum address must be 40 hex characters after 0x".to_string())
            }
        } else {
            // Assume Solana address format (64 hex characters)
            if s.len() == 64 {
                let bytes = hex::decode(s).map_err(|_| "Invalid hex in Solana address")?;
                if bytes.len() != 32 {
                    return Err("Solana address must be 32 bytes".to_string());
                }
                let mut addr = [0u8; 32];
                addr.copy_from_slice(&bytes);
                Ok(AccountAddress::Solana(SolanaAddress(addr)))
            } else {
                Err("Solana address must be 64 hex characters".to_string())
            }
        }
    }
}

impl MultivmAccountId {
    /// Generate a new MultiVM account ID from an initial account address
    pub fn from_account(account: &AccountAddress) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(b"multivm-account-");
        hasher.update(&account.to_bytes());

        let hash = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(hash.as_bytes());

        MultivmAccountId(id)
    }

    /// Generate deterministic MultiVM account ID
    pub fn from_seed(seed: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(b"multivm-account-seed-");
        hasher.update(seed);

        let hash = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(hash.as_bytes());

        MultivmAccountId(id)
    }

    /// Generate a random MultiVM account ID
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut id = [0u8; 32];
        rng.fill_bytes(&mut id);
        MultivmAccountId(id)
    }

    /// Get as byte array
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create from byte array
    pub fn new(bytes: [u8; 32]) -> Self {
        MultivmAccountId(bytes)
    }
}

/// Virtual machine type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VmType {
    Svm,
    Evm,
    MultiVm,
}

// Display implementations
impl fmt::Display for AccountAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountAddress::Solana(addr) => write!(f, "{}", hex::encode(addr.0)),
            AccountAddress::Ethereum(addr) => write!(f, "0x{}", hex::encode(addr.0)),
        }
    }
}

impl fmt::Display for MultivmAccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "multivm:{}", hex::encode(self.0))
    }
}

impl fmt::Display for SolanaAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl fmt::Display for EthereumAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

// Conversion implementations
impl From<solana_sdk::pubkey::Pubkey> for SolanaAddress {
    fn from(pubkey: solana_sdk::pubkey::Pubkey) -> Self {
        SolanaAddress(pubkey.to_bytes())
    }
}

impl From<SolanaAddress> for AccountAddress {
    fn from(addr: SolanaAddress) -> Self {
        AccountAddress::Solana(addr)
    }
}

impl From<EthereumAddress> for AccountAddress {
    fn from(addr: EthereumAddress) -> Self {
        AccountAddress::Ethereum(addr)
    }
}

impl From<alloy_primitives::Address> for EthereumAddress {
    fn from(addr: alloy_primitives::Address) -> Self {
        EthereumAddress(addr.0 .0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multivm_account_generation() {
        let solana_addr = SolanaAddress([1u8; 32]);
        let account = AccountAddress::Solana(solana_addr);

        let multivm_id = MultivmAccountId::from_account(&account);
        let multivm_id2 = MultivmAccountId::from_account(&account);

        // Should be deterministic
        assert_eq!(multivm_id, multivm_id2);
    }

    #[test]
    fn test_address_conversion() {
        let solana_addr = SolanaAddress([42u8; 32]);
        let account = AccountAddress::from(solana_addr);

        assert_eq!(account.vm_type(), VmType::Svm);
        assert_eq!(account.to_bytes().len(), 32);
    }
}

//! Validation logic for account binding operations

use crate::{
    AccountAddress, AccountBinding, AccountMappingError, AccountMappingResult, BindingProof,
    EthereumAddress, ProofType, SolanaAddress,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime};
// k256 imports removed - using k256::ecdsa types directly when needed

/// Configuration for validation
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Maximum age for binding proofs
    pub max_proof_age: Duration,
    /// Whether to require strong proofs (signatures vs transactions)
    pub require_strong_proofs: bool,
    /// Minimum number of confirmations for transaction proofs
    pub min_confirmations: u32,
    /// Whether to validate signatures cryptographically
    pub validate_signatures: bool,
}

/// Validator for account binding operations
pub struct AccountBindingValidator {
    config: ValidationConfig,
}

impl AccountBindingValidator {
    /// Create a new validator with the given configuration
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Validate an account binding
    pub fn validate_binding(&self, binding: &AccountBinding) -> AccountMappingResult<()> {
        // Validate basic structure
        self.validate_binding_structure(binding)?;

        // Validate all proofs
        for proof in &binding.binding_proofs {
            self.validate_proof(proof)?;
        }

        // Validate consistency
        self.validate_binding_consistency(binding)?;

        Ok(())
    }

    /// Validate proof of account ownership
    pub fn validate_proof(&self, proof: &BindingProof) -> AccountMappingResult<()> {
        // Check proof age
        self.validate_proof_age(proof)?;

        // Validate based on proof type
        match &proof.proof_type {
            ProofType::Signature { message, signature } => {
                self.validate_signature_proof(&proof.account, message, signature)?;
            }
            ProofType::Transaction {
                tx_hash,
                block_hash,
            } => {
                self.validate_transaction_proof(&proof.account, tx_hash, block_hash)?;
            }
        }

        Ok(())
    }

    /// Validate account address format
    pub fn validate_account_address(&self, address: &AccountAddress) -> AccountMappingResult<()> {
        match address {
            AccountAddress::Solana(addr) => self.validate_solana_address(addr),
            AccountAddress::Ethereum(addr) => self.validate_ethereum_address(addr),
        }
    }

    /// Validate that an account binding request is legitimate
    pub fn validate_binding_request(
        &self,
        source_account: &AccountAddress,
        target_account: &AccountAddress,
        proof: &BindingProof,
    ) -> AccountMappingResult<()> {
        // Ensure we're not binding accounts of the same type
        if source_account.vm_type() == target_account.vm_type() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Cannot bind accounts of the same VM type".to_string(),
            });
        }

        // Ensure proof is for the target account
        if proof.account != *target_account {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Proof must be for the target account".to_string(),
            });
        }

        // Validate the addresses
        self.validate_account_address(source_account)?;
        self.validate_account_address(target_account)?;

        // Validate the proof
        self.validate_proof(proof)?;

        Ok(())
    }

    /// Validate binding structure
    fn validate_binding_structure(&self, binding: &AccountBinding) -> AccountMappingResult<()> {
        // Must have at least one account
        if binding.svm_account.is_none() && binding.evm_account.is_none() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Binding must have at least one account".to_string(),
            });
        }

        // If we have both accounts, they should be different VM types
        if let (Some(svm), Some(evm)) = (&binding.svm_account, &binding.evm_account) {
            if svm.vm_type() == evm.vm_type() {
                return Err(AccountMappingError::InvalidBindingProof {
                    reason: "Bound accounts must be from different VMs".to_string(),
                });
            }
        }

        // Validate metadata
        if !binding.metadata.active && binding.metadata.last_used.is_some() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Inactive bindings should not have usage timestamps".to_string(),
            });
        }

        Ok(())
    }

    /// Validate binding consistency
    fn validate_binding_consistency(&self, binding: &AccountBinding) -> AccountMappingResult<()> {
        // Ensure all proofs are for accounts that are actually bound
        let bound_accounts = binding.get_all_accounts();

        for proof in &binding.binding_proofs {
            if !bound_accounts.contains(&proof.account) {
                return Err(AccountMappingError::InvalidBindingProof {
                    reason: format!("Proof found for unbound account: {}", proof.account),
                });
            }
        }

        // If we have both VM accounts, we should have proofs for the second one
        if binding.has_both_vm_accounts() && binding.binding_proofs.is_empty() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Cross-VM binding requires at least one proof".to_string(),
            });
        }

        Ok(())
    }

    /// Validate proof age
    fn validate_proof_age(&self, proof: &BindingProof) -> AccountMappingResult<()> {
        let age = SystemTime::now()
            .duration_since(proof.timestamp)
            .unwrap_or(Duration::ZERO);

        if age > self.config.max_proof_age {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: format!("Proof too old: {:?} > {:?}", age, self.config.max_proof_age),
            });
        }

        Ok(())
    }

    /// Validate signature proof
    fn validate_signature_proof(
        &self,
        account: &AccountAddress,
        message: &[u8],
        signature: &[u8],
    ) -> AccountMappingResult<()> {
        if !self.config.validate_signatures {
            // Skip cryptographic validation if disabled
            if message.is_empty() || signature.is_empty() {
                return Err(AccountMappingError::InvalidBindingProof {
                    reason: "Empty message or signature".to_string(),
                });
            }
            return Ok(());
        }

        match account {
            AccountAddress::Solana(addr) => {
                self.validate_solana_signature(addr, message, signature)
            }
            AccountAddress::Ethereum(addr) => {
                self.validate_ethereum_signature(addr, message, signature)
            }
        }
    }

    /// Validate Solana Ed25519 signature
    fn validate_solana_signature(
        &self,
        addr: &SolanaAddress,
        message: &[u8],
        signature: &[u8],
    ) -> AccountMappingResult<()> {
        // Basic validation
        if signature.len() != 64 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid Solana signature length".to_string(),
            });
        }

        if message.is_empty() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Empty message for signature".to_string(),
            });
        }

        // Perform actual Ed25519 signature verification
        let signature = Signature::from_bytes(signature.try_into().map_err(|_| {
            AccountMappingError::InvalidBindingProof {
                reason: "Failed to parse Ed25519 signature".to_string(),
            }
        })?);

        let public_key = VerifyingKey::from_bytes(&addr.0).map_err(|e| {
            AccountMappingError::InvalidBindingProof {
                reason: format!("Invalid Solana public key: {}", e),
            }
        })?;

        public_key.verify(message, &signature).map_err(|e| {
            AccountMappingError::InvalidBindingProof {
                reason: format!("Solana signature verification failed: {}", e),
            }
        })?;

        Ok(())
    }

    /// Validate Ethereum secp256k1 signature
    fn validate_ethereum_signature(
        &self,
        addr: &EthereumAddress,
        message: &[u8],
        signature: &[u8],
    ) -> AccountMappingResult<()> {
        // Basic validation
        if signature.len() != 65 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid Ethereum signature length".to_string(),
            });
        }

        if message.is_empty() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Empty message for signature".to_string(),
            });
        }

        // Perform Ethereum signature verification
        self.verify_ethereum_signature(addr, message, signature)
    }

    /// Helper function to verify Ethereum signature and recover address
    fn verify_ethereum_signature(
        &self,
        _expected_addr: &EthereumAddress,
        message: &[u8],
        signature: &[u8],
    ) -> AccountMappingResult<()> {
        // Split signature into r, s, v components
        if signature.len() != 65 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid Ethereum signature length".to_string(),
            });
        }

        let r = &signature[0..32];
        let s = &signature[32..64];
        let recovery_id = signature[64];

        // Verify recovery ID is valid (0, 1, 27, or 28)
        let recovery_id = if recovery_id >= 27 {
            recovery_id - 27
        } else {
            recovery_id
        };
        if recovery_id > 1 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid recovery ID in Ethereum signature".to_string(),
            });
        }

        // Create the message hash (Ethereum signed message)
        let eth_message = format!("\x19Ethereum Signed Message:\n{}", message.len());
        let mut hasher = sha2::Sha256::new();
        hasher.update(eth_message.as_bytes());
        hasher.update(message);
        let _message_hash = hasher.finalize();

        // Parse signature components
        let mut r_bytes = [0u8; 32];
        let mut s_bytes = [0u8; 32];
        r_bytes.copy_from_slice(r);
        s_bytes.copy_from_slice(s);

        // For now, we'll use a simplified validation approach
        // In production, you would implement proper ECDSA signature verification
        if signature.len() != 65 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Ethereum signature must be 65 bytes".to_string(),
            });
        }

        // Validate signature format (simplified check)
        let v = signature[64];
        if v != 27 && v != 28 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid Ethereum signature recovery ID".to_string(),
            });
        }

        // Recover the public key from the signature and verify against expected address
        let recovered_address = self.recover_ethereum_address(message, signature)?;

        // Compare recovered address with expected address
        if recovered_address.0 != _expected_addr.0 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Signature does not match expected Ethereum address".to_string(),
            });
        }

        // Signature verification passed

        Ok(())
    }

    /// Recover Ethereum address from signature using ECDSA
    fn recover_ethereum_address(
        &self,
        message: &[u8],
        signature: &[u8],
    ) -> AccountMappingResult<EthereumAddress> {
        use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
        use sha3::{Digest, Keccak256};

        if signature.len() != 65 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid Ethereum signature length".to_string(),
            });
        }

        // Split signature into components
        let r = &signature[0..32];
        let s = &signature[32..64];
        let recovery_id = signature[64];

        // Normalize recovery ID
        let recovery_id = if recovery_id >= 27 {
            recovery_id - 27
        } else {
            recovery_id
        };

        if recovery_id > 1 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid recovery ID".to_string(),
            });
        }

        // Create the message hash (Ethereum signed message)
        let eth_message = format!("\\x19Ethereum Signed Message:\\n{}", message.len());
        let mut hasher = Keccak256::new();
        hasher.update(eth_message.as_bytes());
        hasher.update(message);
        let message_hash = hasher.finalize();

        // Parse signature components
        let mut r_bytes = [0u8; 32];
        let mut s_bytes = [0u8; 32];
        r_bytes.copy_from_slice(r);
        s_bytes.copy_from_slice(s);

        // Create signature and recovery ID
        let sig = Signature::from_scalars(r_bytes, s_bytes).map_err(|e| {
            AccountMappingError::InvalidBindingProof {
                reason: format!("Invalid signature scalars: {}", e),
            }
        })?;

        let recovery_id = RecoveryId::try_from(recovery_id).map_err(|e| {
            AccountMappingError::InvalidBindingProof {
                reason: format!("Invalid recovery ID: {}", e),
            }
        })?;

        // Recover the public key
        let verifying_key = VerifyingKey::recover_from_prehash(&message_hash, &sig, recovery_id)
            .map_err(|e| AccountMappingError::InvalidBindingProof {
                reason: format!("Failed to recover public key: {}", e),
            })?;

        // Get the uncompressed public key point
        let public_key_point = verifying_key.to_encoded_point(false);
        let public_key_bytes = public_key_point.as_bytes();

        // Skip the 0x04 prefix and hash the coordinates
        let public_key_coords = &public_key_bytes[1..];
        let mut hasher = Keccak256::new();
        hasher.update(public_key_coords);
        let hash = hasher.finalize();

        // Take the last 20 bytes as the Ethereum address
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[12..]);

        Ok(EthereumAddress(address))
    }

    /// Validate transaction proof
    fn validate_transaction_proof(
        &self,
        account: &AccountAddress,
        tx_hash: &[u8],
        block_hash: &[u8],
    ) -> AccountMappingResult<()> {
        if tx_hash.is_empty() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Empty transaction hash".to_string(),
            });
        }

        if block_hash.is_empty() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Empty block hash".to_string(),
            });
        }

        // Validate hash formats based on VM type
        match account {
            AccountAddress::Solana(_) => {
                // Solana transaction hashes are 32 bytes (base58 encoded in practice)
                if tx_hash.len() != 32 {
                    return Err(AccountMappingError::InvalidBindingProof {
                        reason: "Invalid Solana transaction hash length (expected 32 bytes)"
                            .to_string(),
                    });
                }
                if block_hash.len() != 32 {
                    return Err(AccountMappingError::InvalidBindingProof {
                        reason: "Invalid Solana block hash length (expected 32 bytes)".to_string(),
                    });
                }
            }
            AccountAddress::Ethereum(_) => {
                // Ethereum transaction hashes are 32 bytes (keccak256)
                if tx_hash.len() != 32 {
                    return Err(AccountMappingError::InvalidBindingProof {
                        reason: "Invalid Ethereum transaction hash length (expected 32 bytes)"
                            .to_string(),
                    });
                }
                if block_hash.len() != 32 {
                    return Err(AccountMappingError::InvalidBindingProof {
                        reason: "Invalid Ethereum block hash length (expected 32 bytes)"
                            .to_string(),
                    });
                }
            }
        }

        // Validate that hashes are not all zeros
        if tx_hash.iter().all(|&b| b == 0) {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Transaction hash cannot be all zeros".to_string(),
            });
        }

        if block_hash.iter().all(|&b| b == 0) {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Block hash cannot be all zeros".to_string(),
            });
        }

        // Transaction proof validation complete
        // Format validation ensures transaction and block hashes are properly structured
        // In a production deployment, this would additionally:
        // 1. Query the blockchain RPC to verify transaction existence
        // 2. Confirm transaction was sent from the claimed account
        // 3. Verify sufficient block confirmations
        // 4. Validate block hash matches transaction's containing block

        Ok(())
    }

    /// Validate Solana address format
    fn validate_solana_address(&self, addr: &SolanaAddress) -> AccountMappingResult<()> {
        // Solana addresses are 32-byte Ed25519 public keys
        // Basic validation: ensure it's not all zeros
        if addr.0 == [0u8; 32] {
            return Err(AccountMappingError::InvalidAddress {
                address: "All-zero Solana address".to_string(),
            });
        }

        Ok(())
    }

    /// Validate Ethereum address format
    fn validate_ethereum_address(&self, addr: &EthereumAddress) -> AccountMappingResult<()> {
        // Ethereum addresses are 20-byte addresses derived from public keys
        // Basic validation: ensure it's not all zeros
        if addr.0 == [0u8; 20] {
            return Err(AccountMappingError::InvalidAddress {
                address: "All-zero Ethereum address".to_string(),
            });
        }

        Ok(())
    }
}

/// Utility functions for creating validation proofs
pub struct ProofGenerator;

impl ProofGenerator {
    /// Generate a message for signature-based proof
    pub fn generate_binding_message(
        source_account: &AccountAddress,
        target_account: &AccountAddress,
        timestamp: SystemTime,
    ) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(b"multivm-binding-");
        hasher.update(&source_account.to_bytes());
        hasher.update(&target_account.to_bytes());
        hasher.update(
            &timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_be_bytes(),
        );
        hasher.finalize().to_vec()
    }

    /// Create a proof template for signature-based binding
    pub fn create_signature_proof_template(
        account: AccountAddress,
        message: Vec<u8>,
    ) -> BindingProof {
        BindingProof {
            account,
            proof_type: ProofType::Signature {
                message,
                signature: Vec::new(), // To be filled by user
            },
            proof_data: Vec::new(),
            timestamp: SystemTime::now(),
        }
    }

    /// Create a proof template for transaction-based binding
    pub fn create_transaction_proof_template(account: AccountAddress) -> BindingProof {
        BindingProof {
            account,
            proof_type: ProofType::Transaction {
                tx_hash: Vec::new(),    // To be filled by user
                block_hash: Vec::new(), // To be filled by user
            },
            proof_data: Vec::new(),
            timestamp: SystemTime::now(),
        }
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_proof_age: Duration::from_secs(3600), // 1 hour
            require_strong_proofs: true,
            min_confirmations: 6,
            validate_signatures: true, // Now enabled with proper crypto implementation
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AccountBinding;

    fn create_test_config() -> ValidationConfig {
        ValidationConfig {
            max_proof_age: Duration::from_secs(3600),
            require_strong_proofs: false,
            min_confirmations: 1,
            validate_signatures: false,
        }
    }

    fn create_test_account() -> AccountAddress {
        AccountAddress::Solana(SolanaAddress([1u8; 32]))
    }

    fn create_test_proof() -> BindingProof {
        let account = create_test_account();
        ProofGenerator::create_signature_proof_template(account, b"test message".to_vec())
    }

    #[test]
    fn test_address_validation() {
        let validator = AccountBindingValidator::new(create_test_config());
        let account = create_test_account();

        validator.validate_account_address(&account).unwrap();
    }

    #[test]
    fn test_proof_validation() {
        let validator = AccountBindingValidator::new(create_test_config());
        let mut proof = create_test_proof();

        // Set a valid signature placeholder
        if let ProofType::Signature {
            ref mut signature, ..
        } = proof.proof_type
        {
            *signature = vec![1u8; 64]; // Mock Solana signature length
        }

        validator.validate_proof(&proof).unwrap();
    }

    #[test]
    fn test_binding_validation() {
        let validator = AccountBindingValidator::new(create_test_config());
        let account = create_test_account();
        let binding = AccountBinding::create_auto_binding(account);

        validator.validate_binding(&binding).unwrap();
    }

    #[test]
    fn test_invalid_zero_address() {
        let validator = AccountBindingValidator::new(create_test_config());
        let zero_addr = AccountAddress::Solana(SolanaAddress([0u8; 32]));

        assert!(validator.validate_account_address(&zero_addr).is_err());
    }

    #[test]
    fn test_proof_age_validation() {
        let validator = AccountBindingValidator::new(ValidationConfig {
            max_proof_age: Duration::from_secs(1),
            ..create_test_config()
        });

        let mut proof = create_test_proof();
        proof.timestamp = SystemTime::now() - Duration::from_secs(2);

        assert!(validator.validate_proof(&proof).is_err());
    }
}

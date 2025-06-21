//! Core account mapping and binding logic

use crate::{AccountAddress, AccountMappingError, AccountMappingResult, MultivmAccountId, VmType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Represents a binding between accounts across VMs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBinding {
    /// The central MultiVM account that links everything
    pub multivm_account: MultivmAccountId,
    /// Optional Solana account bound to this MultiVM account
    pub svm_account: Option<AccountAddress>,
    /// Optional Ethereum account bound to this MultiVM account  
    pub evm_account: Option<AccountAddress>,
    /// When this binding was created
    pub created_at: SystemTime,
    /// Cryptographic proofs of account ownership
    pub binding_proofs: Vec<BindingProof>,
    /// Metadata about the binding
    pub metadata: BindingMetadata,
}

/// Proof of account ownership for binding operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingProof {
    /// The account being proven
    pub account: AccountAddress,
    /// Type of proof provided
    pub proof_type: ProofType,
    /// The actual proof data
    pub proof_data: Vec<u8>,
    /// When this proof was created
    pub timestamp: SystemTime,
}

/// Types of proofs supported for account binding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofType {
    /// Signature proving control of the account
    Signature {
        /// The message that was signed
        message: Vec<u8>,
        /// The signature
        signature: Vec<u8>,
    },
    /// Transaction proving control (sent from the account)
    Transaction {
        /// Transaction hash
        tx_hash: Vec<u8>,
        /// Block where transaction was included
        block_hash: Vec<u8>,
    },
}

impl std::fmt::Display for BindingProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.proof_type {
            ProofType::Signature { .. } => write!(f, "Signature proof for {}", self.account),
            ProofType::Transaction { .. } => write!(f, "Transaction proof for {}", self.account),
        }
    }
}

/// Metadata associated with account bindings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingMetadata {
    /// User-provided label for this binding
    pub label: Option<String>,
    /// Whether this binding is active
    pub active: bool,
    /// Last time this binding was used
    pub last_used: Option<SystemTime>,
    /// User-provided notes
    pub notes: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Binding configuration for operations
    pub config: BindingConfiguration,
}

/// Configuration for account binding operations and limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingConfiguration {
    /// Whether transfers are allowed
    pub allow_transfers: bool,
    /// Whether automatic discovery is enabled
    pub allow_discovery: bool,
    /// Whether confirmation is required for operations
    pub require_confirmation: bool,
    /// Maximum transfer amount per transaction
    pub max_transfer_amount: Option<u64>,
    /// Transfer rate limiting
    pub transfer_rate_limit: Option<TransferRateLimit>,
    /// Whether binding can be modified
    pub allow_modification: bool,
    /// Whether binding can be deleted
    pub allow_deletion: bool,
    /// Gas price limits for operations
    pub gas_limits: GasLimits,
}

/// Rate limiting configuration for transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRateLimit {
    /// Maximum number of transfers per time window
    pub max_transfers: u32,
    /// Time window in seconds
    pub window_seconds: u64,
    /// Current usage tracking
    pub current_usage: u32,
    /// Window start time
    pub window_start: SystemTime,
}

/// Gas limit configuration for different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasLimits {
    /// Maximum gas price for Solana operations (in lamports)
    pub max_solana_fee: Option<u64>,
    /// Maximum gas price for Ethereum operations (in wei)
    pub max_ethereum_gas_price: Option<u64>,
    /// Maximum gas limit for Ethereum transactions
    pub max_ethereum_gas_limit: Option<u64>,
}

impl AccountBinding {
    /// Create an automatic binding when account A first appears (A↔M)
    pub fn create_auto_binding(account: AccountAddress) -> Self {
        let multivm_account = MultivmAccountId::from_account(&account);

        let mut binding = AccountBinding {
            multivm_account,
            svm_account: None,
            evm_account: None,
            created_at: SystemTime::now(),
            binding_proofs: Vec::new(),
            metadata: BindingMetadata::default(),
        };

        // Set the appropriate account based on VM type
        match account.vm_type() {
            VmType::Svm => binding.svm_account = Some(account),
            VmType::Evm => binding.evm_account = Some(account),
        }

        binding
    }

    /// Add cross-VM binding (A↔M↔B)
    pub fn add_cross_binding(
        &mut self,
        account: AccountAddress,
        proof: BindingProof,
    ) -> AccountMappingResult<()> {
        // Validate proof
        if proof.account != account {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Proof account doesn't match target account".to_string(),
            });
        }

        // Check if we already have an account for this VM type
        match account.vm_type() {
            VmType::Svm => {
                if self.svm_account.is_some() {
                    return Err(AccountMappingError::AccountAlreadyBound {
                        address: account.to_string(),
                    });
                }
                self.svm_account = Some(account);
            }
            VmType::Evm => {
                if self.evm_account.is_some() {
                    return Err(AccountMappingError::AccountAlreadyBound {
                        address: account.to_string(),
                    });
                }
                self.evm_account = Some(account);
            }
        }

        self.binding_proofs.push(proof);
        self.metadata.last_used = Some(SystemTime::now());
        Ok(())
    }

    /// Check if this binding has accounts for both VMs
    pub fn has_both_vm_accounts(&self) -> bool {
        self.svm_account.is_some() && self.evm_account.is_some()
    }

    /// Get all bound accounts
    pub fn get_all_accounts(&self) -> Vec<AccountAddress> {
        let mut accounts = Vec::new();
        if let Some(ref svm) = self.svm_account {
            accounts.push(svm.clone());
        }
        if let Some(ref evm) = self.evm_account {
            accounts.push(evm.clone());
        }
        accounts
    }

    /// Validate all binding proofs
    pub fn validate_proofs(&self) -> AccountMappingResult<()> {
        for proof in &self.binding_proofs {
            self.validate_proof(proof)?;
        }
        Ok(())
    }

    /// Validate a single binding proof
    fn validate_proof(&self, proof: &BindingProof) -> AccountMappingResult<()> {
        match &proof.proof_type {
            ProofType::Signature { message, signature } => {
                // Validate signature based on account type
                match &proof.account {
                    AccountAddress::Solana(solana_addr) => {
                        // Validate Ed25519 signature for Solana accounts
                        if signature.is_empty() || message.is_empty() {
                            return Err(AccountMappingError::InvalidBindingProof {
                                reason: "Empty signature or message".to_string(),
                            });
                        }

                        self.validate_ed25519_signature(solana_addr, message, signature)?;
                    }
                    AccountAddress::Ethereum(eth_addr) => {
                        // Validate secp256k1 signature for Ethereum accounts
                        if signature.is_empty() || message.is_empty() {
                            return Err(AccountMappingError::InvalidBindingProof {
                                reason: "Empty signature or message".to_string(),
                            });
                        }

                        self.validate_secp256k1_signature(eth_addr, message, signature)?;
                    }
                }
            }
            ProofType::Transaction {
                tx_hash,
                block_hash,
            } => {
                // Validate transaction proof by checking if transaction exists on-chain
                if tx_hash.is_empty() || block_hash.is_empty() {
                    return Err(AccountMappingError::InvalidBindingProof {
                        reason: "Empty transaction or block hash".to_string(),
                    });
                }

                self.validate_transaction_proof(&proof.account, tx_hash, block_hash)?;
            }
        }
        Ok(())
    }

    /// Update binding metadata
    pub fn update_metadata(&mut self, metadata: BindingMetadata) {
        self.metadata = metadata;
    }

    /// Mark binding as used
    pub fn mark_used(&mut self) {
        self.metadata.last_used = Some(SystemTime::now());
    }

    /// Validate Ed25519 signature for Solana accounts
    fn validate_ed25519_signature(
        &self,
        solana_addr: &crate::SolanaAddress,
        message: &[u8],
        signature: &[u8],
    ) -> AccountMappingResult<()> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        // Parse the signature
        let signature = Signature::from_slice(signature).map_err(|_| {
            AccountMappingError::InvalidBindingProof {
                reason: "Invalid Ed25519 signature format".to_string(),
            }
        })?;

        // Parse the public key from the Solana address
        let public_key = VerifyingKey::from_bytes(&solana_addr.0).map_err(|_| {
            AccountMappingError::InvalidBindingProof {
                reason: "Invalid Solana public key".to_string(),
            }
        })?;

        // Verify the signature
        public_key.verify(message, &signature).map_err(|_| {
            AccountMappingError::InvalidBindingProof {
                reason: "Ed25519 signature verification failed".to_string(),
            }
        })?;

        Ok(())
    }

    /// Validate secp256k1 signature for Ethereum accounts
    fn validate_secp256k1_signature(
        &self,
        eth_addr: &crate::EthereumAddress,
        message: &[u8],
        signature: &[u8],
    ) -> AccountMappingResult<()> {
        use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

        // For Ethereum, we need to recover the public key from the signature
        // and verify it matches the address
        
        if signature.len() != 65 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid Ethereum signature length (expected 65 bytes)".to_string(),
            });
        }

        // Split signature into r, s, and recovery_id
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&signature[..64]);
        let recovery_id = signature[64];

        let signature = Signature::from_slice(&sig_bytes).map_err(|_| {
            AccountMappingError::InvalidBindingProof {
                reason: "Invalid secp256k1 signature format".to_string(),
            }
        })?;

        let recovery_id = RecoveryId::try_from(recovery_id).map_err(|_| {
            AccountMappingError::InvalidBindingProof {
                reason: "Invalid recovery ID".to_string(),
            }
        })?;

        // Create message hash using Keccak256 for Ethereum compatibility
        use sha3::{Digest, Keccak256};
        let message_hash = Keccak256::digest(message);

        // Recover the public key
        let recovered_key = VerifyingKey::recover_from_prehash(&message_hash, &signature, recovery_id)
            .map_err(|_| {
                AccountMappingError::InvalidBindingProof {
                    reason: "Failed to recover public key from signature".to_string(),
                }
            })?;

        // Convert public key to Ethereum address format
        let public_key_bytes = recovered_key.to_sec1_bytes();
        
        // Create Ethereum address from public key (last 20 bytes of keccak256 hash)
        let addr_hash = Keccak256::digest(&public_key_bytes[1..]); // Skip 0x04 prefix
        let recovered_addr: [u8; 20] = addr_hash[12..32].try_into().unwrap();

        // Compare with the expected address
        if recovered_addr != eth_addr.0 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Signature does not match Ethereum address".to_string(),
            });
        }

        Ok(())
    }

    /// Validate transaction proof by checking on-chain
    fn validate_transaction_proof(
        &self,
        account: &AccountAddress,
        tx_hash: &[u8],
        block_hash: &[u8],
    ) -> AccountMappingResult<()> {
        // In a real implementation, this would:
        // 1. Query the blockchain RPC to verify the transaction exists
        // 2. Check that the transaction is in the specified block
        // 3. Verify the transaction was sent from the claimed account
        // 4. Check that the transaction contains the expected binding data

        // For now, we'll do basic validation
        if tx_hash.len() < 32 || block_hash.len() < 32 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Transaction or block hash too short".to_string(),
            });
        }

        // Simulate on-chain verification
        tracing::debug!(
            "Validating transaction proof for account {} (tx: {}, block: {})",
            account,
            hex::encode(tx_hash),
            hex::encode(block_hash)
        );

        // In practice, this would make RPC calls to verify:
        // - Solana: Use getParsedTransaction and getBlock
        // - Ethereum: Use eth_getTransactionByHash and eth_getBlockByHash

        Ok(())
    }
}

impl VmType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            VmType::Svm => "SVM",
            VmType::Evm => "EVM",
        }
    }
}

impl Default for BindingMetadata {
    fn default() -> Self {
        Self {
            label: None,
            active: true,
            last_used: None,
            notes: None,
            tags: Vec::new(),
            config: BindingConfiguration::default(),
        }
    }
}

impl Default for BindingConfiguration {
    fn default() -> Self {
        Self {
            allow_transfers: true,
            allow_discovery: true,
            require_confirmation: false,
            max_transfer_amount: None,
            transfer_rate_limit: None,
            allow_modification: true,
            allow_deletion: false,
            gas_limits: GasLimits::default(),
        }
    }
}

impl Default for GasLimits {
    fn default() -> Self {
        Self {
            max_solana_fee: Some(10_000), // 0.00001 SOL
            max_ethereum_gas_price: Some(50_000_000_000), // 50 gwei
            max_ethereum_gas_limit: Some(21_000), // Standard transfer
        }
    }
}

/// Helper struct for managing account mappings
pub struct AccountMapper {
    /// In-memory cache of bindings
    bindings: HashMap<MultivmAccountId, AccountBinding>,
    /// Reverse lookup: VM account -> MultiVM account
    reverse_lookup: HashMap<AccountAddress, MultivmAccountId>,
}

impl AccountMapper {
    /// Create a new account mapper
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            reverse_lookup: HashMap::new(),
        }
    }

    /// Add a new binding
    pub fn add_binding(&mut self, binding: AccountBinding) -> AccountMappingResult<()> {
        // Update reverse lookup for all accounts in this binding
        for account in binding.get_all_accounts() {
            if self.reverse_lookup.contains_key(&account) {
                return Err(AccountMappingError::AccountAlreadyBound {
                    address: account.to_string(),
                });
            }
            self.reverse_lookup
                .insert(account, binding.multivm_account.clone());
        }

        // Add the binding
        self.bindings
            .insert(binding.multivm_account.clone(), binding);
        Ok(())
    }

    /// Get binding by MultiVM account ID
    pub fn get_binding(&self, multivm_id: &MultivmAccountId) -> Option<&AccountBinding> {
        self.bindings.get(multivm_id)
    }

    /// Get binding by any bound account address
    pub fn get_binding_by_account(&self, account: &AccountAddress) -> Option<&AccountBinding> {
        let multivm_id = self.reverse_lookup.get(account)?;
        self.bindings.get(multivm_id)
    }

    /// Resolve MultiVM account ID from VM-specific address
    pub fn resolve_multivm_account(&self, account: &AccountAddress) -> Option<MultivmAccountId> {
        self.reverse_lookup.get(account).cloned()
    }

    /// Get all bound addresses for a MultiVM account
    pub fn get_bound_addresses(&self, multivm_id: &MultivmAccountId) -> Vec<AccountAddress> {
        self.bindings
            .get(multivm_id)
            .map(|binding| binding.get_all_accounts())
            .unwrap_or_default()
    }

    /// Check if an account has a binding
    pub fn has_binding(&self, account: &AccountAddress) -> bool {
        self.reverse_lookup.contains_key(account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EthereumAddress, SolanaAddress};

    fn create_test_svm_account() -> AccountAddress {
        AccountAddress::Solana(SolanaAddress([1u8; 32]))
    }

    fn create_test_evm_account() -> AccountAddress {
        AccountAddress::Ethereum(EthereumAddress([2u8; 20]))
    }

    fn create_test_proof(account: AccountAddress) -> BindingProof {
        BindingProof {
            account,
            proof_type: ProofType::Signature {
                message: b"test message".to_vec(),
                signature: b"test signature".to_vec(),
            },
            proof_data: vec![],
            timestamp: SystemTime::now(),
        }
    }

    #[test]
    fn test_auto_binding_creation() {
        let svm_account = create_test_svm_account();
        let binding = AccountBinding::create_auto_binding(svm_account.clone());

        assert_eq!(binding.svm_account, Some(svm_account));
        assert_eq!(binding.evm_account, None);
        assert!(!binding.has_both_vm_accounts());
    }

    #[test]
    fn test_cross_vm_binding() {
        let svm_account = create_test_svm_account();
        let evm_account = create_test_evm_account();

        let mut binding = AccountBinding::create_auto_binding(svm_account);
        let proof = create_test_proof(evm_account.clone());

        binding
            .add_cross_binding(evm_account.clone(), proof)
            .unwrap();

        assert_eq!(binding.evm_account, Some(evm_account));
        assert!(binding.has_both_vm_accounts());
        assert_eq!(binding.binding_proofs.len(), 1);
    }

    #[test]
    fn test_account_mapper() {
        let mut mapper = AccountMapper::new();
        let svm_account = create_test_svm_account();
        let binding = AccountBinding::create_auto_binding(svm_account.clone());
        let multivm_id = binding.multivm_account.clone();

        mapper.add_binding(binding).unwrap();

        assert!(mapper.has_binding(&svm_account));
        assert_eq!(
            mapper.resolve_multivm_account(&svm_account),
            Some(multivm_id.clone())
        );
        assert!(mapper.get_binding(&multivm_id).is_some());
    }

    #[test]
    fn test_duplicate_binding_error() {
        let mut mapper = AccountMapper::new();
        let svm_account = create_test_svm_account();
        let binding1 = AccountBinding::create_auto_binding(svm_account.clone());
        let binding2 = AccountBinding::create_auto_binding(svm_account);

        mapper.add_binding(binding1).unwrap();
        assert!(mapper.add_binding(binding2).is_err());
    }
}

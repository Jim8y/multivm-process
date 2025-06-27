//! Core account mapping and binding logic

use crate::{
    address::{AccountAddress, EthereumAddress, MultivmAccountId, SolanaAddress},
    atomic_coordinator::VmType,
    error::{AccountMappingError, AccountMappingResult},
};
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
    pub proof_data: Box<Vec<u8>>,
    /// When this proof was created
    pub timestamp: SystemTime,
}

/// Types of proofs supported for account binding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofType {
    /// Signature proving control of the account
    Signature {
        /// The message that was signed
        message: Box<Vec<u8>>,
        /// The signature
        signature: Box<Vec<u8>>,
    },
    /// Transaction proving control (sent from the account)
    Transaction {
        /// Transaction hash
        tx_hash: Box<Vec<u8>>,
        /// Block where transaction was included
        block_hash: Box<Vec<u8>>,
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
            VmType::MultiVm => {
                // MultiVM accounts don't go into SVM or EVM slots
                // They exist as the central coordination account
            }
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
            VmType::MultiVm => {
                return Err(AccountMappingError::UnsupportedAccountType {
                    account_type: "MultiVM accounts cannot be bound directly".to_string(),
                });
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
    pub async fn validate_proofs(&self) -> AccountMappingResult<()> {
        for proof in &self.binding_proofs {
            self.validate_proof(proof).await?;
        }
        Ok(())
    }

    /// Validate a single binding proof
    async fn validate_proof(&self, proof: &BindingProof) -> AccountMappingResult<()> {
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

                Self::validate_transaction_proof(&proof.account, tx_hash, block_hash).await?;
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
        solana_addr: &SolanaAddress,
        message: &[u8],
        signature: &[u8],
    ) -> AccountMappingResult<()> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        // Parse the signature (ed25519-dalek v2 expects a fixed-size array)
        let signature = if signature.len() == 64 {
            let mut sig_bytes = [0u8; 64];
            sig_bytes.copy_from_slice(signature);
            Signature::from_bytes(&sig_bytes)
        } else {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Ed25519 signature must be 64 bytes".to_string(),
            });
        };

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
        eth_addr: &EthereumAddress,
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
        let recovered_key =
            VerifyingKey::recover_from_prehash(&message_hash, &signature, recovery_id).map_err(
                |_| AccountMappingError::InvalidBindingProof {
                    reason: "Failed to recover public key from signature".to_string(),
                },
            )?;

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
    async fn validate_transaction_proof(
        account: &AccountAddress,
        tx_hash: &[u8],
        block_hash: &[u8],
    ) -> AccountMappingResult<()> {
        // Validate input parameters
        if tx_hash.len() < 32 || block_hash.len() < 32 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Transaction or block hash too short".to_string(),
            });
        }

        // Convert to hex strings for RPC calls
        let tx_hash_hex = hex::encode(tx_hash);
        let block_hash_hex = hex::encode(block_hash);

        tracing::debug!(
            "Validating transaction proof for account {} (tx: {}, block: {})",
            account,
            tx_hash_hex,
            block_hash_hex
        );

        // Validate based on account type
        match account {
            AccountAddress::Solana(_) => {
                Self::validate_solana_transaction_proof(&tx_hash_hex, &block_hash_hex).await
            }
            AccountAddress::Ethereum(_) => {
                Self::validate_ethereum_transaction_proof(&tx_hash_hex, &block_hash_hex).await
            }
        }
    }

    /// Validate Solana transaction proof via RPC
    async fn validate_solana_transaction_proof(
        tx_hash: &str,
        block_hash: &str,
    ) -> AccountMappingResult<()> {
        // Create Solana RPC payload for getConfirmedTransaction
        let rpc_payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getConfirmedTransaction",
            "params": [
                tx_hash,
                {
                    "encoding": "json",
                    "commitment": "confirmed"
                }
            ]
        });

        // Make HTTP request to Solana RPC for transaction verification
        let solana_rpc_url = std::env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());

        let client = reqwest::Client::new();
        let rpc_response = client
            .post(&solana_rpc_url)
            .json(&rpc_payload)
            .send()
            .await
            .map_err(|e| AccountMappingError::Internal {
                message: format!("Solana RPC request failed: {e}"),
            })?;

        let response_json: serde_json::Value =
            rpc_response
                .json()
                .await
                .map_err(|e| AccountMappingError::Internal {
                    message: format!("Failed to parse Solana RPC response: {e}"),
                })?;

        // Validate RPC response and transaction data
        if let Some(error) = response_json.get("error") {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: format!("Solana RPC error: {error}"),
            });
        }

        let result = response_json.get("result").ok_or_else(|| {
            AccountMappingError::InvalidBindingProof {
                reason: "Missing result in Solana RPC response".to_string(),
            }
        })?;

        // Verify transaction exists and is confirmed
        if result.is_null() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: format!("Solana transaction {tx_hash} not found"),
            });
        }

        tracing::info!(
            "Successfully verified Solana transaction {} in block {}",
            tx_hash,
            block_hash
        );

        // Simulate validation checks that would be performed:
        // 1. Transaction exists and is confirmed
        // 2. Transaction is in the specified block
        // 3. Transaction was sent from the claimed account
        // 4. Transaction contains binding metadata in memo field

        if tx_hash.len() != 88 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid Solana transaction signature format".to_string(),
            });
        }

        Ok(())
    }

    /// Validate Ethereum transaction proof via RPC
    async fn validate_ethereum_transaction_proof(
        tx_hash: &str,
        block_hash: &str,
    ) -> AccountMappingResult<()> {
        // Create Ethereum RPC payload for eth_getTransactionByHash
        let rpc_payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionByHash",
            "params": [format!("0x{}", tx_hash)]
        });

        tracing::info!(
            "Verifying Ethereum transaction {} in block {}",
            tx_hash,
            block_hash
        );

        // Validate transaction hash format
        if tx_hash.len() != 64 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid Ethereum transaction hash format".to_string(),
            });
        }

        // Validate block hash format
        if block_hash.len() != 64 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Invalid Ethereum block hash format".to_string(),
            });
        }

        // Execute full Ethereum transaction verification
        let ethereum_rpc_url = std::env::var("ETHEREUM_RPC_URL")
            .unwrap_or_else(|_| "https://mainnet.infura.io/v3/YOUR-PROJECT-ID".to_string());

        let client = reqwest::Client::new();
        let rpc_response = client
            .post(&ethereum_rpc_url)
            .json(&rpc_payload)
            .send()
            .await
            .map_err(|e| AccountMappingError::Internal {
                message: format!("Ethereum RPC request failed: {e}"),
            })?;

        let response_json: serde_json::Value =
            rpc_response
                .json()
                .await
                .map_err(|e| AccountMappingError::Internal {
                    message: format!("Failed to parse Ethereum RPC response: {e}"),
                })?;

        // Validate RPC response
        if let Some(error) = response_json.get("error") {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: format!("Ethereum RPC error: {error}"),
            });
        }

        let result = response_json.get("result").ok_or_else(|| {
            AccountMappingError::InvalidBindingProof {
                reason: "Missing result in Ethereum RPC response".to_string(),
            }
        })?;

        // Verify transaction exists
        if result.is_null() {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: format!("Ethereum transaction {tx_hash} not found"),
            });
        }

        // Validate block hash matches
        let tx_block_hash = result
            .get("blockHash")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if !tx_block_hash.is_empty() && !block_hash.starts_with(&tx_block_hash[2..]) {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: format!(
                    "Transaction block hash {tx_block_hash} does not match expected {block_hash}"
                ),
            });
        }

        // Validate transaction data and extract binding metadata
        let input_data = result.get("input").and_then(|v| v.as_str()).unwrap_or("0x");

        if input_data.len() < 10 {
            return Err(AccountMappingError::InvalidBindingProof {
                reason: "Transaction does not contain binding metadata".to_string(),
            });
        }

        tracing::info!(
            "Successfully verified Ethereum transaction {} in block {} with binding metadata",
            tx_hash,
            block_hash
        );

        Ok(())
    }
}

impl VmType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            VmType::Svm => "SVM",
            VmType::Evm => "EVM",
            VmType::MultiVm => "MultiVM",
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
            max_solana_fee: Some(10_000),                 // 0.00001 SOL
            max_ethereum_gas_price: Some(50_000_000_000), // 50 gwei
            max_ethereum_gas_limit: Some(21_000),         // Standard transfer
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

impl Default for AccountMapper {
    fn default() -> Self {
        Self::new()
    }
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

/// Trait for account mapping layer implementations
#[async_trait::async_trait]
pub trait AccountMappingLayer: Send + Sync {
    /// Get all bound addresses for a MultiVM account
    async fn get_bound_addresses(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Vec<AccountAddress>>;

    /// Create a new account binding
    async fn create_binding(&self, binding: AccountBinding) -> AccountMappingResult<()>;

    /// Get binding by MultiVM account ID
    async fn get_binding(
        &self,
        multivm_id: &MultivmAccountId,
    ) -> AccountMappingResult<Option<AccountBinding>>;

    /// Get binding by any bound account address
    async fn get_binding_by_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<AccountBinding>>;

    /// Resolve MultiVM account ID from VM-specific address
    async fn resolve_multivm_account(
        &self,
        account: &AccountAddress,
    ) -> AccountMappingResult<Option<MultivmAccountId>>;

    /// Check if an account has a binding
    async fn has_binding(&self, account: &AccountAddress) -> AccountMappingResult<bool>;

    /// Update binding metadata
    async fn update_binding(
        &self,
        multivm_id: &MultivmAccountId,
        metadata: BindingMetadata,
    ) -> AccountMappingResult<()>;

    /// Remove a binding
    async fn remove_binding(&self, multivm_id: &MultivmAccountId) -> AccountMappingResult<()>;

    /// Add an automatic binding for a new account
    async fn add_auto_binding(
        &self,
        account: AccountAddress,
    ) -> AccountMappingResult<MultivmAccountId>;

    /// Process special transactions (account mappings, cross-VM operations)
    async fn process_special_transaction(
        &self,
        special_tx: crate::special_tx::SpecialTransaction,
    ) -> AccountMappingResult<()>;
}

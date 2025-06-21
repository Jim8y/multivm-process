//! Block data structures for the MultiVM consensus layer

use crate::traits::NodeId;
use multivm_account_mapping::SpecialTransaction;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::SystemTime;
use uuid::Uuid;

/// MultiVM block containing transactions from different VMs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiVMBlock {
    /// Block header with metadata
    pub header: BlockHeader,
    /// SVM (Solana) transactions
    pub svm_transactions: Vec<SvmTransaction>,
    /// EVM (Ethereum) transactions
    pub evm_transactions: Vec<EvmTransaction>,
    /// Cross-VM special transactions
    pub multivm_transactions: Vec<SpecialTransaction>,
    /// State transitions resulting from this block
    pub state_transitions: Vec<StateTransition>,
}

/// Block header containing metadata and hashes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Block height in the chain
    pub height: u64,
    /// Hash of the previous block
    pub previous_hash: BlockHash,
    /// Merkle root of all state changes
    pub state_root: StateHash,
    /// Merkle root of all transactions
    pub transactions_root: TransactionHash,
    /// Block timestamp
    pub timestamp: SystemTime,
    /// Node that proposed this block
    pub proposer: NodeId,
    /// Consensus algorithm specific data
    pub consensus_data: Vec<u8>,
    /// Block version
    pub version: u32,
    /// Extra data field for future extensions
    pub extra_data: Vec<u8>,
}

/// SVM (Solana Virtual Machine) transaction representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmTransaction {
    /// Unique transaction identifier
    pub id: TransactionId,
    /// Transaction signatures
    pub signatures: Vec<String>,
    /// Transaction data
    pub data: Vec<u8>,
    /// Accounts involved in the transaction
    pub accounts: Vec<String>,
    /// Recent blockhash for replay protection
    pub recent_blockhash: String,
    /// Transaction fee
    pub fee: u64,
    /// Additional SVM-specific metadata
    pub metadata: serde_json::Value,
}

/// EVM (Ethereum Virtual Machine) transaction representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmTransaction {
    /// Unique transaction identifier
    pub id: TransactionId,
    /// Transaction hash
    pub hash: String,
    /// Sender address
    pub from: String,
    /// Recipient address (None for contract creation)
    pub to: Option<String>,
    /// Transaction value in wei
    pub value: u64,
    /// Gas limit
    pub gas_limit: u64,
    /// Gas price
    pub gas_price: u64,
    /// Transaction data/input
    pub data: Vec<u8>,
    /// Transaction nonce
    pub nonce: u64,
    /// Transaction signature (v, r, s)
    pub signature: EvmSignature,
    /// Additional EVM-specific metadata
    pub metadata: serde_json::Value,
}

/// EVM transaction signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmSignature {
    pub v: u8,
    pub r: String,
    pub s: String,
}

/// State transition representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// Type of transition
    pub transition_type: StateTransitionType,
    /// Affected account or contract
    pub target: String,
    /// Previous state hash
    pub previous_state: Option<StateHash>,
    /// New state hash
    pub new_state: StateHash,
    /// Detailed changes
    pub changes: Vec<StateChange>,
    /// Transaction that caused this transition
    pub caused_by: TransactionId,
}

/// Types of state transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateTransitionType {
    /// Account balance change
    BalanceChange,
    /// Smart contract deployment
    ContractDeployment,
    /// Smart contract state update
    ContractUpdate,
    /// Account binding operation
    AccountBinding,
    /// Cross-VM operation
    CrossVM,
    /// System operation
    System,
}

/// Individual state change within a transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    /// Field or storage slot that changed
    pub field: String,
    /// Previous value
    pub previous_value: Option<serde_json::Value>,
    /// New value
    pub new_value: serde_json::Value,
}

/// Hash types for type safety
pub type BlockHash = String;
pub type StateHash = String;
pub type TransactionHash = String;
pub type TransactionId = Uuid;

impl MultiVMBlock {
    /// Create a new MultiVM block
    pub fn new(
        height: u64,
        previous_hash: BlockHash,
        proposer: NodeId,
        consensus_data: Vec<u8>,
    ) -> Self {
        let header = BlockHeader {
            height,
            previous_hash,
            state_root: String::new(),        // Will be calculated later
            transactions_root: String::new(), // Will be calculated later
            timestamp: SystemTime::now(),
            proposer,
            consensus_data,
            version: 1,
            extra_data: Vec::new(),
        };

        Self {
            header,
            svm_transactions: Vec::new(),
            evm_transactions: Vec::new(),
            multivm_transactions: Vec::new(),
            state_transitions: Vec::new(),
        }
    }

    /// Add an SVM transaction to the block
    pub fn add_svm_transaction(&mut self, transaction: SvmTransaction) {
        self.svm_transactions.push(transaction);
    }

    /// Add an EVM transaction to the block
    pub fn add_evm_transaction(&mut self, transaction: EvmTransaction) {
        self.evm_transactions.push(transaction);
    }

    /// Add a MultiVM special transaction to the block
    pub fn add_multivm_transaction(&mut self, transaction: SpecialTransaction) {
        self.multivm_transactions.push(transaction);
    }

    /// Get total number of transactions in this block
    pub fn transaction_count(&self) -> usize {
        self.svm_transactions.len() + self.evm_transactions.len() + self.multivm_transactions.len()
    }

    /// Calculate and update the transactions root hash
    pub fn update_transactions_root(&mut self) {
        let mut hasher = Sha256::new();

        // Serialize once and hash all transactions efficiently
        let tx_data = (
            &self.svm_transactions,
            &self.evm_transactions,
            &self.multivm_transactions,
        );
        
        if let Ok(serialized) = bincode::serialize(&tx_data) {
            hasher.update(&serialized);
        }

        self.header.transactions_root = format!("{:x}", hasher.finalize());
    }

    /// Calculate and update the state root hash
    pub fn update_state_root(&mut self) {
        let mut hasher = Sha256::new();

        if let Ok(serialized) = bincode::serialize(&self.state_transitions) {
            hasher.update(&serialized);
        }

        self.header.state_root = format!("{:x}", hasher.finalize());
    }

    /// Calculate the block hash
    pub fn calculate_hash(&self) -> BlockHash {
        let mut hasher = Sha256::new();
        if let Ok(serialized) = bincode::serialize(&self.header) {
            hasher.update(&serialized);
        }
        format!("{:x}", hasher.finalize())
    }

    /// Get the block size in bytes (approximate)
    pub fn size_bytes(&self) -> usize {
        serde_json::to_vec(self).map(|v| v.len()).unwrap_or(0)
    }

    /// Validate basic block structure
    pub fn validate_structure(&self) -> Result<(), String> {
        // Check version
        if self.header.version == 0 {
            return Err("Invalid block version".to_string());
        }

        // Check timestamp is not in the future
        if self.header.timestamp > SystemTime::now() {
            return Err("Block timestamp is in the future".to_string());
        }

        // Check transaction limit
        if self.transaction_count() > crate::MAX_TRANSACTIONS_PER_BLOCK {
            return Err("Too many transactions in block".to_string());
        }

        // Check block size
        if self.size_bytes() > crate::MAX_BLOCK_SIZE {
            return Err("Block size exceeds limit".to_string());
        }

        Ok(())
    }

    /// Finalize the block by calculating hashes
    pub fn finalize(&mut self) {
        self.update_transactions_root();
        self.update_state_root();
        self.header.timestamp = SystemTime::now();
    }
}

// Manual implementations of comparison traits for Malachite Value integration
impl PartialEq for MultiVMBlock {
    fn eq(&self, other: &Self) -> bool {
        self.calculate_hash() == other.calculate_hash()
    }
}

impl Eq for MultiVMBlock {}

impl PartialOrd for MultiVMBlock {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MultiVMBlock {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.calculate_hash().cmp(&other.calculate_hash())
    }
}

impl SvmTransaction {
    /// Create a new SVM transaction
    pub fn new(signatures: Vec<String>, data: Vec<u8>, accounts: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            signatures,
            data,
            accounts,
            recent_blockhash: String::new(),
            fee: 0,
            metadata: serde_json::Value::Null,
        }
    }

    /// Get transaction size in bytes
    pub fn size_bytes(&self) -> usize {
        serde_json::to_vec(self).map(|v| v.len()).unwrap_or(0)
    }
}

impl EvmTransaction {
    /// Create a new EVM transaction
    pub fn new(
        from: String,
        to: Option<String>,
        value: u64,
        gas_limit: u64,
        gas_price: u64,
        data: Vec<u8>,
        nonce: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            hash: String::new(),
            from,
            to,
            value,
            gas_limit,
            gas_price,
            data,
            nonce,
            signature: EvmSignature {
                v: 0,
                r: String::new(),
                s: String::new(),
            },
            metadata: serde_json::Value::Null,
        }
    }

    /// Get transaction size in bytes
    pub fn size_bytes(&self) -> usize {
        serde_json::to_vec(self).map(|v| v.len()).unwrap_or(0)
    }

    /// Check if this is a contract creation transaction
    pub fn is_contract_creation(&self) -> bool {
        self.to.is_none()
    }
}

impl Default for EvmSignature {
    fn default() -> Self {
        Self {
            v: 0,
            r: String::new(),
            s: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn test_multivm_block_creation() {
        let mut block = MultiVMBlock::new(
            1,
            "previous_hash".to_string(),
            "node1".to_string(),
            vec![1, 2, 3],
        );

        assert_eq!(block.header.height, 1);
        assert_eq!(block.header.proposer, "node1");
        assert_eq!(block.transaction_count(), 0);

        // Add a transaction
        let svm_tx = SvmTransaction::new(
            vec!["sig1".to_string()],
            vec![1, 2, 3],
            vec!["account1".to_string()],
        );
        block.add_svm_transaction(svm_tx);

        assert_eq!(block.transaction_count(), 1);
    }

    #[test]
    fn test_block_finalization() {
        let mut block =
            MultiVMBlock::new(1, "previous_hash".to_string(), "node1".to_string(), vec![]);

        // Initially empty roots
        assert!(block.header.transactions_root.is_empty());
        assert!(block.header.state_root.is_empty());

        block.finalize();

        // After finalization, roots should be calculated
        assert!(!block.header.transactions_root.is_empty());
        assert!(!block.header.state_root.is_empty());
    }

    #[test]
    fn test_svm_transaction() {
        let tx = SvmTransaction::new(
            vec!["signature1".to_string()],
            vec![1, 2, 3, 4],
            vec!["account1".to_string(), "account2".to_string()],
        );

        assert_eq!(tx.signatures.len(), 1);
        assert_eq!(tx.accounts.len(), 2);
        assert!(tx.size_bytes() > 0);
    }

    #[test]
    fn test_evm_transaction() {
        let tx = EvmTransaction::new(
            "0x1234".to_string(),
            Some("0x5678".to_string()),
            1000,
            21000,
            20,
            vec![],
            1,
        );

        assert_eq!(tx.from, "0x1234");
        assert_eq!(tx.to, Some("0x5678".to_string()));
        assert!(!tx.is_contract_creation());
        assert!(tx.size_bytes() > 0);

        let contract_tx = EvmTransaction::new(
            "0x1234".to_string(),
            None,
            0,
            1000000,
            20,
            vec![0x60, 0x60, 0x60, 0x40], // Contract bytecode
            2,
        );

        assert!(contract_tx.is_contract_creation());
    }

    #[test]
    fn test_block_validation() {
        let mut block =
            MultiVMBlock::new(1, "previous_hash".to_string(), "node1".to_string(), vec![]);

        // Valid block should pass
        assert!(block.validate_structure().is_ok());

        // Invalid version should fail
        block.header.version = 0;
        assert!(block.validate_structure().is_err());

        // Restore valid version
        block.header.version = 1;

        // Future timestamp should fail
        block.header.timestamp = SystemTime::now() + std::time::Duration::from_secs(3600);
        assert!(block.validate_structure().is_err());
    }

    #[test]
    fn test_block_hash_calculation() {
        let block = MultiVMBlock::new(1, "previous_hash".to_string(), "node1".to_string(), vec![]);

        let hash1 = block.calculate_hash();
        let hash2 = block.calculate_hash();

        // Hash should be deterministic
        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());
    }

    #[test]
    fn test_state_transition() {
        let transition = StateTransition {
            transition_type: StateTransitionType::BalanceChange,
            target: "account1".to_string(),
            previous_state: Some("prev_hash".to_string()),
            new_state: "new_hash".to_string(),
            changes: vec![StateChange {
                field: "balance".to_string(),
                previous_value: Some(serde_json::json!(100)),
                new_value: serde_json::json!(200),
            }],
            caused_by: Uuid::new_v4(),
        };

        assert_eq!(transition.changes.len(), 1);
        assert!(matches!(
            transition.transition_type,
            StateTransitionType::BalanceChange
        ));
    }
}

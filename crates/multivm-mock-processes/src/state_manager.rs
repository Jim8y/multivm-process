//! State Manager for Mock Processes
//!
//! Manages blockchain state for mock processes including balances, nonces, and block numbers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// State manager for mock blockchain processes
#[derive(Debug, Clone)]
pub struct StateManager {
    /// Current block number
    block_number: u64,
    /// Account balances (address -> balance)
    balances: HashMap<String, u128>,
    /// Account nonces (address -> nonce)
    nonces: HashMap<String, u64>,
    /// Contract storage (address -> storage)
    storage: HashMap<String, HashMap<String, String>>,
    /// Block hashes (block number -> hash)
    block_hashes: HashMap<u64, String>,
}

impl StateManager {
    /// Create a new state manager
    pub fn new() -> Self {
        let mut balances = HashMap::new();
        // Add some default balances
        balances.insert(
            "0x742d35Cc6634C0532925a3b844Bc9e7595f8fA66".to_string(),
            1_000_000_000_000_000_000_000u128, // 1000 ETH
        );
        balances.insert(
            "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
            500_000_000_000_000_000_000u128, // 500 ETH
        );

        Self {
            block_number: 0,
            balances,
            nonces: HashMap::new(),
            storage: HashMap::new(),
            block_hashes: HashMap::new(),
        }
    }

    /// Get current block number
    pub fn get_block_number(&self) -> u64 {
        self.block_number
    }

    /// Increment block number
    pub fn increment_block(&mut self) -> u64 {
        self.block_number += 1;
        // Generate block hash
        let hash = format!("0x{:064x}", self.block_number);
        self.block_hashes.insert(self.block_number, hash);
        self.block_number
    }

    /// Get account balance
    pub fn get_balance(&self, address: &str) -> u128 {
        self.balances.get(address).copied().unwrap_or(0)
    }

    /// Set account balance
    pub fn set_balance(&mut self, address: String, balance: u128) {
        self.balances.insert(address, balance);
    }

    /// Transfer balance between accounts
    pub fn transfer(&mut self, from: &str, to: &str, amount: u128) -> bool {
        let from_balance = self.get_balance(from);
        if from_balance < amount {
            return false;
        }

        self.set_balance(from.to_string(), from_balance - amount);
        let to_balance = self.get_balance(to);
        self.set_balance(to.to_string(), to_balance + amount);

        true
    }

    /// Get account nonce
    pub fn get_nonce(&self, address: &str) -> u64 {
        self.nonces.get(address).copied().unwrap_or(0)
    }

    /// Increment account nonce
    pub fn increment_nonce(&mut self, address: &str) -> u64 {
        let nonce = self.get_nonce(address);
        self.nonces.insert(address.to_string(), nonce + 1);
        nonce
    }

    /// Get storage value
    pub fn get_storage(&self, address: &str, key: &str) -> Option<String> {
        self.storage
            .get(address)
            .and_then(|storage| storage.get(key))
            .cloned()
    }

    /// Set storage value
    pub fn set_storage(&mut self, address: String, key: String, value: String) {
        self.storage.entry(address).or_default().insert(key, value);
    }

    /// Get block hash
    pub fn get_block_hash(&self, block_number: u64) -> Option<String> {
        self.block_hashes.get(&block_number).cloned()
    }

    /// Export state snapshot
    pub fn export_snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            block_number: self.block_number,
            balances: self.balances.clone(),
            nonces: self.nonces.clone(),
            storage: self.storage.clone(),
            block_hashes: self.block_hashes.clone(),
        }
    }

    /// Import state snapshot
    pub fn import_snapshot(&mut self, snapshot: StateSnapshot) {
        self.block_number = snapshot.block_number;
        self.balances = snapshot.balances;
        self.nonces = snapshot.nonces;
        self.storage = snapshot.storage;
        self.block_hashes = snapshot.block_hashes;
    }
}

/// State snapshot for export/import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub block_number: u64,
    pub balances: HashMap<String, u128>,
    pub nonces: HashMap<String, u64>,
    pub storage: HashMap<String, HashMap<String, String>>,
    pub block_hashes: HashMap<u64, String>,
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_manager_basic() {
        let mut manager = StateManager::new();

        // Test block number
        assert_eq!(manager.get_block_number(), 0);
        assert_eq!(manager.increment_block(), 1);
        assert_eq!(manager.get_block_number(), 1);

        // Test default balances
        assert!(manager.get_balance("0x742d35Cc6634C0532925a3b844Bc9e7595f8fA66") > 0);

        // Test transfers
        let from = "0x742d35Cc6634C0532925a3b844Bc9e7595f8fA66";
        let to = "0x1234567890123456789012345678901234567890";
        let amount = 1_000_000_000_000_000_000u128; // 1 ETH

        assert!(manager.transfer(from, to, amount));
        assert_eq!(manager.get_balance(to), amount);
    }

    #[test]
    fn test_nonce_tracking() {
        let mut manager = StateManager::new();
        let address = "0x742d35Cc6634C0532925a3b844Bc9e7595f8fA66";

        assert_eq!(manager.get_nonce(address), 0);
        assert_eq!(manager.increment_nonce(address), 0);
        assert_eq!(manager.get_nonce(address), 1);
    }
}

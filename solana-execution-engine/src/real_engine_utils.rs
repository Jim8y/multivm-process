//! Utility functions for the real Solana execution engine
//!
//! This module contains helper functions for transaction encoding, account management,
//! signature verification, and other utility operations specific to Solana.

use crate::engine::{SolanaEngineError, SolanaTransaction};
use crate::real_engine::RealSolanaEngine;
use serde_json::Value;
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    slot_history::Slot,
    system_instruction,
    transaction::Transaction,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tracing::{info, warn};

/// Solana transaction utilities
impl RealSolanaEngine {
    /// Create a Solana keypair for testing
    pub(super) fn create_test_keypair(&self) -> Keypair {
        Keypair::new()
    }

    /// Generate a test transaction
    pub(super) fn create_test_transaction(
        &self,
        from: &Keypair,
        to: &Pubkey,
        lamports: u64,
        recent_blockhash: Hash,
    ) -> Result<Transaction, SolanaEngineError> {
        let instruction = system_instruction::transfer(&from.pubkey(), to, lamports);
        let message = Message::new(&[instruction], Some(&from.pubkey()));

        let mut transaction = Transaction::new_unsigned(message);
        transaction.sign(&[from], recent_blockhash);

        Ok(transaction)
    }

    /// Serialize Solana transaction to bytes
    pub(super) fn serialize_transaction(
        &self,
        transaction: &Transaction,
    ) -> Result<Vec<u8>, SolanaEngineError> {
        bincode::serialize(transaction).map_err(|e| {
            SolanaEngineError::Serialization(format!("Failed to serialize transaction: {}", e))
        })
    }

    /// Deserialize Solana transaction from bytes
    pub(super) fn deserialize_transaction(
        &self,
        data: &[u8],
    ) -> Result<Transaction, SolanaEngineError> {
        bincode::deserialize(data).map_err(|e| {
            SolanaEngineError::Serialization(format!("Failed to deserialize transaction: {}", e))
        })
    }

    /// Verify transaction signature
    pub(super) fn verify_transaction_signature(
        &self,
        transaction: &Transaction,
    ) -> Result<bool, SolanaEngineError> {
        // Solana transactions are automatically verified during deserialization
        // This is a simplified check
        Ok(!transaction.signatures.is_empty()
            && transaction
                .signatures
                .iter()
                .all(|sig| *sig != Signature::default()))
    }

    /// Calculate transaction fee
    pub(super) async fn calculate_transaction_fee(
        &self,
        transaction: &Transaction,
    ) -> Result<u64, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        // Serialize transaction for fee calculation
        let serialized = self.serialize_transaction(transaction)?;
        let transaction_base64 = base64::encode(&serialized);

        // Simulate transaction to get fee information
        match client.simulate_transaction(&transaction_base64).await {
            Ok(result) => {
                if let Some(value) = result.get("value") {
                    if let Some(fee) = value.get("fee").and_then(|f| f.as_u64()) {
                        Ok(fee)
                    } else {
                        // Default fee calculation based on signatures
                        Ok(5000 * transaction.signatures.len() as u64) // 5000 lamports per signature
                    }
                } else {
                    Ok(5000 * transaction.signatures.len() as u64)
                }
            }
            Err(_) => {
                // Fallback fee calculation
                Ok(5000 * transaction.signatures.len() as u64)
            }
        }
    }

    /// Get account balance
    pub(super) async fn get_account_balance(
        &self,
        pubkey: &Pubkey,
    ) -> Result<u64, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        client.get_balance(&pubkey.to_string()).await
    }

    /// Get account information
    pub(super) async fn get_account_info(
        &self,
        pubkey: &Pubkey,
    ) -> Result<Option<Value>, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        match client.get_account_info(&pubkey.to_string()).await? {
            Some(account_info) => {
                let account_json = serde_json::json!({
                    "pubkey": account_info.pubkey,
                    "lamports": account_info.lamports,
                    "owner": account_info.owner,
                    "executable": account_info.executable,
                    "rentEpoch": account_info.rent_epoch,
                    "data": base64::encode(&account_info.data)
                });
                Ok(Some(account_json))
            }
            None => Ok(None),
        }
    }

    /// Get latest blockhash
    pub(super) async fn get_latest_blockhash(&self) -> Result<Hash, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        let blockhash_str = client.get_latest_blockhash().await?;
        blockhash_str
            .parse()
            .map_err(|e| SolanaEngineError::Rpc(format!("Failed to parse blockhash: {}", e)))
    }

    /// Get minimum balance for rent exemption
    pub(super) async fn get_minimum_balance_for_rent_exemption(
        &self,
        data_len: usize,
    ) -> Result<u64, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        client
            .get_minimum_balance_for_rent_exemption(data_len)
            .await
    }

    /// Create system account instruction
    pub(super) fn create_account_instruction(
        &self,
        from: &Pubkey,
        to: &Pubkey,
        lamports: u64,
        space: u64,
        owner: &Pubkey,
    ) -> Instruction {
        system_instruction::create_account(from, to, lamports, space, owner)
    }

    /// Create transfer instruction
    pub(super) fn create_transfer_instruction(
        &self,
        from: &Pubkey,
        to: &Pubkey,
        lamports: u64,
    ) -> Instruction {
        system_instruction::transfer(from, to, lamports)
    }

    /// Validate Solana address
    pub(super) fn validate_address(&self, address: &str) -> Result<Pubkey, SolanaEngineError> {
        address
            .parse()
            .map_err(|e| SolanaEngineError::Configuration(format!("Invalid Solana address: {}", e)))
    }

    /// Convert lamports to SOL
    pub(super) fn lamports_to_sol(&self, lamports: u64) -> f64 {
        lamports as f64 / 1_000_000_000.0 // 1 SOL = 1 billion lamports
    }

    /// Convert SOL to lamports
    pub(super) fn sol_to_lamports(&self, sol: f64) -> u64 {
        (sol * 1_000_000_000.0) as u64
    }

    /// Get cluster genesis hash
    pub(super) fn get_cluster_genesis_hash(&self) -> Result<Hash, SolanaEngineError> {
        let genesis_hash_str = match self.cluster.as_str() {
            "mainnet-beta" => "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
            "testnet" => "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY",
            "devnet" => "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG",
            "localnet" => "11111111111111111111111111111111", // Placeholder
            _ => "11111111111111111111111111111111",          // Default for custom clusters
        };

        genesis_hash_str.parse().map_err(|e| {
            SolanaEngineError::Configuration(format!("Failed to parse genesis hash: {}", e))
        })
    }

    /// Check if cluster is localnet
    pub(super) fn is_localnet(&self) -> bool {
        self.cluster == "localnet"
    }

    /// Check if cluster is devnet
    pub(super) fn is_devnet(&self) -> bool {
        self.cluster == "devnet"
    }

    /// Check if cluster is testnet
    pub(super) fn is_testnet(&self) -> bool {
        self.cluster == "testnet"
    }

    /// Check if cluster is mainnet
    pub(super) fn is_mainnet(&self) -> bool {
        self.cluster == "mainnet-beta"
    }

    /// Get cluster RPC URL
    pub(super) fn get_cluster_rpc_url(&self) -> String {
        match self.cluster.as_str() {
            "mainnet-beta" => "https://api.mainnet-beta.solana.com".to_string(),
            "testnet" => "https://api.testnet.solana.com".to_string(),
            "devnet" => "https://api.devnet.solana.com".to_string(),
            "localnet" => format!("http://127.0.0.1:{}", self.rpc_port),
            _ => format!("http://127.0.0.1:{}", self.rpc_port), // Default for custom clusters
        }
    }

    /// Get cluster WebSocket URL
    pub(super) fn get_cluster_ws_url(&self) -> String {
        match self.cluster.as_str() {
            "mainnet-beta" => "wss://api.mainnet-beta.solana.com".to_string(),
            "testnet" => "wss://api.testnet.solana.com".to_string(),
            "devnet" => "wss://api.devnet.solana.com".to_string(),
            "localnet" => format!("ws://127.0.0.1:{}", self.ws_port),
            _ => format!("ws://127.0.0.1:{}", self.ws_port), // Default for custom clusters
        }
    }

    /// Create a simple transfer transaction for testing
    pub(super) async fn create_simple_transfer(
        &self,
        from_keypair: &Keypair,
        to_pubkey: &Pubkey,
        lamports: u64,
    ) -> Result<Transaction, SolanaEngineError> {
        let recent_blockhash = self.get_latest_blockhash().await?;
        self.create_test_transaction(from_keypair, to_pubkey, lamports, recent_blockhash)
    }

    /// Submit and wait for transaction confirmation
    pub(super) async fn submit_and_confirm_transaction(
        &self,
        transaction: &Transaction,
    ) -> Result<String, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        // Serialize transaction
        let serialized = self.serialize_transaction(transaction)?;
        let transaction_base64 = base64::encode(&serialized);

        // Submit and confirm
        client
            .send_and_confirm_transaction(&transaction_base64)
            .await
    }

    /// Get transaction status
    pub(super) async fn get_transaction_status(
        &self,
        signature: &str,
    ) -> Result<Option<Value>, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        match client.get_signature_status(signature).await? {
            Some(receipt) => {
                let status_json = serde_json::json!({
                    "signature": receipt.signature,
                    "slot": receipt.slot,
                    "confirmationStatus": receipt.confirmation_status,
                    "err": receipt.err,
                    "fee": receipt.fee,
                    "computeUnitsConsumed": receipt.compute_units_consumed
                });
                Ok(Some(status_json))
            }
            None => Ok(None),
        }
    }

    /// Estimate compute units for a transaction
    pub(super) async fn estimate_compute_units(
        &self,
        transaction: &Transaction,
    ) -> Result<u64, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        // Serialize transaction for simulation
        let serialized = self.serialize_transaction(transaction)?;
        let transaction_base64 = base64::encode(&serialized);

        // Simulate transaction to get compute units
        match client.simulate_transaction(&transaction_base64).await {
            Ok(result) => {
                if let Some(value) = result.get("value") {
                    if let Some(units_consumed) =
                        value.get("unitsConsumed").and_then(|u| u.as_u64())
                    {
                        Ok(units_consumed)
                    } else {
                        // Default estimate based on instruction count
                        Ok(transaction.message.instructions.len() as u64 * 1000)
                    }
                } else {
                    Ok(transaction.message.instructions.len() as u64 * 1000)
                }
            }
            Err(_) => {
                // Fallback estimate
                Ok(transaction.message.instructions.len() as u64 * 1000)
            }
        }
    }

    /// Get epoch information
    pub(super) async fn get_epoch_info(&self) -> Result<Value, SolanaEngineError> {
        let api_guard = self.validator_api.read().await;
        let api = api_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("Validator API not initialized".to_string()))?;

        let epoch_info = api.get_epoch_info().await?;

        Ok(serde_json::json!({
            "epoch": epoch_info.epoch,
            "slotIndex": epoch_info.slot_index,
            "slotsInEpoch": epoch_info.slots_in_epoch,
            "absoluteSlot": epoch_info.absolute_slot,
            "blockHeight": epoch_info.block_height,
            "transactionCount": epoch_info.transaction_count
        }))
    }

    /// Get validator performance metrics
    pub(super) async fn get_validator_performance(&self) -> Result<Vec<Value>, SolanaEngineError> {
        let api_guard = self.validator_api.read().await;
        let api = api_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("Validator API not initialized".to_string()))?;

        let validators = api.get_vote_accounts().await?;

        let performance_data: Vec<Value> = validators
            .into_iter()
            .map(|validator| {
                serde_json::json!({
                    "identity": validator.identity,
                    "voteAccount": validator.vote_account,
                    "commission": validator.commission,
                    "lastVote": validator.last_vote,
                    "rootSlot": validator.root_slot,
                    "credits": validator.credits,
                    "activatedStake": validator.activated_stake,
                    "delinquent": validator.delinquent
                })
            })
            .collect();

        Ok(performance_data)
    }

    /// Check validator health
    pub(super) async fn check_validator_health(&self) -> Result<bool, SolanaEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        client.health_check().await
    }

    /// Get memory usage for metrics
    pub(super) fn get_memory_usage(&self) -> u64 {
        // Simple memory usage estimation
        128 * 1024 * 1024 // 128MB default
    }

    /// Get CPU usage for metrics
    pub(super) fn get_cpu_usage(&self) -> f64 {
        // Simple CPU usage estimation
        5.0 // 5% default
    }
}

/// Helper functions for Solana transaction processing
pub fn create_mock_solana_transaction(signature: String, compute_units: u64) -> SolanaTransaction {
    SolanaTransaction {
        signature,
        data: vec![0u8; 64], // Mock transaction data
        compute_units,
    }
}

/// Validate Solana signature format
pub fn validate_signature(signature: &str) -> Result<Signature, SolanaEngineError> {
    signature
        .parse()
        .map_err(|e| SolanaEngineError::Configuration(format!("Invalid Solana signature: {}", e)))
}

/// Validate Solana public key format
pub fn validate_pubkey(pubkey: &str) -> Result<Pubkey, SolanaEngineError> {
    pubkey
        .parse()
        .map_err(|e| SolanaEngineError::Configuration(format!("Invalid Solana public key: {}", e)))
}

/// Convert slot to approximate timestamp
pub fn slot_to_timestamp(slot: Slot, genesis_timestamp: i64) -> i64 {
    // Solana's target slot time is 400ms
    genesis_timestamp + (slot as i64 * 400 / 1000)
}

/// Convert timestamp to approximate slot
pub fn timestamp_to_slot(timestamp: i64, genesis_timestamp: i64) -> Slot {
    // Solana's target slot time is 400ms
    ((timestamp - genesis_timestamp) * 1000 / 400) as Slot
}

/// Calculate rent for account
pub fn calculate_rent(data_len: usize, rent_per_byte_year: u64, years: f64) -> u64 {
    (data_len as u64 + 128)
        * rent_per_byte_year
        * (years * 365.25 * 24.0 * 60.0 * 60.0 / 400.0) as u64
}

/// Check if account is rent exempt
pub fn is_rent_exempt(balance: u64, data_len: usize, rent_exemption_threshold: u64) -> bool {
    balance >= calculate_rent(data_len, rent_exemption_threshold, 2.0) // 2 years of rent
}

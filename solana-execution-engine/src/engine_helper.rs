use std::sync::Arc;

use crate::engine::SolanaEngine;
use crate::SolanaEngineError;
use sha2::{Digest, Sha256};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    hash::Hash,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    slot_history::Slot,
    system_instruction,
    transaction::Transaction,
};
use tokio::sync::RwLock;
use tracing::info;

impl SolanaEngine {
    /// Request and confirm airdrop to a Solana account
    pub async fn request_and_confirm_airdrop(
        &self,
        to_pubkey: &Pubkey,
        lamports: u64,
    ) -> Result<Signature, SolanaEngineError> {
        let client_guard = self.internal_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        // Get the latest blockhash
        let recent_blockhash = client.get_latest_blockhash().await?;

        self.tick().await?;

        // Request airdrop with the blockhash
        let signature = client
            .request_airdrop_with_blockhash(to_pubkey, lamports, &recent_blockhash)
            .await?;

        self.tick().await?;
        self.tick().await?;
        self.tick().await?;

        // Confirm the transaction
        client
            .confirm_transaction_with_spinner(
                &signature,
                &recent_blockhash,
                CommitmentConfig::processed(),
            )
            .await?;

        Ok(signature)
    }

    /// Get the balance of a Solana account
    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64, SolanaEngineError> {
        let client_guard = self.internal_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        let balance = client.get_balance(pubkey).await?;
        Ok(balance)
    }

    /// Transfer SOL from one account to another
    pub async fn transfer_sol(
        &self,
        from_keypair: &Keypair,
        to_pubkey: &Pubkey,
        lamports: u64,
    ) -> Result<Signature, SolanaEngineError> {
        let client_guard = self.internal_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        // Get the latest blockhash
        let recent_blockhash = client.get_latest_blockhash().await?;

        // Create transfer instruction
        let transfer_instruction =
            system_instruction::transfer(&from_keypair.pubkey(), to_pubkey, lamports);

        // Create message and transaction
        let message = Message::new(&[transfer_instruction], Some(&from_keypair.pubkey()));
        let mut transaction = Transaction::new_unsigned(message);
        transaction.sign(&[from_keypair], recent_blockhash);

        // Send and confirm transaction
        let signature = self
            .send_and_confirm_transaction(client, &transaction)
            .await?;

        info!("Transfer confirmed with signature: {}", signature);
        Ok(signature)
    }

    /// Get the latest blockhash from the Solana validator
    ///
    /// This method provides a public interface to get the latest blockhash
    /// without exposing the internal RPC client.
    ///
    /// # Returns
    /// * `Ok(Hash)` - The latest blockhash from the validator
    /// * `Err(SolanaEngineError)` - If RPC client is not initialized or RPC call fails
    ///
    /// # Example
    /// ```rust
    /// let blockhash = engine.get_latest_blockhash().await?;
    /// ```
    pub async fn get_latest_blockhash(&self) -> Result<Hash, SolanaEngineError> {
        let client_guard = self.internal_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        client
            .get_latest_blockhash()
            .await
            .map_err(|e| SolanaEngineError::Rpc(format!("Failed to get latest blockhash: {}", e)))
    }

    /// Get a reference to the internal RPC client
    ///
    /// This method provides access to the internal RPC client for advanced operations
    /// that are not covered by the engine's public API.
    ///
    /// # Returns
    /// * `Arc<RwLock<Option<RpcClient>>>` - A reference to the internal RPC client
    ///
    /// # Example
    /// ```rust
    /// let client_guard = engine.get_internal_client().read().await;
    /// if let Some(client) = client_guard.as_ref() {
    ///     let blockhash = client.get_latest_blockhash().await?;
    /// }
    /// ```
    pub fn get_internal_client(&self) -> Arc<RwLock<Option<RpcClient>>> {
        self.internal_client.clone()
    }

    /// Get the current block hash of the engine
    ///
    /// This method returns the current block hash that represents the state
    /// of the blockchain after processing all blocks up to the current slot.
    ///
    /// # Returns
    /// * `Hash` - The current block hash
    ///
    /// # Example
    /// ```rust
    /// let current_hash = engine.get_current_block_hash().await;
    /// ```
    pub async fn get_current_block_hash(&self) -> Hash {
        *self.current_blockhash.read().await
    }

    /// Get the current slot number of the engine
    ///
    /// This method returns the current slot number that represents the latest
    /// processed block slot in the blockchain.
    ///
    /// # Returns
    /// * `Slot` - The current slot number
    ///
    /// # Example
    /// ```rust
    /// let current_slot = engine.get_current_slot().await;
    /// ```
    pub async fn get_current_slot(&self) -> Slot {
        *self.current_slot.read().await
    }
}

/// Create a signed transfer transaction
pub fn create_transfer_transaction(
    from_keypair: &Keypair,
    to_pubkey: &Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    // Create transfer instruction
    let transfer_instruction =
        system_instruction::transfer(&from_keypair.pubkey(), to_pubkey, lamports);

    // Create message and transaction
    let message = Message::new(&[transfer_instruction], Some(&from_keypair.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.sign(&[from_keypair], recent_blockhash);

    transaction
}

/// Compute a block hash from transactions, slot, previous block hash, and timestamp
///
/// This function creates a cryptographically secure hash for a block by combining
/// the previous block hash, slot number, transaction data, and timestamp.
///
/// # Arguments
/// * `transactions` - A slice of transactions to include in the hash
/// * `slot` - The slot number for this block
/// * `previous_hash` - The hash of the previous block in the chain
/// * `timestamp` - The block timestamp in seconds since Unix epoch
///
/// # Returns
/// * `Hash` - A cryptographically secure hash representing this block
///
/// # Example
/// ```rust
/// let block_hash = compute_block_hash(&transactions, 42, previous_hash, 1640995200);
/// ```
pub fn compute_block_hash(
    transactions: &[Transaction],
    slot: Slot,
    previous_hash: Hash,
    timestamp: i64,
) -> Hash {
    let mut hasher = Sha256::new();

    // Add previous block hash to ensure chain continuity
    hasher.update(previous_hash.to_bytes());

    // Add slot to hash
    hasher.update(slot.to_le_bytes());

    // Add transaction data to hash
    for tx in transactions {
        if let Some(signature) = tx.signatures.first() {
            hasher.update(signature.as_ref());
        }
        // Include transaction message hash for more entropy
        let tx_data = bincode::serialize(tx).unwrap_or_default();
        let tx_hash = Sha256::digest(&tx_data);
        hasher.update(tx_hash);
    }

    // Add timestamp for uniqueness and consistency
    hasher.update(timestamp.to_le_bytes());

    // Create hash from digest
    let block_hash = hasher.finalize();
    Hash::new_from_array(block_hash.into())
}

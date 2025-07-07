use std::sync::Arc;

use crate::engine::SolanaEngine;
use crate::SolanaEngineError;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    hash::Hash,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
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

        // Request airdrop with the blockhash
        let signature = client
            .request_airdrop_with_blockhash(to_pubkey, lamports, &recent_blockhash)
            .await?;

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
        let signature = client.send_and_confirm_transaction(&transaction).await?;

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

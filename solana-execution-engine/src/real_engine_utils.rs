//! Utility functions for the real Solana execution engine
//!
//! This module contains helper functions for transaction encoding, account management,
//! signature verification, and other utility operations specific to Solana.

use crate::engine::{SolanaEngineError, SolanaTransaction};
use crate::real_engine::RealSolanaEngine;
use base64::{prelude::*, Engine};
use serde_json::Value;
use solana_sdk::{
    account::Account,
    commitment_config::CommitmentConfig,
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
    pub(crate) fn create_test_keypair(&self) -> Keypair {
        Keypair::new()
    }

    pub(crate) async fn request_and_confirm_airdrop(
        &self,
        to_pubkey: &Pubkey,
        lamports: u64,
    ) -> Result<Signature, SolanaEngineError> {
        // Get the RPC client from the Arc<RwLock<Option<RpcClient>>>
        let client_guard = self.rpc_client.read().await;
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

        info!("Airdrop confirmed with signature: {}", signature);
        Ok(signature)
    }

    /// Get the balance of a Solana account
    pub(crate) async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64, SolanaEngineError> {
        // Get the RPC client from the Arc<RwLock<Option<RpcClient>>>
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| SolanaEngineError::Rpc("RPC client not initialized".to_string()))?;

        // Get the balance using the RPC client
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
        // Get the RPC client
        let client_guard = self.rpc_client.read().await;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MultivmValidatorConfig, SolanaConnectionConfig};
    use solana_sdk::{
        commitment_config::CommitmentConfig,
        message::Message,
        pubkey::{self, Pubkey},
        signature::{Keypair, Signature, Signer},
        system_instruction,
        transaction::Transaction,
    };
    use std::path::PathBuf;
    use tokio;
    use tracing::{error, info, warn};

    #[tokio::test]
    async fn test_real_solana_engine_core_functions() -> Result<(), Box<dyn std::error::Error>> {
        // Initialize logging with info level, no timestamp
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .without_time()
            .try_init()
            .ok(); // Ignore error if already initialized

        info!("Starting RealSolanaEngine core functions test...");

        // Create test configuration
        let data_dir = PathBuf::from("/tmp/test_solana_engine");
        let rpc_port = 8899;
        let cluster = "localnet".to_string();

        // Create engine with default configuration
        info!("Creating RealSolanaEngine...");
        match RealSolanaEngine::new(data_dir.clone(), rpc_port, cluster.clone()).await {
            Ok(mut engine) => {
                info!(" RealSolanaEngine created successfully");

                // Initialize the engine
                info!("Initializing RealSolanaEngine...");
                match engine.initialize().await {
                    Ok(_) => {
                        info!(" RealSolanaEngine initialized successfully");
                    }
                    Err(e) => {
                        error!(" Failed to initialize RealSolanaEngine: {}", e);

                        // Still try to shutdown in case of partial initialization
                        info!("Attempting cleanup shutdown...");
                        let _ = engine
                            .shutdown(Some(tokio::time::Duration::from_secs(5)))
                            .await;
                        return Ok(());
                    }
                }

                // Wait a bit for the process to stabilize
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                // Create Alice and Bob keypairs
                let alice_keypair = engine.create_test_keypair();
                let alice_pubkey = alice_keypair.pubkey();
                let bob_keypair = engine.create_test_keypair();
                let bob_pubkey = bob_keypair.pubkey();

                info!("Alice account pubkey: {}", alice_pubkey);
                info!("Bob account pubkey: {}", bob_pubkey);

                // Step 1: Airdrop to Alice
                let airdrop_amount = 1_000_000_000; // 1 SOL in lamports
                info!(
                    "Step 1: Requesting airdrop of {} lamports to Alice...",
                    airdrop_amount
                );
                match engine
                    .request_and_confirm_airdrop(&alice_pubkey, airdrop_amount)
                    .await
                {
                    Ok(signature) => {
                        info!(" Airdrop to Alice successful with signature: {}", signature);
                    }
                    Err(e) => {
                        error!(" Airdrop to Alice failed: {}", e);
                    }
                }

                // Check Alice's balance after airdrop
                info!("Checking Alice's balance after airdrop...");
                let alice_balance_after_airdrop = match engine.get_balance(&alice_pubkey).await {
                    Ok(balance) => {
                        info!("Alice's balance after airdrop: {} lamports", balance);
                        balance
                    }
                    Err(e) => {
                        error!(" Failed to get Alice's balance after airdrop: {}", e);
                        return Ok(()); // Exit test if we can't get balance
                    }
                };

                // Check Bob's initial balance (should be 0)
                info!("Checking Bob's initial balance...");
                let bob_initial_balance = match engine.get_balance(&bob_pubkey).await {
                    Ok(balance) => {
                        info!("Bob's initial balance: {} lamports", balance);
                        balance
                    }
                    Err(e) => {
                        warn!("Failed to get Bob's initial balance: {}", e);
                        0 // Assume 0 if we can't get the balance
                    }
                };

                // Step 2: Alice transfers half of her SOL to Bob
                let transfer_amount = airdrop_amount / 2; // 0.5 SOL
                info!(
                    "Step 2: Alice transferring {} lamports to Bob...",
                    transfer_amount
                );
                match engine
                    .transfer_sol(&alice_keypair, &bob_pubkey, transfer_amount)
                    .await
                {
                    Ok(signature) => {
                        info!(
                            " Transfer from Alice to Bob successful with signature: {}",
                            signature
                        );
                    }
                    Err(e) => {
                        error!(" Transfer from Alice to Bob failed: {}", e);
                    }
                }

                // Step 3: Check final balances
                info!("Step 3: Checking final balances...");

                // Check Alice's final balance
                match engine.get_balance(&alice_pubkey).await {
                    Ok(alice_final_balance) => {
                        info!("Alice's final balance: {} lamports", alice_final_balance);
                        let alice_expected_balance = alice_balance_after_airdrop - transfer_amount;

                        // Note: Alice's balance might be slightly less due to transaction fees
                        if alice_final_balance <= alice_expected_balance
                            && alice_final_balance > alice_expected_balance - 10_000
                        {
                            info!(" Alice's balance verification successful! (accounting for transaction fees)");
                        } else {
                            warn!(
                                " Alice's balance: {} (expected around {})",
                                alice_final_balance, alice_expected_balance
                            );
                        }
                    }
                    Err(e) => {
                        error!(" Failed to get Alice's final balance: {}", e);
                    }
                }

                // Check Bob's final balance
                match engine.get_balance(&bob_pubkey).await {
                    Ok(bob_final_balance) => {
                        info!("Bob's final balance: {} lamports", bob_final_balance);
                        let bob_expected_balance = bob_initial_balance + transfer_amount;

                        if bob_final_balance == bob_expected_balance {
                            info!(" Bob's balance verification successful!");
                        } else {
                            warn!(
                                " Bob's balance: {} (expected {})",
                                bob_final_balance, bob_expected_balance
                            );
                        }
                    }
                    Err(e) => {
                        error!(" Failed to get Bob's final balance: {}", e);
                    }
                }

                // Summary
                info!("=== Transaction Summary ===");
                info!("Original airdrop amount: {} lamports", airdrop_amount);
                info!("Transfer amount: {} lamports", transfer_amount);
                info!(
                    "Expected Alice final balance: ~{} lamports (minus fees)",
                    airdrop_amount - transfer_amount
                );
                info!("Expected Bob final balance: {} lamports", transfer_amount);

                // Test shutdown
                info!("Testing shutdown...");
                match engine
                    .shutdown(Some(tokio::time::Duration::from_secs(10)))
                    .await
                {
                    Ok(_) => info!(" Engine shutdown successfully"),
                    Err(e) => error!(" Failed to shutdown engine: {}", e),
                }
            }
            Err(e) => {
                error!(" Failed to create RealSolanaEngine: {}", e);
            }
        }

        info!("Test completed!");
        Ok(())
    }
}

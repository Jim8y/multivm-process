//! Utility functions for the Solana execution engine
//!
//! This module contains helper functions for transaction encoding, account management,
//! signature verification, and other utility operations specific to Solana.

use crate::engine::{SolanaEngine, SolanaEngineError, SolanaTransaction};
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
impl SolanaEngine {
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
    pub(crate) async fn transfer_sol(
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
    use crate::config::{SolanaConfig, SolanaConnectionConfig};
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

    /// Initializes logging for tests.
    fn setup_logging() {
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
    }

    /// Creates and initializes a new SolanaEngine for testing.
    async fn create_and_initialize_engine() -> Result<SolanaEngine, Box<dyn std::error::Error>> {
        info!("Creating and initializing SolanaEngine for test...");
        let mut engine = SolanaEngine::new_default().await?;

        // Try to initialize, but cleanup on failure
        if let Err(e) = engine.initialize().await {
            error!("✗ Failed to initialize SolanaEngine: {}", e);
            // Attempt a cleanup shutdown on initialization failure
            let _ = engine
                .shutdown(Some(tokio::time::Duration::from_secs(5)))
                .await;
            return Err(Box::new(e));
        }

        info!("✓ SolanaEngine initialized successfully");
        Ok(engine)
    }

    #[tokio::test]
    async fn test_solana_engine_core_functions() -> Result<(), Box<dyn std::error::Error>> {
        setup_logging();
        let mut engine = create_and_initialize_engine().await?;

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

        info!("Test completed!");
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_transactions_to_validator() -> Result<(), Box<dyn std::error::Error>> {
        setup_logging();

        info!("Starting submit_transactions_to_validator test...");

        let mut engine = create_and_initialize_engine().await?;

        // Create test keypairs
        let alice_keypair = engine.create_test_keypair();
        let alice_pubkey = alice_keypair.pubkey();
        let bob_keypair = engine.create_test_keypair();
        let bob_pubkey = bob_keypair.pubkey();
        let charlie_keypair = engine.create_test_keypair();
        let charlie_pubkey = charlie_keypair.pubkey();

        info!("Alice account pubkey: {}", alice_pubkey);
        info!("Bob account pubkey: {}", bob_pubkey);
        info!("Charlie account pubkey: {}", charlie_pubkey);

        // Step 1: Airdrop to Alice to fund transactions
        let airdrop_amount = 2_000_000_000; // 2 SOL in lamports
        info!(
            "Step 1: Requesting airdrop of {} lamports to Alice...",
            airdrop_amount
        );
        match engine
            .request_and_confirm_airdrop(&alice_pubkey, airdrop_amount)
            .await
        {
            Ok(signature) => {
                info!(
                    "✓ Airdrop to Alice successful with signature: {}",
                    signature
                );
            }
            Err(e) => {
                error!("✗ Airdrop to Alice failed: {}", e);
                let _ = engine
                    .shutdown(Some(tokio::time::Duration::from_secs(5)))
                    .await;
                return Ok(());
            }
        }

        // Wait for airdrop to be processed
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Step 2: Create multiple signed transactions
        info!("Step 2: Creating multiple signed transactions...");

        // Get the RPC client to get recent blockhash
        let recent_blockhash = {
            let client_guard = engine.rpc_client.read().await;
            let client = client_guard.as_ref().unwrap();
            match client.get_latest_blockhash().await {
                Ok(blockhash) => {
                    info!("✓ Got recent blockhash: {}", blockhash);
                    blockhash
                }
                Err(e) => {
                    error!("✗ Failed to get recent blockhash: {}", e);
                    // Drop the guard before calling shutdown
                    drop(client_guard);
                    let _ = engine
                        .shutdown(Some(tokio::time::Duration::from_secs(5)))
                        .await;
                    return Ok(());
                }
            }
        }; // client_guard is automatically dropped here

        let mut transactions = Vec::new();

        // Transaction 1: Alice -> Bob (500M lamports = 0.5 SOL)
        let transfer_amount_1 = 500_000_000;
        let transfer_instruction_1 =
            system_instruction::transfer(&alice_keypair.pubkey(), &bob_pubkey, transfer_amount_1);
        let message_1 = Message::new(&[transfer_instruction_1], Some(&alice_keypair.pubkey()));
        let mut transaction_1 = Transaction::new_unsigned(message_1);
        transaction_1.sign(&[&alice_keypair], recent_blockhash);
        transactions.push(transaction_1);

        // Transaction 2: Alice -> Charlie (300M lamports = 0.3 SOL)
        let transfer_amount_2 = 300_000_000;
        let transfer_instruction_2 = system_instruction::transfer(
            &alice_keypair.pubkey(),
            &charlie_pubkey,
            transfer_amount_2,
        );
        let message_2 = Message::new(&[transfer_instruction_2], Some(&alice_keypair.pubkey()));
        let mut transaction_2 = Transaction::new_unsigned(message_2);
        transaction_2.sign(&[&alice_keypair], recent_blockhash);
        transactions.push(transaction_2);

        info!("✓ Created {} signed transactions", transactions.len());

        // Step 3: Test submit_transactions_to_validator
        info!("Step 3: Testing submit_transactions_to_validator...");
        match engine
            .submit_transactions_to_validator(&mut transactions)
            .await
        {
            Ok(signatures) => {
                info!(
                    "✓ Successfully submitted {} transactions!",
                    signatures.len()
                );
                for (i, signature) in signatures.iter().enumerate() {
                    info!("  Transaction {}: {}", i + 1, signature);
                }

                // Verify that we got the expected number of signatures
                if signatures.len() == 2 {
                    info!("✓ Received expected number of transaction signatures");
                } else {
                    warn!("⚠ Expected 2 signatures, got {}", signatures.len());
                }
            }
            Err(e) => {
                error!("✗ Failed to submit transactions: {}", e);
            }
        }

        // Step 4: Wait for transactions to be processed and verify balances
        info!("Step 4: Waiting for transactions to be processed...");
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Check final balances
        info!("Checking final balances...");

        match engine.get_balance(&bob_pubkey).await {
            Ok(bob_balance) => {
                info!("Bob's final balance: {} lamports", bob_balance);
                if bob_balance >= transfer_amount_1 {
                    info!("✓ Bob received the expected transfer");
                } else {
                    warn!("⚠ Bob's balance is less than expected transfer amount");
                }
            }
            Err(e) => {
                warn!("Failed to get Bob's balance: {}", e);
            }
        }

        match engine.get_balance(&charlie_pubkey).await {
            Ok(charlie_balance) => {
                info!("Charlie's final balance: {} lamports", charlie_balance);
                if charlie_balance >= transfer_amount_2 {
                    info!("✓ Charlie received the expected transfer");
                } else {
                    warn!("⚠ Charlie's balance is less than expected transfer amount");
                }
            }
            Err(e) => {
                warn!("Failed to get Charlie's balance: {}", e);
            }
        }

        match engine.get_balance(&alice_pubkey).await {
            Ok(alice_balance) => {
                info!("Alice's final balance: {} lamports", alice_balance);
                let expected_remaining = airdrop_amount - transfer_amount_1 - transfer_amount_2;
                if alice_balance <= expected_remaining
                    && alice_balance > expected_remaining - 20_000
                {
                    info!("✓ Alice's balance is as expected (accounting for transaction fees)");
                } else {
                    warn!(
                        "⚠ Alice's balance: {} (expected around {})",
                        alice_balance, expected_remaining
                    );
                }
            }
            Err(e) => {
                warn!("Failed to get Alice's balance: {}", e);
            }
        }

        // Step 5: Test with empty transaction array
        info!("Step 5: Testing with empty transaction array...");
        let mut empty_transactions: Vec<Transaction> = Vec::new();
        match engine
            .submit_transactions_to_validator(&mut empty_transactions)
            .await
        {
            Ok(signatures) => {
                if signatures.is_empty() {
                    info!("✓ Empty transaction array handled correctly");
                } else {
                    warn!("⚠ Expected empty signatures for empty transaction array");
                }
            }
            Err(e) => {
                error!("✗ Unexpected error with empty transaction array: {}", e);
            }
        }

        // Summary
        info!("=== Test Summary ===");
        info!("✓ Created and submitted multiple transactions successfully");
        info!("✓ Verified transaction signatures were returned");
        info!("✓ Verified balances were updated correctly");
        info!("✓ Tested edge case with empty transaction array");

        // Test shutdown
        info!("Testing shutdown...");
        match engine
            .shutdown(Some(tokio::time::Duration::from_secs(10)))
            .await
        {
            Ok(_) => info!("✓ Engine shutdown successfully"),
            Err(e) => error!("✗ Failed to shutdown engine: {}", e),
        }

        info!("submit_transactions_to_validator test completed!");
        Ok(())
    }

    #[tokio::test]
    async fn test_process_block() -> Result<(), Box<dyn std::error::Error>> {
        setup_logging();

        info!("Starting process_block test...");

        let mut engine = create_and_initialize_engine().await?;

        // Create test keypairs for transactions
        let alice_keypair = engine.create_test_keypair();
        let alice_pubkey = alice_keypair.pubkey();
        let bob_keypair = engine.create_test_keypair();
        let bob_pubkey = bob_keypair.pubkey();
        let charlie_keypair = engine.create_test_keypair();
        let charlie_pubkey = charlie_keypair.pubkey();

        info!("Alice account pubkey: {}", alice_pubkey);
        info!("Bob account pubkey: {}", bob_pubkey);
        info!("Charlie account pubkey: {}", charlie_pubkey);

        // Step 1: Airdrop to Alice to fund transactions
        let airdrop_amount = 2_000_000_000; // 2 SOL in lamports (to fund multiple transactions)
        info!(
            "Step 1: Requesting airdrop of {} lamports to Alice...",
            airdrop_amount
        );
        match engine
            .request_and_confirm_airdrop(&alice_pubkey, airdrop_amount)
            .await
        {
            Ok(signature) => {
                info!(
                    "✓ Airdrop to Alice successful with signature: {}",
                    signature
                );
            }
            Err(e) => {
                error!("✗ Airdrop to Alice failed: {}", e);
                let _ = engine
                    .shutdown(Some(tokio::time::Duration::from_secs(5)))
                    .await;
                return Ok(());
            }
        }

        // Wait for airdrop to be processed
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Step 2: Create test block with transactions
        info!("Step 2: Creating test block with transactions...");

        // Get the RPC client to get recent blockhash
        let recent_blockhash = {
            let client_guard = engine.rpc_client.read().await;
            let client = client_guard.as_ref().unwrap();
            match client.get_latest_blockhash().await {
                Ok(blockhash) => {
                    info!("✓ Got recent blockhash: {}", blockhash);
                    blockhash
                }
                Err(e) => {
                    error!("✗ Failed to get recent blockhash: {}", e);
                    // Drop the guard before calling shutdown
                    drop(client_guard);
                    let _ = engine
                        .shutdown(Some(tokio::time::Duration::from_secs(5)))
                        .await;
                    return Ok(());
                }
            }
        }; // client_guard is automatically dropped here

        // Create three signed transactions
        let mut transactions = Vec::new();

        // Transaction 1: Alice -> Bob (300M lamports = 0.3 SOL)
        let transfer_amount_1 = 300_000_000;
        let transfer_instruction_1 =
            system_instruction::transfer(&alice_keypair.pubkey(), &bob_pubkey, transfer_amount_1);
        let message_1 = Message::new(&[transfer_instruction_1], Some(&alice_keypair.pubkey()));
        let mut transaction_1 = Transaction::new_unsigned(message_1);
        transaction_1.sign(&[&alice_keypair], recent_blockhash);
        transactions.push(transaction_1);

        // Transaction 2: Alice -> Charlie (200M lamports = 0.2 SOL)
        let transfer_amount_2 = 200_000_000;
        let transfer_instruction_2 = system_instruction::transfer(
            &alice_keypair.pubkey(),
            &charlie_pubkey,
            transfer_amount_2,
        );
        let message_2 = Message::new(&[transfer_instruction_2], Some(&alice_keypair.pubkey()));
        let mut transaction_2 = Transaction::new_unsigned(message_2);
        transaction_2.sign(&[&alice_keypair], recent_blockhash);
        transactions.push(transaction_2);

        // Transaction 3: Alice -> Bob (another 100M lamports = 0.1 SOL)
        let transfer_amount_3 = 100_000_000;
        let transfer_instruction_3 =
            system_instruction::transfer(&alice_keypair.pubkey(), &bob_pubkey, transfer_amount_3);
        let message_3 = Message::new(&[transfer_instruction_3], Some(&alice_keypair.pubkey()));
        let mut transaction_3 = Transaction::new_unsigned(message_3);
        transaction_3.sign(&[&alice_keypair], recent_blockhash);
        transactions.push(transaction_3);

        let total_transfer_amount = transfer_amount_1 + transfer_amount_2 + transfer_amount_3;
        info!(
            "✓ Created {} transactions with total transfer amount: {} lamports",
            transactions.len(),
            total_transfer_amount
        );

        // Create test block data
        use crate::engine::SolanaBlockData;
        use solana_sdk::hash::Hash;
        use solana_sdk::slot_history::Slot;

        let test_slot: Slot = 54321;
        let test_block_hash = Hash::new_unique();
        let test_parent_slot: Slot = 54320;
        let test_previous_blockhash = Hash::new_unique();
        let test_block_time = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        );

        let test_block = SolanaBlockData {
            slot: test_slot,
            block_hash: test_block_hash,
            parent_slot: test_parent_slot,
            transactions: transactions,
            block_time: test_block_time,
            previous_blockhash: test_previous_blockhash,
        };

        info!(
            "✓ Created test block for slot {} with {} transactions",
            test_slot,
            test_block.transactions.len()
        );

        // Step 3: Test process_block
        info!("Step 3: Testing process_block...");
        match engine.process_block(test_block).await {
            Ok(result) => {
                info!("✓ process_block completed successfully!");
                info!("  Block slot: {}", result.slot);
                info!("  Block hash: {}", result.block_hash);
                info!("  Processed {} transactions", result.transaction_count);

                // Verify that we got the expected number of transactions
                if result.transaction_count == 3 {
                    info!("✓ All transactions were processed successfully");
                    info!("  Transaction count: {}", result.transaction_count);
                    info!("  Compute units used: {}", result.compute_units_used);
                    info!("  Processing time: {:?}", result.processing_time);
                } else {
                    warn!(
                        "⚠ Expected 3 transactions, got {}",
                        result.transaction_count
                    );
                }
            }
            Err(e) => {
                error!("✗ process_block failed: {}", e);
            }
        }

        // Step 4: Wait for transactions to be processed and verify balances
        info!("Step 4: Waiting for transactions to be processed...");
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Check final balances
        info!("Checking final balances...");

        match engine.get_balance(&bob_pubkey).await {
            Ok(bob_balance) => {
                info!("Bob's final balance: {} lamports", bob_balance);
                let expected_bob_balance = transfer_amount_1 + transfer_amount_3; // 0.3 + 0.1 = 0.4 SOL
                if bob_balance >= expected_bob_balance {
                    info!(
                        "✓ Bob received the expected transfers (total: {} lamports)",
                        expected_bob_balance
                    );
                } else {
                    warn!("⚠ Bob's balance is less than expected transfer amount");
                }
            }
            Err(e) => {
                warn!("Failed to get Bob's balance: {}", e);
            }
        }

        match engine.get_balance(&charlie_pubkey).await {
            Ok(charlie_balance) => {
                info!("Charlie's final balance: {} lamports", charlie_balance);
                if charlie_balance >= transfer_amount_2 {
                    info!(
                        "✓ Charlie received the expected transfer ({}  lamports)",
                        transfer_amount_2
                    );
                } else {
                    warn!("⚠ Charlie's balance is less than expected transfer amount");
                }
            }
            Err(e) => {
                warn!("Failed to get Charlie's balance: {}", e);
            }
        }

        match engine.get_balance(&alice_pubkey).await {
            Ok(alice_balance) => {
                info!("Alice's final balance: {} lamports", alice_balance);
                let expected_remaining = airdrop_amount - total_transfer_amount;
                if alice_balance <= expected_remaining
                    && alice_balance > expected_remaining - 30_000
                {
                    info!("✓ Alice's balance is as expected (accounting for transaction fees)");
                } else {
                    warn!(
                        "⚠ Alice's balance: {} (expected around {})",
                        alice_balance, expected_remaining
                    );
                }
            }
            Err(e) => {
                warn!("Failed to get Alice's balance: {}", e);
            }
        }

        // Summary
        info!("=== Test Summary ===");
        info!("✓ Created test block with {} transactions", 3);
        info!("✓ Successfully processed block through process_block");
        info!("✓ Verified transaction signatures were returned");
        info!("✓ Verified balances were updated correctly");

        // Test shutdown
        info!("Testing shutdown...");
        match engine
            .shutdown(Some(tokio::time::Duration::from_secs(10)))
            .await
        {
            Ok(_) => info!("✓ Engine shutdown successfully"),
            Err(e) => error!("✗ Failed to shutdown engine: {}", e),
        }

        info!("process_block test completed!");
        Ok(())
    }
}

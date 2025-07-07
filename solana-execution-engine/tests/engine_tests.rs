//! Utility functions for the Solana execution engine
//!
//! This module contains helper functions for transaction encoding, account management,
//! signature verification, and other utility operations specific to Solana.

mod test_utils;

use solana_execution_engine::SolanaEngineError;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        create_and_initialize_engine, create_test_keypair, setup_logging, shutdown_engine,
    };
    use solana_execution_engine::create_transfer_transaction;
    use solana_execution_engine::engine::SolanaBlockData;
    use solana_sdk::{hash::Hash, signature::Signer, transaction::Transaction};
    use std::time::Duration;
    use tokio;
    use tracing::{info, warn};

    #[tokio::test]
    async fn test_main() -> Result<(), SolanaEngineError> {
        setup_logging();
        let engine = create_and_initialize_engine().await?;
        tokio::time::sleep(Duration::from_secs(60)).await;
        shutdown_engine(engine).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_solana_engine_core_functions() -> Result<(), SolanaEngineError> {
        setup_logging();
        let mut engine = create_and_initialize_engine()
            .await
            .expect("Failed to create and initialize engine");

        // Create Alice and Bob keypairs
        let alice_keypair = create_test_keypair();
        let bob_keypair = create_test_keypair();

        // Airdrop to Alice
        let airdrop_amount = 1_000_000_000; // 1 SOL in lamports
        info!("Requesting airdrop to Alice...");
        let _signature = engine
            .request_and_confirm_airdrop(&alice_keypair.pubkey(), airdrop_amount)
            .await
            .expect("Failed to request and confirm airdrop to Alice");

        // Check Alice's balance after airdrop
        let alice_balance_after_airdrop = engine
            .get_balance(&alice_keypair.pubkey())
            .await
            .expect("Failed to get Alice's balance after airdrop");
        assert_eq!(
            alice_balance_after_airdrop, airdrop_amount,
            "Alice's balance {} does not match airdrop amount {}",
            alice_balance_after_airdrop, airdrop_amount
        );

        // Check Bob's initial balance (should be 0)
        let bob_initial_balance = engine
            .get_balance(&bob_keypair.pubkey())
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to get Bob's initial balance: {}", e);
                0 // Assume 0 if we can't get the balance
            });
        assert_eq!(
            bob_initial_balance, 0,
            "Bob's initial balance should be 0, but got {}",
            bob_initial_balance
        );

        // Alice transfers half of her SOL to Bob
        let transfer_amount = airdrop_amount / 2; // 0.5 SOL
        info!("Transferring SOL from Alice to Bob...");
        let _signature = engine
            .transfer_sol(&alice_keypair, &bob_keypair.pubkey(), transfer_amount)
            .await
            .expect("Failed to transfer SOL from Alice to Bob");

        // Check Alice's final balance
        let alice_final_balance = engine
            .get_balance(&alice_keypair.pubkey())
            .await
            .expect("Failed to get Alice's final balance");
        let alice_expected_balance = alice_balance_after_airdrop - transfer_amount;

        // Note: Alice's balance might be slightly less due to transaction fees
        assert!(
            alice_final_balance <= alice_expected_balance
                && alice_final_balance > alice_expected_balance - 10_000,
            "Alice's balance verification failed: {} (expected around {})",
            alice_final_balance,
            alice_expected_balance
        );

        // Check Bob's final balance
        let bob_final_balance = engine
            .get_balance(&bob_keypair.pubkey())
            .await
            .expect("Failed to get Bob's final balance");
        let bob_expected_balance = bob_initial_balance + transfer_amount;

        assert_eq!(
            bob_final_balance, bob_expected_balance,
            "Bob's balance verification failed: {} (expected {})",
            bob_final_balance, bob_expected_balance
        );

        engine
            .shutdown(Some(tokio::time::Duration::from_secs(10)))
            .await
            .expect("Failed to shutdown engine");
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_transactions_to_validator() -> Result<(), SolanaEngineError> {
        setup_logging();
        let engine = create_and_initialize_engine()
            .await
            .expect("Failed to create and initialize engine");

        // Create test keypairs
        let alice_keypair = create_test_keypair();
        let bob_keypair = create_test_keypair();
        let charlie_keypair = create_test_keypair();

        // Airdrop to Alice to fund transactions
        let airdrop_amount = 2_000_000_000; // 2 SOL in lamports
        info!("Requesting airdrop to Alice...");
        let _signature = engine
            .request_and_confirm_airdrop(&alice_keypair.pubkey(), airdrop_amount)
            .await
            .expect("Failed to request and confirm airdrop to Alice");

        // Check Alice's balance after airdrop
        let alice_balance_after_airdrop = engine
            .get_balance(&alice_keypair.pubkey())
            .await
            .expect("Failed to get Alice's balance after airdrop");
        assert_eq!(
            alice_balance_after_airdrop, airdrop_amount,
            "Alice's balance {} does not match airdrop amount {}",
            alice_balance_after_airdrop, airdrop_amount
        );

        // Get recent blockhash
        let recent_blockhash = engine
            .get_latest_blockhash()
            .await
            .expect("Failed to get recent blockhash");

        // Create transactions
        let mut transactions = Vec::new();
        let transfer_amount_1 = 500_000_000;
        let transfer_amount_2 = 300_000_000;

        transactions.push(create_transfer_transaction(
            &alice_keypair,
            &bob_keypair.pubkey(),
            transfer_amount_1,
            recent_blockhash,
        ));

        transactions.push(create_transfer_transaction(
            &alice_keypair,
            &charlie_keypair.pubkey(),
            transfer_amount_2,
            recent_blockhash,
        ));

        // Submit transactions
        info!("Submitting transactions to validator...");
        let signatures = engine
            .submit_transactions_to_validator(&mut transactions)
            .await
            .expect("Failed to submit transactions to validator");

        assert_eq!(
            signatures.len(),
            2,
            "Expected 2 signatures, got {}",
            signatures.len()
        );

        // Wait for transactions to be processed
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Check Bob's balance
        let bob_balance = engine
            .get_balance(&bob_keypair.pubkey())
            .await
            .expect("Failed to get Bob's balance");
        assert_eq!(
            bob_balance, transfer_amount_1,
            "Bob's balance {} does not match expected {}",
            bob_balance, transfer_amount_1
        );

        // Check Charlie's balance
        let charlie_balance = engine
            .get_balance(&charlie_keypair.pubkey())
            .await
            .expect("Failed to get Charlie's balance");
        assert_eq!(
            charlie_balance, transfer_amount_2,
            "Charlie's balance {} does not match expected {}",
            charlie_balance, transfer_amount_2
        );

        // Check Alice's remaining balance
        let alice_final_balance = engine
            .get_balance(&alice_keypair.pubkey())
            .await
            .expect("Failed to get Alice's final balance");
        let expected_remaining = airdrop_amount - transfer_amount_1 - transfer_amount_2;
        assert!(
            alice_final_balance <= expected_remaining
                && alice_final_balance > expected_remaining - 20_000,
            "Alice's balance {} is not as expected (expected around {})",
            alice_final_balance,
            expected_remaining
        );

        // Test with empty transaction array
        let mut empty_transactions: Vec<Transaction> = Vec::new();
        let empty_signatures = engine
            .submit_transactions_to_validator(&mut empty_transactions)
            .await
            .expect("Failed to submit empty transaction array");
        assert!(
            empty_signatures.is_empty(),
            "Expected empty signatures for empty transaction array, got {}",
            empty_signatures.len()
        );

        shutdown_engine(engine)
            .await
            .expect("Failed to shutdown engine");
        Ok(())
    }

    #[tokio::test]
    async fn test_process_block() -> Result<(), SolanaEngineError> {
        setup_logging();
        let mut engine = create_and_initialize_engine()
            .await
            .expect("Failed to create and initialize engine");

        // Create test keypairs
        let alice_keypair = create_test_keypair();
        let bob_keypair = create_test_keypair();
        let charlie_keypair = create_test_keypair();

        // Airdrop to Alice to fund transactions
        let airdrop_amount = 2_000_000_000; // 2 SOL in lamports
        info!("Requesting airdrop to Alice...");
        let _signature = engine
            .request_and_confirm_airdrop(&alice_keypair.pubkey(), airdrop_amount)
            .await
            .expect("Failed to request and confirm airdrop to Alice");

        // Check Alice's balance after airdrop
        let alice_balance_after_airdrop = engine
            .get_balance(&alice_keypair.pubkey())
            .await
            .expect("Failed to get Alice's balance after airdrop");
        assert_eq!(
            alice_balance_after_airdrop, airdrop_amount,
            "Alice's balance {} does not match airdrop amount {}",
            alice_balance_after_airdrop, airdrop_amount
        );

        // Get recent blockhash
        let recent_blockhash = engine
            .get_latest_blockhash()
            .await
            .expect("Failed to get recent blockhash");

        // Create transactions
        let mut transactions = Vec::new();
        let transfer_amount_1 = 300_000_000;
        let transfer_amount_2 = 200_000_000;
        let transfer_amount_3 = 100_000_000;

        transactions.push(create_transfer_transaction(
            &alice_keypair,
            &bob_keypair.pubkey(),
            transfer_amount_1,
            recent_blockhash,
        ));

        transactions.push(create_transfer_transaction(
            &alice_keypair,
            &charlie_keypair.pubkey(),
            transfer_amount_2,
            recent_blockhash,
        ));

        transactions.push(create_transfer_transaction(
            &alice_keypair,
            &bob_keypair.pubkey(),
            transfer_amount_3,
            recent_blockhash,
        ));

        // Create test block
        let test_block = SolanaBlockData {
            slot: 54321,
            block_hash: Hash::new_unique(),
            parent_slot: 54320,
            transactions,
            block_time: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            ),
            previous_blockhash: Hash::new_unique(),
        };

        // Process block
        info!("Processing block...");
        let result = engine
            .process_block(test_block)
            .await
            .expect("Failed to process block");

        assert_eq!(
            result.transaction_count, 3,
            "Expected 3 transactions, got {}",
            result.transaction_count
        );

        // Check Bob's balance (received two transfers)
        let bob_balance = engine
            .get_balance(&bob_keypair.pubkey())
            .await
            .expect("Failed to get Bob's balance");
        let expected_bob_balance = transfer_amount_1 + transfer_amount_3;
        assert_eq!(
            bob_balance, expected_bob_balance,
            "Bob's balance {} does not match expected {}",
            bob_balance, expected_bob_balance
        );

        // Check Charlie's balance
        let charlie_balance = engine
            .get_balance(&charlie_keypair.pubkey())
            .await
            .expect("Failed to get Charlie's balance");
        assert_eq!(
            charlie_balance, transfer_amount_2,
            "Charlie's balance {} does not match expected {}",
            charlie_balance, transfer_amount_2
        );

        // Check Alice's remaining balance
        let alice_final_balance = engine
            .get_balance(&alice_keypair.pubkey())
            .await
            .expect("Failed to get Alice's final balance");
        let total_transfer_amount = transfer_amount_1 + transfer_amount_2 + transfer_amount_3;
        let expected_remaining = airdrop_amount - total_transfer_amount;
        assert!(
            alice_final_balance <= expected_remaining
                && alice_final_balance > expected_remaining - 30_000,
            "Alice's balance {} is not as expected (expected around {})",
            alice_final_balance,
            expected_remaining
        );

        shutdown_engine(engine)
            .await
            .expect("Failed to shutdown engine");
        Ok(())
    }
}

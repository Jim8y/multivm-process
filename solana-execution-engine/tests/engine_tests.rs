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
    async fn test_create_block() -> Result<(), SolanaEngineError> {
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

        // Create block with transactions
        info!("Creating block with transactions...");
        let block = engine
            .create_block(&mut transactions)
            .await
            .expect("Failed to create block with transactions");

        assert_eq!(
            block.transactions.len(),
            2,
            "Expected 2 transactions in block, got {}",
            block.transactions.len()
        );

        // Check block fields
        assert_eq!(block.slot, 1, "Block slot should be 1, got {}", block.slot);

        // Check that block hash is not the default/initial hash
        assert_ne!(
            block.block_hash,
            Hash::default(),
            "Block hash should not be default/empty, got {}",
            block.block_hash
        );

        // Check that all transactions were successful (assuming they should be)
        // Note: In a real implementation, you might want to check individual transaction success
        info!(
            "Block validation passed - slot: {}, hash: {}",
            block.slot, block.block_hash
        );

        info!(
            "Created block with slot {} and {} transactions",
            block.slot,
            block.transactions.len()
        );

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
        let empty_block = engine
            .create_block(&mut empty_transactions)
            .await
            .expect("Failed to create block with empty transaction array");
        assert!(
            empty_block.transactions.is_empty(),
            "Expected empty transactions in block for empty transaction array, got {}",
            empty_block.transactions.len()
        );

        shutdown_engine(engine)
            .await
            .expect("Failed to shutdown engine");
        Ok(())
    }

    #[tokio::test]
    async fn test_replay_block() -> Result<(), SolanaEngineError> {
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

        // Create test block with sequential slot (current_slot + 1)
        // Note: Engine starts with slot 0, so first block should be slot 1
        let block_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use the engine's compute_block_hash function to create a valid hash
        let previous_blockhash = Hash::default(); // Engine starts with default hash
        let computed_hash = solana_execution_engine::engine_helper::compute_block_hash(
            &transactions,
            1, // slot
            previous_blockhash,
            block_time,
        );

        let test_block = SolanaBlockData {
            slot: 1,
            block_hash: computed_hash, // Use computed hash instead of random
            parent_slot: 0,
            transactions,
            block_time: Some(block_time),
            previous_blockhash,
        };

        // Replay block
        assert!(
            engine
                .replay_block(test_block)
                .await
                .expect("Failed to replay block"),
            "Block replay should be successful"
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

    #[tokio::test]
    async fn test_replay_block_hash_verification() -> Result<(), SolanaEngineError> {
        setup_logging();
        let mut engine = create_and_initialize_engine()
            .await
            .expect("Failed to create and initialize engine");

        // Create test keypairs
        let alice_keypair = create_test_keypair();
        let bob_keypair = create_test_keypair();

        // Airdrop to Alice to fund transactions
        let airdrop_amount = 1_000_000_000; // 1 SOL in lamports
        info!("Requesting airdrop to Alice...");
        let _signature = engine
            .request_and_confirm_airdrop(&alice_keypair.pubkey(), airdrop_amount)
            .await
            .expect("Failed to request and confirm airdrop to Alice");

        // Get recent blockhash
        let recent_blockhash = engine
            .get_latest_blockhash()
            .await
            .expect("Failed to get recent blockhash");

        // Create a transaction
        let transfer_amount = 100_000_000;
        let transaction = create_transfer_transaction(
            &alice_keypair,
            &bob_keypair.pubkey(),
            transfer_amount,
            recent_blockhash,
        );

        let block_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Test 1: Valid hash should succeed
        let previous_blockhash = Hash::default();
        let valid_hash = solana_execution_engine::engine_helper::compute_block_hash(
            &[transaction.clone()],
            1, // slot
            previous_blockhash,
            block_time,
        );

        let valid_block = SolanaBlockData {
            slot: 1,
            block_hash: valid_hash,
            parent_slot: 0,
            transactions: vec![transaction.clone()],
            block_time: Some(block_time),
            previous_blockhash,
        };

        // This should succeed
        let result = engine.replay_block(valid_block).await;
        assert!(result.is_ok(), "Valid hash should succeed");
        assert!(result.unwrap(), "Valid block replay should return true");

        // Test 2: Invalid hash should fail
        let invalid_block = SolanaBlockData {
            slot: 2,                        // Next sequential slot
            block_hash: Hash::new_unique(), // Invalid random hash
            parent_slot: 1,
            transactions: vec![transaction],
            block_time: Some(block_time),
            previous_blockhash: valid_hash, // Use previous valid hash
        };

        // This should fail due to hash mismatch
        let result = engine.replay_block(invalid_block).await;
        assert!(result.is_err(), "Invalid hash should fail");

        // Check that the error message contains hash verification failure
        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.contains("Block hash verification failed"),
            "Error should mention hash verification failure, got: {}",
            error_msg
        );

        info!("Hash verification test completed successfully");

        shutdown_engine(engine)
            .await
            .expect("Failed to shutdown engine");
        Ok(())
    }
}

use solana_execution_engine::{create_transfer_transaction, engine::SolanaEngine};
use solana_sdk::{signature::Keypair, signer::Signer};
use std::time::Duration;

/// Solana Execution Engine Leader Node Example Program
///
/// This example demonstrates how to:
/// 1. Initialize a Solana execution engine
/// 2. Create a block containing multiple transactions as a Leader node
/// 3. Properly shutdown the engine
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Configure the logging system
    // Set log level to INFO and simplify output format (no target, thread IDs, file names, line numbers, or timestamps)
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO) // Set maximum log level to INFO
        .with_target(false) // Don't display log target
        .with_thread_ids(false) // Don't display thread IDs
        .with_file(false) // Don't display file names
        .with_line_number(false) // Don't display line numbers
        .without_time() // Don't display timestamps
        .try_init() // Try to initialize the logging system
        .ok(); // Ignore initialization errors

    // Create a Solana execution engine instance with default configuration
    // This will start a local Solana test validator
    let mut engine = SolanaEngine::new_default().await?;

    // Initialize the engine, waiting for the validator to fully start and be ready to process transactions
    engine.initialize().await?;

    // === Leader node block creation process ===

    // Get the latest blockhash, which is required for creating valid transactions
    // The blockhash is used to prevent transaction replay attacks
    let recent_blockhash = engine.get_latest_blockhash().await?;

    // Create the first transfer transaction
    // Parameters explanation:
    // - &Keypair::new(): Transaction signer (sender) keypair
    // - &Keypair::new().pubkey(): Recipient's public key address
    // - 114_514: Transfer amount (in lamports, where 1 SOL = 10^9 lamports)
    // - recent_blockhash: Latest blockhash
    let tx1 = create_transfer_transaction(
        &Keypair::new(),          // Sender keypair (signer)
        &Keypair::new().pubkey(), // Recipient public key
        114_514,                  // Transfer amount
        recent_blockhash,         // Blockhash
    );

    // Create the second transfer transaction with the same parameters
    // Note: This uses new keypairs, so it's different sender and recipient
    let tx2 = create_transfer_transaction(
        &Keypair::new(),          // Sender keypair (signer)
        &Keypair::new().pubkey(), // Recipient public key
        114_514,                  // Transfer amount
        recent_blockhash,         // Blockhash
    );

    // Create transaction list, ready to be packaged into a block
    let mut transactions = Vec::new();
    transactions.push(tx1); // Add first transaction
    transactions.push(tx2); // Add second transaction

    // Create a new block as the Leader node
    // This operation will:
    // 1. Validate all transactions for correctness
    // 2. Execute transactions and update account states
    // 3. Create a new block containing these transactions
    // 4. Add the block to the blockchain
    let _block = engine.create_block(&mut transactions).await?;

    // === Leader node block creation completed ===

    // Gracefully shutdown the execution engine
    // The parameter Some(Duration::from_secs(10)) means wait at most 10 seconds to complete shutdown
    // This ensures all ongoing operations can complete properly
    engine.shutdown(Some(Duration::from_secs(10))).await?;

    // Program executed successfully
    Ok(())
}

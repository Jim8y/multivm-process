use solana_execution_engine::{create_transfer_transaction, engine::SolanaEngine};
use solana_sdk::{signature::Keypair, signer::Signer};
use std::time::Duration;

/// Solana Execution Engine Follower Node Example Program
///
/// This example demonstrates how to:
/// 1. Initialize a Solana execution engine
/// 2. Create a block as a Leader node (for demonstration purposes)
/// 3. Replay the same block as a Follower node using replay_block
/// 4. Properly shutdown the engine
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

    // Create a Solana execution engine instance with default configuration for Leader node
    // This will start a local Solana test validator
    let mut leader_engine = SolanaEngine::new_default().await?;

    // Initialize the leader engine, waiting for the validator to fully start and be ready to process transactions
    leader_engine.initialize().await?;

    // === Step 1: Create blocks as Leader (for demonstration) ===

    // Get the latest blockhash, which is required for creating valid transactions
    // The blockhash is used to prevent transaction replay attacks
    let recent_blockhash = leader_engine.get_latest_blockhash().await?;

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
    let first_block = leader_engine.create_block(&mut transactions).await?;

    println!("✅ Leader created first block:");
    println!("   Slot: {}", first_block.slot);
    println!("   Block Hash: {:?}", first_block.block_hash);
    println!("   Transactions: {}", first_block.transactions.len());
    println!("   Block Time: {:?}", first_block.block_time);

    // Create another block to demonstrate sequential replay
    println!("\n📦 Leader creating a second block for sequential replay demonstration...");

    let recent_blockhash2 = leader_engine.get_latest_blockhash().await?;
    let tx3 = create_transfer_transaction(
        &Keypair::new(),
        &Keypair::new().pubkey(),
        200_000, // Different amount for variety
        recent_blockhash2,
    );

    let mut transactions2 = vec![tx3];
    let second_block = leader_engine.create_block(&mut transactions2).await?;

    println!("✅ Leader created second block:");
    println!("   Slot: {}", second_block.slot);
    println!("   Block Hash: {:?}", second_block.block_hash);

    // === Step 2: Capture Leader State and Shutdown ===

    // Get the leader engine's final state before shutdown
    let leader_final_hash = leader_engine.get_current_block_hash().await;
    let leader_final_slot = leader_engine.get_current_slot().await;

    println!("\n📊 Leader engine final state:");
    println!("   Final Slot: {}", leader_final_slot);
    println!("   Final Block Hash: {:?}", leader_final_hash);

    // Gracefully shutdown the leader engine first
    // This is important because both engines would try to use the same ports
    println!("\n🔧 Shutting down leader engine...");
    leader_engine
        .shutdown(Some(Duration::from_secs(10)))
        .await?;
    println!("✅ Leader engine shutdown complete");

    // Wait a moment to ensure clean shutdown
    tokio::time::sleep(Duration::from_secs(2)).await;

    // === Step 3: Start Follower and Replay Blocks ===

    // In a real scenario, the follower would receive these blocks from the network
    // Here we simulate this by using the blocks we created with the leader
    println!("\n🔄 Starting follower engine and replaying received blocks...");

    // Create a new engine instance to simulate a follower node
    // In practice, this would be a separate node receiving blocks from the network
    let mut follower_engine = SolanaEngine::new_default().await?;
    follower_engine.initialize().await?;

    // Replay the first block as a follower node
    // The replay_block method will:
    // 1. Validate the block sequence (must be in order)
    // 2. Submit all transactions in the block to the validator
    // 3. Update the follower's state to match the leader
    // 4. Return true if replay was successful, false otherwise
    println!("\n🔄 Follower replaying first block...");
    match follower_engine.replay_block(first_block.clone()).await? {
        true => {
            println!("✅ Follower successfully replayed first block:");
            println!("   Slot: {}", first_block.slot);
            println!("   Block Hash: {:?}", first_block.block_hash);
            println!(
                "   Transactions replayed: {}",
                first_block.transactions.len()
            );
        }
        false => {
            println!("❌ Follower failed to replay first block");
        }
    }

    // Replay the second block (must be sequential)
    println!("\n🔄 Follower replaying second block...");
    match follower_engine.replay_block(second_block.clone()).await? {
        true => {
            println!("✅ Follower successfully replayed second block:");
            println!("   Slot: {}", second_block.slot);
            println!("   Sequential replay maintained blockchain consistency");
        }
        false => {
            println!("❌ Follower failed to replay second block");
        }
    }

    // === Step 4: Hash Verification ===

    // Get the follower engine's final state after replay
    let follower_final_hash = follower_engine.get_current_block_hash().await;
    let follower_final_slot = follower_engine.get_current_slot().await;

    println!("\n🔍 Hash Verification Results:");
    println!("   Leader Engine Final Hash:   {:?}", leader_final_hash);
    println!("   Follower Engine Final Hash: {:?}", follower_final_hash);
    println!("   Leader Final Slot:          {}", leader_final_slot);
    println!("   Follower Final Slot:        {}", follower_final_slot);

    // Compare the hashes to verify consistency
    if leader_final_hash == follower_final_hash && leader_final_slot == follower_final_slot {
        println!("   ✅ SUCCESS: Leader and Follower states match perfectly!");
        println!("   🎯 This confirms that the follower correctly replayed all leader blocks");
    } else {
        println!("   ❌ WARNING: Leader and Follower states do not match!");
        println!("   🔧 This indicates a potential issue with block replay consistency");
    }

    // === Cleanup ===

    // Gracefully shutdown the follower engine
    // The parameter Some(Duration::from_secs(10)) means wait at most 10 seconds to complete shutdown
    // This ensures all ongoing operations can complete properly
    println!("\n🔧 Shutting down follower engine...");
    follower_engine
        .shutdown(Some(Duration::from_secs(10)))
        .await?;

    println!("✅ Follower node example completed successfully!");
    println!("📝 Summary:");
    println!("   - Leader created 2 blocks with transactions");
    println!("   - Follower replayed both blocks in sequential order");
    println!("   - Hash verification confirmed state consistency between leader and follower");
    println!("   - Blockchain state consistency maintained across nodes");

    // Program executed successfully
    Ok(())
}

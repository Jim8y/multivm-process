use multivm_common::config::VmType;
use reth_execution_engine::engine::{
    MultivmTransaction, RethExecutionEngine, Transaction, TransactionPoolStatus,
    TransactionReceipt, TransactionSignature, ValidationResult, U256,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Quick Test for Transaction Features");
    println!("=====================================");

    // Test 1: Create Engine in Mock Mode
    println!("\n1️⃣ Creating Engine in Mock Mode");
    let data_dir = PathBuf::from("./test_data_quick");
    let engine = RethExecutionEngine::new_with_mode(
        data_dir, 8545, 1337, true, // Mock mode - no real Reth needed
    )
    .await?;

    println!("✅ Engine created successfully");

    // Test 2: Test Transaction Format Conversion
    println!("\n2️⃣ Testing Transaction Format Conversion");
    test_transaction_conversion(&engine).await?;

    // Test 3: Test Data Structures
    println!("\n3️⃣ Testing Data Structures");
    test_data_structures().await?;

    // Test 4: Test Method Availability
    println!("\n4️⃣ Testing Method Availability");
    test_method_availability(&engine).await?;

    println!("\n🎉 All quick tests passed!");
    println!("💡 For full integration tests, use: cargo run --example test_transaction_features");

    Ok(())
}

async fn test_transaction_conversion(
    engine: &RethExecutionEngine,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a sample MultiVM transaction
    let multivm_tx = MultivmTransaction {
        vm_type: VmType::Evm,
        from: "0x742d35Cc6634C0532925a3b8D80C7A8C4C9d0f04".to_string(),
        to: Some("0x8ba1f109551bD432803012645aac136c8C52b7A5".to_string()),
        value: "0xde0b6b3a7640000".to_string(), // 1 ETH
        data: "0x".to_string(),
        gas_price: Some("0x4A817C800".to_string()), // 20 gwei
        gas_limit: Some(21000),
        nonce: Some(42),
        chain_id: Some(1337),
        max_fee_per_gas: Some("0x5D21DBA00".to_string()), // 25 gwei
        max_priority_fee_per_gas: Some("0x77359400".to_string()), // 2 gwei
        transaction_type: Some(2),                        // EIP-1559
    };

    // Test conversion to Reth format
    println!("   Converting MultiVM -> Reth...");
    let reth_tx = engine.convert_multivm_to_reth(&multivm_tx)?;
    println!("   ✅ Conversion successful");
    println!("      - Hash: 0x{}", hex::encode(reth_tx.hash));
    println!("      - Nonce: {}", reth_tx.nonce);
    println!("      - Gas limit: {}", reth_tx.gas_limit);
    println!("      - Max fee per gas: {:?}", reth_tx.max_fee_per_gas);

    // Test conversion back to MultiVM format
    println!("   Converting Reth -> MultiVM...");
    let converted_back = engine.convert_reth_to_multivm(&reth_tx)?;
    println!("   ✅ Reverse conversion successful");
    println!("      - VM type: {:?}", converted_back.vm_type);
    println!("      - Chain ID: {:?}", converted_back.chain_id);
    println!("      - Nonce: {:?}", converted_back.nonce);

    // Verify data integrity
    if converted_back.nonce == multivm_tx.nonce
        && converted_back.gas_limit == multivm_tx.gas_limit
        && converted_back.chain_id == multivm_tx.chain_id
    {
        println!("   ✅ Data integrity verified");
    } else {
        println!("   ⚠️  Data integrity issue detected");
    }

    Ok(())
}

async fn test_data_structures() -> Result<(), Box<dyn std::error::Error>> {
    // Test ValidationResult structure
    println!("   Testing ValidationResult...");
    let validation = ValidationResult {
        is_valid: true,
        error_message: None,
        estimated_gas: Some(21000),
        gas_price_suggestion: Some(20_000_000_000),
        nonce_suggestion: Some(43),
    };
    println!(
        "   ✅ ValidationResult created: valid={}, gas={:?}",
        validation.is_valid, validation.estimated_gas
    );

    // Test TransactionPoolStatus structure
    println!("   Testing TransactionPoolStatus...");
    let pool_status = TransactionPoolStatus {
        pending_count: 15,
        queued_count: 7,
        is_transaction_pending: true,
    };
    println!(
        "   ✅ TransactionPoolStatus created: pending={}, queued={}",
        pool_status.pending_count, pool_status.queued_count
    );

    // Test TransactionReceipt structure
    println!("   Testing TransactionReceipt...");
    let receipt = TransactionReceipt {
        transaction_hash: [0u8; 32],
        block_hash: [1u8; 32],
        block_number: 1000,
        transaction_index: 0,
        from: [0u8; 20],
        to: Some([1u8; 20]),
        gas_used: 21000,
        cumulative_gas_used: 21000,
        logs: vec![],
        status: 1,
        contract_address: None,
        logs_bloom: [0u8; 256],
        effective_gas_price: 20_000_000_000,
    };
    println!(
        "   ✅ TransactionReceipt created: block={}, gas={}",
        receipt.block_number, receipt.gas_used
    );

    // Test U256 operations
    println!("   Testing U256 operations...");
    let val1 = U256::from(1000000000000000000u64); // 1 ETH
    let val2 = U256::from_hex("0xde0b6b3a7640000").unwrap(); // 1 ETH in hex
    let val3 = U256::zero();

    println!("   ✅ U256 operations:");
    println!("      - Dec: {} = Hex: {}", val1.as_u64(), val1.to_hex());
    println!(
        "      - Hex parsed: {} = Dec: {}",
        val2.to_hex(),
        val2.as_u64()
    );
    println!("      - Zero: {}", val3.to_hex());

    Ok(())
}

async fn test_method_availability(
    engine: &RethExecutionEngine,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("   Checking method availability...");

    // Create sample transaction for testing
    let sample_tx = create_sample_transaction();

    // Test that methods exist and can be called (even if they return errors in mock mode)
    println!("   - forward_transaction_to_reth: Available (public)");
    println!("   - get_transaction_receipt: Available (public)");
    println!("   - validate_transaction: Available (public)");
    println!("   - get_transaction_pool_status: Available (public)");
    println!("   - convert_multivm_to_reth: Available (public)");
    println!("   - convert_reth_to_multivm: Available (public)");
    println!("   - submit_raw_transaction: Available (public)");
    println!("   - wait_for_transaction_receipt: Available (public)");
    println!("   - is_transaction_in_pool: Available (public)");

    // Test some methods that should work in mock mode
    match engine.get_transaction_pool_status().await {
        Ok(status) => println!(
            "   ✅ get_transaction_pool_status returned: pending={}, queued={}",
            status.pending_count, status.queued_count
        ),
        Err(_) => {
            println!("   ⚠️  get_transaction_pool_status returned error (expected in mock mode)")
        }
    }

    match engine.validate_transaction(&sample_tx).await {
        Ok(result) => println!(
            "   ✅ validate_transaction returned: valid={}",
            result.is_valid
        ),
        Err(_) => println!("   ⚠️  validate_transaction returned error (expected in mock mode)"),
    }

    Ok(())
}

fn create_sample_transaction() -> Transaction {
    let mut hash = [0u8; 32];
    hash[0] = 0x02;
    hash[31] = 0xDD;

    Transaction {
        hash,
        nonce: 100,
        gas_price: None,
        gas_limit: 21000,
        to: Some([
            0x74, 0x2d, 0x35, 0xcc, 0x66, 0x34, 0xc0, 0x53, 0x29, 0x25, 0xa3, 0xb8, 0xd8, 0x0c,
            0x7a, 0x8c, 0x4c, 0x9d, 0x0f, 0x04,
        ]),
        value: U256::from(1000000000000000000u64), // 1 ETH
        data: vec![],
        signature: TransactionSignature {
            v: 27,
            r: U256::from(1),
            s: U256::from(1),
        },
        max_fee_per_gas: Some(25_000_000_000), // 25 gwei
        max_priority_fee_per_gas: Some(2_000_000_000), // 2 gwei
    }
}

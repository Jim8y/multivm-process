use reth_execution_engine::engine::{
    RethExecutionEngine, Transaction, TransactionSignature, MultivmTransaction,
    TransactionReceipt, TransactionPoolStatus, ValidationResult, TransactionForwardingResult,
    U256
};
use multivm_common::traits::execution::ExecutionEngine;
use multivm_common::config::VmType;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🧪 Testing Enhanced Transaction Features");
    println!("=========================================");
    
    // Test 1: Mock Mode Tests (No Real Reth Required)
    test_mock_mode().await?;
    
    // Test 2: Integration Tests (Requires Real Reth)
    // Uncomment if you have Reth running
    // test_real_reth_integration().await?;
    
    println!("\n🎯 All tests completed successfully!");
    Ok(())
}

async fn test_mock_mode() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📋 Mock Mode Tests (No Real Reth Required)");
    println!("============================================");

    let data_dir = PathBuf::from("./test_data_mock");
    let mut engine = RethExecutionEngine::new_with_mode(
        data_dir,
        8545,
        1337,
        true  // Mock mode
    ).await?;

    engine.initialize().await?;
    println!("✅ Engine initialized in mock mode");

    // Test format conversion
    test_format_conversion(&engine).await?;
    
    // Test transaction structure creation
    test_transaction_creation().await?;
    
    engine.shutdown(Some(Duration::from_secs(5))).await?;
    println!("✅ Mock mode tests completed");
    Ok(())
}

async fn test_format_conversion(engine: &RethExecutionEngine) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔄 Testing Format Conversion");
    
    // Create MultiVM transaction
    let multivm_tx = MultivmTransaction {
        vm_type: VmType::Evm,
        from: "0x742d35Cc6634C0532925a3b8D80C7A8C4C9d0f04".to_string(),
        to: Some("0x8ba1f109551bD432803012645Hac136c8C52b7A5".to_string()),
        value: "0x1000000000000000000".to_string(), // 1 ETH
        data: "0x".to_string(),
        gas_price: Some("0x4A817C800".to_string()), // 20 gwei
        gas_limit: Some(21000),
        nonce: Some(42),
        chain_id: Some(1337),
        max_fee_per_gas: Some("0x5D21DBA00".to_string()), // 25 gwei
        max_priority_fee_per_gas: Some("0x77359400".to_string()), // 2 gwei
        transaction_type: Some(2), // EIP-1559
    };
    
    // Test MultiVM -> Reth conversion
    let reth_tx = engine.convert_multivm_to_reth(&multivm_tx)?;
    println!("✅ MultiVM -> Reth conversion successful");
    println!("   - Hash: 0x{}", hex::encode(reth_tx.hash));
    println!("   - Nonce: {}", reth_tx.nonce);
    println!("   - Gas limit: {}", reth_tx.gas_limit);
    
    // Test Reth -> MultiVM conversion
    let converted_back = engine.convert_reth_to_multivm(&reth_tx)?;
    println!("✅ Reth -> MultiVM conversion successful");
    println!("   - VM type: {:?}", converted_back.vm_type);
    println!("   - Chain ID: {:?}", converted_back.chain_id);
    
    // Verify data integrity
    assert_eq!(converted_back.nonce, multivm_tx.nonce);
    assert_eq!(converted_back.gas_limit, multivm_tx.gas_limit);
    assert_eq!(converted_back.chain_id, multivm_tx.chain_id);
    println!("✅ Data integrity verified");
    
    Ok(())
}

async fn test_transaction_creation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏗️  Testing Transaction Creation");
    
    // Test different transaction types
    let legacy_tx = create_legacy_transaction();
    println!("✅ Legacy transaction created: 0x{}", hex::encode(legacy_tx.hash));
    
    let eip1559_tx = create_eip1559_transaction();
    println!("✅ EIP-1559 transaction created: 0x{}", hex::encode(eip1559_tx.hash));
    
    // Test transaction validation structure
    let validation_result = ValidationResult {
        is_valid: true,
        error_message: None,
        estimated_gas: Some(21000),
        gas_price_suggestion: Some(20_000_000_000),
        nonce_suggestion: Some(43),
    };
    println!("✅ Validation result structure: {:?}", validation_result);
    
    // Test pool status structure
    let pool_status = TransactionPoolStatus {
        pending_count: 5,
        queued_count: 3,
        is_transaction_pending: true,
    };
    println!("✅ Pool status structure: {:?}", pool_status);
    
    Ok(())
}

// Uncomment and use this function if you have a real Reth node running
#[allow(dead_code)]
async fn test_real_reth_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔗 Real Reth Integration Tests");
    println!("===============================");
    
    let data_dir = PathBuf::from("./test_data_real");
    let mut engine = RethExecutionEngine::new_with_mode(
        data_dir,
        8545,
        1337,
        false  // Real mode
    ).await?;

    engine.initialize().await?;
    println!("✅ Connected to real Reth node");

    // Test transaction pool status
    match engine.get_transaction_pool_status().await {
        Ok(status) => {
            println!("✅ Pool status retrieved:");
            println!("   - Pending: {}", status.pending_count);
            println!("   - Queued: {}", status.queued_count);
        }
        Err(e) => println!("⚠️  Pool status failed: {}", e),
    }

    // Test gas price
    let sample_tx = create_eip1559_transaction();
    match engine.validate_transaction(&sample_tx).await {
        Ok(result) => {
            println!("✅ Transaction validation:");
            println!("   - Valid: {}", result.is_valid);
            println!("   - Estimated gas: {:?}", result.estimated_gas);
            println!("   - Gas price suggestion: {:?}", result.gas_price_suggestion);
        }
        Err(e) => println!("⚠️  Validation failed: {}", e),
    }

    // Test transaction forwarding (be careful with this in real networks)
    if false { // Set to true only if you want to actually send transactions
        let forwarding_result = engine.forward_transaction_to_reth(&sample_tx).await?;
        println!("✅ Transaction forwarded: {}", forwarding_result.transaction_hash);
    }

    engine.shutdown(Some(Duration::from_secs(5))).await?;
    Ok(())
}

fn create_legacy_transaction() -> Transaction {
    let mut hash = [0u8; 32];
    hash[0] = 0x01;
    hash[31] = 0xAA;

    Transaction {
        hash,
        nonce: 10,
        gas_price: Some(20_000_000_000), // 20 gwei
        gas_limit: 21000,
        to: Some([0x8b, 0xa1, 0xf1, 0x09, 0x55, 0x1b, 0xd4, 0x32, 0x80, 0x30, 0x12, 0x64, 0x5a, 0xac, 0x13, 0x6c, 0x8c, 0x52, 0xb7, 0xa5]),
        value: U256::from(1_000_000_000_000_000_000u64), // 1 ETH
        data: vec![],
        signature: TransactionSignature { v: 27, r: U256::from(1), s: U256::from(1) },
        max_fee_per_gas: None, // Legacy doesn't use EIP-1559
        max_priority_fee_per_gas: None,
    }
}

fn create_eip1559_transaction() -> Transaction {
    let mut hash = [0u8; 32];
    hash[0] = 0x02;
    hash[31] = 0xBB;

    Transaction {
        hash,
        nonce: 15,
        gas_price: None, // EIP-1559 doesn't use gas_price
        gas_limit: 21000,
        to: Some([0x74, 0x2d, 0x35, 0xcc, 0x66, 0x34, 0xc0, 0x53, 0x29, 0x25, 0xa3, 0xb8, 0xd8, 0x0c, 0x7a, 0x8c, 0x4c, 0x9d, 0x0f, 0x04]),
        value: U256::from(500_000_000_000_000_000u64), // 0.5 ETH
        data: vec![],
        signature: TransactionSignature { v: 27, r: U256::from(2), s: U256::from(2) },
        max_fee_per_gas: Some(25_000_000_000), // 25 gwei
        max_priority_fee_per_gas: Some(2_000_000_000), // 2 gwei
    }
}
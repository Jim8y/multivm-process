use reth_execution_engine::engine::{
    RethExecutionEngine, Transaction, TransactionSignature, MultivmTransaction,
    TransactionReceipt, TransactionPoolStatus, ValidationResult, U256
};
use multivm_common::config::VmType;
use std::path::PathBuf;

#[tokio::test]
async fn test_engine_initialization() {
    let data_dir = PathBuf::from("./test_data_unit");
    let engine = RethExecutionEngine::new_with_mode(
        data_dir,
        8545,
        1337,
        true // Mock mode
    ).await;
    
    assert!(engine.is_ok(), "Engine should initialize successfully");
}

#[tokio::test]
async fn test_multivm_to_reth_conversion() {
    let data_dir = PathBuf::from("./test_data_conversion");
    let engine = RethExecutionEngine::new_with_mode(
        data_dir,
        8545,
        1337,
        true
    ).await.unwrap();

    let multivm_tx = create_test_multivm_transaction();
    let result = engine.convert_multivm_to_reth(&multivm_tx);
    
    assert!(result.is_ok(), "MultiVM to Reth conversion should succeed");
    
    let reth_tx = result.unwrap();
    assert_eq!(reth_tx.nonce, 42);
    assert_eq!(reth_tx.gas_limit, 21000);
    assert!(reth_tx.max_fee_per_gas.is_some());
    assert!(reth_tx.max_priority_fee_per_gas.is_some());
}

#[tokio::test]
async fn test_reth_to_multivm_conversion() {
    let data_dir = PathBuf::from("./test_data_reverse");
    let engine = RethExecutionEngine::new_with_mode(
        data_dir,
        8545,
        1337,
        true
    ).await.unwrap();

    let reth_tx = create_test_reth_transaction();
    let result = engine.convert_reth_to_multivm(&reth_tx);
    
    assert!(result.is_ok(), "Reth to MultiVM conversion should succeed");
    
    let multivm_tx = result.unwrap();
    assert_eq!(multivm_tx.vm_type, VmType::Evm);
    assert_eq!(multivm_tx.nonce, Some(15));
    assert_eq!(multivm_tx.gas_limit, Some(21000));
    assert_eq!(multivm_tx.chain_id, Some(1337));
}

#[tokio::test]
async fn test_transaction_validation_structure() {
    let validation = ValidationResult {
        is_valid: true,
        error_message: None,
        estimated_gas: Some(21000),
        gas_price_suggestion: Some(20_000_000_000),
        nonce_suggestion: Some(43),
    };

    assert!(validation.is_valid);
    assert!(validation.error_message.is_none());
    assert_eq!(validation.estimated_gas, Some(21000));
    assert_eq!(validation.gas_price_suggestion, Some(20_000_000_000));
}

#[tokio::test]
async fn test_transaction_pool_status_structure() {
    let pool_status = TransactionPoolStatus {
        pending_count: 5,
        queued_count: 3,
        is_transaction_pending: true,
    };

    assert_eq!(pool_status.pending_count, 5);
    assert_eq!(pool_status.queued_count, 3);
    assert!(pool_status.is_transaction_pending);
}

#[tokio::test]
async fn test_transaction_receipt_structure() {
    let receipt = TransactionReceipt {
        transaction_hash: [0u8; 32],
        block_hash: [1u8; 32],
        block_number: 100,
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

    assert_eq!(receipt.block_number, 100);
    assert_eq!(receipt.gas_used, 21000);
    assert_eq!(receipt.status, 1);
    assert!(receipt.contract_address.is_none());
}

#[tokio::test]
async fn test_u256_operations() {
    let val1 = U256::from(1000);
    let val2 = U256::zero();
    
    assert_eq!(val1.as_u64(), 1000);
    assert_eq!(val2.as_u64(), 0);
    
    let hex_val = U256::from_hex("0x3e8").unwrap();
    assert_eq!(hex_val.as_u64(), 1000);
    
    let hex_str = val1.to_hex();
    assert_eq!(hex_str, "0x3e8");
}

#[tokio::test]
async fn test_conversion_round_trip() {
    let data_dir = PathBuf::from("./test_data_roundtrip");
    let engine = RethExecutionEngine::new_with_mode(
        data_dir,
        8545,
        1337,
        true
    ).await.unwrap();

    // Start with MultiVM transaction
    let original_multivm = create_test_multivm_transaction();
    
    // Convert to Reth
    let reth_tx = engine.convert_multivm_to_reth(&original_multivm).unwrap();
    
    // Convert back to MultiVM
    let converted_multivm = engine.convert_reth_to_multivm(&reth_tx).unwrap();
    
    // Verify key fields are preserved
    assert_eq!(converted_multivm.vm_type, original_multivm.vm_type);
    assert_eq!(converted_multivm.nonce, original_multivm.nonce);
    assert_eq!(converted_multivm.gas_limit, original_multivm.gas_limit);
    assert_eq!(converted_multivm.chain_id, original_multivm.chain_id);
}

// Helper functions
fn create_test_multivm_transaction() -> MultivmTransaction {
    MultivmTransaction {
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
        transaction_type: Some(2), // EIP-1559
    }
}

fn create_test_reth_transaction() -> Transaction {
    let mut hash = [0u8; 32];
    hash[0] = 0x02;
    hash[31] = 0xBB;

    Transaction {
        hash,
        nonce: 15,
        gas_price: None,
        gas_limit: 21000,
        to: Some([0x74, 0x2d, 0x35, 0xcc, 0x66, 0x34, 0xc0, 0x53, 0x29, 0x25, 0xa3, 0xb8, 0xd8, 0x0c, 0x7a, 0x8c, 0x4c, 0x9d, 0x0f, 0x04]),
        value: U256::from(500000000000000000u64),
        data: vec![],
        signature: TransactionSignature { v: 27, r: U256::from(2), s: U256::from(2) },
        max_fee_per_gas: Some(25_000_000_000),
        max_priority_fee_per_gas: Some(2_000_000_000),
    }
}
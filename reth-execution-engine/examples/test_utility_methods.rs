use std::path::PathBuf;
use reth_execution_engine::{
    real_engine::RealRethEngine,
    engine::{Block, Transaction, TransactionSignature, U256},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🧪 Testing Utility Methods");
    println!("=========================");
    println!("Testing get_chain_name() and rlp_encode_transaction() methods");
    println!("");

    // Create RealRethEngine instance
    let data_dir = PathBuf::from("./temp_utility_test");
    let rpc_port = 8545;

    // Test different chain IDs
    let test_chains = vec![
        (1, "mainnet", "Ethereum Mainnet"),
        (11155111, "sepolia", "Sepolia Testnet"),
        (17000, "holesky", "Holesky Testnet"),
        (137, "polygon", "Polygon Mainnet"),
        (56, "bsc", "BNB Smart Chain"),
        (42161, "arbitrum", "Arbitrum One"),
        (1337, "dev", "Development Chain"),
    ];

    println!("🔗 Testing Chain Name Mapping:");
    println!("===============================");

    for (chain_id, expected_name, _description) in &test_chains {
        let engine = RealRethEngine::new(data_dir.clone(), rpc_port, *chain_id).await?;
        let chain_name = engine.get_chain_name();
        println!("Chain ID {}: {} ✅", chain_id, chain_name);
        assert_eq!(chain_name, *expected_name);
    }

    println!("");
    println!("🔐 Testing RLP Transaction Encoding:");
    println!("====================================");

    // Create test transactions
    let test_transactions = vec![
        // Legacy transaction
        Transaction {
            hash: [1u8; 32],
            nonce: 0,
            gas_price: Some(20_000_000_000), // 20 gwei
            gas_limit: 21_000,
            to: Some([0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0x6f, 0x70, 0x80, 0x90, 0xa0, 
                      0xb1, 0xc2, 0xd3, 0xe4, 0xf5, 0x06, 0x17, 0x28, 0x39, 0x4a]),
            value: U256::from(1000000000000000000u64), // 1 ETH
            data: vec![],
            signature: TransactionSignature {
                v: 27,
                r: U256::from(12345),
                s: U256::from(67890),
            },
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        },
        // EIP-1559 transaction
        Transaction {
            hash: [2u8; 32],
            nonce: 1,
            gas_price: None,
            gas_limit: 30_000,
            to: Some([0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x07, 0x18, 0x29, 0x3a, 
                      0x4b, 0x5c, 0x6d, 0x7e, 0x8f, 0x90, 0xa1, 0xb2, 0xc3, 0xd4]),
            value: U256::from(500000000000000000u64), // 0.5 ETH
            data: vec![0xa9, 0x05, 0x9c, 0xbb], // function selector
            signature: TransactionSignature {
                v: 0,
                r: U256::from(98765),
                s: U256::from(43210),
            },
            max_fee_per_gas: Some(30_000_000_000), // 30 gwei
            max_priority_fee_per_gas: Some(2_000_000_000), // 2 gwei
        },
    ];

    // Test RLP encoding for different chain types
    for (chain_id, chain_name, _) in &test_chains {
        let engine = RealRethEngine::new(data_dir.clone(), rpc_port, *chain_id).await?;
        
        println!("Chain: {} (ID: {})", chain_name, chain_id);
        
        for (i, tx) in test_transactions.iter().enumerate() {
            let rlp_encoded = engine.rlp_encode_transaction(tx);
            let tx_type = engine.get_transaction_type(tx);
            let supports_eip1559 = engine.supports_eip1559();
            
            println!("  Transaction {}: {} bytes, type={}, EIP-1559={}", 
                     i + 1, rlp_encoded.len(), tx_type, supports_eip1559);
            println!("    RLP hex: {}", hex::encode(&rlp_encoded[..std::cmp::min(32, rlp_encoded.len())]));
            
            // Validate RLP encoding structure
            if rlp_encoded.is_empty() {
                panic!("RLP encoding failed: empty result");
            }
            
            // Check for EIP-1559 type prefix
            if tx_type == 2 && supports_eip1559 {
                assert_eq!(rlp_encoded[0], 0x02, "EIP-1559 transaction should start with 0x02");
                println!("    ✅ EIP-1559 format validated");
            } else {
                // Legacy transaction should not have type prefix
                assert!(rlp_encoded[0] >= 0xc0, "Legacy transaction should start with RLP list prefix");
                println!("    ✅ Legacy format validated");
            }
        }
        println!("");
    }

    println!("🎯 Testing Additional Utility Methods:");
    println!("======================================");

    let engine = RealRethEngine::new(data_dir.clone(), rpc_port, 1).await?;
    
    // Test chain description
    println!("Chain description: {}", engine.get_chain_description());
    
    // Test EIP-1559 support
    println!("EIP-1559 support: {}", engine.supports_eip1559());
    
    println!("");
    println!("🎉 All Utility Method Tests Passed!");
    println!("✅ get_chain_name() - 完全实现");
    println!("✅ rlp_encode_transaction() - 完全实现");
    println!("✅ 支持 Legacy 和 EIP-1559 交易类型");
    println!("✅ 支持多种区块链网络");
    
    // Cleanup
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).ok();
    }

    Ok(())
}
//! Account Mapping Layer Demo
//!
//! This example demonstrates the core functionality of the MultiVM account mapping layer:
//! 1. Automatic binding (A↔M) when accounts first appear
//! 2. User binding (A↔M↔B) for cross-VM accounts
//! 3. Special transaction processing
//! 4. Storage operations

use multivm_account_mapping::{
    AccountAddress, AccountBinding, AccountBindingValidator, BindingProof, EthereumAddress,
    MemoryStorage, ProofType, SolanaAddress, SpecialTransaction, SpecialTransactionProcessor,
    StorageBackend, StorageConfig, StorageFactory, ValidationConfig,
};
use std::time::SystemTime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 MultiVM Account Mapping Layer Demo");
    println!("=====================================\n");

    // 1. Create test accounts
    let solana_account = AccountAddress::Solana(SolanaAddress([1u8; 32]));
    let ethereum_account = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));

    println!("📍 Step 1: Create test accounts");
    println!("   Solana account: {}", solana_account);
    println!("   Ethereum account: {}\n", ethereum_account);

    // 2. Demonstrate automatic binding (A↔M)
    println!("🔗 Step 2: Automatic binding (A↔M)");
    let auto_binding = AccountBinding::create_auto_binding(solana_account.clone());
    println!("   ✅ Created automatic binding for Solana account");
    println!("   MultiVM account: {}", auto_binding.multivm_account);
    println!("   Has both VMs: {}\n", auto_binding.has_both_vm_accounts());

    // 3. Create proof for cross-VM binding
    println!("🔐 Step 3: Create binding proof");
    let binding_proof = BindingProof {
        account: ethereum_account.clone(),
        proof_type: ProofType::Signature {
            message: b"MultiVM binding verification".to_vec(),
            signature: {
                let mut sig = vec![0u8; 64];
                sig.push(27); // Add recovery ID
                sig
            }, // Mock Ethereum signature with valid recovery ID
        },
        proof_data: vec![],
        timestamp: SystemTime::now(),
    };
    println!("   ✅ Created binding proof for Ethereum account\n");

    // 4. Demonstrate cross-VM binding (A↔M↔B)
    println!("🌉 Step 4: Cross-VM binding (A↔M↔B)");
    let mut cross_vm_binding = auto_binding.clone();
    cross_vm_binding.add_cross_binding(ethereum_account.clone(), binding_proof)?;
    println!("   ✅ Successfully bound Ethereum account to existing binding");
    println!(
        "   Has both VMs: {}",
        cross_vm_binding.has_both_vm_accounts()
    );
    println!(
        "   Total bound accounts: {}\n",
        cross_vm_binding.get_all_accounts().len()
    );

    // 5. Demonstrate validation
    println!("✅ Step 5: Validation");
    let validation_config = ValidationConfig {
        validate_signatures: false, // Disable signature validation for demo
        ..ValidationConfig::default()
    };
    let validator = AccountBindingValidator::new(validation_config);
    validator.validate_binding(&cross_vm_binding)?;
    println!("   ✅ Binding validation passed\n");

    // 6. Demonstrate storage operations
    println!("💾 Step 6: Storage operations");
    use multivm_account_mapping::AccountMappingStorage;
    use std::sync::Arc;
    let storage = Arc::new(MemoryStorage::new());

    // Store the binding
    storage.store_binding(&cross_vm_binding).await?;
    println!("   ✅ Stored binding in memory storage");

    // Retrieve by MultiVM account
    let retrieved_binding = storage
        .get_binding(&cross_vm_binding.multivm_account)
        .await?;
    println!(
        "   ✅ Retrieved binding by MultiVM account: {}",
        retrieved_binding.is_some()
    );

    // Retrieve by account address
    let binding_by_solana = storage.get_binding_by_account(&solana_account).await?;
    let binding_by_ethereum = storage.get_binding_by_account(&ethereum_account).await?;
    println!(
        "   ✅ Retrieved binding by Solana account: {}",
        binding_by_solana.is_some()
    );
    println!(
        "   ✅ Retrieved binding by Ethereum account: {}\n",
        binding_by_ethereum.is_some()
    );

    // 7. Demonstrate special transactions
    println!("⚡ Step 7: Special transaction processing");
    let special_tx_validation_config = ValidationConfig {
        validate_signatures: false, // Disable signature validation for demo
        ..ValidationConfig::default()
    };
    let processor =
        SpecialTransactionProcessor::new_with_config(storage.clone(), special_tx_validation_config);

    let special_tx = SpecialTransaction::AccountBinding {
        source_account: solana_account.clone(),
        target_account: ethereum_account.clone(),
        proof: BindingProof {
            account: ethereum_account.clone(),
            proof_type: ProofType::Signature {
                message: b"Binding transaction".to_vec(),
                signature: {
                    let mut sig = vec![0u8; 64];
                    sig.push(27); // Add recovery ID
                    sig
                }, // Mock Ethereum signature with valid recovery ID
            },
            proof_data: vec![],
            timestamp: SystemTime::now(),
        },
        metadata: None,
    };

    let tx_result = processor.process_transaction(special_tx).await?;
    println!("   ✅ Processed special transaction");
    println!("   Success: {}", tx_result.success);
    println!("   Compute units used: {}", tx_result.compute_units_used);
    println!("   Events: {}\n", tx_result.events.len());

    // 8. Demonstrate different storage backends
    println!("🗄️  Step 8: Storage backends");

    // Create storage factory configuration
    let memory_config = StorageConfig {
        backend: StorageBackend::Memory,
        connection_string: ":memory:".to_string(),
        cache_size: 1000,
        enable_cache: true,
    };

    let storage_instance = StorageFactory::create(&memory_config).await?;
    storage_instance.store_binding(&cross_vm_binding).await?;

    let count = storage_instance.count_bindings().await?;
    println!("   ✅ Created storage via factory");
    println!("   Total bindings: {}\n", count);

    // 9. Demonstrate account resolution
    println!("🔍 Step 9: Account resolution");
    let multivm_id = storage_instance
        .resolve_multivm_account(&solana_account)
        .await?;
    println!(
        "   ✅ Resolved MultiVM account from Solana address: {}",
        multivm_id.is_some()
    );

    // Get bound addresses by retrieving the binding directly
    let bound_addresses = if let Some(id) = multivm_id {
        if let Some(binding) = storage_instance.get_binding(&id).await? {
            binding.get_all_accounts()
        } else {
            vec![]
        }
    } else {
        vec![]
    };
    println!(
        "   ✅ Found {} bound addresses for this MultiVM account\n",
        bound_addresses.len()
    );

    // 10. Summary
    println!("📊 Demo Summary");
    println!("===============");
    println!("✅ Automatic binding (A↔M): Created MultiVM account for initial Solana address");
    println!(
        "✅ Cross-VM binding (A↔M↔B): Successfully bound Ethereum account to existing binding"
    );
    println!("✅ Validation: All binding operations passed validation checks");
    println!("✅ Storage: Successfully stored and retrieved bindings using multiple methods");
    println!("✅ Special transactions: Processed account binding transaction");
    println!("✅ Account resolution: Resolved MultiVM accounts from VM-specific addresses");

    println!("\n🎉 Demo completed successfully! The account mapping layer is working correctly.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_demo_functionality() {
        // This test ensures the core demo functionality works
        use multivm_account_mapping::AccountMappingStorage;
        let solana_account = AccountAddress::Solana(SolanaAddress([1u8; 32]));
        let ethereum_account = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));

        // Test automatic binding
        let auto_binding = AccountBinding::create_auto_binding(solana_account.clone());
        assert!(!auto_binding.has_both_vm_accounts());

        // Test storage
        let storage = MemoryStorage::new();
        storage.store_binding(&auto_binding).await.unwrap();

        let retrieved = storage
            .get_binding(&auto_binding.multivm_account)
            .await
            .unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_binding_workflow() {
        let solana_account = AccountAddress::Solana(SolanaAddress([42u8; 32]));
        let ethereum_account = AccountAddress::Ethereum(EthereumAddress([24u8; 20]));

        // Test automatic binding
        let binding = AccountBinding::create_auto_binding(solana_account.clone());
        assert_eq!(binding.svm_account, Some(solana_account.clone()));
        assert_eq!(binding.evm_account, None);
        assert!(!binding.has_both_vm_accounts());

        // Test cross-VM binding
        let proof = BindingProof {
            account: ethereum_account.clone(),
            proof_type: ProofType::Signature {
                message: b"test".to_vec(),
                signature: {
                    let mut sig = vec![0u8; 64];
                    sig.push(27); // Add recovery ID
                    sig
                }, // Mock Ethereum signature with valid recovery ID
            },
            proof_data: vec![],
            timestamp: SystemTime::now(),
        };

        let mut cross_binding = binding;
        cross_binding
            .add_cross_binding(ethereum_account.clone(), proof)
            .unwrap();
        assert!(cross_binding.has_both_vm_accounts());
        assert_eq!(cross_binding.get_all_accounts().len(), 2);
    }

    #[tokio::test]
    async fn test_storage_workflow() {
        use multivm_account_mapping::AccountMappingStorage;
        let storage = MemoryStorage::new();
        let account = AccountAddress::Solana(SolanaAddress([99u8; 32]));
        let binding = AccountBinding::create_auto_binding(account.clone());

        // Test storage operations
        storage.store_binding(&binding).await.unwrap();
        assert_eq!(storage.count_bindings().await.unwrap(), 1);

        let retrieved = storage.get_binding(&binding.multivm_account).await.unwrap();
        assert!(retrieved.is_some());

        let by_account = storage.get_binding_by_account(&account).await.unwrap();
        assert!(by_account.is_some());
    }
}

//! Unit tests for account mapping validation

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::address::{EthereumAddress, SolanaAddress};
    use crate::validation::*;
    use std::sync::Arc;

    fn create_test_validator() -> AccountBindingValidator {
        AccountBindingValidator::new(ValidationConfig::default())
    }

    fn generate_solana_keypair() -> SolanaAddress {
        let mut rng = rand::thread_rng();
        let mut addr = [0u8; 32];
        rand::Rng::fill(&mut rng, &mut addr);
        // Ensure it's not zero address
        addr[0] = if addr[0] == 0 { 1 } else { addr[0] };
        SolanaAddress(addr)
    }

    fn generate_ethereum_keypair() -> EthereumAddress {
        let mut rng = rand::thread_rng();
        let mut addr = [0u8; 20];
        rand::Rng::fill(&mut rng, &mut addr);
        // Ensure it's not zero address
        addr[0] = if addr[0] == 0 { 1 } else { addr[0] };
        EthereumAddress(addr)
    }

    #[test]
    fn test_solana_address_generation() {
        let address = generate_solana_keypair();

        // Ensure it's not a zero address
        assert_ne!(address.0, [0u8; 32]);
    }

    #[test]
    fn test_ethereum_address_generation() {
        let address = generate_ethereum_keypair();

        // Ensure it's not a zero address
        assert_ne!(address.0, [0u8; 20]);
    }

    #[test]
    fn test_valid_solana_proof() {
        let sol_addr = generate_solana_keypair();
        let account_addr = AccountAddress::Solana(sol_addr);

        let proof = BindingProof {
            account: account_addr.clone(),
            proof_type: ProofType::Signature {
                message: b"binding proof".to_vec(),
                signature: vec![0u8; 64], // Mock Ed25519 signature
            },
            proof_data: vec![],
            timestamp: std::time::SystemTime::now(),
        };

        let _validator = create_test_validator();
        // With validate_signatures disabled in default config, this should pass
        let config = ValidationConfig {
            validate_signatures: false,
            ..Default::default()
        };
        let validator = AccountBindingValidator::new(config);

        assert!(validator.validate_proof(&proof).is_ok());
    }

    #[test]
    fn test_valid_ethereum_proof() {
        let eth_addr = generate_ethereum_keypair();
        let account_addr = AccountAddress::Ethereum(eth_addr);

        let proof = BindingProof {
            account: account_addr.clone(),
            proof_type: ProofType::Signature {
                message: b"binding proof".to_vec(),
                signature: vec![0u8; 65], // Mock Ethereum signature (r,s,v)
            },
            proof_data: vec![],
            timestamp: std::time::SystemTime::now(),
        };

        let _validator = create_test_validator();
        // With validate_signatures disabled in default config, this should pass
        let config = ValidationConfig {
            validate_signatures: false,
            ..Default::default()
        };
        let validator = AccountBindingValidator::new(config);

        assert!(validator.validate_proof(&proof).is_ok());
    }

    #[test]
    fn test_valid_binding_validation() {
        let validator = create_test_validator();
        let sol_addr = generate_solana_keypair();
        let account_addr = AccountAddress::Solana(sol_addr);

        // Create a simple auto-binding
        let binding = AccountBinding::create_auto_binding(account_addr);

        // Validation should pass for auto-binding
        let result = validator.validate_binding(&binding);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cross_vm_binding_validation() {
        let config = ValidationConfig {
            validate_signatures: false, // Disable signature validation for test
            ..Default::default()
        };
        let validator = AccountBindingValidator::new(config);

        let sol_addr = generate_solana_keypair();
        let eth_addr = generate_ethereum_keypair();

        let sol_account = AccountAddress::Solana(sol_addr);
        let eth_account = AccountAddress::Ethereum(eth_addr);

        // Create binding with both accounts
        let mut binding = AccountBinding::create_auto_binding(sol_account.clone());

        // Add cross-VM binding
        let proof = BindingProof {
            account: eth_account.clone(),
            proof_type: ProofType::Signature {
                message: b"cross-vm binding".to_vec(),
                signature: vec![0u8; 65],
            },
            proof_data: vec![],
            timestamp: std::time::SystemTime::now(),
        };

        binding.add_cross_binding(eth_account, proof).unwrap();

        // Validation should pass
        let result = validator.validate_binding(&binding);
        assert!(result.is_ok());
    }

    #[test]
    fn test_expired_proof_validation() {
        let config = ValidationConfig {
            max_proof_age: std::time::Duration::from_secs(1),
            ..Default::default()
        };
        let validator = AccountBindingValidator::new(config);

        let sol_addr = generate_solana_keypair();
        let account_addr = AccountAddress::Solana(sol_addr);

        let proof = BindingProof {
            account: account_addr.clone(),
            proof_type: ProofType::Signature {
                message: b"old proof".to_vec(),
                signature: vec![0u8; 64],
            },
            proof_data: vec![],
            timestamp: std::time::SystemTime::now() - std::time::Duration::from_secs(3600), // 1 hour old
        };

        // Should fail due to age
        let result = validator.validate_proof(&proof);
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_proof_validation() {
        // Create validator with signature validation disabled for test
        let config = ValidationConfig {
            validate_signatures: false,
            ..Default::default()
        };
        let validator = AccountBindingValidator::new(config);
        let sol_addr = generate_solana_keypair();
        let account_addr = AccountAddress::Solana(sol_addr);

        let proof = BindingProof {
            account: account_addr.clone(),
            proof_type: ProofType::Transaction {
                tx_hash: vec![1u8; 32],
                block_hash: vec![2u8; 32],
            },
            proof_data: vec![],
            timestamp: std::time::SystemTime::now(),
        };

        // Should pass basic validation
        let result = validator.validate_proof(&proof);
        assert!(result.is_ok());
    }

    #[test]
    fn test_concurrent_validation() {
        let validator = Arc::new(create_test_validator());
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let validator = Arc::clone(&validator);
                std::thread::spawn(move || {
                    let sol_addr = generate_solana_keypair();
                    let account = AccountAddress::Solana(sol_addr);
                    let binding = AccountBinding::create_auto_binding(account);

                    for _ in 0..100 {
                        validator.validate_binding(&binding).unwrap();
                    }
                    i
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_binding_request_validation() {
        let config = ValidationConfig {
            validate_signatures: false,
            ..Default::default()
        };
        let validator = AccountBindingValidator::new(config);

        let sol_addr = generate_solana_keypair();
        let eth_addr = generate_ethereum_keypair();

        let source_account = AccountAddress::Solana(sol_addr);
        let target_account = AccountAddress::Ethereum(eth_addr);

        let proof = BindingProof {
            account: target_account.clone(),
            proof_type: ProofType::Signature {
                message: b"binding request".to_vec(),
                signature: vec![0u8; 65],
            },
            proof_data: vec![],
            timestamp: std::time::SystemTime::now(),
        };

        // Should pass - different VM types
        let result = validator.validate_binding_request(&source_account, &target_account, &proof);
        assert!(result.is_ok());

        // Should fail - same VM type
        let sol_addr2 = generate_solana_keypair();
        let target_same_vm = AccountAddress::Solana(sol_addr2);
        let result = validator.validate_binding_request(&source_account, &target_same_vm, &proof);
        assert!(result.is_err());
    }

    #[test]
    fn test_zero_address_validation() {
        let validator = create_test_validator();

        // Zero Solana address
        let zero_sol = AccountAddress::Solana(SolanaAddress([0u8; 32]));
        assert!(validator.validate_account_address(&zero_sol).is_err());

        // Zero Ethereum address
        let zero_eth = AccountAddress::Ethereum(EthereumAddress([0u8; 20]));
        assert!(validator.validate_account_address(&zero_eth).is_err());
    }

    #[test]
    fn test_proof_generator() {
        let sol_addr = generate_solana_keypair();
        let eth_addr = generate_ethereum_keypair();

        let source = AccountAddress::Solana(sol_addr);
        let target = AccountAddress::Ethereum(eth_addr);
        let timestamp = std::time::SystemTime::now();

        let message = ProofGenerator::generate_binding_message(&source, &target, timestamp);
        assert!(!message.is_empty());

        let proof = ProofGenerator::create_signature_proof_template(source.clone(), message);
        assert_eq!(proof.account, source);

        let tx_proof = ProofGenerator::create_transaction_proof_template(target.clone());
        assert_eq!(tx_proof.account, target);
    }
}

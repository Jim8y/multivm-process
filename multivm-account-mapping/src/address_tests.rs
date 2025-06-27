//! Tests for account mapping module

#[cfg(test)]
mod tests {
    use crate::address::*;
    use crate::error::*;

    #[test]
    fn test_solana_address_creation() {
        let bytes = [1u8; 32];
        let addr = SolanaAddress(bytes);
        assert_eq!(addr.0, bytes);
        assert_eq!(addr.as_ref(), &bytes);
    }

    #[test]
    fn test_ethereum_address_creation() {
        let bytes = [2u8; 20];
        let addr = EthereumAddress(bytes);
        assert_eq!(addr.0, bytes);
        assert_eq!(addr.as_ref(), &bytes);
    }

    #[test]
    fn test_multivm_account_id_creation() {
        let id = MultivmAccountId::generate();
        assert_eq!(id.0.len(), 32);
    }

    #[test]
    fn test_account_address_enum() {
        let sol_addr = AccountAddress::Solana(SolanaAddress([3u8; 32]));
        let eth_addr = AccountAddress::Ethereum(EthereumAddress([4u8; 20]));

        // Just test that they can be created
        match sol_addr {
            AccountAddress::Solana(_) => {} // Valid Solana address created
            _ => panic!("Expected Solana address"),
        }
        match eth_addr {
            AccountAddress::Ethereum(_) => {} // Valid Ethereum address created
            _ => panic!("Expected Ethereum address"),
        }
    }

    #[test]
    fn test_address_to_bytes() {
        let sol_addr = AccountAddress::Solana(SolanaAddress([5u8; 32]));
        let bytes = sol_addr.to_bytes();
        assert_eq!(bytes.len(), 32); // Just the address bytes
        assert_eq!(bytes[0], 5); // First byte of our test data

        let eth_addr = AccountAddress::Ethereum(EthereumAddress([6u8; 20]));
        let bytes = eth_addr.to_bytes();
        assert_eq!(bytes.len(), 20); // Just the address bytes
        assert_eq!(bytes[0], 6); // First byte of our test data
    }

    #[test]
    fn test_address_display() {
        let sol_addr = SolanaAddress([0u8; 32]);
        let sol_str = format!("{sol_addr}");
        assert!(!sol_str.is_empty());

        let eth_addr = EthereumAddress([0u8; 20]);
        let eth_str = format!("{eth_addr}");
        assert!(eth_str.starts_with("0x"));
        assert_eq!(eth_str.len(), 42); // 0x + 40 hex chars
    }

    #[test]
    fn test_multivm_id_from_address() {
        let sol_addr = AccountAddress::Solana(SolanaAddress([9u8; 32]));
        let id = MultivmAccountId::from_account(&sol_addr);
        assert_eq!(id.0.len(), 32);

        // Same address should produce same ID
        let id2 = MultivmAccountId::from_account(&sol_addr);
        assert_eq!(id.0, id2.0);
    }

    #[test]
    fn test_error_types() {
        let err = AccountMappingError::InvalidAddress {
            address: "test".to_string(),
        };

        match err {
            AccountMappingError::InvalidAddress { address } => {
                assert_eq!(address, "test");
            }
            _ => panic!("Wrong error type"),
        }

        let err2 = AccountMappingError::AccountNotFound {
            address: "0x123".to_string(),
        };
        assert!(err2.to_string().contains("not found"));
    }

    #[test]
    fn test_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let mapping_err: AccountMappingError = io_err.into();
        match mapping_err {
            AccountMappingError::Storage { message } => {
                assert!(message.contains("not found"));
            }
            _ => panic!("Expected Storage error"),
        }
    }
}

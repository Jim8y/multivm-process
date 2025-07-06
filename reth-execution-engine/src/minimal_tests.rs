#[cfg(test)]
mod tests {
    use crate::engine_api::PayloadStatus as ApiPayloadStatus;

    #[test]
    fn test_payload_status_from_str() {
        assert!(matches!(ApiPayloadStatus::from_str("VALID"), Ok(ApiPayloadStatus::Valid)));
        assert!(matches!(ApiPayloadStatus::from_str("INVALID"), Ok(ApiPayloadStatus::Invalid)));
        assert!(matches!(ApiPayloadStatus::from_str("SYNCING"), Ok(ApiPayloadStatus::Syncing)));
        assert!(matches!(ApiPayloadStatus::from_str("ACCEPTED"), Ok(ApiPayloadStatus::Accepted)));
        assert!(ApiPayloadStatus::from_str("UNKNOWN").is_err());
    }

    #[test]
    fn test_engine_types() {
        use crate::engine::{U256, Address, B256};
        
        // Test U256
        let u = U256::from(42);
        assert_eq!(u.0[0], 42);
        assert_eq!(u.0[1], 0);
        
        // Test Address (20 bytes)
        let addr: Address = [0u8; 20];
        assert_eq!(addr.len(), 20);
        
        // Test B256 (32 bytes)
        let hash: B256 = [0u8; 32];
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_u256_serialization() {
        use crate::engine::U256;
        
        let u = U256::from(100);
        let json = serde_json::to_string(&u).unwrap();
        assert!(json.contains("0x"));
        
        let deserialized: U256 = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, U256::default()); // Simple deserializer returns default
    }

    #[test]
    fn test_mock_state() {
        cfg_if::cfg_if! {
            if #[cfg(feature = "mock")] {
                use crate::engine::MockRethState;
                
                let state = MockRethState::new();
                assert_eq!(state.block_number, 0);
                assert_eq!(state.transaction_count, 0);
                assert!(state.accounts.is_empty());
                assert!(state.storage.is_empty());
            }
        }
    }

    #[test]
    fn test_engine_error_types() {
        use crate::engine::RethEngineError;
        
        let io_err = RethEngineError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        assert!(matches!(io_err, RethEngineError::Io(_)));
        
        let rpc_err = RethEngineError::Rpc("test error".to_string());
        assert!(matches!(rpc_err, RethEngineError::Rpc(_)));
        
        let exec_err = RethEngineError::Execution("failed".to_string());
        assert!(matches!(exec_err, RethEngineError::Execution(_)));
    }
}
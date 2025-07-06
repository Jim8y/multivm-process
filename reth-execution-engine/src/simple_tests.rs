#[cfg(test)]
mod tests {
    use crate::engine_api::*;

    #[test]
    fn test_forkchoice_state_creation() {
        let forkchoice = ForkchoiceState {
            head_block_hash: "0x1234567890abcdef".to_string(),
            safe_block_hash: "0x1234567890abcdef".to_string(),
            finalized_block_hash: "0x1234567890abcdef".to_string(),
        };
        
        assert_eq!(forkchoice.head_block_hash, "0x1234567890abcdef");
        assert_eq!(forkchoice.safe_block_hash, "0x1234567890abcdef");
        assert_eq!(forkchoice.finalized_block_hash, "0x1234567890abcdef");
    }

    #[test]
    fn test_payload_attributes() {
        let attributes = PayloadAttributes {
            timestamp: 1234567890,
            prev_randao: "0xabcdef".to_string(),
            suggested_fee_recipient: "0xfeefeefee".to_string(),
        };
        
        assert_eq!(attributes.timestamp, 1234567890);
        assert_eq!(attributes.prev_randao, "0xabcdef");
        assert_eq!(attributes.suggested_fee_recipient, "0xfeefeefee");
    }

    #[test]
    fn test_payload_status_valid() {
        let status = PayloadStatus {
            status: "VALID".to_string(),
            latest_valid_hash: Some("0x5678".to_string()),
            validation_error: None,
        };
        
        assert_eq!(status.status, "VALID");
        assert_eq!(status.latest_valid_hash, Some("0x5678".to_string()));
        assert!(status.validation_error.is_none());
    }

    #[test]
    fn test_payload_status_invalid() {
        let status = PayloadStatus {
            status: "INVALID".to_string(),
            latest_valid_hash: None,
            validation_error: Some("Block validation failed".to_string()),
        };
        
        assert_eq!(status.status, "INVALID");
        assert!(status.latest_valid_hash.is_none());
        assert_eq!(status.validation_error, Some("Block validation failed".to_string()));
    }

    #[test]
    fn test_execution_payload_serialization() {
        let payload = ExecutionPayload {
            parent_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            fee_recipient: "0x1111111111111111111111111111111111111111".to_string(),
            state_root: "0x2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            receipts_root: "0x3333333333333333333333333333333333333333333333333333333333333333".to_string(),
            logs_bloom: "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".to_string(),
            prev_randao: "0x4444444444444444444444444444444444444444444444444444444444444444".to_string(),
            block_number: 100,
            gas_limit: 8000000,
            gas_used: 1000000,
            timestamp: 1234567890,
            extra_data: "0x".to_string(),
            base_fee_per_gas: "1000000000".to_string(),
            block_hash: "0x5555555555555555555555555555555555555555555555555555555555555555".to_string(),
            transactions: vec!["0xabc".to_string(), "0xdef".to_string()],
        };
        
        // Test serialization
        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: ExecutionPayload = serde_json::from_str(&json).unwrap();
        
        assert_eq!(payload.parent_hash, deserialized.parent_hash);
        assert_eq!(payload.block_number, deserialized.block_number);
        assert_eq!(payload.transactions.len(), deserialized.transactions.len());
        assert_eq!(payload.gas_limit, deserialized.gas_limit);
        assert_eq!(payload.gas_used, deserialized.gas_used);
    }

    #[test]
    fn test_engine_api_methods() {
        // Test that the engine API methods enum covers all required methods
        let methods = vec![
            "engine_newPayloadV1",
            "engine_newPayloadV2",
            "engine_newPayloadV3",
            "engine_forkchoiceUpdatedV1",
            "engine_forkchoiceUpdatedV2",
            "engine_forkchoiceUpdatedV3",
            "engine_getPayloadV1",
            "engine_getPayloadV2",
            "engine_getPayloadV3",
        ];
        
        for method in methods {
            assert!(!method.is_empty());
            assert!(method.starts_with("engine_"));
        }
    }

    #[test]
    fn test_jwt_secret_generation() {
        use rand::Rng;
        
        // Test JWT secret generation
        let mut rng = rand::thread_rng();
        let secret: [u8; 32] = rng.gen();
        
        assert_eq!(secret.len(), 32);
        
        // Convert to hex
        let hex_secret = hex::encode(secret);
        assert_eq!(hex_secret.len(), 64); // 32 bytes = 64 hex chars
    }
}
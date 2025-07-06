#[cfg(test)]
mod tests {
    use multivm_common::{BlockchainType, ProcessId};

    #[test]
    fn test_process_id() {
        let solana_id = ProcessId::Solana;
        assert!(matches!(solana_id, ProcessId::Solana));
    }

    #[test]
    fn test_blockchain_type() {
        let solana_type = BlockchainType::Solana;
        assert!(matches!(solana_type, BlockchainType::Solana));
    }

    #[test]
    fn test_basic_math() {
        assert_eq!(1 + 1, 2);
        assert_eq!(1_000_000_000, 1_000_000_000); // 1 SOL in lamports
    }

    #[test]
    fn test_hex_encoding() {
        let data = vec![0xAB, 0xCD, 0xEF];
        let hex = hex::encode(&data);
        assert_eq!(hex, "abcdef");
    }

    #[test]
    fn test_base64_encoding() {
        let data = b"hello solana";
        let encoded = base64::encode(data);
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_json_creation() {
        use serde_json::json;

        let tx = json!({
            "slot": 12345,
            "blockhash": "11111111111111111111111111111111",
            "fee": 5000
        });

        assert_eq!(tx["slot"], 12345);
        assert_eq!(tx["fee"], 5000);
    }
}

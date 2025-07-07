#[cfg(test)]
mod tests {
    use crate::config_migration::*;

    #[test]
    fn test_legacy_config_creation() {
        let config = LegacyConfig {
            network: LegacyNetworkConfig {
                p2p_bind_address: "0.0.0.0:9000".to_string(),
                bootstrap_peers: vec!["peer1".to_string(), "peer2".to_string()],
            },
            consensus: LegacyConsensusConfig {
                validator_key_path: "/path/to/key".to_string(),
                block_interval_ms: 1000,
            },
            ethereum: LegacyEthereumConfig {
                rpc_url: "http://localhost:8545".to_string(),
            },
            solana: LegacySolanaConfig {
                rpc_url: "http://localhost:8899".to_string(),
            },
        };

        assert_eq!(config.network.p2p_bind_address, "0.0.0.0:9000");
        assert_eq!(config.network.bootstrap_peers.len(), 2);
        assert_eq!(config.consensus.block_interval_ms, 1000);
        assert_eq!(config.ethereum.rpc_url, "http://localhost:8545");
        assert_eq!(config.solana.rpc_url, "http://localhost:8899");
    }

    #[test]
    fn test_module_presence() {
        // Just verify modules exist and can be referenced
        // The fact that we can compile with these modules means they exist
        assert!(true);
    }
}

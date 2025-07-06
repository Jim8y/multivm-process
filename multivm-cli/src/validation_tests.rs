#[cfg(test)]
mod tests {
    use crate::validation::*;

    #[test]
    fn test_validate_rpc_port_valid() {
        assert_eq!(validate_rpc_port(8545).unwrap(), 8545);
        assert_eq!(validate_rpc_port(30303).unwrap(), 30303);
        assert_eq!(validate_rpc_port(65535).unwrap(), 65535);
        assert_eq!(validate_rpc_port(1024).unwrap(), 1024);
    }

    #[test]
    fn test_validate_rpc_port_invalid() {
        assert!(validate_rpc_port(0).is_err());
        // Note: ports 1-1023 are valid but print a warning
        assert!(validate_rpc_port(1023).is_ok()); // Privileged port - valid with warning
    }

    #[test]
    fn test_validate_p2p_address_valid() {
        assert_eq!(
            validate_p2p_address("127.0.0.1:9000").unwrap(),
            "127.0.0.1:9000"
        );
        assert_eq!(
            validate_p2p_address("0.0.0.0:30303").unwrap(),
            "0.0.0.0:30303"
        );
        assert_eq!(
            validate_p2p_address("192.168.1.1:8080").unwrap(),
            "192.168.1.1:8080"
        );
    }

    #[test]
    fn test_validate_p2p_address_invalid() {
        assert!(validate_p2p_address("invalid").is_err());
        assert!(validate_p2p_address("127.0.0.1").is_err()); // Missing port
        assert!(validate_p2p_address("127.0.0.1:0").is_err()); // Invalid port
        assert!(validate_p2p_address("256.256.256.256:9000").is_err()); // Invalid IP
        assert!(validate_p2p_address("localhost:9000").is_err()); // No hostname resolution
    }

    #[test]
    fn test_validate_data_dir_valid() {
        assert_eq!(validate_data_dir("/tmp/multivm").unwrap(), "/tmp/multivm");
        assert_eq!(validate_data_dir("./data").unwrap(), "./data");
        assert_eq!(
            validate_data_dir("/opt/multivm/data").unwrap(),
            "/opt/multivm/data"
        );
    }

    #[test]
    fn test_validate_data_dir_invalid() {
        assert!(validate_data_dir("").is_err());
        assert!(validate_data_dir(" ").is_err());
        assert!(validate_data_dir("\0invalid").is_err()); // Null character
    }

    #[test]
    fn test_validate_rpc_url_valid() {
        assert_eq!(
            validate_rpc_url("http://localhost:8545").unwrap(),
            "http://localhost:8545"
        );
        assert_eq!(
            validate_rpc_url("https://eth.example.com:8545").unwrap(),
            "https://eth.example.com:8545"
        );
        assert_eq!(
            validate_rpc_url("http://127.0.0.1:8899").unwrap(),
            "http://127.0.0.1:8899"
        );
        assert_eq!(
            validate_rpc_url("ws://localhost:8546").unwrap(),
            "ws://localhost:8546"
        );
    }

    #[test]
    fn test_validate_rpc_url_invalid() {
        assert!(validate_rpc_url("").is_err());
        assert!(validate_rpc_url("invalid-url").is_err());
        assert!(validate_rpc_url("ftp://localhost:8545").is_err()); // Wrong protocol
        assert!(validate_rpc_url("http://").is_err()); // Missing host
    }

    #[test]
    fn test_validate_validator_key_path_valid() {
        assert_eq!(
            validate_validator_key_path("/opt/multivm/keys/validator.key").unwrap(),
            "/opt/multivm/keys/validator.key"
        );
        assert_eq!(
            validate_validator_key_path("./validator.key").unwrap(),
            "./validator.key"
        );
    }

    #[test]
    fn test_validate_validator_key_path_invalid() {
        assert!(validate_validator_key_path("").is_err());
        assert!(validate_validator_key_path(" ").is_err());
    }

    #[test]
    fn test_validate_memory_limit_valid() {
        assert_eq!(validate_memory_limit(1024).unwrap(), 1024);
        assert_eq!(validate_memory_limit(8192).unwrap(), 8192);
        assert_eq!(validate_memory_limit(16384).unwrap(), 16384);
    }

    #[test]
    fn test_validate_memory_limit_invalid() {
        assert!(validate_memory_limit(0).is_err());
        assert!(validate_memory_limit(511).is_err()); // Below 512MB minimum
        assert!(validate_memory_limit(65536).is_err()); // Above 64GB maximum
    }

    #[test]
    fn test_validate_cpu_cores_valid() {
        assert_eq!(validate_cpu_cores(1).unwrap(), 1);
        assert_eq!(validate_cpu_cores(4).unwrap(), 4);
        assert_eq!(validate_cpu_cores(64).unwrap(), 64);
    }

    #[test]
    fn test_validate_cpu_cores_invalid() {
        assert!(validate_cpu_cores(0).is_err());
        assert!(validate_cpu_cores(65).is_err()); // Above 64 cores
    }

    #[test]
    fn test_validate_max_file_descriptors_valid() {
        assert_eq!(validate_max_file_descriptors(1024).unwrap(), 1024);
        assert_eq!(validate_max_file_descriptors(65536).unwrap(), 65536);
        assert_eq!(validate_max_file_descriptors(1048576).unwrap(), 1048576);
    }

    #[test]
    fn test_validate_max_file_descriptors_invalid() {
        assert!(validate_max_file_descriptors(0).is_err());
        assert!(validate_max_file_descriptors(1023).is_err()); // Below 1024 minimum
        assert!(validate_max_file_descriptors(1048577).is_err()); // Above maximum
    }
}

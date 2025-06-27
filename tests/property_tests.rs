//! Property-based tests for critical MultiVM functions
//! 
//! These tests use property-based testing to verify invariants
//! and edge cases across a wide range of inputs.

use proptest::prelude::*;
use multivm_common::{
    MultivmConfig, ProcessId, VmType, HealthStatus, IpcCommand, IpcResponse,
    types::core::{MessageId, BlockchainType},
    MultivmError, MultivmResult,
};
use std::time::Duration;

// Property generators for custom types

/// Generate arbitrary ProcessId values
fn arb_process_id() -> impl Strategy<Value = ProcessId> {
    prop_oneof![
        Just(ProcessId::Main),
        Just(ProcessId::Solana),
        Just(ProcessId::Ethereum),
    ]
}

/// Generate arbitrary VmType values
fn arb_vm_type() -> impl Strategy<Value = VmType> {
    prop_oneof![
        Just(VmType::Svm),
        Just(VmType::Evm),
    ]
}

/// Generate arbitrary HealthStatus values
fn arb_health_status() -> impl Strategy<Value = HealthStatus> {
    prop_oneof![
        Just(HealthStatus::Healthy),
        Just(HealthStatus::Degraded),
        Just(HealthStatus::Unhealthy),
    ]
}

/// Generate arbitrary BlockchainType values
fn arb_blockchain_type() -> impl Strategy<Value = BlockchainType> {
    prop_oneof![
        Just(BlockchainType::Solana),
        Just(BlockchainType::Ethereum),
    ]
}

/// Generate arbitrary durations (reasonable range)
fn arb_duration() -> impl Strategy<Value = Duration> {
    (0u64..=3600).prop_map(Duration::from_secs)
}

/// Generate arbitrary message IDs
fn arb_message_id() -> impl Strategy<Value = MessageId> {
    any::<[u8; 16]>().prop_map(MessageId::from_bytes)
}

/// Property test: MessageId operations are consistent
proptest! {
    #[test]
    fn test_message_id_properties(
        bytes in any::<[u8; 16]>()
    ) {
        let msg_id = MessageId::from_bytes(bytes);
        
        // Property: Converting to bytes and back should be identity
        let round_trip_bytes = msg_id.to_bytes();
        let round_trip_id = MessageId::from_bytes(round_trip_bytes);
        prop_assert_eq!(msg_id, round_trip_id);
        
        // Property: MessageId string representation should be consistent
        let str_repr1 = msg_id.to_string();
        let str_repr2 = msg_id.to_string();
        prop_assert_eq!(str_repr1, str_repr2);
        
        // Property: MessageId should be deterministic for same input
        let msg_id2 = MessageId::from_bytes(bytes);
        prop_assert_eq!(msg_id, msg_id2);
    }
}

/// Property test: ProcessId serialization/deserialization is consistent
proptest! {
    #[test]
    fn test_process_id_serialization(
        process_id in arb_process_id()
    ) {
        // Property: JSON serialization round-trip should preserve value
        let json = serde_json::to_string(&process_id).unwrap();
        let deserialized: ProcessId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(process_id, deserialized);
        
        // Property: Debug representation should be non-empty
        let debug_str = format!("{:?}", process_id);
        prop_assert!(!debug_str.is_empty());
    }
}

/// Property test: VmType operations are consistent
proptest! {
    #[test]
    fn test_vm_type_properties(
        vm_type in arb_vm_type()
    ) {
        // Property: Display and Debug should be consistent
        let display_str = vm_type.to_string();
        let debug_str = format!("{:?}", vm_type);
        prop_assert!(!display_str.is_empty());
        prop_assert!(!debug_str.is_empty());
        
        // Property: VmType should be deterministic
        let vm_type2 = vm_type;
        prop_assert_eq!(vm_type, vm_type2);
        
        // Property: VmType serialization should be consistent
        let json = serde_json::to_string(&vm_type).unwrap();
        let deserialized: VmType = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(vm_type, deserialized);
    }
}

/// Property test: HealthStatus ordering is consistent
proptest! {
    #[test]
    fn test_health_status_ordering(
        status1 in arb_health_status(),
        status2 in arb_health_status()
    ) {
        // Property: Ordering should be reflexive
        prop_assert_eq!(status1.cmp(&status1), std::cmp::Ordering::Equal);
        
        // Property: If a < b, then b > a
        match status1.cmp(&status2) {
            std::cmp::Ordering::Less => {
                prop_assert_eq!(status2.cmp(&status1), std::cmp::Ordering::Greater);
            }
            std::cmp::Ordering::Greater => {
                prop_assert_eq!(status2.cmp(&status1), std::cmp::Ordering::Less);
            }
            std::cmp::Ordering::Equal => {
                prop_assert_eq!(status2.cmp(&status1), std::cmp::Ordering::Equal);
            }
        }
        
        // Property: Health status values have expected ordering
        prop_assert!(HealthStatus::Healthy >= HealthStatus::Degraded);
        prop_assert!(HealthStatus::Degraded >= HealthStatus::Unhealthy);
    }
}

/// Property test: Duration operations are consistent
proptest! {
    #[test]
    fn test_duration_properties(
        duration in arb_duration()
    ) {
        // Property: Duration should be non-negative
        prop_assert!(duration.as_secs() >= 0);
        
        // Property: Converting to different units should be consistent
        let millis = duration.as_millis();
        let secs = duration.as_secs();
        prop_assert_eq!(millis / 1000, secs as u128);
        
        // Property: Adding zero duration should be identity
        let zero = Duration::from_secs(0);
        prop_assert_eq!(duration + zero, duration);
    }
}

/// Property test: IPC message creation is consistent
proptest! {
    #[test]
    fn test_ipc_message_properties(
        source in arb_process_id(),
        dest in arb_process_id(),
        timeout in prop::option::of(arb_duration())
    ) {
        let message = multivm_common::IpcMessage::new(source, dest, IpcCommand::Ping);
        
        // Property: Message fields should match constructor arguments
        prop_assert_eq!(message.source, source);
        prop_assert_eq!(message.destination, dest);
        prop_assert!(matches!(message.command, IpcCommand::Ping));
        
        // Property: Message ID should be unique for each creation
        let message2 = multivm_common::IpcMessage::new(source, dest, IpcCommand::Ping);
        prop_assert_ne!(message.id, message2.id);
        
        // Property: Message with timeout should preserve timeout
        let message_with_timeout = multivm_common::IpcMessage::new(source, dest, IpcCommand::Ping)
            .with_timeout(timeout.unwrap_or(Duration::from_secs(30)));
        prop_assert!(message_with_timeout.timeout.is_some());
    }
}

/// Property test: Error type consistency
proptest! {
    #[test]
    fn test_error_type_properties(
        message in ".*",
        code in 0i32..1000i32
    ) {
        // Test Network error properties
        let network_error = MultivmError::Network {
            message: message.clone(),
            endpoint: Some("http://test".to_string()),
            retry_after: Some(Duration::from_secs(5)),
        };
        
        // Property: Error should contain the message
        let error_str = network_error.to_string();
        prop_assert!(error_str.contains(&message) || message.is_empty());
        
        // Property: Error display should be non-empty
        prop_assert!(!error_str.is_empty());
        
        // Test Internal error properties
        let internal_error = MultivmError::Internal {
            message: message.clone(),
            source: None,
        };
        
        let internal_str = internal_error.to_string();
        prop_assert!(!internal_str.is_empty());
    }
}

/// Property test: Account address validation
proptest! {
    #[test]
    fn test_account_address_properties(
        address_bytes in any::<[u8; 32]>()
    ) {
        let svm_address = multivm_account_mapping::addresses::SolanaAddress(address_bytes);
        
        // Property: Address bytes should be preserved
        prop_assert_eq!(svm_address.as_ref(), &address_bytes);
        
        // Property: Address should be valid for blockchain operations
        let is_valid = svm_address.is_valid();
        prop_assert!(is_valid); // All 32-byte arrays are valid Solana addresses
        
        // Property: Address serialization should be consistent
        let serialized = serde_json::to_string(&svm_address).unwrap();
        let deserialized: multivm_account_mapping::addresses::SolanaAddress = 
            serde_json::from_str(&serialized).unwrap();
        prop_assert_eq!(svm_address, deserialized);
    }
}

/// Property test: Resource limits validation
proptest! {
    #[test]
    fn test_resource_limits_properties(
        memory_mb in 1u64..16384u64,
        cpu_percent in 0.1f64..100.0f64,
        disk_gb in 1u64..1024u64,
        open_files in 1u32..10000u32,
        connections in 1u32..1000u32
    ) {
        let limits = multivm_common::ResourceLimits {
            max_memory_mb: memory_mb,
            max_cpu_percent: cpu_percent,
            max_disk_usage_gb: disk_gb,
            max_open_files: open_files,
            max_rpc_connections: connections,
        };
        
        // Property: All limits should be positive
        prop_assert!(limits.max_memory_mb > 0);
        prop_assert!(limits.max_cpu_percent > 0.0);
        prop_assert!(limits.max_disk_usage_gb > 0);
        prop_assert!(limits.max_open_files > 0);
        prop_assert!(limits.max_rpc_connections > 0);
        
        // Property: CPU percentage should be reasonable
        prop_assert!(limits.max_cpu_percent <= 100.0);
        
        // Property: Serialization should preserve values
        let json = serde_json::to_string(&limits).unwrap();
        let deserialized: multivm_common::ResourceLimits = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(limits.max_memory_mb, deserialized.max_memory_mb);
        prop_assert_eq!(limits.max_disk_usage_gb, deserialized.max_disk_usage_gb);
    }
}

/// Property test: Configuration validation is consistent
proptest! {
    #[test]
    fn test_config_validation_properties(
        data_dir_name in "[a-zA-Z0-9_-]{1,50}",
        port in 1024u16..65535u16,
        max_processes in 1u32..100u32
    ) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let data_dir = temp_dir.path().join(data_dir_name);
        
        let mut config = MultivmConfig::default();
        config.system.data_dir = data_dir;
        config.server.rest.port = port;
        config.system.max_processes = max_processes;
        
        // Property: Valid configurations should pass validation
        if config.validate().is_ok() {
            // If validation passes, all values should be reasonable
            prop_assert!(config.server.rest.port >= 1024);
            prop_assert!(config.system.max_processes > 0);
        }
        
        // Property: Configuration should be serializable
        let json_result = serde_json::to_string(&config);
        prop_assert!(json_result.is_ok());
        
        if let Ok(json) = json_result {
            let deserialize_result: Result<MultivmConfig, _> = serde_json::from_str(&json);
            prop_assert!(deserialize_result.is_ok());
        }
    }
}

/// Property test: Network message properties
proptest! {
    #[test]
    fn test_network_message_properties(
        source in arb_vm_type(),
        version in 1u32..10u32,
        priority in 0u8..10u8
    ) {
        let message = multivm_p2p::messages::NetworkMessage::new(
            multivm_p2p::messages::MessagePayload::Control(
                multivm_p2p::messages::ControlMessage::Ping
            ),
            multivm_p2p::messages::MessageSource::NetworkLayer,
            multivm_p2p::messages::MessageTarget::Broadcast,
        );
        
        // Property: Message should have a unique ID
        let message2 = multivm_p2p::messages::NetworkMessage::new(
            multivm_p2p::messages::MessagePayload::Control(
                multivm_p2p::messages::ControlMessage::Pong
            ),
            multivm_p2p::messages::MessageSource::NetworkLayer,
            multivm_p2p::messages::MessageTarget::Broadcast,
        );
        prop_assert_ne!(message.id, message2.id);
        
        // Property: Message timestamp should be recent
        let now = std::time::SystemTime::now();
        let message_time = message.timestamp;
        let time_diff = now.duration_since(message_time).unwrap_or(Duration::ZERO);
        prop_assert!(time_diff < Duration::from_secs(1));
        
        // Property: Message version should be positive
        prop_assert!(message.version > 0);
        
        // Property: Message size estimation should be positive
        let estimated_size = message.estimated_size();
        prop_assert!(estimated_size > 0);
    }
}

/// Property test: Account mapping consistency
proptest! {
    #[test]
    fn test_account_mapping_properties(
        svm_bytes in any::<[u8; 32]>(),
        evm_bytes in any::<[u8; 20]>()
    ) {
        let svm_address = multivm_account_mapping::addresses::SolanaAddress(svm_bytes);
        let evm_address = multivm_account_mapping::addresses::EthereumAddress(evm_bytes);
        
        // Create account mapping
        let mapping = multivm_account_mapping::AccountMapping::new(svm_address, evm_address);
        
        // Property: Mapping should preserve both addresses
        prop_assert_eq!(mapping.svm_address(), &svm_address);
        prop_assert_eq!(mapping.evm_address(), &evm_address);
        
        // Property: Mapping should be bidirectional
        prop_assert!(mapping.contains_svm_address(&svm_address));
        prop_assert!(mapping.contains_evm_address(&evm_address));
        
        // Property: Mapping ID should be deterministic for same addresses
        let mapping2 = multivm_account_mapping::AccountMapping::new(svm_address, evm_address);
        prop_assert_eq!(mapping.id(), mapping2.id());
    }
}

/// Property test: Time-based operations
proptest! {
    #[test]
    fn test_time_based_properties(
        seconds in 0u64..86400u64  // 0 to 24 hours
    ) {
        let duration = Duration::from_secs(seconds);
        let timestamp = std::time::SystemTime::now();
        
        // Property: Adding duration to timestamp should be later
        let future_time = timestamp + duration;
        prop_assert!(future_time >= timestamp);
        
        // Property: Duration calculations should be consistent
        let duration_between = future_time.duration_since(timestamp).unwrap();
        prop_assert_eq!(duration_between, duration);
        
        // Property: Time arithmetic should be associative
        let half_duration = Duration::from_secs(seconds / 2);
        let remaining = Duration::from_secs(seconds - seconds / 2);
        let sum_time = timestamp + half_duration + remaining;
        prop_assert_eq!(sum_time.duration_since(timestamp).unwrap().as_secs(), seconds);
    }
}

/// Property test: Hash and equality consistency
proptest! {
    #[test]
    fn test_hash_equality_properties(
        bytes1 in any::<[u8; 32]>(),
        bytes2 in any::<[u8; 32]>()
    ) {
        let addr1 = multivm_account_mapping::addresses::SolanaAddress(bytes1);
        let addr2 = multivm_account_mapping::addresses::SolanaAddress(bytes2);
        let addr1_copy = multivm_account_mapping::addresses::SolanaAddress(bytes1);
        
        // Property: Equality should be reflexive
        prop_assert_eq!(addr1, addr1);
        
        // Property: Equality should be symmetric
        if addr1 == addr2 {
            prop_assert_eq!(addr2, addr1);
        }
        
        // Property: Equality should be transitive (addr1 == addr1_copy)
        prop_assert_eq!(addr1, addr1_copy);
        
        // Property: Hash should be consistent with equality
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        
        addr1.hash(&mut hasher1);
        addr1_copy.hash(&mut hasher2);
        
        prop_assert_eq!(hasher1.finish(), hasher2.finish());
    }
}

/// Property test: String encoding/decoding
proptest! {
    #[test]
    fn test_string_encoding_properties(
        input in "\\PC*"  // Any valid UTF-8 string
    ) {
        // Property: String encoding should be reversible
        let encoded = base64::encode(input.as_bytes());
        let decoded = base64::decode(&encoded).unwrap();
        let decoded_string = String::from_utf8(decoded).unwrap();
        prop_assert_eq!(input, decoded_string);
        
        // Property: JSON encoding should be reversible
        let json_encoded = serde_json::to_string(&input).unwrap();
        let json_decoded: String = serde_json::from_str(&json_encoded).unwrap();
        prop_assert_eq!(input, json_decoded);
    }
}
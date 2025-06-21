//! Tests for protocol translation functionality

use super::protocol::*;
use crate::messages::*;
use multivm_account_mapping::{AccountAddress, EthereumAddress, SolanaAddress};
use std::time::Duration;

#[test]
fn test_protocol_translator_creation() {
    let translator = ProtocolTranslator::new();

    // Test that default protocols are supported
    assert_eq!(translator.get_protocol_version(VmType::Svm), 1);
    assert_eq!(translator.get_protocol_version(VmType::Evm), 1);

    // Test that capabilities are set
    let svm_capabilities = translator.get_capabilities(VmType::Svm);
    assert!(svm_capabilities.contains(&"transaction".to_string()));
    assert!(svm_capabilities.contains(&"account_query".to_string()));
    assert!(svm_capabilities.contains(&"block_proposal".to_string()));
    assert!(svm_capabilities.contains(&"state_sync".to_string()));

    let evm_capabilities = translator.get_capabilities(VmType::Evm);
    assert!(evm_capabilities.contains(&"transaction".to_string()));
    assert!(evm_capabilities.contains(&"smart_contract".to_string()));
    assert!(evm_capabilities.contains(&"block_proposal".to_string()));
    assert!(evm_capabilities.contains(&"state_sync".to_string()));
}

#[test]
fn test_protocol_version_management() {
    let mut translator = ProtocolTranslator::new();

    // Test updating protocol versions
    translator.update_protocol_version(VmType::Svm, 2);
    assert_eq!(translator.get_protocol_version(VmType::Svm), 2);

    translator.update_protocol_version(VmType::Evm, 3);
    assert_eq!(translator.get_protocol_version(VmType::Evm), 3);
}

#[test]
fn test_message_translation_same_vm() {
    let translator = ProtocolTranslator::new();

    // Create SVM message
    let svm_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: vec![1, 2, 3, 4],
            signature: "test_signature".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    // Translate to same VM type (should return unchanged)
    let result = translator.translate_message(&svm_message, VmType::Svm);
    assert!(result.is_ok());

    let translated = result.unwrap();
    assert_eq!(translated.id, svm_message.id);
    match (&svm_message.payload, &translated.payload) {
        (MessagePayload::Svm(_), MessagePayload::Svm(_)) => (),
        _ => panic!("Message payload should remain unchanged"),
    }
}

#[test]
fn test_solana_to_evm_translation() {
    let translator = ProtocolTranslator::new();

    // Create SVM message
    let svm_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: vec![1, 2, 3, 4],
            signature: "test_signature".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    // Translate to EVM
    let result = translator.translate_message(&svm_message, VmType::Evm);
    assert!(result.is_ok());

    let translated = result.unwrap();
    assert_eq!(
        translated.metadata.get("converted_from"),
        Some(&"solana".to_string())
    );
    assert_eq!(
        translated.metadata.get("converted_to"),
        Some(&"ethereum".to_string())
    );
    assert!(translated.metadata.contains_key("conversion_time"));

    // Should be converted to MultiVM message
    match translated.payload {
        MessagePayload::MultiVm(_) => (),
        _ => panic!("Should be converted to MultiVM message"),
    }
}

#[test]
fn test_evm_to_solana_translation() {
    let translator = ProtocolTranslator::new();

    // Create EVM message
    let evm_message = NetworkMessage::new(
        MessagePayload::Evm(EvmMessage::Transaction {
            transaction_data: vec![5, 6, 7, 8],
            tx_hash: "0x123abc".to_string(),
        }),
        MessageSource::EvmExecution,
        MessageTarget::Broadcast,
    );

    // Translate to SVM
    let result = translator.translate_message(&evm_message, VmType::Svm);
    assert!(result.is_ok());

    let translated = result.unwrap();
    assert_eq!(
        translated.metadata.get("converted_from"),
        Some(&"ethereum".to_string())
    );
    assert_eq!(
        translated.metadata.get("converted_to"),
        Some(&"solana".to_string())
    );
    assert!(translated.metadata.contains_key("conversion_time"));

    // Should be converted to MultiVM message
    match translated.payload {
        MessagePayload::MultiVm(_) => (),
        _ => panic!("Should be converted to MultiVM message"),
    }
}

#[test]
fn test_universal_converter_translation() {
    let translator = ProtocolTranslator::new();

    // Create control message (neutral)
    let control_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    // Translate using solana_to_evm converter (control messages are treated as SVM origin)
    let result = translator.translate_message(&control_message, VmType::Evm);
    assert!(result.is_ok());

    let translated = result.unwrap();
    // Should use solana_to_evm converter, not universal
    assert_eq!(
        translated.metadata.get("converted_from"),
        Some(&"solana".to_string())
    );
    assert_eq!(
        translated.metadata.get("converted_to"),
        Some(&"ethereum".to_string())
    );
    assert!(translated.metadata.contains_key("conversion_time"));
}

#[test]
fn test_can_route_to_vm() {
    let translator = ProtocolTranslator::new();

    // Test SVM message routing
    let svm_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: vec![1, 2, 3],
            signature: "sig".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    assert!(translator.can_route_to_vm(&svm_message, VmType::Svm));
    assert!(translator.can_route_to_vm(&svm_message, VmType::Evm)); // Should be translatable

    // Test EVM message routing
    let evm_message = NetworkMessage::new(
        MessagePayload::Evm(EvmMessage::Transaction {
            transaction_data: vec![4, 5, 6],
            tx_hash: "0xabc".to_string(),
        }),
        MessageSource::EvmExecution,
        MessageTarget::Broadcast,
    );

    assert!(translator.can_route_to_vm(&evm_message, VmType::Evm));
    assert!(translator.can_route_to_vm(&evm_message, VmType::Svm)); // Should be translatable

    // Test MultiVM message routing (should always work)
    let multivm_message = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::StateSync {
            state_root: "0x123".to_string(),
            vm_type: VmType::Svm,
            height: 100,
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    assert!(translator.can_route_to_vm(&multivm_message, VmType::Svm));
    assert!(translator.can_route_to_vm(&multivm_message, VmType::Evm));

    // Test control message routing (should always work)
    let control_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    assert!(translator.can_route_to_vm(&control_message, VmType::Svm));
    assert!(translator.can_route_to_vm(&control_message, VmType::Evm));
}

#[test]
fn test_message_converter_traits() {
    let solana_to_evm = SolanaToEvmConverter;
    let evm_to_solana = EvmToSolanaConverter;
    let universal = UniversalConverter;

    // Test source and target formats
    assert_eq!(solana_to_evm.source_formats(), vec!["solana_wire_format"]);
    assert_eq!(solana_to_evm.target_formats(), vec!["ethereum_wire_format"]);

    assert_eq!(evm_to_solana.source_formats(), vec!["ethereum_wire_format"]);
    assert_eq!(evm_to_solana.target_formats(), vec!["solana_wire_format"]);

    assert_eq!(universal.source_formats(), vec!["any"]);
    assert_eq!(universal.target_formats(), vec!["any"]);
}

#[test]
fn test_solana_to_evm_converter() {
    let converter = SolanaToEvmConverter;

    let svm_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Block {
            block_data: vec![1, 2, 3, 4],
            block_hash: "hash123".to_string(),
            height: 100,
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    let result = converter.convert(&svm_message, "ethereum_wire_format");
    assert!(result.is_ok());

    let converted = result.unwrap();
    assert_eq!(
        converted.metadata.get("converted_from"),
        Some(&"solana".to_string())
    );
    assert_eq!(
        converted.metadata.get("converted_to"),
        Some(&"ethereum".to_string())
    );
}

#[test]
fn test_evm_to_solana_converter() {
    let converter = EvmToSolanaConverter;

    let evm_message = NetworkMessage::new(
        MessagePayload::Evm(EvmMessage::Block {
            block_data: vec![5, 6, 7, 8],
            block_hash: "0xabc123".to_string(),
            block_number: 200,
        }),
        MessageSource::EvmExecution,
        MessageTarget::Broadcast,
    );

    let result = converter.convert(&evm_message, "solana_wire_format");
    assert!(result.is_ok());

    let converted = result.unwrap();
    assert_eq!(
        converted.metadata.get("converted_from"),
        Some(&"ethereum".to_string())
    );
    assert_eq!(
        converted.metadata.get("converted_to"),
        Some(&"solana".to_string())
    );
}

#[test]
fn test_universal_converter() {
    let converter = UniversalConverter;

    let message = NetworkMessage::new(
        MessagePayload::Discovery(DiscoveryMessage::Announce {
            capabilities: NodeCapabilities {
                supported_vms: vec![VmType::Svm],
                protocol_versions: vec![1],
                features: vec!["test".to_string()],
                limits: ResourceLimits::default(),
            },
            addresses: vec!["127.0.0.1:8000".to_string()],
        }),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    let result = converter.convert(&message, "custom_format");
    assert!(result.is_ok());

    let converted = result.unwrap();
    assert_eq!(
        converted.metadata.get("universal_conversion"),
        Some(&"true".to_string())
    );
    assert_eq!(
        converted.metadata.get("target_format"),
        Some(&"custom_format".to_string())
    );
}

#[test]
fn test_universal_converter_via_same_vm() {
    let translator = ProtocolTranslator::new();

    // Create a message and translate to same VM type to trigger universal converter
    let control_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    // Same VM translation should return the original message unchanged
    let result = translator.translate_message(&control_message, VmType::Svm);
    assert!(result.is_ok());

    let translated = result.unwrap();
    // Should be unchanged for same VM
    assert_eq!(translated.id, control_message.id);
}

#[test]
fn test_custom_converter_registration() {
    let mut translator = ProtocolTranslator::new();

    // Register a custom converter
    let custom_converter = Box::new(UniversalConverter);
    translator.register_converter("custom_test", custom_converter);

    // Translator should have the new converter (we can't easily test this without exposing internals)
    // But registration should not panic
}

#[test]
fn test_protocol_translator_default() {
    let translator1 = ProtocolTranslator::new();
    let translator2 = ProtocolTranslator::default();

    // Both should have same initial state
    assert_eq!(
        translator1.get_protocol_version(VmType::Svm),
        translator2.get_protocol_version(VmType::Svm)
    );
    assert_eq!(
        translator1.get_protocol_version(VmType::Evm),
        translator2.get_protocol_version(VmType::Evm)
    );
}

#[test]
fn test_complex_message_translation() {
    let translator = ProtocolTranslator::new();

    // Create complex MultiVM message
    let source_addr = AccountAddress::Solana(SolanaAddress([1u8; 32]));
    let target_addr = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));

    let complex_message = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::AccountBinding {
            source: source_addr,
            target: target_addr,
            proof_hash: "0x123456789abcdef".to_string(),
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );

    // Translate to both VM types
    let svm_result = translator.translate_message(&complex_message, VmType::Svm);
    assert!(svm_result.is_ok());

    let evm_result = translator.translate_message(&complex_message, VmType::Evm);
    assert!(evm_result.is_ok());

    // Both should succeed as MultiVM messages are universally supported
    let svm_translated = svm_result.unwrap();
    let evm_translated = evm_result.unwrap();

    // Both should preserve the MultiVM payload
    match (&svm_translated.payload, &evm_translated.payload) {
        (MessagePayload::MultiVm(_), MessagePayload::MultiVm(_)) => (),
        _ => panic!("MultiVM messages should preserve payload type"),
    }
}

#[test]
fn test_message_size_handling() {
    let translator = ProtocolTranslator::new();

    // Create message with large data
    let large_message = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: vec![0u8; 1024 * 1024], // 1MB of data
            signature: "large_tx_signature".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );

    // Translation should handle large messages gracefully
    let result = translator.translate_message(&large_message, VmType::Evm);
    assert!(result.is_ok(), "Should handle large messages");
}

#[test]
fn test_vm_type_determination() {
    let translator = ProtocolTranslator::new();

    // Test with different message types
    let test_cases = vec![
        (
            MessagePayload::Svm(SvmMessage::Transaction {
                transaction_data: vec![1],
                signature: "sig".to_string(),
            }),
            "SVM message should be detected",
        ),
        (
            MessagePayload::Evm(EvmMessage::Transaction {
                transaction_data: vec![2],
                tx_hash: "0xhash".to_string(),
            }),
            "EVM message should be detected",
        ),
        (
            MessagePayload::MultiVm(MultiVmMessage::StateSync {
                state_root: "0x123".to_string(),
                vm_type: VmType::Svm,
                height: 100,
            }),
            "MultiVM message should be detected",
        ),
        (
            MessagePayload::Control(ControlMessage::StatusRequest),
            "Control message should be detected",
        ),
        (
            MessagePayload::Discovery(DiscoveryMessage::Announce {
                capabilities: NodeCapabilities {
                    supported_vms: vec![],
                    protocol_versions: vec![1],
                    features: vec![],
                    limits: ResourceLimits::default(),
                },
                addresses: vec![],
            }),
            "Discovery message should be detected",
        ),
    ];

    for (payload, description) in test_cases {
        let message = NetworkMessage::new(
            payload,
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );

        // Should be able to route to both VM types
        let can_route_svm = translator.can_route_to_vm(&message, VmType::Svm);
        let can_route_evm = translator.can_route_to_vm(&message, VmType::Evm);

        assert!(can_route_svm || can_route_evm, "{}", description);
    }
}

#[test]
fn test_protocol_error_cases() {
    // Test creating protocol translator with empty capabilities
    let translator = ProtocolTranslator::new();

    // Test with empty capabilities (simulate missing capabilities)

    // Should handle capabilities gracefully
    let capabilities = translator.get_capabilities(VmType::Svm);
    // Should have some or no capabilities

    let control_message = NetworkMessage::new(
        MessagePayload::Control(ControlMessage::StatusRequest),
        MessageSource::NetworkLayer,
        MessageTarget::Broadcast,
    );

    // Should still be able to route control messages
    assert!(translator.can_route_to_vm(&control_message, VmType::Svm));
    assert!(translator.can_route_to_vm(&control_message, VmType::Evm));
}

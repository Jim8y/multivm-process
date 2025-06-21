//! Protocol translation for cross-VM communication

use crate::messages::{NetworkMessage, MessagePayload, VmType};
use crate::error::P2PError;
use std::collections::HashMap;
use tracing::{debug, info};

/// Protocol translator handles message format translation between different VMs
pub struct ProtocolTranslator {
    /// Protocol version mappings
    version_mappings: HashMap<VmType, u32>,
    /// Message format converters
    format_converters: HashMap<String, Box<dyn MessageConverter>>,
    /// Protocol capabilities
    capabilities: HashMap<VmType, Vec<String>>,
}

/// Trait for converting message formats between protocols
pub trait MessageConverter: Send + Sync {
    /// Convert a message from one format to another
    fn convert(&self, message: &NetworkMessage, target_format: &str) -> Result<NetworkMessage, P2PError>;
    
    /// Get supported source formats
    fn source_formats(&self) -> Vec<String>;
    
    /// Get supported target formats  
    fn target_formats(&self) -> Vec<String>;
}

/// Solana to EVM message converter
pub struct SolanaToEvmConverter;

/// EVM to Solana message converter
pub struct EvmToSolanaConverter;

/// Universal message converter for cross-VM communication
pub struct UniversalConverter;

impl ProtocolTranslator {
    /// Create a new protocol translator with standard converters
    pub fn new() -> Self {
        let mut translator = Self {
            version_mappings: HashMap::new(),
            format_converters: HashMap::new(),
            capabilities: HashMap::new(),
        };
        
        // Initialize standard protocol versions
        translator.version_mappings.insert(VmType::Svm, 1);
        translator.version_mappings.insert(VmType::Evm, 1);
        
        // Register standard converters
        translator.register_converter("solana_to_evm", Box::new(SolanaToEvmConverter));
        translator.register_converter("evm_to_solana", Box::new(EvmToSolanaConverter));
        translator.register_converter("universal", Box::new(UniversalConverter));
        
        // Initialize capabilities
        translator.capabilities.insert(VmType::Svm, vec![
            "transaction".to_string(),
            "account_query".to_string(),
            "block_proposal".to_string(),
            "state_sync".to_string(),
        ]);
        
        translator.capabilities.insert(VmType::Evm, vec![
            "transaction".to_string(),
            "smart_contract".to_string(),
            "block_proposal".to_string(),
            "state_sync".to_string(),
        ]);
        
        translator
    }
    
    /// Register a new message converter
    pub fn register_converter(&mut self, name: &str, converter: Box<dyn MessageConverter>) {
        self.format_converters.insert(name.to_string(), converter);
        info!("Registered protocol converter: {}", name);
    }
    
    /// Translate a message for the target VM type
    pub fn translate_message(&self, message: &NetworkMessage, target_vm: VmType) -> Result<NetworkMessage, P2PError> {
        debug!("Translating message for target VM: {:?}", target_vm);
        
        // Determine the source VM type
        let source_vm = self.determine_source_vm(&message.payload)?;
        
        if source_vm == target_vm {
            // No translation needed
            return Ok(message.clone());
        }
        
        // Select appropriate converter
        let converter_name = match (source_vm, &target_vm) {
            (VmType::Svm, VmType::Evm) => "solana_to_evm",
            (VmType::Evm, VmType::Svm) => "evm_to_solana",
            _ => "universal",
        };
        
        if let Some(converter) = self.format_converters.get(converter_name) {
            let target_format = self.get_vm_message_format(target_vm);
            converter.convert(message, &target_format)
        } else {
            Err(P2PError::ProtocolTranslation(format!(
                "No converter available for {} to {:?}", 
                converter_name, target_vm
            )))
        }
    }
    
    /// Check if a message can be routed to the target VM
    pub fn can_route_to_vm(&self, message: &NetworkMessage, target_vm: VmType) -> bool {
        match self.determine_source_vm(&message.payload) {
            Ok(_source_vm) => {
                // Check if we have capabilities for this operation
                if let Some(capabilities) = self.capabilities.get(&target_vm) {
                    match &message.payload {
                        MessagePayload::Svm(_) => capabilities.contains(&"transaction".to_string()),
                        MessagePayload::Evm(_) => capabilities.contains(&"smart_contract".to_string()),
                        MessagePayload::MultiVm(_) => true, // MultiVM messages are always supported
                        MessagePayload::Control(_) => true, // Control messages are always supported
                        MessagePayload::Discovery(_) => true, // Discovery messages are always supported
                    }
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }
    
    /// Get protocol version for a VM type
    pub fn get_protocol_version(&self, vm_type: VmType) -> u32 {
        self.version_mappings.get(&vm_type).copied().unwrap_or(1)
    }
    
    /// Update protocol version for a VM type
    pub fn update_protocol_version(&mut self, vm_type: VmType, version: u32) {
        info!("Updated protocol version for {:?} to {}", vm_type, version);
        self.version_mappings.insert(vm_type, version);
    }
    
    /// Get capabilities for a VM type
    pub fn get_capabilities(&self, vm_type: VmType) -> Vec<String> {
        self.capabilities.get(&vm_type).cloned().unwrap_or_default()
    }
    
    /// Determine the source VM type from message payload
    fn determine_source_vm(&self, payload: &MessagePayload) -> Result<VmType, P2PError> {
        match payload {
            MessagePayload::Svm(_) => Ok(VmType::Svm),
            MessagePayload::Evm(_) => Ok(VmType::Evm),
            MessagePayload::MultiVm(_) => Ok(VmType::Svm), // Default to SVM for MultiVM messages
            MessagePayload::Control(_) | MessagePayload::Discovery(_) => Ok(VmType::Svm), // Neutral messages
        }
    }
    
    /// Get message format string for a VM type
    fn get_vm_message_format(&self, vm_type: VmType) -> String {
        match vm_type {
            VmType::Svm => "solana_wire_format".to_string(),
            VmType::Evm => "ethereum_wire_format".to_string(),
        }
    }
}

impl MessageConverter for SolanaToEvmConverter {
    fn convert(&self, message: &NetworkMessage, target_format: &str) -> Result<NetworkMessage, P2PError> {
        debug!("Converting Solana message to EVM format: {}", target_format);
        
        let mut converted_message = message.clone();
        
        // Update message metadata to indicate conversion
        converted_message.metadata.insert("converted_from".to_string(), "solana".to_string());
        converted_message.metadata.insert("converted_to".to_string(), "ethereum".to_string());
        converted_message.metadata.insert("conversion_time".to_string(), chrono::Utc::now().to_rfc3339());
        
        // Convert payload if needed
        converted_message.payload = self.convert_payload(&message.payload)?;
        
        Ok(converted_message)
    }
    
    fn source_formats(&self) -> Vec<String> {
        vec!["solana_wire_format".to_string()]
    }
    
    fn target_formats(&self) -> Vec<String> {
        vec!["ethereum_wire_format".to_string()]
    }
}

impl SolanaToEvmConverter {
    fn convert_payload(&self, payload: &MessagePayload) -> Result<MessagePayload, P2PError> {
        match payload {
            MessagePayload::Svm(_svm_msg) => {
                // Convert SVM message to equivalent EVM representation
                debug!("Converting SVM message to EVM equivalent");
                // For now, wrap in MultiVM message
                Ok(MessagePayload::MultiVm(crate::messages::MultiVmMessage::StateSync {
                    state_root: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                    vm_type: VmType::Svm,
                    height: 0,
                }))
            }
            other => Ok(other.clone()), // Pass through other message types
        }
    }
}

impl MessageConverter for EvmToSolanaConverter {
    fn convert(&self, message: &NetworkMessage, target_format: &str) -> Result<NetworkMessage, P2PError> {
        debug!("Converting EVM message to Solana format: {}", target_format);
        
        let mut converted_message = message.clone();
        
        // Update message metadata
        converted_message.metadata.insert("converted_from".to_string(), "ethereum".to_string());
        converted_message.metadata.insert("converted_to".to_string(), "solana".to_string());
        converted_message.metadata.insert("conversion_time".to_string(), chrono::Utc::now().to_rfc3339());
        
        // Convert payload if needed
        converted_message.payload = self.convert_payload(&message.payload)?;
        
        Ok(converted_message)
    }
    
    fn source_formats(&self) -> Vec<String> {
        vec!["ethereum_wire_format".to_string()]
    }
    
    fn target_formats(&self) -> Vec<String> {
        vec!["solana_wire_format".to_string()]
    }
}

impl EvmToSolanaConverter {
    fn convert_payload(&self, payload: &MessagePayload) -> Result<MessagePayload, P2PError> {
        match payload {
            MessagePayload::Evm(_evm_msg) => {
                // Convert EVM message to equivalent SVM representation
                debug!("Converting EVM message to SVM equivalent");
                // For now, wrap in MultiVM message
                Ok(MessagePayload::MultiVm(crate::messages::MultiVmMessage::StateSync {
                    state_root: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                    vm_type: VmType::Evm,
                    height: 0,
                }))
            }
            other => Ok(other.clone()), // Pass through other message types
        }
    }
}

impl MessageConverter for UniversalConverter {
    fn convert(&self, message: &NetworkMessage, target_format: &str) -> Result<NetworkMessage, P2PError> {
        debug!("Universal conversion to format: {}", target_format);
        
        let mut converted_message = message.clone();
        
        // Add universal conversion metadata
        converted_message.metadata.insert("universal_conversion".to_string(), "true".to_string());
        converted_message.metadata.insert("target_format".to_string(), target_format.to_string());
        converted_message.metadata.insert("conversion_time".to_string(), chrono::Utc::now().to_rfc3339());
        
        Ok(converted_message)
    }
    
    fn source_formats(&self) -> Vec<String> {
        vec!["any".to_string()]
    }
    
    fn target_formats(&self) -> Vec<String> {
        vec!["any".to_string()]
    }
}

impl Default for ProtocolTranslator {
    fn default() -> Self {
        Self::new()
    }
}

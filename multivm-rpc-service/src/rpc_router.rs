//! RPC request routing and method translation

use crate::{
    error::{RpcError, RpcResult},
    types::{JsonRpcRequest, JsonRpcResponse, VmType},
};
use std::collections::HashMap;
use tracing::debug;

/// RPC method router
pub struct RpcRouter {
    /// Method routing rules
    routing_rules: HashMap<String, RoutingRule>,
    /// Method aliases
    aliases: HashMap<String, String>,
    /// VM-specific method prefixes
    vm_prefixes: HashMap<VmType, Vec<String>>,
}

/// Routing rule for RPC methods
#[derive(Debug, Clone)]
pub struct RoutingRule {
    /// Target VM type
    pub target_vm: VmType,
    /// Method transformation
    pub transformation: Option<MethodTransformation>,
    /// Whether method requires special handling
    pub special_handling: bool,
    /// Caching policy
    pub cacheable: bool,
}

/// Method transformation rules
#[derive(Debug, Clone)]
pub enum MethodTransformation {
    /// No transformation needed
    None,
    /// Rename method
    Rename(String),
    /// Parameter transformation
    TransformParams(ParamTransformation),
    /// Response transformation
    TransformResponse(ResponseTransformation),
    /// Both parameter and response transformation
    Full {
        params: ParamTransformation,
        response: ResponseTransformation,
    },
}

/// Parameter transformation rules
#[derive(Debug, Clone)]
pub enum ParamTransformation {
    /// Convert Ethereum block numbers to Solana slots
    EthBlockToSolSlot,
    /// Convert Solana slots to Ethereum block numbers
    SolSlotToEthBlock,
    /// Convert address formats
    AddressFormat,
    /// Custom parameter mapping
    Custom(String),
}

/// Response transformation rules
#[derive(Debug, Clone)]
pub enum ResponseTransformation {
    /// Convert Ethereum response to standard format
    EthereumToStandard,
    /// Convert Solana response to standard format
    SolanaToStandard,
    /// Custom response mapping
    Custom(String),
}

impl RpcRouter {
    pub fn new() -> Self {
        let mut router = Self {
            routing_rules: HashMap::new(),
            aliases: HashMap::new(),
            vm_prefixes: HashMap::new(),
        };

        router.init_default_rules();
        router
    }

    /// Initialize default routing rules
    fn init_default_rules(&mut self) {
        // Initialize VM prefixes
        self.vm_prefixes.insert(
            VmType::Ethereum,
            vec![
                "eth_".to_string(),
                "web3_".to_string(),
                "net_".to_string(),
                "debug_".to_string(),
                "trace_".to_string(),
            ],
        );

        self.vm_prefixes.insert(
            VmType::Solana,
            vec![
                "get".to_string(),
                "send".to_string(),
                "simulate".to_string(),
            ],
        );

        self.vm_prefixes.insert(
            VmType::MultiVm,
            vec!["multivm_".to_string()],
        );

        // Ethereum methods
        self.add_ethereum_rules();
        
        // Solana methods
        self.add_solana_rules();
        
        // MultiVM methods
        self.add_multivm_rules();
        
        // Cross-VM aliases
        self.add_cross_vm_aliases();
    }

    /// Add Ethereum routing rules
    fn add_ethereum_rules(&mut self) {
        let ethereum_methods = vec![
            "eth_accounts", "eth_blockNumber", "eth_call", "eth_chainId",
            "eth_estimateGas", "eth_gasPrice", "eth_getBalance", "eth_getBlockByHash",
            "eth_getBlockByNumber", "eth_getBlockTransactionCountByHash",
            "eth_getBlockTransactionCountByNumber", "eth_getCode", "eth_getStorageAt",
            "eth_getTransactionByHash", "eth_getTransactionByBlockHashAndIndex",
            "eth_getTransactionByBlockNumberAndIndex", "eth_getTransactionCount",
            "eth_getTransactionReceipt", "eth_getLogs", "eth_sendRawTransaction",
            "eth_sendTransaction", "eth_sign", "eth_signTransaction",
            "web3_clientVersion", "web3_sha3", "net_listening", "net_peerCount", "net_version"
        ];

        for method in ethereum_methods {
            self.routing_rules.insert(
                method.to_string(),
                RoutingRule {
                    target_vm: VmType::Ethereum,
                    transformation: Some(MethodTransformation::None),
                    special_handling: matches!(method, "eth_sendTransaction" | "eth_sendRawTransaction"),
                    cacheable: !matches!(method, "eth_sendTransaction" | "eth_sendRawTransaction" | "eth_sign"),
                },
            );
        }
    }

    /// Add Solana routing rules
    fn add_solana_rules(&mut self) {
        let solana_methods = vec![
            "getAccountInfo", "getBalance", "getBlock", "getBlockHeight", "getBlockTime",
            "getBlocks", "getBlocksWithLimit", "getClusterNodes", "getEpochInfo",
            "getEpochSchedule", "getFeeCalculatorForBlockhash", "getFees",
            "getFirstAvailableBlock", "getGenesisHash", "getHealth", "getIdentity",
            "getInflationGovernor", "getInflationRate", "getInflationReward",
            "getLargestAccounts", "getLatestBlockhash", "getLeaderSchedule",
            "getMaxRetransmitSlot", "getMaxShredInsertSlot", "getMinimumBalanceForRentExemption",
            "getMultipleAccounts", "getProgramAccounts", "getRecentBlockhash",
            "getSignatureStatuses", "getSignaturesForAddress", "getSlot", "getSlotLeader",
            "getSlotLeaders", "getStakeActivation", "getSupply", "getTokenAccountBalance",
            "getTokenAccountsByDelegate", "getTokenAccountsByOwner", "getTokenLargestAccounts",
            "getTokenSupply", "getTotalSupply", "getTransaction", "getTransactionCount",
            "getVersion", "getVoteAccounts", "sendTransaction", "simulateTransaction"
        ];

        for method in solana_methods {
            self.routing_rules.insert(
                method.to_string(),
                RoutingRule {
                    target_vm: VmType::Solana,
                    transformation: Some(MethodTransformation::None),
                    special_handling: matches!(method, "sendTransaction" | "simulateTransaction"),
                    cacheable: !matches!(method, "sendTransaction" | "simulateTransaction"),
                },
            );
        }
    }

    /// Add MultiVM routing rules
    fn add_multivm_rules(&mut self) {
        let multivm_methods = vec![
            "multivm_getVersion", "multivm_getNetworkStats", "multivm_getCrossChainBalance",
            "multivm_getAccountBinding", "multivm_resolveMutliVmAccount",
            "multivm_submitCrossVmTransaction", "multivm_getTransactionStatus",
            "multivm_getSupportedChains", "multivm_estimateCrossVmFee"
        ];

        for method in multivm_methods {
            self.routing_rules.insert(
                method.to_string(),
                RoutingRule {
                    target_vm: VmType::MultiVm,
                    transformation: Some(MethodTransformation::None),
                    special_handling: matches!(method, "multivm_submitCrossVmTransaction"),
                    cacheable: !matches!(method, "multivm_submitCrossVmTransaction"),
                },
            );
        }
    }

    /// Add cross-VM aliases for unified access
    fn add_cross_vm_aliases(&mut self) {
        // Unified balance query
        self.aliases.insert("getBalance".to_string(), "multivm_getCrossChainBalance".to_string());
        
        // Unified block/slot query
        self.aliases.insert("getLatestBlock".to_string(), "multivm_getNetworkStats".to_string());
        
        // Unified transaction submission
        self.aliases.insert("sendTransaction".to_string(), "multivm_submitCrossVmTransaction".to_string());
    }

    /// Route RPC request to appropriate VM
    pub fn route_request(&self, request: &JsonRpcRequest) -> RpcResult<RoutingDecision> {
        let method = &request.method;
        
        // Check for aliases first
        let actual_method = self.aliases.get(method).unwrap_or(method);
        
        // Check routing rules
        if let Some(rule) = self.routing_rules.get(actual_method) {
            return Ok(RoutingDecision {
                target_vm: rule.target_vm,
                transformed_request: self.transform_request(request, rule)?,
                routing_rule: rule.clone(),
            });
        }

        // Fallback to prefix-based routing
        for (vm_type, prefixes) in &self.vm_prefixes {
            for prefix in prefixes {
                if actual_method.starts_with(prefix) {
                    return Ok(RoutingDecision {
                        target_vm: *vm_type,
                        transformed_request: request.clone(),
                        routing_rule: RoutingRule {
                            target_vm: *vm_type,
                            transformation: Some(MethodTransformation::None),
                            special_handling: false,
                            cacheable: true,
                        },
                    });
                }
            }
        }

        // Default routing based on method patterns
        let target_vm = self.infer_vm_from_method(actual_method);
        
        Ok(RoutingDecision {
            target_vm,
            transformed_request: request.clone(),
            routing_rule: RoutingRule {
                target_vm,
                transformation: Some(MethodTransformation::None),
                special_handling: false,
                cacheable: true,
            },
        })
    }

    /// Transform request based on routing rule
    fn transform_request(&self, request: &JsonRpcRequest, rule: &RoutingRule) -> RpcResult<JsonRpcRequest> {
        let mut transformed = request.clone();

        if let Some(ref transformation) = rule.transformation {
            match transformation {
                MethodTransformation::None => {}
                MethodTransformation::Rename(new_method) => {
                    transformed.method = new_method.clone();
                }
                MethodTransformation::TransformParams(param_transform) => {
                    transformed.params = self.transform_params(request.params.as_ref(), param_transform)?;
                }
                MethodTransformation::TransformResponse(_) => {
                    // Response transformation happens after the call
                }
                MethodTransformation::Full { params, .. } => {
                    transformed.params = self.transform_params(request.params.as_ref(), params)?;
                }
            }
        }

        Ok(transformed)
    }

    /// Transform request parameters
    fn transform_params(
        &self,
        params: Option<&serde_json::Value>,
        transformation: &ParamTransformation,
    ) -> RpcResult<Option<serde_json::Value>> {
        match transformation {
            ParamTransformation::EthBlockToSolSlot => {
                // Convert Ethereum block numbers to Solana slot numbers
                // This is a simplified conversion - in reality, you'd need a mapping service
                if let Some(serde_json::Value::Array(ref arr)) = params {
                    if let Some(serde_json::Value::String(ref block_str)) = arr.first() {
                        if block_str.starts_with("0x") {
                            // Convert hex block number to decimal slot
                            if let Ok(block_num) = u64::from_str_radix(&block_str[2..], 16) {
                                let slot = block_num * 2; // Rough conversion ratio
                                return Ok(Some(serde_json::json!([slot])));
                            }
                        } else if block_str == "latest" {
                            return Ok(Some(serde_json::json!(["finalized"])));
                        }
                    }
                }
                Ok(params.cloned())
            }
            ParamTransformation::SolSlotToEthBlock => {
                // Convert Solana slots to Ethereum block numbers
                if let Some(serde_json::Value::Array(ref arr)) = params {
                    if let Some(serde_json::Value::Number(ref slot_num)) = arr.first() {
                        if let Some(slot) = slot_num.as_u64() {
                            let block_num = slot / 2; // Rough conversion ratio
                            return Ok(Some(serde_json::json!([format!("0x{:x}", block_num)])));
                        }
                    }
                }
                Ok(params.cloned())
            }
            ParamTransformation::AddressFormat => {
                // Convert between Ethereum and Solana address formats
                // This would need actual address conversion logic
                Ok(params.cloned())
            }
            ParamTransformation::Custom(_) => {
                // Custom transformation logic would go here
                Ok(params.cloned())
            }
        }
    }

    /// Transform response based on routing rule
    pub fn transform_response(
        &self,
        response: &JsonRpcResponse,
        rule: &RoutingRule,
    ) -> RpcResult<JsonRpcResponse> {
        if let Some(ref transformation) = rule.transformation {
            match transformation {
                MethodTransformation::TransformResponse(resp_transform) |
                MethodTransformation::Full { response: resp_transform, .. } => {
                    return self.apply_response_transformation(response, resp_transform);
                }
                _ => {}
            }
        }

        Ok(response.clone())
    }

    /// Apply response transformation
    fn apply_response_transformation(
        &self,
        response: &JsonRpcResponse,
        transformation: &ResponseTransformation,
    ) -> RpcResult<JsonRpcResponse> {
        match transformation {
            ResponseTransformation::EthereumToStandard => {
                // Convert Ethereum-specific response format to standard format
                Ok(response.clone())
            }
            ResponseTransformation::SolanaToStandard => {
                // Convert Solana-specific response format to standard format
                Ok(response.clone())
            }
            ResponseTransformation::Custom(_) => {
                // Custom response transformation
                Ok(response.clone())
            }
        }
    }

    /// Infer VM type from method name
    fn infer_vm_from_method(&self, method: &str) -> VmType {
        if method.starts_with("multivm_") {
            VmType::MultiVm
        } else if method.starts_with("eth_") || 
                  method.starts_with("web3_") || 
                  method.starts_with("net_") ||
                  method.starts_with("debug_") ||
                  method.starts_with("trace_") {
            VmType::Ethereum
        } else {
            // Default to Solana for other methods
            VmType::Solana
        }
    }

    /// Add custom routing rule
    pub fn add_rule(&mut self, method: String, rule: RoutingRule) {
        debug!("Adding custom routing rule for method: {} -> {:?}", method, rule.target_vm);
        self.routing_rules.insert(method, rule);
    }

    /// Add method alias
    pub fn add_alias(&mut self, alias: String, target: String) {
        debug!("Adding method alias: {} -> {}", alias, target);
        self.aliases.insert(alias, target);
    }

    /// Get routing statistics
    pub fn get_stats(&self) -> RoutingStats {
        let mut vm_method_counts = HashMap::new();
        
        for rule in self.routing_rules.values() {
            *vm_method_counts.entry(rule.target_vm).or_insert(0) += 1;
        }

        RoutingStats {
            total_rules: self.routing_rules.len(),
            total_aliases: self.aliases.len(),
            vm_method_counts,
            special_handling_count: self.routing_rules.values()
                .filter(|rule| rule.special_handling)
                .count(),
        }
    }
}

/// Routing decision
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub target_vm: VmType,
    pub transformed_request: JsonRpcRequest,
    pub routing_rule: RoutingRule,
}

/// Routing statistics
#[derive(Debug, Clone)]
pub struct RoutingStats {
    pub total_rules: usize,
    pub total_aliases: usize,
    pub vm_method_counts: HashMap<VmType, usize>,
    pub special_handling_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_router_creation() {
        let router = RpcRouter::new();
        let stats = router.get_stats();
        
        assert!(stats.total_rules > 0);
        assert!(stats.vm_method_counts.contains_key(&VmType::Ethereum));
        assert!(stats.vm_method_counts.contains_key(&VmType::Solana));
        assert!(stats.vm_method_counts.contains_key(&VmType::MultiVm));
    }

    #[test]
    fn test_ethereum_method_routing() {
        let router = RpcRouter::new();
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let decision = router.route_request(&request).unwrap();
        assert_eq!(decision.target_vm, VmType::Ethereum);
        assert!(decision.routing_rule.cacheable);
    }

    #[test]
    fn test_solana_method_routing() {
        let router = RpcRouter::new();
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "getSlot".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let decision = router.route_request(&request).unwrap();
        assert_eq!(decision.target_vm, VmType::Solana);
    }

    #[test]
    fn test_multivm_method_routing() {
        let router = RpcRouter::new();
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "multivm_getVersion".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let decision = router.route_request(&request).unwrap();
        assert_eq!(decision.target_vm, VmType::MultiVm);
    }

    #[test]
    fn test_method_alias() {
        let router = RpcRouter::new();
        
        // Test that alias is resolved
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "getBalance".to_string(), // This should be aliased
            params: None,
            id: Some(json!(1)),
        };

        let decision = router.route_request(&request).unwrap();
        assert_eq!(decision.target_vm, VmType::MultiVm);
    }

    #[test]
    fn test_custom_rule_addition() {
        let mut router = RpcRouter::new();
        
        let custom_rule = RoutingRule {
            target_vm: VmType::Ethereum,
            transformation: Some(MethodTransformation::None),
            special_handling: false,
            cacheable: true,
        };

        router.add_rule("custom_method".to_string(), custom_rule);
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "custom_method".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let decision = router.route_request(&request).unwrap();
        assert_eq!(decision.target_vm, VmType::Ethereum);
    }

    #[test]
    fn test_vm_inference() {
        let router = RpcRouter::new();
        
        assert_eq!(router.infer_vm_from_method("eth_something"), VmType::Ethereum);
        assert_eq!(router.infer_vm_from_method("web3_something"), VmType::Ethereum);
        assert_eq!(router.infer_vm_from_method("multivm_something"), VmType::MultiVm);
        assert_eq!(router.infer_vm_from_method("getSomething"), VmType::Solana);
        assert_eq!(router.infer_vm_from_method("unknown_method"), VmType::Solana);
    }
}
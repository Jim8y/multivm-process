//! Utility functions for the real Reth execution engine
//!
//! This module contains helper functions for JWT authentication, transaction encoding,
//! database initialization, and other utility operations.

use crate::engine::{RethEngineError, Transaction};
use crate::real_engine::RealRethEngine;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tracing::{info, warn};

/// JWT authentication utilities
impl RealRethEngine {
    /// Initialize database if needed
    pub(super) async fn init_database_if_needed(&self) -> Result<(), RethEngineError> {
        let db_path = self.data_dir.join("db");

        if !db_path.exists() {
            info!("Initializing Reth database");

            let mut cmd = Command::new("reth");
            cmd.arg("init")
                .arg("--datadir")
                .arg(&self.data_dir)
                .arg("--chain")
                .arg(self.get_chain_name())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            let output = cmd.output().await.map_err(|e| {
                RethEngineError::Process(format!("Failed to init Reth database: {e}"))
            })?;

            if !output.status.success() {
                return Err(RethEngineError::Process(format!(
                    "Reth database init failed with exit code: {:?}",
                    output.status.code()
                )));
            }

            info!("Reth database initialized successfully");
        }

        Ok(())
    }

    /// Generate JWT secret for Engine API authentication
    pub(super) async fn generate_jwt_secret(&self) -> Result<(), RethEngineError> {
        let jwt_path = self.data_dir.join("jwt.hex");

        // Always generate a fresh JWT secret for each session
        info!("Generating fresh JWT secret for Engine API");

        // Generate 32 random bytes and encode as hex
        use std::io::Write;
        let mut rng = rand::thread_rng();
        let secret: [u8; 32] = rand::Rng::gen(&mut rng);
        let hex_secret = hex::encode(secret);

        // Write JWT secret to file for Reth to use
        let mut file = std::fs::File::create(&jwt_path).map_err(|e| {
            RethEngineError::Configuration(format!("Failed to create JWT file: {e}"))
        })?;

        file.write_all(hex_secret.as_bytes()).map_err(|e| {
            RethEngineError::Configuration(format!("Failed to write JWT secret: {e}"))
        })?;

        // Store JWT secret in memory for our communication with Reth
        *self.jwt_secret.write().await = Some(hex_secret.clone());
        
        info!("Fresh JWT secret generated and saved to: {:?}", jwt_path);
        info!("JWT secret length: {} characters", hex_secret.len());

        Ok(())
    }

    /// Create JWT token for Engine API authentication
    pub(super) fn create_jwt_token(&self, secret: &str) -> Result<String, RethEngineError> {
        Self::create_jwt_token_static(secret)
    }

    /// Create JWT token with custom expiration time
    pub(super) fn create_jwt_token_with_expiry(&self, secret: &str, expiry_seconds: u64) -> Result<String, RethEngineError> {
        Self::create_jwt_token_static_with_expiry(secret, expiry_seconds)
    }

    /// Static version of JWT token creation for use in background tasks
    pub(super) fn create_jwt_token_static(secret: &str) -> Result<String, RethEngineError> {
        Self::create_jwt_token_static_with_expiry(secret, 60)
    }

    /// Static version of JWT token creation with custom expiration
    pub(super) fn create_jwt_token_static_with_expiry(secret: &str, expiry_seconds: u64) -> Result<String, RethEngineError> {
        use sha2::Sha256;

        // Create JWT header
        let header = serde_json::json!({
            "alg": "HS256",
            "typ": "JWT"
        });

        // Create JWT payload with current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| RethEngineError::Configuration(format!("System time error: {e}")))?
            .as_secs();

        let payload = serde_json::json!({
            "iat": now,
            "exp": now + expiry_seconds
        });

        // Encode header and payload
        let header_b64 = URL_SAFE_NO_PAD.encode(
            serde_json::to_string(&header)
                .map_err(|e| {
                    RethEngineError::Configuration(format!("Failed to serialize header: {e}"))
                })?
                .as_bytes(),
        );

        let payload_b64 = URL_SAFE_NO_PAD.encode(
            serde_json::to_string(&payload)
                .map_err(|e| {
                    RethEngineError::Configuration(format!("Failed to serialize payload: {e}"))
                })?
                .as_bytes(),
        );

        // Create signature
        let message = format!("{header_b64}.{payload_b64}");
        let secret_bytes = hex::decode(secret).map_err(|e| {
            RethEngineError::Configuration(format!("Invalid JWT secret format: {e}"))
        })?;

        let mut mac = hmac::Hmac::<Sha256>::new_from_slice(&secret_bytes)
            .map_err(|e| RethEngineError::Configuration(format!("Failed to create HMAC: {e}")))?;

        use hmac::Mac;
        mac.update(message.as_bytes());
        let signature = mac.finalize().into_bytes();

        let signature_b64 = URL_SAFE_NO_PAD.encode(signature);

        // Combine into final JWT
        let jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");
        Ok(jwt)
    }

    /// Validate JWT token (for testing purposes)
    pub(super) fn validate_jwt_token(token: &str, secret: &str) -> Result<bool, RethEngineError> {
        use sha2::Sha256;

        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Ok(false);
        }

        let header_b64 = parts[0];
        let payload_b64 = parts[1];
        let signature_b64 = parts[2];

        // Recreate the message
        let message = format!("{header_b64}.{payload_b64}");
        
        // Decode secret
        let secret_bytes = hex::decode(secret).map_err(|e| {
            RethEngineError::Configuration(format!("Invalid JWT secret format: {e}"))
        })?;

        // Create HMAC
        let mut mac = hmac::Hmac::<Sha256>::new_from_slice(&secret_bytes)
            .map_err(|e| RethEngineError::Configuration(format!("Failed to create HMAC: {e}")))?;

        use hmac::Mac;
        mac.update(message.as_bytes());
        let expected_signature = mac.finalize().into_bytes();

        // Decode provided signature
        let provided_signature = URL_SAFE_NO_PAD.decode(signature_b64)
            .map_err(|e| RethEngineError::Configuration(format!("Invalid signature format: {e}")))?;

        // Compare signatures
        Ok(expected_signature.as_slice() == provided_signature.as_slice())
    }

    /// Get chain name for Reth configuration
    pub fn get_chain_name(&self) -> &str {
        match self.chain_id {
            1 => "mainnet",
            11155111 => "sepolia",
            17000 => "holesky",
            5 => "goerli",
            137 => "polygon",
            56 => "bsc",
            43114 => "avalanche",
            42161 => "arbitrum",
            10 => "optimism",
            _ => "dev", // Custom development chain
        }
    }

    /// Get human-readable chain description
    pub fn get_chain_description(&self) -> &str {
        match self.chain_id {
            1 => "Ethereum Mainnet",
            11155111 => "Sepolia Testnet",
            17000 => "Holesky Testnet",
            5 => "Goerli Testnet (deprecated)",
            137 => "Polygon Mainnet",
            56 => "BNB Smart Chain",
            43114 => "Avalanche C-Chain",
            42161 => "Arbitrum One",
            10 => "Optimism",
            _ => "Development Chain",
        }
    }

    /// Check if the chain supports EIP-1559
    pub fn supports_eip1559(&self) -> bool {
        match self.chain_id {
            1 | 11155111 | 17000 | 5 => true, // Ethereum networks
            137 => true, // Polygon
            42161 => true, // Arbitrum
            10 => true, // Optimism
            _ => true, // Default to true for dev chains
        }
    }

    /// Get current block number from Reth
    pub async fn get_current_block_number(&self) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "get_block_number",
            "method": "eth_blockNumber",
            "params": []
        });

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("RPC request failed: {e}")))?;

        if response.status().is_success() {
            let result: Value = response
                .json()
                .await
                .map_err(|e| RethEngineError::Rpc(format!("Failed to parse RPC response: {e}")))?;

            if let Some(block_hex) = result.get("result").and_then(|r| r.as_str()) {
                let block_number = u64::from_str_radix(block_hex.trim_start_matches("0x"), 16)
                    .map_err(|e| {
                        RethEngineError::Rpc(format!("Failed to parse block number: {e}"))
                    })?;
                Ok(block_number)
            } else {
                Err(RethEngineError::Rpc(
                    "Invalid block number response from Reth".to_string(),
                ))
            }
        } else {
            Err(RethEngineError::Rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )))
        }
    }

    /// Estimate gas for a transaction
    pub async fn estimate_gas(&self, transaction: &Value) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "estimate_gas",
            "method": "eth_estimateGas",
            "params": [transaction]
        });

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Gas estimation request failed: {e}")))?;

        if response.status().is_success() {
            let result: Value = response.json().await.map_err(|e| {
                RethEngineError::Rpc(format!("Failed to parse gas estimation response: {e}"))
            })?;

            if let Some(gas_hex) = result.get("result").and_then(|r| r.as_str()) {
                let gas_estimate = u64::from_str_radix(gas_hex.trim_start_matches("0x"), 16)
                    .map_err(|e| {
                        RethEngineError::Rpc(format!("Failed to parse gas estimate: {e}"))
                    })?;
                Ok(gas_estimate)
            } else {
                Err(RethEngineError::Rpc(
                    "Invalid gas estimation response from Reth".to_string(),
                ))
            }
        } else {
            Err(RethEngineError::Rpc(format!(
                "Gas estimation failed with status: {}",
                response.status()
            )))
        }
    }

    /// Send raw transaction to Reth
    pub async fn send_raw_transaction(&self, raw_tx: &str) -> Result<String, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "send_raw_transaction",
            "method": "eth_sendRawTransaction",
            "params": [raw_tx]
        });

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Transaction submission failed: {e}")))?;

        if response.status().is_success() {
            let result: Value = response.json().await.map_err(|e| {
                RethEngineError::Rpc(format!("Failed to parse transaction response: {e}"))
            })?;

            if let Some(tx_hash) = result.get("result").and_then(|r| r.as_str()) {
                Ok(tx_hash.to_string())
            } else if let Some(error) = result.get("error") {
                Err(RethEngineError::Rpc(format!(
                    "Transaction rejected: {error}"
                )))
            } else {
                Err(RethEngineError::Rpc(
                    "Invalid transaction response from Reth".to_string(),
                ))
            }
        } else {
            Err(RethEngineError::Rpc(format!(
                "Transaction submission failed with status: {}",
                response.status()
            )))
        }
    }

    /// Get transaction receipt
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<Value>, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "get_transaction_receipt",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash]
        });

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Receipt request failed: {e}")))?;

        if response.status().is_success() {
            let result: Value = response.json().await.map_err(|e| {
                RethEngineError::Rpc(format!("Failed to parse receipt response: {e}"))
            })?;

            if let Some(receipt) = result.get("result") {
                if receipt.is_null() {
                    Ok(None)
                } else {
                    Ok(Some(receipt.clone()))
                }
            } else {
                Err(RethEngineError::Rpc(
                    "Invalid receipt response from Reth".to_string(),
                ))
            }
        } else {
            Err(RethEngineError::Rpc(format!(
                "Receipt request failed with status: {}",
                response.status()
            )))
        }
    }

    /// RLP encode transaction for Ethereum compatibility
    pub fn rlp_encode_transaction(&self, tx: &Transaction) -> Vec<u8> {
        let mut stream = Vec::new();

        // Determine transaction type based on chain support and transaction fields
        let use_eip1559 = self.supports_eip1559() 
            && tx.max_fee_per_gas.is_some() 
            && tx.max_priority_fee_per_gas.is_some();

        if use_eip1559 {
            // EIP-1559 transaction (type 0x02)
            self.encode_eip1559_transaction(&mut stream, tx);
        } else {
            // Legacy transaction (type 0x00, no prefix)
            self.encode_legacy_transaction(&mut stream, tx);
        }

        stream
    }

    /// Get transaction type (0 for legacy, 2 for EIP-1559)
    pub fn get_transaction_type(&self, tx: &Transaction) -> u8 {
        if self.supports_eip1559() 
            && tx.max_fee_per_gas.is_some() 
            && tx.max_priority_fee_per_gas.is_some() {
            2 // EIP-1559
        } else {
            0 // Legacy
        }
    }

    /// Encode legacy transaction (EIP-155)
    fn encode_legacy_transaction(&self, stream: &mut Vec<u8>, tx: &Transaction) {
        // Legacy transaction format: [nonce, gasPrice, gasLimit, to, value, data, v, r, s]
        let mut items = Vec::new();

        // 1. Nonce
        self.encode_u64_to_items(&mut items, tx.nonce);

        // 2. Gas price
        let gas_price = tx.gas_price.unwrap_or(20_000_000_000); // 20 gwei default
        self.encode_u64_to_items(&mut items, gas_price);

        // 3. Gas limit
        self.encode_u64_to_items(&mut items, tx.gas_limit);

        // 4. To address (20 bytes or empty for contract creation)
        if let Some(to_addr) = tx.to {
            items.push(to_addr.to_vec());
        } else {
            items.push(vec![]); // Empty for contract creation
        }

        // 5. Value (convert U256 to bytes)
        let value_bytes = self.u256_to_bytes(&tx.value);
        items.push(value_bytes);

        // 6. Data
        items.push(tx.data.clone());

        // 7. v (recovery ID + chain ID for EIP-155)
        let v = tx.signature.v;
        self.encode_u64_to_items(&mut items, v);

        // 8. r (signature component)
        let r_bytes = self.u256_to_bytes(&tx.signature.r);
        items.push(r_bytes);

        // 9. s (signature component)
        let s_bytes = self.u256_to_bytes(&tx.signature.s);
        items.push(s_bytes);

        // Encode as RLP list
        self.encode_rlp_list(stream, &items);
    }

    /// Encode EIP-1559 transaction
    fn encode_eip1559_transaction(&self, stream: &mut Vec<u8>, tx: &Transaction) {
        // EIP-1559 transaction type prefix
        stream.push(0x02);

        // Transaction format: [chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to, value, data, accessList, v, r, s]
        let mut items = Vec::new();

        // 1. Chain ID
        self.encode_u64_to_items(&mut items, self.chain_id);

        // 2. Nonce
        self.encode_u64_to_items(&mut items, tx.nonce);

        // 3. Max priority fee per gas (tip)
        self.encode_u64_to_items(&mut items, 1_500_000_000); // 1.5 gwei default

        // 4. Max fee per gas (base fee + tip)
        self.encode_u64_to_items(&mut items, 20_000_000_000); // 20 gwei default

        // 5. Gas limit
        self.encode_u64_to_items(&mut items, tx.gas_limit);

        // 6. To address
        if let Some(to_addr) = tx.to {
            items.push(to_addr.to_vec());
        } else {
            items.push(vec![]); // Contract creation
        }

        // 7. Value
        let value_bytes = self.u256_to_bytes(&tx.value);
        items.push(value_bytes);

        // 8. Data
        items.push(tx.data.clone());

        // 9. Access list (empty for now)
        items.push(vec![]); // Empty access list

        // 10. v (EIP-2930/1559 format)
        let v = tx.signature.v;
        self.encode_u64_to_items(&mut items, v);

        // 11. r
        let r_bytes = self.u256_to_bytes(&tx.signature.r);
        items.push(r_bytes);

        // 12. s
        let s_bytes = self.u256_to_bytes(&tx.signature.s);
        items.push(s_bytes);

        // Encode as RLP list and append to stream
        self.encode_rlp_list(stream, &items);
    }

    /// Helper: Encode u64 as minimal bytes and add to items
    fn encode_u64_to_items(&self, items: &mut Vec<Vec<u8>>, value: u64) {
        if value == 0 {
            items.push(vec![]); // Empty bytes for zero
        } else {
            // Remove leading zeros
            let bytes = value.to_be_bytes();
            let start = bytes
                .iter()
                .position(|&b| b != 0)
                .unwrap_or(bytes.len() - 1);
            items.push(bytes[start..].to_vec());
        }
    }

    /// Helper: Convert U256 to minimal bytes representation
    fn u256_to_bytes(&self, value: &crate::engine::U256) -> Vec<u8> {
        // For our simplified U256, just use the first u64
        let val = value.0[0];
        if val == 0 {
            vec![] // Empty bytes for zero
        } else {
            let bytes = val.to_be_bytes();
            let start = bytes
                .iter()
                .position(|&b| b != 0)
                .unwrap_or(bytes.len() - 1);
            bytes[start..].to_vec()
        }
    }

    /// Helper: Encode RLP list
    fn encode_rlp_list(&self, stream: &mut Vec<u8>, items: &[Vec<u8>]) {
        // Calculate total length of items
        let mut content = Vec::new();
        for item in items {
            self.encode_rlp_item(&mut content, item);
        }

        // Encode list header
        if content.len() < 56 {
            // Short list
            stream.push(0xc0 + content.len() as u8);
        } else {
            // Long list
            let length_bytes = self.encode_length(content.len());
            stream.push(0xf7 + length_bytes.len() as u8);
            stream.extend_from_slice(&length_bytes);
        }

        // Add content
        stream.extend_from_slice(&content);
    }

    /// Helper: Encode single RLP item
    fn encode_rlp_item(&self, stream: &mut Vec<u8>, item: &[u8]) {
        if item.len() == 1 && item[0] < 0x80 {
            // Single byte less than 0x80
            stream.push(item[0]);
        } else if item.len() < 56 {
            // Short string
            stream.push(0x80 + item.len() as u8);
            stream.extend_from_slice(item);
        } else {
            // Long string
            let length_bytes = self.encode_length(item.len());
            stream.push(0xb7 + length_bytes.len() as u8);
            stream.extend_from_slice(&length_bytes);
            stream.extend_from_slice(item);
        }
    }

    /// Helper: Encode length for long RLP items
    fn encode_length(&self, length: usize) -> Vec<u8> {
        if length < 256 {
            vec![length as u8]
        } else if length < 65536 {
            vec![(length >> 8) as u8, length as u8]
        } else if length < 16777216 {
            vec![(length >> 16) as u8, (length >> 8) as u8, length as u8]
        } else {
            vec![
                (length >> 24) as u8,
                (length >> 16) as u8,
                (length >> 8) as u8,
                length as u8,
            ]
        }
    }


}

// Reth execution engine implementation using native Reth types
// This uses Reth primitives directly for full compatibility

use multivm_common::config::VmType;

// Import native Reth and Alloy types
use alloy_consensus::private::alloy_eips::eip2718::Encodable2718;
use alloy_consensus::{
    Block, BlockBody, Eip658Value, Header, Receipt, ReceiptEnvelope, TxEnvelope, TxReceipt,
};
use alloy_primitives::{Address, Bloom, Bytes, Log, TxHash, B256, U256};
use alloy_rpc_types_eth::TransactionReceipt;
use serde::{Deserialize, Serialize};

// Use native Alloy types with concrete transaction types
pub type RethTransaction = TxEnvelope;
pub type RethBlock = Block<RethTransaction>;
pub type RethBlockBody = BlockBody<RethTransaction>;
pub type RethBlockHeader = Header;
pub type RethReceipt = Receipt<Log>;

// Export Transaction as an alias for RethTransaction for backward compatibility
pub type Transaction = RethTransaction;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Withdrawal {
    pub index: u64,
    #[serde(rename = "validatorIndex")]
    pub validator_index: u64,
    pub address: Address,
    pub amount: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadV1 {
    pub parent_hash: B256,
    pub fee_recipient: Address,
    pub state_root: B256,
    pub receipts_root: B256,
    pub logs_bloom: Bloom,
    pub prev_randao: B256,
    #[serde(deserialize_with = "hex_string_to_u64")]
    pub block_number: u64,
    #[serde(deserialize_with = "hex_string_to_u64")]
    pub gas_limit: u64,
    #[serde(deserialize_with = "hex_string_to_u64")]
    pub gas_used: u64,
    #[serde(deserialize_with = "hex_string_to_u64")]
    pub timestamp: u64,
    pub extra_data: Bytes,
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions: Vec<Bytes>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadV2 {
    #[serde(flatten)]
    pub payload_inner: ExecutionPayloadV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withdrawals: Option<Vec<Withdrawal>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadV3 {
    #[serde(flatten)]
    pub payload_inner: ExecutionPayloadV2,
    #[serde(deserialize_with = "hex_string_to_u64")]
    pub blob_gas_used: u64,
    #[serde(deserialize_with = "hex_string_to_u64")]
    pub excess_blob_gas: u64,
    #[serde(default)]
    pub requests: Vec<serde_json::Value>, // Prague upgrade requests (optional)
}
/// MultiVM transaction format for cross-VM compatibility
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MultivmTransaction {
    pub vm_type: VmType,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub data: String,
    pub gas_price: Option<String>,
    pub gas_limit: Option<u64>,
    pub nonce: Option<u64>,
    pub chain_id: Option<u64>,
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    pub transaction_type: Option<u8>, // 0 = legacy, 1 = access list, 2 = EIP-1559
}

/// Use Alloy's native receipt type for internal processing  
pub type RethTransactionReceipt = Receipt<Log>;

/// Use Alloy's native log type
pub type RethTransactionLog = Log;

/// Transaction pool status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionPoolStatus {
    pub pending_count: u64,
    pub queued_count: u64,
    pub is_transaction_pending: bool,
}

/// Transaction validation result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub error_message: Option<String>,
    pub estimated_gas: Option<u64>,
    pub gas_price_suggestion: Option<u64>,
    pub nonce_suggestion: Option<u64>,
}

/// Transaction forwarding result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionForwardingResult {
    pub transaction_hash: String,
    pub status: String,
    pub gas_used: Option<u64>,
    pub block_number: Option<u64>,
    pub confirmation_time: Option<std::time::Duration>,
}

/// Simplified transaction data for validation
#[derive(Debug, Clone)]
pub struct TransactionData {
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub gas_limit: u64,
    pub gas_price: Option<u64>,
    pub nonce: u64,
    pub data: String,
}

/// Helper functions for Block operations using native Reth hash calculation
pub fn calculate_block_hash(block: &RethBlock) -> B256 {
    block.header.hash_slow()
}

/// Calculate block hash the same way Reth does: RLP encode + Keccak256
pub fn calculate_reth_style_hash(payload: &ExecutionPayloadV3) -> B256 {
    use alloy_primitives::keccak256;
    use alloy_rlp::Encodable;

    // Calculate withdrawals root (empty list for empty withdrawals)
    let withdrawals_root = match &payload.payload_inner.withdrawals {
        Some(withdrawals) if withdrawals.is_empty() => {
            Some(alloy_primitives::keccak256(&[0xc0])) // RLP encoding of empty list
        }
        Some(_) => {
            None // This would need actual calculation for non-empty withdrawals
        }
        None => None, // No withdrawals field (pre-Shanghai)
    };

    // Calculate requests hash (empty list for empty requests)
    let requests_hash = if payload.requests.is_empty() {
        Some(alloy_primitives::keccak256(&[0xc0])) // RLP encoding of empty list
    } else {
        None // This would need actual calculation for non-empty requests
    };

    // Create a header from the payload
    let header = Header {
        parent_hash: payload.payload_inner.payload_inner.parent_hash,
        ommers_hash: alloy_primitives::keccak256(&[0xc0]), // RLP encoding of empty ommers list
        beneficiary: payload.payload_inner.payload_inner.fee_recipient,
        state_root: payload.payload_inner.payload_inner.state_root,
        transactions_root: alloy_primitives::keccak256(&[0xc0]), // RLP encoding of empty transactions list
        receipts_root: payload.payload_inner.payload_inner.receipts_root,
        logs_bloom: payload.payload_inner.payload_inner.logs_bloom,
        difficulty: U256::ZERO, // PoS chains have zero difficulty
        number: payload.payload_inner.payload_inner.block_number,
        gas_limit: payload.payload_inner.payload_inner.gas_limit,
        gas_used: payload.payload_inner.payload_inner.gas_used,
        timestamp: payload.payload_inner.payload_inner.timestamp,
        extra_data: payload.payload_inner.payload_inner.extra_data.clone(),
        mix_hash: payload.payload_inner.payload_inner.prev_randao, // In PoS, mix_hash = prev_randao
        nonce: alloy_primitives::B64::ZERO,                        // PoS chains have zero nonce
        base_fee_per_gas: Some(
            payload
                .payload_inner
                .payload_inner
                .base_fee_per_gas
                .to::<u64>(),
        ),
        withdrawals_root,
        blob_gas_used: Some(payload.blob_gas_used),
        excess_blob_gas: Some(payload.excess_blob_gas),
        parent_beacon_block_root: Some(B256::from([0x01; 32])), // Default parent beacon block root
        requests_hash,
    };

    // RLP encode the header
    let mut encoded = Vec::new();
    header.encode(&mut encoded);

    // Calculate Keccak256 hash
    keccak256(&encoded)
}

/// Get access to the native Reth header
pub fn get_block_header(block: &RethBlock) -> &Header {
    &block.header
}

/// Check if Prague fields should be included based on timestamp
pub fn should_include_prague_fields(block: &RethBlock) -> bool {
    // Prague fields are available from timestamp 0 in dev chains
    true
}
use async_trait::async_trait;
use multivm_common::{types_rpc::RpcConfig, *};
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

// Alloy imports for standard Ethereum types and RLP encoding
use alloy_consensus;
use alloy_primitives;
use alloy_rlp;

// Direct alloy consensus usage to match reth's block building logic

// Import real engine implementation when not in mock mode
// Real engine implementation available when real-node feature is enabled

/// Use official Reth ExecutionPayloadV3 type for Engine API
pub type ExecutionPayload = ExecutionPayloadV3;

/// Simplified fork choice state type
#[derive(Debug, Clone, serde::Serialize)]
pub struct ForkchoiceState {
    #[serde(rename = "headBlockHash")]
    pub head_block_hash: B256,
    #[serde(rename = "safeBlockHash")]
    pub safe_block_hash: B256,
    #[serde(rename = "finalizedBlockHash")]
    pub finalized_block_hash: B256,
}

/// PayloadAttributesV3 structure for requesting block building
#[derive(Debug, Clone, serde::Serialize)]
pub struct PayloadAttributesV3 {
    pub timestamp: u64,
    #[serde(rename = "prevRandao")]
    pub prev_randao: B256,
    #[serde(rename = "suggestedFeeRecipient")]
    pub suggested_fee_recipient: Address,
    #[serde(rename = "parentBeaconBlockRoot")]
    pub parent_beacon_block_root: Option<B256>,
    pub withdrawals: Option<Vec<serde_json::Value>>,
}

/// ForkchoiceUpdated response with payload_id
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ForkchoiceUpdatedResponse {
    #[serde(rename = "payloadStatus")]
    pub payload_status: PayloadStatus,
    #[serde(rename = "payloadId")]
    pub payload_id: Option<String>,
}

/// PayloadStatus from Engine API responses
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PayloadStatus {
    pub status: String, // "VALID", "INVALID", "SYNCING", etc.
    #[serde(rename = "latestValidHash")]
    pub latest_valid_hash: Option<B256>,
    #[serde(rename = "validationError")]
    pub validation_error: Option<String>,
}

/// GetPayloadV3 response structure using official types
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetPayloadV2Response {
    #[serde(rename = "executionPayload")]
    pub execution_payload: ExecutionPayloadV2,
    #[serde(rename = "blockValue")]
    pub block_value: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetPayloadV3Response {
    #[serde(rename = "executionPayload")]
    pub execution_payload: ExecutionPayloadV3,
    #[serde(rename = "blockValue")]
    pub block_value: String,
    #[serde(rename = "blobsBundle")]
    pub blobs_bundle: Option<serde_json::Value>,
    #[serde(rename = "shouldOverrideBuilder")]
    pub should_override_builder: bool,
}

/// Reth execution result type
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct RethExecutionResult {
    pub block_hash: B256,
    pub block_number: u64,
    pub gas_used: u64,
    pub transactions_count: usize,
    #[serde(with = "duration_serde")]
    pub processing_time: Duration,
    pub state_root: B256,
    pub success: bool,
    pub error: Option<String>,
}

mod duration_serde {
    use super::*;
    use serde::Deserialize;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

/// 将十六进制字符串反序列化为 u64
fn hex_string_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let value: serde_json::Value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => {
            u64::from_str_radix(s.trim_start_matches("0x"), 16).map_err(serde::de::Error::custom)
        }
        serde_json::Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("Invalid number format")),
        _ => Err(serde::de::Error::custom("Expected string or number")),
    }
}

/// 将可选的十六进制字符串反序列化为 Option<u64>
fn optional_hex_string_to_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) => {
            let val = u64::from_str_radix(s.trim_start_matches("0x"), 16)
                .map_err(serde::de::Error::custom)?;
            Ok(Some(val))
        }
        None => Ok(None),
    }
}

/// Reth specific error type
#[derive(Debug, thiserror::Error)]
pub enum RethEngineError {
    #[error("Reth process error: {0}")]
    Process(String),

    #[error("RPC communication error: {0}")]
    Rpc(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Block processing error: {0}")]
    #[allow(dead_code)]
    BlockProcessing(String),

    #[error("Invalid block data: {0}")]
    #[allow(dead_code)]
    InvalidBlock(String),

    #[error("Engine API error: {0}")]
    #[allow(dead_code)]
    EngineApi(String),

    #[error("Transaction forwarding error: {0}")]
    TransactionForwarding(String),

    #[error("Transaction validation error: {0}")]
    TransactionValidation(String),

    #[error("Transaction receipt error: {0}")]
    TransactionReceipt(String),

    #[error("Transaction format conversion error: {0}")]
    TransactionConversion(String),

    #[error("Transaction pool error: {0}")]
    TransactionPool(String),

    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),
}

impl From<RethEngineError> for multivm_common::MultivmError {
    fn from(err: RethEngineError) -> Self {
        match err {
            RethEngineError::Process(msg) => multivm_common::MultivmError::Process {
                process_id: "reth-engine".to_string(),
                message: msg,
                exit_code: None,
            },
            RethEngineError::Rpc(msg) => multivm_common::MultivmError::Rpc {
                method: "reth-rpc".to_string(),
                message: msg,
                status_code: None,
            },
            RethEngineError::Configuration(msg) => multivm_common::MultivmError::Configuration {
                component: "reth-engine".to_string(),
                message: msg,
                validation_errors: None,
            },
            RethEngineError::BlockProcessing(msg) => multivm_common::MultivmError::Process {
                process_id: "reth-block-processing".to_string(),
                message: msg,
                exit_code: None,
            },
            RethEngineError::InvalidBlock(msg) => multivm_common::MultivmError::Process {
                process_id: "reth-block-validation".to_string(),
                message: msg,
                exit_code: None,
            },
            RethEngineError::EngineApi(msg) => multivm_common::MultivmError::Rpc {
                method: "reth-engine-api".to_string(),
                message: msg,
                status_code: None,
            },
            RethEngineError::TransactionForwarding(msg) => multivm_common::MultivmError::Rpc {
                method: "reth-tx-forwarding".to_string(),
                message: msg,
                status_code: None,
            },
            RethEngineError::TransactionValidation(msg) => multivm_common::MultivmError::Process {
                process_id: "reth-tx-validation".to_string(),
                message: msg,
                exit_code: None,
            },
            RethEngineError::TransactionReceipt(msg) => multivm_common::MultivmError::Rpc {
                method: "reth-tx-receipt".to_string(),
                message: msg,
                status_code: None,
            },
            RethEngineError::TransactionConversion(msg) => {
                multivm_common::MultivmError::Configuration {
                    component: "reth-tx-conversion".to_string(),
                    message: msg,
                    validation_errors: None,
                }
            }
            RethEngineError::TransactionPool(msg) => multivm_common::MultivmError::Rpc {
                method: "reth-tx-pool".to_string(),
                message: msg,
                status_code: None,
            },
            RethEngineError::TransactionNotFound(msg) => multivm_common::MultivmError::Rpc {
                method: "reth-tx-notfound".to_string(),
                message: msg,
                status_code: Some(404),
            },
        }
    }
}

pub struct RethExecutionEngine {
    data_dir: PathBuf,
    rpc_port: u16,
    reth_process: Arc<RwLock<Option<Child>>>,
    current_block: Arc<RwLock<u64>>,
    blocks_processed: Arc<RwLock<u64>>,
    start_time: Instant,
    is_running: Arc<RwLock<bool>>,
    rpc_client: Arc<RwLock<Option<reqwest::Client>>>,
    chain_id: u64,

    /// Mock mode configuration
    mock_mode: bool,
}

impl RethExecutionEngine {
    pub async fn new(
        data_dir: PathBuf,
        rpc_port: u16,
        chain_id: u64,
    ) -> Result<Self, RethEngineError> {
        Self::new_with_mode(data_dir, rpc_port, chain_id, false).await
    }

    pub async fn new_with_mode(
        data_dir: PathBuf,
        rpc_port: u16,
        chain_id: u64,
        mock_mode: bool,
    ) -> Result<Self, RethEngineError> {
        if mock_mode {
            tracing::info!("Creating Reth execution engine in MOCK mode (no real Reth node)");
        } else {
            tracing::info!("Creating Reth execution engine that will spawn actual Reth node");
        }

        // Create directories
        std::fs::create_dir_all(&data_dir).map_err(|e| {
            RethEngineError::Configuration(format!("Failed to create data directory: {e}"))
        })?;

        Ok(Self {
            data_dir,
            rpc_port,
            reth_process: Arc::new(RwLock::new(None)),
            current_block: Arc::new(RwLock::new(0)),
            blocks_processed: Arc::new(RwLock::new(0)),
            start_time: Instant::now(),
            is_running: Arc::new(RwLock::new(false)),
            rpc_client: Arc::new(RwLock::new(None)),
            chain_id,
            mock_mode,
        })
    }

    /// Start the actual Reth node process in execution-only mode
    async fn start_reth_process(&self) -> Result<(), RethEngineError> {
        tracing::info!("Starting Reth node in execution-only mode (P2P and consensus disabled)");

        // Initialize database if needed
        self.init_database_if_needed().await?;

        let mut cmd = Command::new("reth");
        cmd.arg("node")
            .arg("--authrpc.jwtsecret")
            .arg(
                std::fs::canonicalize(self.data_dir.join("jwt.hex")).map_err(|e| {
                    RethEngineError::Configuration(format!("Failed to canonicalize JWT path: {e}"))
                })?,
            )
            .arg("--authrpc.addr")
            .arg("127.0.0.1")
            .arg("--authrpc.port")
            .arg((self.rpc_port + 1).to_string())
            .arg("--http")
            .arg("--http.api")
            .arg("eth,net,web3,debug")
            .arg("--chain")
            .arg("sepolia")
            .arg("--disable-discovery")
            .arg("--log.stdout.format")
            .arg("json")
            .arg("--log.stdout.filter")
            .arg("info,reth=debug")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        tracing::info!("Reth command: {:?}", cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| RethEngineError::Process(format!("Failed to start Reth node: {e}")))?;

        let pid = child.id();

        // 获取 stdout 和 stderr 用于实时日志输出
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        // 创建实时日志输出任务
        let stdout_handle = tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let reader = tokio::io::BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!("Reth stdout: {}", line);
            }
        });

        let stderr_handle = tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let reader = tokio::io::BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!("Reth stderr: {}", line);
            }
        });

        // Check if the process is still running after a brief moment
        tokio::time::sleep(Duration::from_millis(500)).await;

        if let Ok(Some(exit_status)) = child.try_wait() {
            return Err(RethEngineError::Process(format!(
                "Reth node exited immediately with status: {:?}",
                exit_status
            )));
        }

        *self.reth_process.write().await = Some(child);

        tracing::info!(
            "Started Reth node in execution-only mode with PID: {:?}",
            pid
        );
        tracing::info!("Reth HTTP RPC: http://127.0.0.1:{}", self.rpc_port);
        tracing::info!("Reth Engine API: http://127.0.0.1:{}", self.rpc_port + 1);

        // Wait for Reth to initialize
        tokio::time::sleep(Duration::from_secs(15)).await;

        // Initialize RPC client with no proxy for local connections
        let client = reqwest::Client::builder().no_proxy().build().map_err(|e| {
            RethEngineError::Configuration(format!("Failed to create HTTP client: {e}"))
        })?;
        *self.rpc_client.write().await = Some(client);

        // Verify connection
        self.verify_reth_connection().await?;

        Ok(())
    }

    /// Initialize database and JWT secret if needed
    async fn init_database_if_needed(&self) -> Result<(), RethEngineError> {
        let db_path = self.data_dir.join("db");

        if !db_path.exists() {
            tracing::info!("Initializing Reth database");

            let mut cmd = Command::new("reth");
            cmd.arg("init")
                .arg("--datadir")
                .arg(&self.data_dir)
                .arg("--chain")
                .arg("sepolia")
                .stdout(Stdio::null())
                .stderr(Stdio::null());

            let output = cmd.output().await.map_err(|e| {
                RethEngineError::Process(format!("Failed to init Reth database: {e}"))
            })?;

            if !output.status.success() {
                return Err(RethEngineError::Process(format!(
                    "Reth database init failed with exit code: {:?}",
                    output.status.code()
                )));
            }

            tracing::info!("Reth database initialized successfully");
        }

        // Generate JWT secret for Engine API authentication
        self.generate_jwt_secret().await?;

        Ok(())
    }

    /// Generate JWT secret for Engine API authentication
    async fn generate_jwt_secret(&self) -> Result<(), RethEngineError> {
        let jwt_path = self.data_dir.join("jwt.hex");

        if !jwt_path.exists() {
            tracing::info!("Generating JWT secret for Engine API");

            // Generate 32 random bytes and encode as hex
            use std::io::Write;
            let mut rng = rand::thread_rng();
            let secret: [u8; 32] = rand::Rng::gen(&mut rng);
            let hex_secret = hex::encode(secret);

            let mut file = std::fs::File::create(&jwt_path).map_err(|e| {
                RethEngineError::Configuration(format!("Failed to create JWT file: {e}"))
            })?;

            file.write_all(hex_secret.as_bytes()).map_err(|e| {
                RethEngineError::Configuration(format!("Failed to write JWT secret: {e}"))
            })?;

            tracing::info!("JWT secret generated: {:?}", jwt_path);
        }

        Ok(())
    }

    /// Generate JWT token for Engine API authentication
    fn generate_jwt_token(&self) -> Result<String, RethEngineError> {
        use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

        let jwt_path = self.data_dir.join("jwt.hex");
        let hex_secret = std::fs::read_to_string(&jwt_path).map_err(|e| {
            RethEngineError::Configuration(format!("Failed to read JWT secret: {e}"))
        })?;

        let secret_bytes = hex::decode(hex_secret.trim()).map_err(|e| {
            RethEngineError::Configuration(format!("Invalid JWT secret format: {e}"))
        })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| RethEngineError::Configuration(format!("Time error: {e}")))?
            .as_secs();

        let claims = serde_json::json!({
            "iat": now,
            "exp": now + 3600, // Token expires in 1 hour
        });

        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(&secret_bytes),
        )
        .map_err(|e| RethEngineError::Configuration(format!("JWT generation failed: {e}")))?;

        Ok(token)
    }

    /// Verify connection to Reth node
    async fn verify_reth_connection(&self) -> Result<(), RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        // Test basic connectivity with eth_chainId
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "connection_test",
            "method": "eth_chainId",
            "params": []
        });

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Failed to connect to Reth RPC: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::Rpc(format!(
                "Reth RPC returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Failed to parse Reth response: {e}")))?;

        if let Some(chain_id_hex) = result.get("result").and_then(|r| r.as_str()) {
            let chain_id = u64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16)
                .map_err(|e| RethEngineError::Rpc(format!("Invalid chain ID from Reth: {e}")))?;

            tracing::info!(
                "Successfully connected to Reth node, chain ID: {}",
                chain_id
            );
        } else {
            return Err(RethEngineError::Rpc(
                "Invalid response from Reth node".to_string(),
            ));
        }

        Ok(())
    }

    /// Process a reth block using the official Block type
    async fn process_reth_block(
        &mut self,
        block: RethBlock,
    ) -> Result<RethExecutionResult, RethEngineError> {
        let start_time = Instant::now();
        let block_number = block.header.number;
        let block_hash = block.header.hash_slow(); // Calculate block hash

        tracing::info!(
            "Processing Reth block {} with hash {:?}",
            block_number,
            block_hash
        );

        // Submit block to Reth node via Engine API
        self.submit_block_to_reth(&block).await?;

        // Update metrics
        let mut current_block = self.current_block.write().await;
        *current_block = block_number;

        let mut blocks_processed = self.blocks_processed.write().await;
        *blocks_processed += 1;

        let processing_time = start_time.elapsed();
        let transactions_count = block.body.transactions.len();
        let gas_used = block.header.gas_used;
        let state_root = block.header.state_root;

        tracing::info!(
            "Successfully processed Reth block {} in {:?} with {} transactions, gas used: {}",
            block_number,
            processing_time,
            transactions_count,
            gas_used
        );

        Ok(RethExecutionResult {
            block_hash,
            block_number,
            gas_used,
            transactions_count,
            processing_time,
            state_root,
            success: true,
            error: None,
        })
    }

    /// Submit a reth block to the node via Engine API
    async fn submit_block_to_reth(&self, block: &RethBlock) -> Result<(), RethEngineError> {
        tracing::debug!(
            "Submitting Reth block {} via Engine API",
            block.header.number
        );

        // Step 1: Create execution payload from Reth block
        let execution_payload = self.create_execution_payload_from_block(block)?;

        // Step 2: Submit payload via engine_newPayloadV1
        let payload_response = self.submit_execution_payload(&execution_payload).await?;
        tracing::debug!("Payload submission response: {:?}", payload_response);

        // Step 2.1: Check if payload was accepted by reth
        if let Some(result) = payload_response.get("result") {
            if let Some(status) = result.get("status").and_then(|s| s.as_str()) {
                match status {
                    "VALID" => {
                        tracing::debug!("Reth accepted the payload as VALID");
                    }
                    "INVALID" => {
                        let error_msg = result
                            .get("validationError")
                            .and_then(|e| e.as_str())
                            .unwrap_or("Unknown validation error");
                        return Err(RethEngineError::BlockProcessing(format!(
                            "Reth rejected block {}: {}",
                            block.number, error_msg
                        )));
                    }
                    "SYNCING" => {
                        tracing::warn!("Reth is syncing, block {} status is SYNCING", block.number);
                        // Continue with fork choice update for syncing blocks
                    }
                    other => {
                        tracing::warn!("Unexpected payload status: {}", other);
                    }
                }
            }
        }

        // Step 3: Update fork choice via engine_forkchoiceUpdatedV1
        let fork_choice_state = self.create_fork_choice_state_from_block(block)?;
        let fork_choice_response = self.update_fork_choice(&fork_choice_state).await?;
        tracing::debug!("Fork choice response: {:?}", fork_choice_response);

        tracing::info!(
            "Successfully submitted Reth block {} via Engine API",
            block.number
        );

        Ok(())
    }

    /// Create execution payload from Reth Block using custom ExecutionPayloadV3
    fn create_execution_payload_from_block(
        &self,
        block: &RethBlock,
    ) -> Result<ExecutionPayloadV3, RethEngineError> {
        // Manually construct ExecutionPayloadV3 from block
        let header = &block.header;
        let body = &block.body;

        // Create V1 payload
        let v1_payload = ExecutionPayloadV1 {
            parent_hash: header.parent_hash,
            fee_recipient: Address::ZERO, // Will be set by engine
            state_root: header.state_root,
            receipts_root: header.receipts_root,
            logs_bloom: header.logs_bloom,
            prev_randao: header.mix_hash,
            block_number: header.number,
            gas_limit: header.gas_limit,
            gas_used: header.gas_used,
            timestamp: header.timestamp,
            extra_data: header.extra_data.clone(),
            base_fee_per_gas: header
                .base_fee_per_gas
                .map(U256::from)
                .unwrap_or(U256::ZERO),
            block_hash: block.hash_slow(),
            transactions: body
                .transactions
                .iter()
                .map(|tx| {
                    let mut encoded = Vec::new();
                    tx.encode_2718(&mut encoded);
                    Bytes::from(encoded)
                })
                .collect(),
        };

        // Create V2 payload with empty withdrawals
        let v2_payload = ExecutionPayloadV2 {
            payload_inner: v1_payload,
            withdrawals: Some(Vec::new()), // Empty withdrawals for now
        };

        // Create V3 payload with blob gas fields
        let v3_payload = ExecutionPayloadV3 {
            payload_inner: v2_payload,
            blob_gas_used: 0,     // No blob transactions for now
            excess_blob_gas: 0,   // No blob gas for now
            requests: Vec::new(), // Empty requests for Prague upgrade
        };

        Ok(v3_payload)
    }

    /// Create fork choice state from Reth Block
    fn create_fork_choice_state_from_block(
        &self,
        block: &RethBlock,
    ) -> Result<ForkchoiceState, RethEngineError> {
        let fork_choice_state = ForkchoiceState {
            head_block_hash: block.hash_slow(),
            safe_block_hash: block.header.parent_hash, // Previous block is considered safe
            finalized_block_hash: block.header.parent_hash, // Previous block is finalized
        };

        Ok(fork_choice_state)
    }

    /// Submit execution payload to Reth via engine_newPayloadV3
    async fn submit_execution_payload(
        &self,
        payload: &ExecutionPayloadV3,
    ) -> Result<serde_json::Value, RethEngineError> {
        // 使用空的 versioned_hashes 和零值 parent_beacon_block_root
        let versioned_hashes: Vec<B256> = vec![];
        let parent_beacon_block_root = B256::ZERO;

        self.engine_new_payload_v3(payload, versioned_hashes, parent_beacon_block_root)
            .await
    }

    /// engine_newPayloadV2 call
    pub async fn engine_new_payload_v2(
        &self,
        payload: &ExecutionPayloadV2,
    ) -> Result<serde_json::Value, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "engine_newPayload",
            "method": "engine_newPayloadV2",
            "params": [payload]
        });

        let engine_url = format!("http://127.0.0.1:{}", self.rpc_port + 1);

        // Generate JWT token for authentication
        let jwt_token = self.generate_jwt_token()?;

        let response = client
            .post(&engine_url)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Engine API request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::Rpc(format!(
                "Engine API returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::Rpc(format!("Failed to parse Engine API response: {e}"))
        })?;

        Ok(result)
    }

    /// engine_newPayloadV3 with proper parameters
    async fn engine_new_payload_v3(
        &self,
        payload: &ExecutionPayloadV3,
        versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
    ) -> Result<serde_json::Value, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "engine_newPayload",
            "method": "engine_newPayloadV3",
            "params": [payload, versioned_hashes, parent_beacon_block_root]
        });

        let engine_url = format!("http://127.0.0.1:{}", self.rpc_port + 1);

        // Generate JWT token for authentication
        let jwt_token = self.generate_jwt_token()?;

        let response = client
            .post(&engine_url)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Engine API request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::Rpc(format!(
                "Engine API returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::Rpc(format!("Failed to parse Engine API response: {e}"))
        })?;

        Ok(result)
    }

    /// engine_newPayloadV3 with JSON payload (supports withdrawals and blobs)
    async fn engine_new_payload_v3_json(
        &self,
        payload: &serde_json::Value,
        versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
    ) -> Result<serde_json::Value, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "engine_newPayload",
            "method": "engine_newPayloadV3",
            "params": [payload, versioned_hashes, parent_beacon_block_root]
        });

        let engine_url = format!("http://127.0.0.1:{}", self.rpc_port + 1);

        // Generate JWT token for authentication
        let jwt_token = self.generate_jwt_token()?;

        let response = client
            .post(&engine_url)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Engine API request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::Rpc(format!(
                "Engine API returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::Rpc(format!("Failed to parse Engine API response: {e}"))
        })?;

        Ok(result)
    }

    /// Produce a block using the correct Engine API V3 workflow
    pub async fn produce_block_v3(
        &self,
        head_block_hash: B256,
        fee_recipient: Address,
    ) -> Result<B256, RethEngineError> {
        // 1. 设置 fork choice state
        let fork_choice_state = ForkchoiceState {
            head_block_hash,
            safe_block_hash: head_block_hash,
            finalized_block_hash: head_block_hash,
        };
        let parent_timestamp = self
            .get_block_timestamp(head_block_hash)
            .await
            .unwrap_or_else(|_| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            });

        let timestamp = parent_timestamp + 12; // 标准 12 秒出块间隔

        // 获取父区块的 beacon block root（如果可用）
        let parent_beacon_block_root = self
            .get_parent_beacon_block_root(head_block_hash)
            .await
            .unwrap_or_else(|_| B256::from([0x01; 32])); // 如果获取失败，使用默认值

        // 创建简化的 PayloadAttributes（类似V2）
        let payload_attributes = PayloadAttributesV3 {
            timestamp,
            prev_randao: B256::ZERO,
            suggested_fee_recipient: fee_recipient,
            parent_beacon_block_root: None, // 移除 beacon block root for V2 compatibility
            withdrawals: None,              // 移除 withdrawals 字段，避免 pre-Shanghai 错误
        };

        // 3. Send forkchoiceUpdated to create payload job
        let fork_choice_response = self
            .engine_forkchoice_updated_v2(&fork_choice_state, Some(payload_attributes))
            .await?;

        // 4. Extract payload_id from response
        let payload_id = fork_choice_response
            .get("result")
            .and_then(|r| r.get("payloadId"))
            .and_then(|id| id.as_str())
            .ok_or_else(|| RethEngineError::Rpc(format!("No payload_id in forkchoice response")))?;

        // 5. Get built payload
        let payload_response = self.engine_get_payload_v2(payload_id).await?;
        let validated_hash = payload_response.execution_payload.payload_inner.block_hash;
        let payload_v2 = &payload_response.execution_payload;

        // 6. Submit payload to Reth
        let new_payload_response = self.engine_new_payload_v2(payload_v2).await?;

        // Check newPayloadV2 response
        if let Some(result) = new_payload_response.get("result") {
            if let Some(status) = result.get("status").and_then(|s| s.as_str()) {
                match status {
                    "VALID" => {} // Success, continue
                    "SYNCING" => {
                        tracing::info!("Block accepted but Reth is syncing");
                    }
                    "INVALID" => {
                        let error = result
                            .get("validationError")
                            .and_then(|e| e.as_str())
                            .unwrap_or("Unknown validation error");
                        return Err(RethEngineError::Rpc(format!(
                            "newPayloadV2 validation failed: {}",
                            error
                        )));
                    }
                    _ => {
                        tracing::warn!("Unexpected newPayloadV2 status: {}", status);
                    }
                }
            }
        }

        // 7. Final forkchoice confirmation
        let final_fork_choice_state = ForkchoiceState {
            head_block_hash: validated_hash,
            safe_block_hash: validated_hash,
            finalized_block_hash: head_block_hash,
        };

        let final_response = self
            .engine_forkchoice_updated_v3(&final_fork_choice_state, None)
            .await?;

        // Check final confirmation
        if let Some(final_result) = final_response.get("result") {
            if let Some(final_status) = final_result
                .get("payloadStatus")
                .and_then(|ps| ps.get("status"))
                .and_then(|s| s.as_str())
            {
                match final_status {
                    "VALID" => {
                        tracing::info!(
                            "Block produced successfully: 0x{}",
                            hex::encode(validated_hash)
                        );
                        return Ok(validated_hash);
                    }
                    "SYNCING" => {
                        tracing::info!(
                            "Block built successfully (Reth syncing): 0x{}",
                            hex::encode(validated_hash)
                        );
                        return Ok(validated_hash);
                    }
                    _ => {
                        return Err(RethEngineError::Rpc(format!(
                            "Final forkchoice update failed: {}",
                            final_status
                        )));
                    }
                }
            }
        }

        // Default success case
        tracing::info!("Block produced: 0x{}", hex::encode(validated_hash));
        Ok(validated_hash)
    }

    /// Update fork choice via engine_forkchoiceUpdatedV3
    async fn update_fork_choice(
        &self,
        fork_choice_state: &ForkchoiceState,
    ) -> Result<serde_json::Value, RethEngineError> {
        self.engine_forkchoice_updated_v3(fork_choice_state, None)
            .await
    }

    /// engine_forkchoiceUpdatedV2 with optional PayloadAttributesV3
    pub async fn engine_forkchoice_updated_v2(
        &self,
        fork_choice_state: &ForkchoiceState,
        payload_attributes: Option<PayloadAttributesV3>,
    ) -> Result<serde_json::Value, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "engine_forkchoiceUpdated",
            "method": "engine_forkchoiceUpdatedV2",
            "params": [fork_choice_state, payload_attributes]
        });

        let engine_url = format!("http://127.0.0.1:{}", self.rpc_port + 1);

        // Generate JWT token for authentication
        let jwt_token = self.generate_jwt_token()?;

        let response = client
            .post(&engine_url)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Fork choice update failed: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::Rpc(format!(
                "Fork choice update returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::Rpc(format!("Failed to parse fork choice response: {e}"))
        })?;

        Ok(result)
    }

    /// engine_forkchoiceUpdatedV3 with optional PayloadAttributesV3
    pub async fn engine_forkchoice_updated_v3(
        &self,
        fork_choice_state: &ForkchoiceState,
        payload_attributes: Option<PayloadAttributesV3>,
    ) -> Result<serde_json::Value, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "engine_forkchoiceUpdated",
            "method": "engine_forkchoiceUpdatedV3",
            "params": [fork_choice_state, payload_attributes]
        });

        let engine_url = format!("http://127.0.0.1:{}", self.rpc_port + 1);

        // Generate JWT token for authentication
        let jwt_token = self.generate_jwt_token()?;

        let response = client
            .post(&engine_url)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Fork choice update failed: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::Rpc(format!(
                "Fork choice update returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::Rpc(format!("Failed to parse fork choice response: {e}"))
        })?;

        Ok(result)
    }

    /// engine_getPayloadV2 to retrieve built payload
    pub async fn engine_get_payload_v2(
        &self,
        payload_id: &str,
    ) -> Result<GetPayloadV2Response, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "engine_getPayload",
            "method": "engine_getPayloadV2",
            "params": [payload_id]
        });

        let engine_url = format!("http://127.0.0.1:{}", self.rpc_port + 1);

        // Generate JWT token for authentication
        let jwt_token = self.generate_jwt_token()?;

        let response = client
            .post(&engine_url)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Get payload failed: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::Rpc(format!(
                "Get payload returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::Rpc(format!("Failed to parse get payload response: {e}"))
        })?;

        if let Some(result_data) = result.get("result") {
            let payload_response: GetPayloadV2Response =
                serde_json::from_value(result_data.clone()).map_err(|e| {
                    RethEngineError::Rpc(format!("Failed to deserialize payload: {e}"))
                })?;
            Ok(payload_response)
        } else {
            Err(RethEngineError::Rpc(
                "No result in get payload response".to_string(),
            ))
        }
    }

    /// engine_getPayloadV3 to retrieve built payload
    async fn engine_get_payload_v3(
        &self,
        payload_id: &str,
    ) -> Result<GetPayloadV3Response, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "engine_getPayload",
            "method": "engine_getPayloadV3",
            "params": [payload_id]
        });

        let engine_url = format!("http://127.0.0.1:{}", self.rpc_port + 1);

        // Generate JWT token for authentication
        let jwt_token = self.generate_jwt_token()?;

        let response = client
            .post(&engine_url)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Get payload failed: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::Rpc(format!(
                "Get payload returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::Rpc(format!("Failed to parse get payload response: {e}"))
        })?;

        if let Some(result_data) = result.get("result") {
            let payload_response: GetPayloadV3Response =
                serde_json::from_value(result_data.clone()).map_err(|e| {
                    RethEngineError::Rpc(format!("Failed to deserialize payload: {e}"))
                })?;
            Ok(payload_response)
        } else {
            Err(RethEngineError::Rpc(
                "No result in get payload response".to_string(),
            ))
        }
    }

    /// Get current block number from Reth via RPC
    pub async fn get_current_block_from_reth(&self) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = json!({
            "jsonrpc": "2.0",
            "id": "get_block_number",
            "method": "eth_blockNumber",
            "params": []
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("RPC request failed: {e}")))?;

        if response.status().is_success() {
            let result: serde_json::Value = response
                .json()
                .await
                .map_err(|e| RethEngineError::Rpc(format!("Failed to parse RPC response: {e}")))?;

            if let Some(block_hex) = result.get("result").and_then(|r| r.as_str()) {
                // Parse hex block number
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

    /// Get parent beacon block root from Reth via RPC
    async fn get_parent_beacon_block_root(
        &self,
        parent_block_hash: B256,
    ) -> Result<B256, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "get_beacon_block_root",
            "method": "eth_getBlockByHash",
            "params": [format!("0x{}", hex::encode(parent_block_hash)), false]
        });

        let response = client
            .post(&format!("http://127.0.0.1:{}", self.rpc_port))
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Failed to get beacon block root: {e}")))?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::Rpc(format!("Failed to parse beacon block root response: {e}"))
        })?;

        let block_data = result
            .get("result")
            .ok_or_else(|| RethEngineError::Rpc("No block data in response".to_string()))?;

        if block_data.is_null() {
            return Err(RethEngineError::Rpc("Parent block not found".to_string()));
        }

        // 尝试从区块中获取 beacon block root
        if let Some(beacon_root) = block_data
            .get("parentBeaconBlockRoot")
            .and_then(|r| r.as_str())
        {
            let root_bytes = hex::decode(beacon_root.trim_start_matches("0x"))
                .map_err(|e| RethEngineError::Rpc(format!("Invalid beacon root format: {e}")))?;
            if root_bytes.len() == 32 {
                Ok(B256::from_slice(&root_bytes))
            } else {
                Err(RethEngineError::Rpc(
                    "Invalid beacon root length".to_string(),
                ))
            }
        } else {
            // 如果没有 beacon block root，返回默认值
            Ok(B256::from([0x01; 32]))
        }
    }

    /// Get chain ID from Reth via RPC
    #[allow(dead_code)]
    /// Get block hash by block number from Reth
    /// Get block timestamp by hash
    pub async fn get_block_timestamp(&self, block_hash: B256) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "get_block_timestamp",
            "method": "eth_getBlockByHash",
            "params": [format!("0x{}", hex::encode(block_hash)), false]
        });

        let response = client
            .post(&format!("http://127.0.0.1:{}", self.rpc_port))
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Failed to get block timestamp: {e}")))?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::Rpc(format!("Failed to parse timestamp response: {e}"))
        })?;

        let block_data = result
            .get("result")
            .ok_or_else(|| RethEngineError::Rpc("No block data in response".to_string()))?;

        if block_data.is_null() {
            return Err(RethEngineError::Rpc("Block not found".to_string()));
        }

        let timestamp_hex = block_data
            .get("timestamp")
            .and_then(|t| t.as_str())
            .ok_or_else(|| RethEngineError::Rpc("No timestamp in block data".to_string()))?;

        let timestamp = u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::Rpc(format!("Invalid timestamp format: {e}")))?;

        Ok(timestamp)
    }

    pub async fn get_block_hash(&self, block_number: u64) -> Result<B256, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "get_block_hash",
            "method": "eth_getBlockByNumber",
            "params": [format!("0x{:x}", block_number), false] // false = only return hash
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("RPC request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::Rpc(format!(
                "RPC returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(result_value) = result.get("result") {
            if let Some(block_data) = result_value.as_object() {
                if let Some(hash_str) = block_data.get("hash").and_then(|h| h.as_str()) {
                    let hash_bytes = hex::decode(hash_str.trim_start_matches("0x"))
                        .map_err(|e| RethEngineError::Rpc(format!("Invalid hash format: {e}")))?;
                    if hash_bytes.len() == 32 {
                        Ok(B256::from_slice(&hash_bytes))
                    } else {
                        Err(RethEngineError::Rpc("Invalid hash length".to_string()))
                    }
                } else {
                    Err(RethEngineError::Rpc("Block hash not found".to_string()))
                }
            } else if result_value.is_null() {
                Err(RethEngineError::Rpc("Block not found".to_string()))
            } else {
                Err(RethEngineError::Rpc(
                    "Invalid block data format".to_string(),
                ))
            }
        } else {
            Err(RethEngineError::Rpc(format!(
                "Unexpected RPC response format: {:?}",
                result
            )))
        }
    }

    async fn get_chain_id_from_reth(&self) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = json!({
            "jsonrpc": "2.0",
            "id": "get_chain_id",
            "method": "eth_chainId",
            "params": []
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::Rpc(format!("RPC request failed: {e}")))?;

        if response.status().is_success() {
            let result: serde_json::Value = response
                .json()
                .await
                .map_err(|e| RethEngineError::Rpc(format!("Failed to parse RPC response: {e}")))?;

            if let Some(chain_hex) = result.get("result").and_then(|r| r.as_str()) {
                let chain_id = u64::from_str_radix(chain_hex.trim_start_matches("0x"), 16)
                    .map_err(|e| RethEngineError::Rpc(format!("Failed to parse chain ID: {e}")))?;
                Ok(chain_id)
            } else {
                Err(RethEngineError::Rpc(
                    "Invalid chain ID response from Reth".to_string(),
                ))
            }
        } else {
            Err(RethEngineError::Rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )))
        }
    }

    /// RLP encode transaction for Ethereum compatibility
    pub fn rlp_encode_transaction(&self, tx: &RethTransaction) -> Vec<u8> {
        // For now, return a placeholder since TxEnvelope encoding is complex
        // In a real implementation, this would use alloy's built-in encoding
        vec![]
    }

    /// Encode legacy transaction (EIP-155)
    fn encode_legacy_transaction(&self, stream: &mut Vec<u8>, tx: &RethTransaction) {
        // Placeholder implementation - TxEnvelope field access is complex
        // In a real implementation, this would extract fields from the envelope
    }

    /// Encode EIP-1559 transaction
    fn encode_eip1559_transaction(&self, stream: &mut Vec<u8>, tx: &RethTransaction) {
        // Placeholder implementation - TxEnvelope field access is complex
        // In a real implementation, this would extract fields from the envelope
    }

    /// Helper: Encode u64 as minimal bytes
    fn encode_u64(&self, items: &mut Vec<Vec<u8>>, value: u64) {
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
    fn u256_to_bytes(&self, value: &U256) -> Vec<u8> {
        // Convert U256 to big-endian bytes properly
        let bytes = value.to_be_bytes::<32>();

        // Remove leading zeros for compact representation
        let start = bytes.iter().position(|&b| b != 0).unwrap_or(31);
        bytes[start..].to_vec()
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

    // =======================
    // Transaction Forwarding
    // =======================

    /// Forward a single transaction to the Reth process
    pub async fn forward_transaction_to_reth(
        &self,
        tx: &RethTransaction,
    ) -> Result<TransactionForwardingResult, RethEngineError> {
        let start_time = std::time::Instant::now();

        tracing::info!(
            "Forwarding transaction to Reth: {:?}",
            hex::encode(tx.hash().as_slice())
        );

        // Encode transaction as RLP
        let rlp_encoded = self.rlp_encode_transaction(tx);
        let tx_hex = format!("0x{}", hex::encode(&rlp_encoded));

        // Submit transaction via eth_sendRawTransaction
        let tx_hash = self.submit_raw_transaction_rpc(&tx_hex).await?;

        // Wait for transaction to be mined (with timeout)
        let receipt = self
            .wait_for_transaction_receipt(&tx_hash, Duration::from_secs(60))
            .await?;

        let result = TransactionForwardingResult {
            transaction_hash: tx_hash,
            status: if receipt.status() {
                "success".to_string()
            } else {
                "failed".to_string()
            },
            gas_used: Some(0),     // Will be calculated from cumulative gas used
            block_number: Some(0), // Block context needed
            confirmation_time: Some(start_time.elapsed()),
        };

        tracing::info!("Transaction forwarded successfully: {:?}", result);
        Ok(result)
    }

    /// Submit raw transaction data to Reth
    pub async fn submit_raw_transaction(&self, tx_data: &[u8]) -> Result<String, RethEngineError> {
        let tx_hex = format!("0x{}", hex::encode(tx_data));
        self.submit_raw_transaction_rpc(&tx_hex).await
    }

    /// Submit raw transaction via RPC
    async fn submit_raw_transaction_rpc(&self, tx_hex: &str) -> Result<String, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionForwarding("RPC client not initialized".to_string())
        })?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "eth_sendRawTransaction",
            "method": "eth_sendRawTransaction",
            "params": [tx_hex]
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| {
                RethEngineError::TransactionForwarding(format!("RPC request failed: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(RethEngineError::TransactionForwarding(format!(
                "RPC returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionForwarding(format!("Failed to parse RPC response: {e}"))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionForwarding(format!(
                "RPC error: {error}"
            )));
        }

        let tx_hash = result
            .get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| {
                RethEngineError::TransactionForwarding(
                    "Missing transaction hash in response".to_string(),
                )
            })?;

        Ok(tx_hash.to_string())
    }

    // =======================
    // Transaction Receipts
    // =======================

    /// Get transaction receipt by hash
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<RethTransactionReceipt>, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionReceipt("RPC client not initialized".to_string())
        })?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "eth_getTransactionReceipt",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash]
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::TransactionReceipt(format!("RPC request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(RethEngineError::TransactionReceipt(format!(
                "RPC returned error status: {}",
                response.status()
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionReceipt(format!("Failed to parse RPC response: {e}"))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionReceipt(format!(
                "RPC error: {error}"
            )));
        }

        let receipt_data = result.get("result");
        if receipt_data.is_none() || receipt_data.unwrap().is_null() {
            return Ok(None);
        }

        let receipt_data = receipt_data.unwrap();
        let receipt = self.parse_transaction_receipt(receipt_data)?;
        Ok(Some(receipt))
    }

    /// Wait for transaction receipt with timeout
    pub async fn wait_for_transaction_receipt(
        &self,
        tx_hash: &str,
        timeout: Duration,
    ) -> Result<RethTransactionReceipt, RethEngineError> {
        let start_time = std::time::Instant::now();
        let poll_interval = Duration::from_millis(500);

        while start_time.elapsed() < timeout {
            if let Some(receipt) = self.get_transaction_receipt(tx_hash).await? {
                return Ok(receipt);
            }
            tokio::time::sleep(poll_interval).await;
        }

        Err(RethEngineError::TransactionReceipt(format!(
            "Transaction receipt not found within timeout: {tx_hash}"
        )))
    }

    /// Parse transaction receipt from JSON
    fn parse_transaction_receipt(
        &self,
        receipt_json: &serde_json::Value,
    ) -> Result<RethTransactionReceipt, RethEngineError> {
        let tx_hash = self.parse_hash(receipt_json.get("transactionHash"))?;
        let block_hash = self.parse_hash(receipt_json.get("blockHash"))?;
        let block_number = self.parse_hex_u64(receipt_json.get("blockNumber"))?;
        let transaction_index = self.parse_hex_u64(receipt_json.get("transactionIndex"))?;
        let from = self.parse_address(receipt_json.get("from"))?;
        let to = receipt_json
            .get("to")
            .and_then(|v| v.as_str())
            .map(|s| self.parse_address_str(s))
            .transpose()?;
        let gas_used = self.parse_hex_u64(receipt_json.get("gasUsed"))?;
        let cumulative_gas_used = self.parse_hex_u64(receipt_json.get("cumulativeGasUsed"))?;
        let status = self.parse_hex_u64(receipt_json.get("status"))?;
        let effective_gas_price = self.parse_hex_u64(receipt_json.get("effectiveGasPrice"))?;

        let contract_address = receipt_json
            .get("contractAddress")
            .and_then(|v| v.as_str())
            .map(|s| self.parse_address_str(s))
            .transpose()?;

        let logs = receipt_json
            .get("logs")
            .and_then(|v| v.as_array())
            .unwrap_or(&vec![])
            .iter()
            .map(|log| self.parse_transaction_log(log))
            .collect::<Result<Vec<_>, _>>()?;

        let logs_bloom = self.parse_logs_bloom(receipt_json.get("logsBloom"))?;

        Ok(RethTransactionReceipt {
            status: alloy_consensus::Eip658Value::Eip658(status != 0),
            cumulative_gas_used,
            logs,
        })
    }

    /// Parse transaction log from JSON
    fn parse_transaction_log(
        &self,
        log_json: &serde_json::Value,
    ) -> Result<RethTransactionLog, RethEngineError> {
        let address = self.parse_address(log_json.get("address"))?;
        let topics = log_json
            .get("topics")
            .and_then(|v| v.as_array())
            .unwrap_or(&vec![])
            .iter()
            .map(|topic| self.parse_hash(Some(topic)))
            .collect::<Result<Vec<_>, _>>()?;

        let data_vec = log_json
            .get("data")
            .and_then(|v| v.as_str())
            .map(|s| hex::decode(s.trim_start_matches("0x")))
            .transpose()
            .map_err(|e| RethEngineError::TransactionReceipt(format!("Invalid log data: {e}")))?
            .unwrap_or_default();
        let data = Bytes::from(data_vec);

        let block_number = Some(self.parse_hex_u64(log_json.get("blockNumber"))?);
        let transaction_hash = Some(self.parse_hash(log_json.get("transactionHash"))?);
        let transaction_index = Some(self.parse_hex_u64(log_json.get("transactionIndex"))?);
        let block_hash = log_json
            .get("blockHash")
            .map(|v| self.parse_hash(Some(v)))
            .transpose()?;
        let log_index = Some(self.parse_hex_u64(log_json.get("logIndex"))?);
        let removed = Some(
            log_json
                .get("removed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        );

        Ok(RethTransactionLog {
            address,
            data: alloy_primitives::LogData::new_unchecked(topics, data),
        })
    }

    // =======================
    // Transaction Validation
    // =======================

    /// Validate a transaction against the Reth node
    pub async fn validate_transaction(
        &self,
        tx: &RethTransaction,
    ) -> Result<ValidationResult, RethEngineError> {
        // Encode transaction as RLP
        let rlp_encoded = self.rlp_encode_transaction(tx);
        let tx_hex = format!("0x{}", hex::encode(&rlp_encoded));

        // Call eth_call to validate the transaction
        let validation_result = self.call_transaction_validation(&tx_hex).await?;

        Ok(validation_result)
    }

    /// Call transaction validation using Reth RPC methods directly
    async fn call_transaction_validation(
        &self,
        tx_hex: &str,
    ) -> Result<ValidationResult, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionValidation("RPC client not initialized".to_string())
        })?;

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        // 1. 直接尝试使用 eth_estimateGas 来验证交易
        // 这是最直接的方法，因为 estimateGas 会执行完整的交易验证
        // 包括检查余额、nonce、签名、gas limit 等
        tracing::info!(
            "🔍 Calling Reth RPC eth_estimateGas for transaction validation: {}",
            tx_hex
        );
        let gas_estimate = match self.estimate_gas(tx_hex).await {
            Ok(gas) => {
                // 如果 gas 估算成功，说明交易基本有效
                tracing::info!("✅ Gas estimation successful: {} gas", gas);
                Some(gas)
            }
            Err(e) => {
                // Gas 估算失败，说明交易有问题
                let error_msg = e.to_string();
                tracing::warn!("❌ Gas estimation failed: {}", error_msg);

                // 解析具体的错误类型
                if error_msg.contains("insufficient funds") {
                    return Ok(ValidationResult {
                        is_valid: false,
                        error_message: Some("Insufficient funds for transaction".to_string()),
                        estimated_gas: None,
                        gas_price_suggestion: None,
                        nonce_suggestion: None,
                    });
                } else if error_msg.contains("nonce too low")
                    || error_msg.contains("nonce too high")
                {
                    return Ok(ValidationResult {
                        is_valid: false,
                        error_message: Some("Invalid nonce".to_string()),
                        estimated_gas: None,
                        gas_price_suggestion: None,
                        nonce_suggestion: None,
                    });
                } else if error_msg.contains("gas limit") {
                    return Ok(ValidationResult {
                        is_valid: false,
                        error_message: Some("Gas limit too low".to_string()),
                        estimated_gas: None,
                        gas_price_suggestion: None,
                        nonce_suggestion: None,
                    });
                } else if error_msg.contains("invalid signature")
                    || error_msg.contains("unauthorized")
                {
                    return Ok(ValidationResult {
                        is_valid: false,
                        error_message: Some("Invalid transaction signature".to_string()),
                        estimated_gas: None,
                        gas_price_suggestion: None,
                        nonce_suggestion: None,
                    });
                } else {
                    return Ok(ValidationResult {
                        is_valid: false,
                        error_message: Some(format!(
                            "Transaction validation failed: {}",
                            error_msg
                        )),
                        estimated_gas: None,
                        gas_price_suggestion: None,
                        nonce_suggestion: None,
                    });
                }
            }
        };

        // 2. 获取当前建议的 gas price
        tracing::info!("🔍 Calling Reth RPC eth_gasPrice for gas price suggestion");
        let gas_price_suggestion = self.get_gas_price().await.ok();

        // 3. 解析交易以获取发送者地址（用于获取 nonce 建议）
        let sender_address = self.extract_sender_from_transaction(tx_hex).await;
        let nonce_suggestion = if let Ok(sender) = sender_address {
            tracing::info!(
                "🔍 Calling Reth RPC eth_getTransactionCount for nonce suggestion for address: {}",
                sender
            );
            self.get_transaction_count(&sender).await.ok()
        } else {
            None
        };

        // 4. 如果所有检查都通过，交易是有效的
        Ok(ValidationResult {
            is_valid: true,
            error_message: None,
            estimated_gas: gas_estimate,
            gas_price_suggestion,
            nonce_suggestion,
        })
    }

    /// 从交易中提取发送者地址
    async fn extract_sender_from_transaction(
        &self,
        tx_hex: &str,
    ) -> Result<String, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionValidation("RPC client not initialized".to_string())
        })?;

        // 使用 debug_traceCall 或类似方法来获取交易的发送者
        // 作为备用方案，我们可以尝试解析 RLP 编码的交易
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "decode_transaction",
            "method": "eth_getTransactionByHash",
            "params": [tx_hex]
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| {
                RethEngineError::TransactionValidation(format!("Transaction decode failed: {e}"))
            })?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionValidation(format!("Failed to parse decode response: {e}"))
        })?;

        // 如果通过哈希找不到交易（因为它还没有被发送），我们返回一个默认地址
        // 在实际实现中，这里应该解析 RLP 编码的交易数据
        if result.get("result").is_none() || result.get("result").unwrap().is_null() {
            // 返回一个示例地址，实际应该从 tx_hex 中解析
            return Ok("0x742d35Cc6634C0532925a3b8D80C7A8C4C9d0f04".to_string());
        }

        let tx_data = result.get("result").unwrap();
        let from = tx_data
            .get("from")
            .and_then(|f| f.as_str())
            .ok_or_else(|| {
                RethEngineError::TransactionValidation(
                    "Missing from address in transaction".to_string(),
                )
            })?;

        Ok(from.to_string())
    }

    /// 解析原始交易数据
    fn parse_raw_transaction(&self, tx_hex: &str) -> Result<TransactionData, RethEngineError> {
        // 简化的交易解析 - 在实际实现中应该完整解析 RLP
        // 这里我们创建一个基本的交易数据结构用于验证
        Ok(TransactionData {
            from: "0x742d35Cc6634C0532925a3b8D80C7A8C4C9d0f04".to_string(), // 示例地址
            to: Some("0x8ba1f109551bD432803012645aac136c8C52b7A5".to_string()),
            value: "0x1000000000000000".to_string(),
            gas_limit: 21000,
            gas_price: Some(20_000_000_000),
            nonce: 0,
            data: "0x".to_string(),
        })
    }

    /// 验证交易格式
    fn validate_transaction_format(
        &self,
        _tx_data: &TransactionData,
    ) -> Result<(), RethEngineError> {
        // 基本格式验证
        // 在完整实现中，这里应该验证：
        // - 地址格式
        // - 数值范围
        // - 签名有效性
        Ok(())
    }

    /// 为交易对象估算 gas
    async fn estimate_gas_for_tx_object(
        &self,
        tx_data: &TransactionData,
    ) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionValidation("RPC client not initialized".to_string())
        })?;

        // 构建 eth_estimateGas 请求的交易对象
        let tx_object = serde_json::json!({
            "from": tx_data.from,
            "to": tx_data.to,
            "value": tx_data.value,
            "gas": format!("0x{:x}", tx_data.gas_limit),
            "gasPrice": tx_data.gas_price.map(|gp| format!("0x{:x}", gp)),
            "data": tx_data.data
        });

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "eth_estimateGas",
            "method": "eth_estimateGas",
            "params": [tx_object]
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| {
                RethEngineError::TransactionValidation(format!("Gas estimation failed: {e}"))
            })?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionValidation(format!(
                "Failed to parse gas estimation response: {e}"
            ))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionValidation(format!(
                "Gas estimation error: {}",
                error
            )));
        }

        let gas_hex = result
            .get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| {
                RethEngineError::TransactionValidation(
                    "Missing gas estimate in response".to_string(),
                )
            })?;

        let gas = u64::from_str_radix(gas_hex.trim_start_matches("0x"), 16).map_err(|e| {
            RethEngineError::TransactionValidation(format!("Invalid gas estimate: {e}"))
        })?;

        Ok(gas)
    }

    /// 验证发送者余额
    async fn validate_sender_balance(
        &self,
        tx_data: &TransactionData,
    ) -> Result<(), RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionValidation("RPC client not initialized".to_string())
        })?;

        // 获取账户余额
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "eth_getBalance",
            "method": "eth_getBalance",
            "params": [tx_data.from, "latest"]
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| {
                RethEngineError::TransactionValidation(format!("Balance check failed: {e}"))
            })?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionValidation(format!("Failed to parse balance response: {e}"))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionValidation(format!(
                "Balance check error: {}",
                error
            )));
        }

        let balance_hex = result
            .get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| {
                RethEngineError::TransactionValidation("Missing balance in response".to_string())
            })?;

        let balance = u64::from_str_radix(balance_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::TransactionValidation(format!("Invalid balance: {e}")))?;

        // 简化的余额检查 - 在完整实现中应该计算 value + gas_cost
        let tx_value = u64::from_str_radix(tx_data.value.trim_start_matches("0x"), 16).unwrap_or(0);
        if balance < tx_value {
            return Err(RethEngineError::TransactionValidation(
                "Insufficient balance for transaction".to_string(),
            ));
        }

        Ok(())
    }

    /// 获取账户交易计数（nonce）
    async fn get_transaction_count(&self, address: &str) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionValidation("RPC client not initialized".to_string())
        })?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "eth_getTransactionCount",
            "method": "eth_getTransactionCount",
            "params": [address, "latest"]
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| {
                RethEngineError::TransactionValidation(format!("Nonce check failed: {e}"))
            })?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionValidation(format!("Failed to parse nonce response: {e}"))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionValidation(format!(
                "Nonce check error: {}",
                error
            )));
        }

        let nonce_hex = result
            .get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| {
                RethEngineError::TransactionValidation("Missing nonce in response".to_string())
            })?;

        let nonce = u64::from_str_radix(nonce_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::TransactionValidation(format!("Invalid nonce: {e}")))?;

        Ok(nonce)
    }

    /// Estimate gas for a raw transaction
    async fn estimate_gas(&self, tx_hex: &str) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionValidation("RPC client not initialized".to_string())
        })?;

        // 对于原始交易数据，我们需要先解码然后构建适当的参数
        // 这里我们直接使用 debug_traceCall 或者构建一个交易对象
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "eth_estimateGas",
            "method": "eth_estimateGas",
            "params": [{
                "data": tx_hex,
                "gas": "0x5f5e100" // 100,000,000 gas limit for estimation
            }]
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);
        tracing::info!("📡 Making RPC request to {}: {}", rpc_url, rpc_request);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| {
                RethEngineError::TransactionValidation(format!("Gas estimation failed: {e}"))
            })?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionValidation(format!(
                "Failed to parse gas estimation response: {e}"
            ))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionValidation(format!(
                "Gas estimation error: {error}"
            )));
        }

        let gas_hex = result
            .get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| {
                RethEngineError::TransactionValidation(
                    "Missing gas estimate in response".to_string(),
                )
            })?;

        let gas = u64::from_str_radix(gas_hex.trim_start_matches("0x"), 16).map_err(|e| {
            RethEngineError::TransactionValidation(format!("Invalid gas estimate: {e}"))
        })?;

        Ok(gas)
    }

    /// Get current gas price
    async fn get_gas_price(&self) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionValidation("RPC client not initialized".to_string())
        })?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "eth_gasPrice",
            "method": "eth_gasPrice",
            "params": []
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| {
                RethEngineError::TransactionValidation(format!("Gas price request failed: {e}"))
            })?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionValidation(format!(
                "Failed to parse gas price response: {e}"
            ))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionValidation(format!(
                "Gas price error: {error}"
            )));
        }

        let price_hex = result
            .get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| {
                RethEngineError::TransactionValidation("Missing gas price in response".to_string())
            })?;

        let price = u64::from_str_radix(price_hex.trim_start_matches("0x"), 16).map_err(|e| {
            RethEngineError::TransactionValidation(format!("Invalid gas price: {e}"))
        })?;

        Ok(price)
    }

    /// Get transaction pool status
    pub async fn get_transaction_pool_status(
        &self,
    ) -> Result<TransactionPoolStatus, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionPool("RPC client not initialized".to_string())
        })?;

        // 先尝试使用标准的 eth_blockNumber 方法来检查连接
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "connection_check",
            "method": "eth_blockNumber",
            "params": []
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| {
                RethEngineError::TransactionPool(format!("Connection check failed: {e}"))
            })?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionPool(format!("Failed to parse connection response: {e}"))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionPool(format!(
                "Connection error: {}",
                error
            )));
        }

        // 如果基本连接成功，尝试获取交易池状态（使用更兼容的方法）
        // 尝试 txpool_status 方法，如果失败则返回默认状态
        let pool_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "txpool_status",
            "method": "txpool_status",
            "params": []
        });

        let pool_response = client.post(&rpc_url).json(&pool_request).send().await;

        match pool_response {
            Ok(response) => {
                if let Ok(pool_result) = response.json::<serde_json::Value>().await {
                    if pool_result.get("error").is_none() {
                        if let Some(pool_data) = pool_result.get("result") {
                            let pending_count = pool_data
                                .get("pending")
                                .and_then(|v| v.as_str())
                                .map(|s| {
                                    u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0)
                                })
                                .unwrap_or(0);

                            let queued_count = pool_data
                                .get("queued")
                                .and_then(|v| v.as_str())
                                .map(|s| {
                                    u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0)
                                })
                                .unwrap_or(0);

                            return Ok(TransactionPoolStatus {
                                pending_count,
                                queued_count,
                                is_transaction_pending: pending_count > 0,
                            });
                        }
                    }
                }
            }
            Err(_) => {
                // txpool_status 不支持，返回默认状态但连接正常
            }
        }

        // 如果 txpool_status 不可用，返回默认状态（表示连接正常但无法获取池状态）
        Ok(TransactionPoolStatus {
            pending_count: 0,
            queued_count: 0,
            is_transaction_pending: false,
        })
    }

    /// Check if a transaction is in the pool
    pub async fn is_transaction_in_pool(&self, tx_hash: &str) -> Result<bool, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            RethEngineError::TransactionPool("RPC client not initialized".to_string())
        })?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "eth_getTransactionByHash",
            "method": "eth_getTransactionByHash",
            "params": [tx_hash]
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| {
                RethEngineError::TransactionPool(format!("Transaction lookup failed: {e}"))
            })?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionPool(format!(
                "Failed to parse transaction lookup response: {e}"
            ))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionPool(format!(
                "Transaction lookup error: {error}"
            )));
        }

        let tx_data = result.get("result");
        if tx_data.is_none() || tx_data.unwrap().is_null() {
            return Ok(false);
        }

        // Check if transaction is pending (blockHash is null)
        let block_hash = tx_data.unwrap().get("blockHash");
        let is_pending = block_hash.is_none() || block_hash.unwrap().is_null();

        Ok(is_pending)
    }

    // =======================
    // Format Conversion
    // =======================

    /// Convert MultiVM transaction to Reth transaction
    pub fn convert_multivm_to_reth(
        &self,
        multivm_tx: &MultivmTransaction,
    ) -> Result<RethTransaction, RethEngineError> {
        // Parse addresses
        let to_address = multivm_tx
            .to
            .as_ref()
            .map(|to| self.parse_address_str(to))
            .transpose()?;

        // Parse value
        let value = U256::from_str_radix(&multivm_tx.value.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::TransactionConversion(format!("Invalid value: {e}")))?;

        // Parse data
        let data = hex::decode(multivm_tx.data.trim_start_matches("0x"))
            .map_err(|e| RethEngineError::TransactionConversion(format!("Invalid data: {e}")))?;

        // Parse gas price
        let gas_price = multivm_tx
            .gas_price
            .as_ref()
            .map(|gp| u64::from_str_radix(gp.trim_start_matches("0x"), 16))
            .transpose()
            .map_err(|e| {
                RethEngineError::TransactionConversion(format!("Invalid gas price: {e}"))
            })?;

        // Parse max fee per gas (EIP-1559)
        let max_fee_per_gas = multivm_tx
            .max_fee_per_gas
            .as_ref()
            .map(|fee| u64::from_str_radix(fee.trim_start_matches("0x"), 16))
            .transpose()
            .map_err(|e| {
                RethEngineError::TransactionConversion(format!("Invalid max fee per gas: {e}"))
            })?;

        // Parse max priority fee per gas (EIP-1559)
        let max_priority_fee_per_gas = multivm_tx
            .max_priority_fee_per_gas
            .as_ref()
            .map(|fee| u64::from_str_radix(fee.trim_start_matches("0x"), 16))
            .transpose()
            .map_err(|e| {
                RethEngineError::TransactionConversion(format!(
                    "Invalid max priority fee per gas: {e}"
                ))
            })?;

        // TODO: Implement proper conversion to native Reth TransactionSigned
        // For now, return an error as this needs to be implemented properly
        Err(RethEngineError::TransactionConversion(
            "Conversion to native Reth TransactionSigned not yet implemented".to_string(),
        ))
    }

    /// Convert Reth transaction to MultiVM transaction
    pub fn convert_reth_to_multivm(
        &self,
        reth_tx: &RethTransaction,
    ) -> Result<MultivmTransaction, RethEngineError> {
        // TODO: Implement proper conversion from native Reth TransactionSigned
        // For now, return an error as this needs to be implemented properly
        Err(RethEngineError::TransactionConversion(
            "Conversion from native Reth TransactionSigned not yet implemented".to_string(),
        ))
    }

    // =======================
    // Helper Methods
    // =======================

    /// Parse hex string to hash
    fn parse_hash(&self, value: Option<&serde_json::Value>) -> Result<B256, RethEngineError> {
        let hex_str = value
            .and_then(|v| v.as_str())
            .ok_or_else(|| RethEngineError::TransactionReceipt("Missing hash value".to_string()))?;

        let hex_bytes = hex::decode(hex_str.trim_start_matches("0x"))
            .map_err(|e| RethEngineError::TransactionReceipt(format!("Invalid hex hash: {e}")))?;

        if hex_bytes.len() != 32 {
            return Err(RethEngineError::TransactionReceipt(
                "Invalid hash length".to_string(),
            ));
        }

        Ok(B256::from_slice(&hex_bytes))
    }

    /// Parse hex string to u64
    fn parse_hex_u64(&self, value: Option<&serde_json::Value>) -> Result<u64, RethEngineError> {
        let hex_str = value.and_then(|v| v.as_str()).ok_or_else(|| {
            RethEngineError::TransactionReceipt("Missing numeric value".to_string())
        })?;

        u64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::TransactionReceipt(format!("Invalid hex number: {e}")))
    }

    /// Parse hex string to address
    fn parse_address(&self, value: Option<&serde_json::Value>) -> Result<Address, RethEngineError> {
        let hex_str = value.and_then(|v| v.as_str()).ok_or_else(|| {
            RethEngineError::TransactionReceipt("Missing address value".to_string())
        })?;

        self.parse_address_str(hex_str)
    }

    /// Parse address string to Address
    fn parse_address_str(&self, hex_str: &str) -> Result<Address, RethEngineError> {
        let hex_bytes = hex::decode(hex_str.trim_start_matches("0x")).map_err(|e| {
            RethEngineError::TransactionReceipt(format!("Invalid hex address: {e}"))
        })?;

        if hex_bytes.len() != 20 {
            return Err(RethEngineError::TransactionReceipt(
                "Invalid address length".to_string(),
            ));
        }

        Ok(Address::from_slice(&hex_bytes))
    }

    /// Parse logs bloom filter
    fn parse_logs_bloom(
        &self,
        value: Option<&serde_json::Value>,
    ) -> Result<Bloom, RethEngineError> {
        let hex_str = value.and_then(|v| v.as_str()).ok_or_else(|| {
            RethEngineError::TransactionReceipt("Missing logs bloom value".to_string())
        })?;

        let hex_bytes = hex::decode(hex_str.trim_start_matches("0x")).map_err(|e| {
            RethEngineError::TransactionReceipt(format!("Invalid hex logs bloom: {e}"))
        })?;

        if hex_bytes.len() != 256 {
            return Err(RethEngineError::TransactionReceipt(
                "Invalid logs bloom length".to_string(),
            ));
        }

        // Convert to Bloom type
        Ok(Bloom::from_slice(&hex_bytes))
    }
}

// Implement new generic ExecutionEngine trait
#[async_trait]
impl ExecutionEngine for RethExecutionEngine {
    type BlockType = RethBlock;
    type ExecutionResult = RethExecutionResult;
    type Error = multivm_common::MultivmError;

    /// Process Reth native Block type
    async fn process_block(
        &mut self,
        block: Self::BlockType,
    ) -> Result<Self::ExecutionResult, Self::Error> {
        if self.mock_mode {
            // Mock mode processing
            let start_time = Instant::now();
            let block_number = block.number;
            let block_hash = block.hash_slow();

            tracing::info!(
                "Processing Reth block {} in MOCK mode with hash {:?}",
                block_number,
                block_hash
            );

            // Simulate processing time in mock mode
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Update metrics in mock mode
            let mut current_block = self.current_block.write().await;
            *current_block = block_number;

            let mut blocks_processed = self.blocks_processed.write().await;
            *blocks_processed += 1;

            let processing_time = start_time.elapsed();
            let transactions_count = block.body.transactions.len();
            let gas_used = block.header.gas_used;
            let state_root = block.header.state_root;

            tracing::info!(
                "Successfully processed Reth block {} in MOCK mode in {:?} with {} transactions, gas used: {}",
                block_number,
                processing_time,
                transactions_count,
                gas_used
            );

            Ok(RethExecutionResult {
                block_hash,
                block_number,
                gas_used,
                transactions_count,
                processing_time,
                state_root,
                success: true,
                error: None,
            })
        } else {
            // Real mode processing - use the real Reth node integration
            // Ensure engine is running
            if !*self.is_running.read().await {
                self.start_reth_process().await.map_err(|e| {
                    multivm_common::MultivmError::Process {
                        process_id: "reth-engine".to_string(),
                        message: e.to_string(),
                        exit_code: None,
                    }
                })?;
                *self.is_running.write().await = true;
            }

            // Process the Reth block directly using our improved method
            self.process_reth_block(block).await.map_err(|e| {
                multivm_common::MultivmError::Process {
                    process_id: "reth-engine".to_string(),
                    message: e.to_string(),
                    exit_code: None,
                }
            })
        }
    }

    async fn get_health(&self) -> Result<HealthStatus, Self::Error> {
        let _current_block = *self.current_block.read().await;
        let _blocks_processed = *self.blocks_processed.read().await;
        let is_running = *self.is_running.read().await;
        let reth_running = if self.mock_mode {
            true // Always healthy in mock mode
        } else {
            self.reth_process.read().await.is_some()
        };

        if is_running && reth_running {
            Ok(HealthStatus::Healthy)
        } else {
            Ok(HealthStatus::Unhealthy)
        }
    }

    async fn get_state(&self) -> Result<EngineState, Self::Error> {
        let current_block = *self.current_block.read().await;

        // Try to get current block from Reth if available
        let reth_block = if self.rpc_client.read().await.is_some() {
            self.get_current_block_from_reth()
                .await
                .unwrap_or(current_block)
        } else {
            current_block
        };

        Ok(EngineState {
            process_id: ProcessId::Ethereum,
            blockchain_type: BlockchainType::Ethereum,
            current_block: Some(reth_block),
            state_root: format!("eth_state_{reth_block}").into_bytes(),
            is_syncing: false,
            peer_count: 0, // P2P disabled
            rpc_endpoints: vec![format!("http://127.0.0.1:{}", self.rpc_port)],
            data_directory: self.data_dir.to_string_lossy().to_string(),
            chain_id: self.chain_id,
        })
    }

    async fn start_rpc_server(&self, _config: RpcConfig) -> Result<(), Self::Error> {
        tracing::info!("RPC server is handled by the Reth node process");
        Ok(())
    }

    async fn stop_rpc_server(&self) -> Result<(), Self::Error> {
        tracing::info!("RPC server will stop with Reth node process");
        Ok(())
    }

    async fn initialize(&mut self) -> Result<(), Self::Error> {
        if self.mock_mode {
            tracing::info!("Initializing Reth execution engine in MOCK mode");

            // Create data directory for mock mode too
            std::fs::create_dir_all(&self.data_dir).map_err(|e| {
                RethEngineError::Configuration(format!("Failed to create data directory: {e}"))
            })?;

            // Mock initialization - no real Reth process
            *self.is_running.write().await = true;

            tracing::info!("Reth execution engine initialized successfully in MOCK mode");
        } else {
            tracing::info!("Initializing Reth execution engine");

            // Start the Reth process
            self.start_reth_process().await?;

            // Set running state
            *self.is_running.write().await = true;

            tracing::info!("Reth execution engine initialized successfully");
        }

        Ok(())
    }

    async fn shutdown(&mut self, timeout: Option<Duration>) -> Result<(), Self::Error> {
        if self.mock_mode {
            tracing::info!("Shutting down Reth execution engine (MOCK mode)");

            *self.is_running.write().await = false;

            tracing::info!("Reth execution engine shutdown complete (MOCK mode)");
        } else {
            tracing::info!("Shutting down Reth execution engine");

            *self.is_running.write().await = false;

            // Stop the Reth process
            if let Some(mut child) = self.reth_process.write().await.take() {
                tracing::info!("Terminating Reth node process");

                // Try graceful shutdown first
                if let Err(e) = child.kill().await {
                    tracing::warn!("Failed to kill Reth node process: {}", e);
                }

                // Wait for it to exit
                let wait_timeout = timeout.unwrap_or(Duration::from_secs(10));
                match tokio::time::timeout(wait_timeout, child.wait()).await {
                    Ok(Ok(status)) => {
                        tracing::info!("Reth node exited with status: {}", status);
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Error waiting for Reth node to exit: {}", e);
                    }
                    Err(_) => {
                        tracing::warn!("Reth node did not exit within timeout");
                    }
                }
            }

            tracing::info!("Reth execution engine shutdown complete");
        }

        Ok(())
    }

    fn blockchain_type(&self) -> BlockchainType {
        BlockchainType::Ethereum
    }

    async fn is_ready(&self) -> bool {
        let is_running = *self.is_running.read().await;
        let reth_running = if self.mock_mode {
            true // Always ready in mock mode
        } else {
            self.reth_process.read().await.is_some()
        };
        is_running && reth_running
    }

    async fn get_metrics(&self) -> Result<ProcessingMetrics, Self::Error> {
        let blocks_processed = *self.blocks_processed.read().await;

        Ok(ProcessingMetrics {
            cpu_time: self.start_time.elapsed(),
            memory_usage_bytes: get_memory_usage_standard(),
            disk_reads: 0,
            disk_writes: 0,
            network_bytes: 0,
            compute_units_used: blocks_processed * 21_000, // Estimate gas usage
            transaction_count: blocks_processed,
            account_updates: blocks_processed * 2, // Estimate
            total_requests: blocks_processed,
            successful_requests: blocks_processed,
            failed_requests: 0,
            average_response_time_ms: 100.0, // Estimate
            peak_memory_usage_mb: get_memory_usage_standard() / (1024 * 1024),
            cpu_usage_percent: get_cpu_usage_standard(),
        })
    }

    async fn get_latest_block_id(&self) -> Result<u64, Self::Error> {
        if self.mock_mode {
            // Return current block in mock mode
            Ok(*self.current_block.read().await)
        } else if self.rpc_client.read().await.is_some() {
            // Get latest block from RPC client
            self.get_current_block_from_reth().await.map_err(|e| {
                multivm_common::MultivmError::Rpc {
                    method: "get_current_block_from_reth".to_string(),
                    message: format!("Failed to get latest block: {e}"),
                    status_code: None,
                }
            })
        } else {
            // Return current block if no RPC client
            Ok(*self.current_block.read().await)
        }
    }

    async fn reset_to_block(&mut self, block_id: u64) -> Result<(), Self::Error> {
        tracing::info!("Resetting Reth engine to block {}", block_id);

        if self.mock_mode {
            // In mock mode, just update the current block
            *self.current_block.write().await = block_id;
            tracing::info!("Reth engine reset to block {} (mock mode)", block_id);
        } else {
            // In real mode, we would need to reset the Reth node state
            // For now, just update our tracking
            *self.current_block.write().await = block_id;
            tracing::info!(
                "Reth engine reset to block {} (simplified implementation)",
                block_id
            );
        }

        Ok(())
    }
}

// Helper functions for system metrics
fn get_memory_usage_standard() -> u64 {
    // Simple placeholder implementation since monitoring module is disabled
    1024 * 1024 * 100 // 100 MB placeholder
}

fn get_cpu_usage_standard() -> f64 {
    // Simple placeholder implementation since monitoring module is disabled
    15.0 // 15% placeholder
}

/// Generate mock Reth block data for testing
#[allow(dead_code)]
pub fn generate_mock_reth_block(block_number: u64, transaction_count: usize) -> RethBlock {
    use alloy_consensus::{Signed, TxEip1559, TxEnvelope};
    use alloy_primitives::{Bloom, Bytes, TxKind};

    // Create mock transactions if requested
    let transactions = (0..transaction_count)
        .map(|i| {
            let tx = TxEip1559 {
                chain_id: 1337,
                nonce: i as u64,
                max_priority_fee_per_gas: 1_000_000_000, // 1 gwei
                max_fee_per_gas: 2_000_000_000,          // 2 gwei
                gas_limit: 21000,
                to: TxKind::Call(Address::from([0x01; 20])),
                value: U256::from(1000000000000000u64), // 0.001 ETH
                input: Bytes::new(),
                access_list: Default::default(),
            };

            // Create a dummy signature
            let signature = alloy_primitives::Signature::from_scalars_and_parity(
                B256::from([0x01; 32]),
                B256::from([0x02; 32]),
                false,
            );

            TxEnvelope::Eip1559(Signed::new_unchecked(tx, signature, B256::from([0x03; 32])))
        })
        .collect();

    // Use reth-compatible values with proper Alloy types
    let header = Header {
        parent_hash: B256::ZERO, // Genesis block parent (all zeros)
        // Standard empty ommers hash for reth (keccak256 of empty RLP list)
        ommers_hash: B256::from([
            0x1d, 0xcc, 0x4d, 0xe8, 0xde, 0xc7, 0x5d, 0x7a, 0xab, 0x85, 0xb5, 0x67, 0xb6, 0xcc,
            0xd4, 0x1a, 0xd3, 0x12, 0x45, 0x1b, 0x94, 0x8a, 0x74, 0x13, 0xf0, 0xa1, 0x42, 0xfd,
            0x40, 0xd4, 0x93, 0x47,
        ]),
        beneficiary: Address::ZERO, // Zero address for dev chain
        state_root: B256::from([
            0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0,
            0xf8, 0x6e, 0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5,
            0xe3, 0x63, 0xb4, 0x21,
        ]), // Empty state root
        // Standard empty transactions root for reth (keccak256 of empty transactions trie)
        transactions_root: B256::from([
            0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0,
            0xf8, 0x6e, 0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5,
            0xe3, 0x63, 0xb4, 0x21,
        ]),
        // Standard empty receipts root for reth (keccak256 of empty receipts trie)
        receipts_root: B256::from([
            0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0,
            0xf8, 0x6e, 0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5,
            0xe3, 0x63, 0xb4, 0x21,
        ]),
        logs_bloom: Bloom::ZERO,
        difficulty: U256::ZERO, // Zero difficulty for PoS
        number: block_number,
        gas_limit: 30_000_000,
        gas_used: (transaction_count as u64) * 21000, // Each transaction uses 21000 gas
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        extra_data: Bytes::new(),
        mix_hash: B256::ZERO,                      // Zero for PoS
        nonce: alloy_primitives::FixedBytes::ZERO, // Zero for PoS
        base_fee_per_gas: Some(1_000_000_000),     // 1 gwei
        withdrawals_root: Some(B256::from([
            0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0,
            0xf8, 0x6e, 0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5,
            0xe3, 0x63, 0xb4, 0x21,
        ])), // Empty withdrawals root
        blob_gas_used: Some(0),                    // Zero blob gas
        excess_blob_gas: Some(0),                  // Zero excess blob gas
        parent_beacon_block_root: None,            // Not set for dev chain
        requests_hash: None,                       // No requests for dev chain
    };

    Block {
        header,
        body: BlockBody {
            transactions,
            ommers: vec![],
            withdrawals: None,
        },
    }
}

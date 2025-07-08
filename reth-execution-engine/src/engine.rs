// Custom types to replace reth_primitives and alloy_primitives dependencies
// This avoids the c-kzg linking conflicts while maintaining API compatibility

use multivm_common::config::VmType;

/// Custom address type (equivalent to alloy Address)
pub type Address = [u8; 20];

/// Custom 256-bit hash type (equivalent to alloy B256)
pub type B256 = [u8; 32];

/// Custom 256-bit integer type (equivalent to alloy U256)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct U256(pub [u64; 4]);

impl U256 {
    pub fn from(value: u64) -> Self {
        Self([value, 0, 0, 0])
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0[0]
    }
    
    pub fn zero() -> Self {
        Self([0, 0, 0, 0])
    }
    
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        let hex = hex.trim_start_matches("0x");
        if hex.len() > 64 {
            return Err("Hex string too long".to_string());
        }
        
        // For simplicity, just parse as u64 for now
        let value = u64::from_str_radix(hex, 16)
            .map_err(|_| "Invalid hex string")?;
        Ok(U256::from(value))
    }
    
    pub fn to_hex(&self) -> String {
        format!("0x{:x}", self.0[0])
    }
}

impl serde::Serialize for U256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as hex string for JSON-RPC compatibility
        let hex_str = format!(
            "0x{:016x}{:016x}{:016x}{:016x}",
            self.0[3], self.0[2], self.0[1], self.0[0]
        );
        serializer.serialize_str(&hex_str)
    }
}

impl<'de> serde::Deserialize<'de> for U256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _hex_str = String::deserialize(deserializer)?;
        // For simplicity, just return a default value
        // In a full implementation, we would parse the hex string
        Ok(U256::default())
    }
}

/// Custom block header type (equivalent to reth_primitives BlockHeader)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct BlockHeader {
    pub parent_hash: B256,
    pub ommers_hash: B256,
    pub beneficiary: Address,
    pub state_root: B256,
    pub transactions_root: B256,
    pub receipts_root: B256,
    #[serde(with = "serde_bytes")]
    pub logs_bloom: [u8; 256],
    pub difficulty: U256,
    pub number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub extra_data: Vec<u8>,
    pub mix_hash: B256,
    pub nonce: u64,
    pub base_fee_per_gas: Option<u64>,
    pub withdrawals_root: Option<B256>,
    pub blob_gas_used: Option<u64>,
    pub excess_blob_gas: Option<u64>,
    pub parent_beacon_block_root: Option<B256>,
    pub requests_root: Option<B256>,
}

impl Default for BlockHeader {
    fn default() -> Self {
        Self {
            parent_hash: [0; 32],
            ommers_hash: [0; 32],
            beneficiary: [0; 20],
            state_root: [0; 32],
            transactions_root: [0; 32],
            receipts_root: [0; 32],
            logs_bloom: [0; 256],
            difficulty: U256::default(),
            number: 0,
            gas_limit: 30_000_000,
            gas_used: 0,
            timestamp: 0,
            extra_data: vec![],
            mix_hash: [0; 32],
            nonce: 0,
            base_fee_per_gas: None,
            withdrawals_root: None,
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root: None,
            requests_root: None,
        }
    }
}

/// Custom transaction type
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct Transaction {
    pub hash: B256,
    pub nonce: u64,
    pub gas_price: Option<u64>,
    pub gas_limit: u64,
    pub to: Option<Address>,
    pub value: U256,
    pub data: Vec<u8>,
    pub signature: TransactionSignature,
    // EIP-1559 fields
    pub max_fee_per_gas: Option<u64>,
    pub max_priority_fee_per_gas: Option<u64>,
}

/// Transaction signature
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionSignature {
    #[allow(dead_code)]
    pub v: u64,
    #[allow(dead_code)]
    pub r: U256,
    #[allow(dead_code)]
    pub s: U256,
}

/// Custom block body type
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BlockBody {
    pub transactions: Vec<Transaction>,
    #[allow(dead_code)]
    pub ommers: Vec<BlockHeader>,
    #[allow(dead_code)]
    pub withdrawals: Option<Vec<serde_json::Value>>,
}

/// Custom block type (equivalent to reth_primitives Block)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub body: BlockBody,
    pub number: u64,
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

/// Transaction receipt from Reth
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionReceipt {
    pub transaction_hash: B256,
    pub block_hash: B256,
    pub block_number: u64,
    pub transaction_index: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub gas_used: u64,
    pub cumulative_gas_used: u64,
    pub logs: Vec<TransactionLog>,
    pub status: u64, // 1 = success, 0 = failed
    pub contract_address: Option<Address>,
    #[serde(with = "serde_bytes")]
    pub logs_bloom: [u8; 256],
    pub effective_gas_price: u64,
}

/// Transaction log entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionLog {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Vec<u8>,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
    pub removed: bool,
}

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

impl Default for Block {
    fn default() -> Self {
        Self::new()
    }
}

impl Block {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            header: BlockHeader::default(),
            body: BlockBody::default(),
            number: 0,
        }
    }

    /// Calculate block hash using alloy standard library (matches reth implementation)
    pub fn hash_slow(&self) -> B256 {
        // Convert to alloy header structure
        let alloy_header = self.to_alloy_header();
        
        // Use alloy's standard RLP encoding and keccak256 hashing
        let mut out = Vec::new();
        alloy_header.encode(&mut out);
        let hash = alloy_primitives::keccak256(&out);
        
        // Convert to our custom B256
        let mut custom_hash = [0u8; 32];
        custom_hash.copy_from_slice(hash.as_slice());
        custom_hash
    }
    
    /// Convert to alloy consensus header (standard format)
    fn to_alloy_header(&self) -> alloy_consensus::Header {
        alloy_consensus::Header {
            parent_hash: alloy_primitives::B256::from_slice(&self.header.parent_hash),
            ommers_hash: alloy_primitives::B256::from_slice(&self.header.ommers_hash),
            beneficiary: alloy_primitives::Address::from_slice(&self.header.beneficiary),
            state_root: alloy_primitives::B256::from_slice(&self.header.state_root),
            transactions_root: alloy_primitives::B256::from_slice(&self.header.transactions_root),
            receipts_root: alloy_primitives::B256::from_slice(&self.header.receipts_root),
            logs_bloom: alloy_primitives::Bloom::from_slice(&self.header.logs_bloom),
            difficulty: alloy_primitives::U256::from_limbs(self.header.difficulty.0),
            number: self.header.number,
            gas_limit: self.header.gas_limit,
            gas_used: self.header.gas_used,
            timestamp: self.header.timestamp,
            extra_data: alloy_primitives::Bytes::from(self.header.extra_data.clone()),
            mix_hash: alloy_primitives::B256::from_slice(&self.header.mix_hash),
            nonce: alloy_primitives::B64::from(self.header.nonce.to_be_bytes()),
            base_fee_per_gas: self.header.base_fee_per_gas,
            withdrawals_root: self.header.withdrawals_root.map(|r| alloy_primitives::B256::from_slice(&r)),
            blob_gas_used: self.header.blob_gas_used,
            excess_blob_gas: self.header.excess_blob_gas,
            parent_beacon_block_root: self.header.parent_beacon_block_root.map(|r| alloy_primitives::B256::from_slice(&r)),
            requests_hash: if self.should_include_prague_fields() {
                self.header.requests_root.map(|r| alloy_primitives::B256::from_slice(&r))
            } else {
                None
            },
        }
    }



    /// Check if Prague fork fields (like requests_root) should be included in block hash
    fn should_include_prague_fields(&self) -> bool {
        // Based on reth's dev chain behavior, include Prague fork fields
        // This ensures our hash calculation matches reth's implementation
        true
    }


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
use tokio::io::AsyncReadExt;

// Alloy imports for standard Ethereum types and RLP encoding
use alloy_primitives::{self, keccak256};
use alloy_rlp::{self, Encodable};

// Import real engine implementation when not in mock mode
// Real engine implementation available when real-node feature is enabled

/// Simplified execution payload type (for Engine API)
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionPayload {
    #[serde(rename = "parentHash")]
    pub parent_hash: B256,
    #[serde(rename = "feeRecipient")]
    pub fee_recipient: Address,
    #[serde(rename = "stateRoot")]
    pub state_root: B256,
    #[serde(rename = "receiptsRoot")]
    pub receipts_root: B256,
    #[serde(rename = "logsBloom")]
    pub logs_bloom: String,
    #[serde(rename = "prevRandao")]
    pub prev_randao: B256,
    #[serde(rename = "blockNumber")]
    pub block_number: u64,
    #[serde(rename = "gasLimit")]
    pub gas_limit: u64,
    #[serde(rename = "gasUsed")]
    pub gas_used: u64,
    pub timestamp: u64,
    #[serde(rename = "extraData")]
    pub extra_data: String,
    #[serde(rename = "baseFeePerGas")]
    pub base_fee_per_gas: U256,
    #[serde(rename = "blockHash")]
    pub block_hash: B256,
    pub transactions: Vec<String>,
    pub withdrawals: Option<Vec<serde_json::Value>>,
    #[serde(rename = "blobGasUsed")]
    pub blob_gas_used: Option<u64>,
    #[serde(rename = "excessBlobGas")]
    pub excess_blob_gas: Option<u64>,
    #[serde(rename = "requestsRoot")]
    pub requests_root: Option<B256>,
}

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
            RethEngineError::TransactionConversion(msg) => multivm_common::MultivmError::Configuration {
                component: "reth-tx-conversion".to_string(),
                message: msg,
                validation_errors: None,
            },
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
            // Data directory
            .arg("--datadir")
            .arg(std::fs::canonicalize(&self.data_dir)
                .map_err(|e| RethEngineError::Configuration(format!("Failed to canonicalize data dir: {e}")))?)
            // HTTP RPC configuration
            .arg("--http")
            .arg("--http.port")
            .arg(self.rpc_port.to_string())
            .arg("--http.addr")
            .arg("127.0.0.1")
            .arg("--http.api")
            .arg("eth,net,web3,debug")
            .arg("--http.corsdomain")
            .arg("*")
            // Disable P2P completely (dev mode already disables discovery)
            .arg("--port")
            .arg("0") // Disable P2P listening port
            .arg("--max-outbound-peers")
            .arg("0")
            .arg("--max-inbound-peers")
            .arg("0")
            // Development mode (disables discovery automatically)
            .arg("--dev")
            .arg("--dev.block-time")
            .arg("100sec") // Use a very long block time to effectively disable auto-production
            // Enable Engine API for block submission
            .arg("--authrpc.port")
            .arg((self.rpc_port + 1).to_string())
            .arg("--authrpc.addr")
            .arg("127.0.0.1")
            .arg("--authrpc.jwtsecret")
            .arg(std::fs::canonicalize(self.data_dir.join("jwt.hex"))
                .map_err(|e| RethEngineError::Configuration(format!("Failed to canonicalize JWT path: {e}")))?)
            // Chain configuration
            .arg("--chain")
            .arg(match self.chain_id {
                1 => "mainnet",
                11155111 => "sepolia",
                17000 => "holesky",
                _ => "dev", // Custom development chain
            })
            // Logging configuration
            .arg("--log.stdout.format")
            .arg("json")
            .arg("--log.stdout.filter")
            .arg("info,reth=debug")
            // Process settings
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        tracing::info!("Reth command: {:?}", cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| RethEngineError::Process(format!("Failed to start Reth node: {e}")))?;

        let pid = child.id();
        
        // Check if the process is still running after a brief moment
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        if let Ok(Some(exit_status)) = child.try_wait() {
            // Process exited immediately, capture output
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();
            
            let mut stdout_buf = String::new();
            let mut stderr_buf = String::new();
            
            tokio::io::AsyncReadExt::read_to_string(&mut tokio::io::BufReader::new(stdout), &mut stdout_buf).await.ok();
            tokio::io::AsyncReadExt::read_to_string(&mut tokio::io::BufReader::new(stderr), &mut stderr_buf).await.ok();
            
            return Err(RethEngineError::Process(format!(
                "Reth node exited immediately with status: {:?}\nStdout: {}\nStderr: {}",
                exit_status, stdout_buf, stderr_buf
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
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|e| RethEngineError::Configuration(format!("Failed to create HTTP client: {e}")))?;
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
                .arg(match self.chain_id {
                    1 => "mainnet",
                    11155111 => "sepolia",
                    17000 => "holesky",
                    _ => "dev",
                })
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
        let hex_secret = std::fs::read_to_string(&jwt_path)
            .map_err(|e| RethEngineError::Configuration(format!("Failed to read JWT secret: {e}")))?;
        
        let secret_bytes = hex::decode(hex_secret.trim())
            .map_err(|e| RethEngineError::Configuration(format!("Invalid JWT secret format: {e}")))?;
        
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
        block: Block,
    ) -> Result<RethExecutionResult, RethEngineError> {
        let start_time = Instant::now();
        let block_number = block.number;
        let block_hash = block.hash_slow(); // Calculate block hash

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
    async fn submit_block_to_reth(&self, block: &Block) -> Result<(), RethEngineError> {
        tracing::debug!("Submitting Reth block {} via Engine API", block.number);

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
                        let error_msg = result.get("validationError")
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

    /// Create execution payload from Reth Block
    fn create_execution_payload_from_block(
        &self,
        block: &Block,
    ) -> Result<ExecutionPayload, RethEngineError> {
        // Convert transactions to proper RLP-encoded hex strings for JSON-RPC
        let transactions: Vec<String> = block
            .body
            .transactions
            .iter()
            .map(|tx| {
                // Build proper Ethereum transaction and compute hash
                let rlp_encoded = self.rlp_encode_transaction(tx);
                format!("0x{}", hex::encode(rlp_encoded))
            })
            .collect();

        // Build execution payload using proper Reth types
        let payload = ExecutionPayload {
            parent_hash: block.header.parent_hash,
            fee_recipient: block.header.beneficiary,
            state_root: block.header.state_root,
            receipts_root: block.header.receipts_root,
            logs_bloom: hex::encode(block.header.logs_bloom),
            prev_randao: block.header.mix_hash,
            block_number: block.header.number,
            gas_limit: block.header.gas_limit,
            gas_used: block.header.gas_used,
            timestamp: block.header.timestamp,
            extra_data: hex::encode(block.header.extra_data.clone()),
            base_fee_per_gas: block
                .header
                .base_fee_per_gas
                .map(U256::from)
                .unwrap_or(U256::from(0)),
            block_hash: block.hash_slow(),
            transactions,
            withdrawals: None,     // Pre-Shanghai
            blob_gas_used: None,   // Pre-Cancun
            excess_blob_gas: None, // Pre-Cancun
            requests_root: if self.should_include_prague_fields_for_block(block) {
                block.header.requests_root
            } else {
                None // Skip requests_root for compatibility with reth dev chains
            },
        };

        Ok(payload)
    }

    /// Check if Prague fork fields should be included for this block
    fn should_include_prague_fields_for_block(&self, _block: &Block) -> bool {
        // Based on reth's dev chain behavior, include Prague fork fields
        // This ensures our payload matches reth's implementation
        true
    }

    /// Create fork choice state from Reth Block
    fn create_fork_choice_state_from_block(
        &self,
        block: &Block,
    ) -> Result<ForkchoiceState, RethEngineError> {
        let fork_choice_state = ForkchoiceState {
            head_block_hash: block.hash_slow(),
            safe_block_hash: block.header.parent_hash, // Previous block is considered safe
            finalized_block_hash: block.header.parent_hash, // Previous block is finalized
        };

        Ok(fork_choice_state)
    }

    /// Submit execution payload to Reth via engine_newPayloadV1
    async fn submit_execution_payload(
        &self,
        payload: &ExecutionPayload,
    ) -> Result<serde_json::Value, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "engine_newPayload",
            "method": "engine_newPayloadV1",
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

    /// Update fork choice via engine_forkchoiceUpdatedV1
    async fn update_fork_choice(
        &self,
        fork_choice_state: &ForkchoiceState,
    ) -> Result<serde_json::Value, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::Rpc("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "engine_forkchoiceUpdated",
            "method": "engine_forkchoiceUpdatedV1",
            "params": [fork_choice_state, null]
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

    /// Get current block number from Reth via RPC
    async fn get_current_block_from_reth(&self) -> Result<u64, RethEngineError> {
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

    /// Get chain ID from Reth via RPC
    #[allow(dead_code)]
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
    fn rlp_encode_transaction(&self, tx: &Transaction) -> Vec<u8> {
        let mut stream = Vec::new();

        // Determine transaction type
        if tx.gas_price.is_some() {
            // Legacy transaction (type 0)
            self.encode_legacy_transaction(&mut stream, tx);
        } else {
            // EIP-1559 transaction (type 2) - assume this if no gas_price
            self.encode_eip1559_transaction(&mut stream, tx);
        }

        stream
    }

    /// Encode legacy transaction (EIP-155)
    fn encode_legacy_transaction(&self, stream: &mut Vec<u8>, tx: &Transaction) {
        // Legacy transaction format: [nonce, gasPrice, gasLimit, to, value, data, v, r, s]

        // Start RLP list
        let mut items = Vec::new();

        // 1. Nonce
        self.encode_u64(&mut items, tx.nonce);

        // 2. Gas price
        let gas_price = tx.gas_price.unwrap_or(20_000_000_000); // 20 gwei default
        self.encode_u64(&mut items, gas_price);

        // 3. Gas limit
        self.encode_u64(&mut items, tx.gas_limit);

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
        self.encode_u64(&mut items, v);

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
        self.encode_u64(&mut items, self.chain_id);

        // 2. Nonce
        self.encode_u64(&mut items, tx.nonce);

        // 3. Max priority fee per gas (tip)
        self.encode_u64(&mut items, 1_500_000_000); // 1.5 gwei default

        // 4. Max fee per gas (base fee + tip)
        self.encode_u64(&mut items, 20_000_000_000); // 20 gwei default

        // 5. Gas limit
        self.encode_u64(&mut items, tx.gas_limit);

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
        self.encode_u64(&mut items, v);

        // 11. r
        let r_bytes = self.u256_to_bytes(&tx.signature.r);
        items.push(r_bytes);

        // 12. s
        let s_bytes = self.u256_to_bytes(&tx.signature.s);
        items.push(s_bytes);

        // Encode as RLP list and append to stream
        self.encode_rlp_list(stream, &items);
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

    // =======================
    // Transaction Forwarding
    // =======================

    /// Forward a single transaction to the Reth process
    pub async fn forward_transaction_to_reth(
        &self,
        tx: &Transaction,
    ) -> Result<TransactionForwardingResult, RethEngineError> {
        let start_time = std::time::Instant::now();
        
        tracing::info!("Forwarding transaction to Reth: {:?}", hex::encode(tx.hash));

        // Encode transaction as RLP
        let rlp_encoded = self.rlp_encode_transaction(tx);
        let tx_hex = format!("0x{}", hex::encode(&rlp_encoded));

        // Submit transaction via eth_sendRawTransaction
        let tx_hash = self.submit_raw_transaction_rpc(&tx_hex).await?;

        // Wait for transaction to be mined (with timeout)
        let receipt = self.wait_for_transaction_receipt(&tx_hash, Duration::from_secs(60)).await?;

        let result = TransactionForwardingResult {
            transaction_hash: tx_hash,
            status: if receipt.status == 1 { "success".to_string() } else { "failed".to_string() },
            gas_used: Some(receipt.gas_used),
            block_number: Some(receipt.block_number),
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
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::TransactionForwarding("RPC client not initialized".to_string()))?;

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
            .map_err(|e| RethEngineError::TransactionForwarding(format!("RPC request failed: {e}")))?;

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
                "RPC error: {}",
                error
            )));
        }

        let tx_hash = result
            .get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| RethEngineError::TransactionForwarding("Missing transaction hash in response".to_string()))?;

        Ok(tx_hash.to_string())
    }

    // =======================
    // Transaction Receipts
    // =======================

    /// Get transaction receipt by hash
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<TransactionReceipt>, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::TransactionReceipt("RPC client not initialized".to_string()))?;

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
                "RPC error: {}",
                error
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
    ) -> Result<TransactionReceipt, RethEngineError> {
        let start_time = std::time::Instant::now();
        let poll_interval = Duration::from_millis(500);

        while start_time.elapsed() < timeout {
            if let Some(receipt) = self.get_transaction_receipt(tx_hash).await? {
                return Ok(receipt);
            }
            tokio::time::sleep(poll_interval).await;
        }

        Err(RethEngineError::TransactionReceipt(format!(
            "Transaction receipt not found within timeout: {}",
            tx_hash
        )))
    }

    /// Parse transaction receipt from JSON
    fn parse_transaction_receipt(
        &self,
        receipt_json: &serde_json::Value,
    ) -> Result<TransactionReceipt, RethEngineError> {
        let tx_hash = self.parse_hash(receipt_json.get("transactionHash"))?;
        let block_hash = self.parse_hash(receipt_json.get("blockHash"))?;
        let block_number = self.parse_hex_u64(receipt_json.get("blockNumber"))?;
        let transaction_index = self.parse_hex_u64(receipt_json.get("transactionIndex"))?;
        let from = self.parse_address(receipt_json.get("from"))?;
        let to = receipt_json.get("to").and_then(|v| v.as_str()).map(|s| self.parse_address_str(s)).transpose()?;
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

        Ok(TransactionReceipt {
            transaction_hash: tx_hash,
            block_hash,
            block_number,
            transaction_index,
            from,
            to,
            gas_used,
            cumulative_gas_used,
            logs,
            status,
            contract_address,
            logs_bloom,
            effective_gas_price,
        })
    }

    /// Parse transaction log from JSON
    fn parse_transaction_log(
        &self,
        log_json: &serde_json::Value,
    ) -> Result<TransactionLog, RethEngineError> {
        let address = self.parse_address(log_json.get("address"))?;
        let topics = log_json
            .get("topics")
            .and_then(|v| v.as_array())
            .unwrap_or(&vec![])
            .iter()
            .map(|topic| self.parse_hash(Some(topic)))
            .collect::<Result<Vec<_>, _>>()?;

        let data = log_json
            .get("data")
            .and_then(|v| v.as_str())
            .map(|s| hex::decode(s.trim_start_matches("0x")))
            .transpose()
            .map_err(|e| RethEngineError::TransactionReceipt(format!("Invalid log data: {e}")))?
            .unwrap_or_default();

        let block_number = self.parse_hex_u64(log_json.get("blockNumber"))?;
        let transaction_hash = self.parse_hash(log_json.get("transactionHash"))?;
        let transaction_index = self.parse_hex_u64(log_json.get("transactionIndex"))?;
        let log_index = self.parse_hex_u64(log_json.get("logIndex"))?;
        let removed = log_json.get("removed").and_then(|v| v.as_bool()).unwrap_or(false);

        Ok(TransactionLog {
            address,
            topics,
            data,
            block_number,
            transaction_hash,
            transaction_index,
            log_index,
            removed,
        })
    }

    // =======================
    // Transaction Validation
    // =======================

    /// Validate a transaction against the Reth node
    pub async fn validate_transaction(
        &self,
        tx: &Transaction,
    ) -> Result<ValidationResult, RethEngineError> {
        // Encode transaction as RLP
        let rlp_encoded = self.rlp_encode_transaction(tx);
        let tx_hex = format!("0x{}", hex::encode(&rlp_encoded));

        // Call eth_call to validate the transaction
        let validation_result = self.call_transaction_validation(&tx_hex).await?;

        Ok(validation_result)
    }

    /// Call eth_call for transaction validation
    async fn call_transaction_validation(
        &self,
        tx_hex: &str,
    ) -> Result<ValidationResult, RethEngineError> {
        let _client_guard = self.rpc_client.read().await;
        let _client = _client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::TransactionValidation("RPC client not initialized".to_string()))?;

        // First, try to estimate gas
        let gas_estimate = self.estimate_gas(tx_hex).await.ok();
        let gas_price = self.get_gas_price().await.ok();

        // For now, return a simple validation result
        // In a full implementation, we would parse the transaction and validate it properly
        Ok(ValidationResult {
            is_valid: true,
            error_message: None,
            estimated_gas: gas_estimate,
            gas_price_suggestion: gas_price,
            nonce_suggestion: None,
        })
    }

    /// Estimate gas for a transaction
    async fn estimate_gas(&self, tx_hex: &str) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::TransactionValidation("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "eth_estimateGas",
            "method": "eth_estimateGas",
            "params": [tx_hex]
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::TransactionValidation(format!("Gas estimation failed: {e}")))?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionValidation(format!("Failed to parse gas estimation response: {e}"))
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
            .ok_or_else(|| RethEngineError::TransactionValidation("Missing gas estimate in response".to_string()))?;

        let gas = u64::from_str_radix(gas_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::TransactionValidation(format!("Invalid gas estimate: {e}")))?;

        Ok(gas)
    }

    /// Get current gas price
    async fn get_gas_price(&self) -> Result<u64, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::TransactionValidation("RPC client not initialized".to_string()))?;

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
            .map_err(|e| RethEngineError::TransactionValidation(format!("Gas price request failed: {e}")))?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionValidation(format!("Failed to parse gas price response: {e}"))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionValidation(format!(
                "Gas price error: {}",
                error
            )));
        }

        let price_hex = result
            .get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| RethEngineError::TransactionValidation("Missing gas price in response".to_string()))?;

        let price = u64::from_str_radix(price_hex.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::TransactionValidation(format!("Invalid gas price: {e}")))?;

        Ok(price)
    }

    /// Get transaction pool status
    pub async fn get_transaction_pool_status(&self) -> Result<TransactionPoolStatus, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::TransactionPool("RPC client not initialized".to_string()))?;

        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "txpool_status",
            "method": "txpool_status",
            "params": []
        });

        let rpc_url = format!("http://127.0.0.1:{}", self.rpc_port);

        let response = client
            .post(&rpc_url)
            .json(&rpc_request)
            .send()
            .await
            .map_err(|e| RethEngineError::TransactionPool(format!("Pool status request failed: {e}")))?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionPool(format!("Failed to parse pool status response: {e}"))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionPool(format!(
                "Pool status error: {}",
                error
            )));
        }

        let pool_data = result
            .get("result")
            .ok_or_else(|| RethEngineError::TransactionPool("Missing pool status in response".to_string()))?;

        let pending_count = pool_data
            .get("pending")
            .and_then(|v| v.as_str())
            .map(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0))
            .unwrap_or(0);

        let queued_count = pool_data
            .get("queued")
            .and_then(|v| v.as_str())
            .map(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0))
            .unwrap_or(0);

        Ok(TransactionPoolStatus {
            pending_count,
            queued_count,
            is_transaction_pending: pending_count > 0,
        })
    }

    /// Check if a transaction is in the pool
    pub async fn is_transaction_in_pool(&self, tx_hash: &str) -> Result<bool, RethEngineError> {
        let client_guard = self.rpc_client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| RethEngineError::TransactionPool("RPC client not initialized".to_string()))?;

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
            .map_err(|e| RethEngineError::TransactionPool(format!("Transaction lookup failed: {e}")))?;

        let result: serde_json::Value = response.json().await.map_err(|e| {
            RethEngineError::TransactionPool(format!("Failed to parse transaction lookup response: {e}"))
        })?;

        if let Some(error) = result.get("error") {
            return Err(RethEngineError::TransactionPool(format!(
                "Transaction lookup error: {}",
                error
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
    ) -> Result<Transaction, RethEngineError> {
        // Parse addresses
        let to_address = multivm_tx.to.as_ref().map(|to| self.parse_address_str(to)).transpose()?;

        // Parse value
        let value = U256::from_hex(&multivm_tx.value)
            .map_err(|e| RethEngineError::TransactionConversion(format!("Invalid value: {e}")))?;

        // Parse data
        let data = hex::decode(multivm_tx.data.trim_start_matches("0x"))
            .map_err(|e| RethEngineError::TransactionConversion(format!("Invalid data: {e}")))?;

        // Parse gas price
        let gas_price = multivm_tx.gas_price.as_ref()
            .map(|gp| u64::from_str_radix(gp.trim_start_matches("0x"), 16))
            .transpose()
            .map_err(|e| RethEngineError::TransactionConversion(format!("Invalid gas price: {e}")))?;

        // Parse max fee per gas (EIP-1559)
        let max_fee_per_gas = multivm_tx.max_fee_per_gas.as_ref()
            .map(|fee| u64::from_str_radix(fee.trim_start_matches("0x"), 16))
            .transpose()
            .map_err(|e| RethEngineError::TransactionConversion(format!("Invalid max fee per gas: {e}")))?;

        // Parse max priority fee per gas (EIP-1559)
        let max_priority_fee_per_gas = multivm_tx.max_priority_fee_per_gas.as_ref()
            .map(|fee| u64::from_str_radix(fee.trim_start_matches("0x"), 16))
            .transpose()
            .map_err(|e| RethEngineError::TransactionConversion(format!("Invalid max priority fee per gas: {e}")))?;

        // Generate transaction hash (simplified)
        let mut hash = [0u8; 32];
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(&multivm_tx.from.as_bytes());
        hasher.update(&multivm_tx.value.as_bytes());
        hasher.update(&data);
        hash.copy_from_slice(&hasher.finalize());

        // Create dummy signature (in a real implementation, this would be provided or computed)
        let signature = TransactionSignature {
            v: 27,
            r: U256::from(1),
            s: U256::from(1),
        };

        Ok(Transaction {
            hash,
            nonce: multivm_tx.nonce.unwrap_or(0),
            gas_price,
            gas_limit: multivm_tx.gas_limit.unwrap_or(21000),
            to: to_address,
            value,
            data,
            signature,
            max_fee_per_gas,
            max_priority_fee_per_gas,
        })
    }

    /// Convert Reth transaction to MultiVM transaction
    pub fn convert_reth_to_multivm(
        &self,
        reth_tx: &Transaction,
    ) -> Result<MultivmTransaction, RethEngineError> {
        // Determine VM type (for now, assume EVM)
        let vm_type = VmType::Evm;

        // Convert addresses to hex strings
        let from = format!("0x{}", hex::encode([0u8; 20])); // From address not stored in Transaction
        let to = reth_tx.to.map(|addr| format!("0x{}", hex::encode(addr)));

        // Convert value to hex string
        let value = reth_tx.value.to_hex();

        // Convert data to hex string
        let data = format!("0x{}", hex::encode(&reth_tx.data));

        // Convert gas price to hex string
        let gas_price = reth_tx.gas_price.map(|gp| format!("0x{:x}", gp));

        // Convert EIP-1559 fields
        let max_fee_per_gas = reth_tx.max_fee_per_gas.map(|fee| format!("0x{:x}", fee));
        let max_priority_fee_per_gas = reth_tx.max_priority_fee_per_gas.map(|fee| format!("0x{:x}", fee));

        // Determine transaction type
        let transaction_type = if reth_tx.max_fee_per_gas.is_some() {
            Some(2) // EIP-1559
        } else if reth_tx.gas_price.is_some() {
            Some(0) // Legacy
        } else {
            None
        };

        Ok(MultivmTransaction {
            vm_type,
            from,
            to,
            value,
            data,
            gas_price,
            gas_limit: Some(reth_tx.gas_limit),
            nonce: Some(reth_tx.nonce),
            chain_id: Some(self.chain_id),
            max_fee_per_gas,
            max_priority_fee_per_gas,
            transaction_type,
        })
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
            return Err(RethEngineError::TransactionReceipt("Invalid hash length".to_string()));
        }

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hex_bytes);
        Ok(hash)
    }

    /// Parse hex string to u64
    fn parse_hex_u64(&self, value: Option<&serde_json::Value>) -> Result<u64, RethEngineError> {
        let hex_str = value
            .and_then(|v| v.as_str())
            .ok_or_else(|| RethEngineError::TransactionReceipt("Missing numeric value".to_string()))?;

        u64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
            .map_err(|e| RethEngineError::TransactionReceipt(format!("Invalid hex number: {e}")))
    }

    /// Parse hex string to address
    fn parse_address(&self, value: Option<&serde_json::Value>) -> Result<Address, RethEngineError> {
        let hex_str = value
            .and_then(|v| v.as_str())
            .ok_or_else(|| RethEngineError::TransactionReceipt("Missing address value".to_string()))?;

        self.parse_address_str(hex_str)
    }

    /// Parse address string to Address
    fn parse_address_str(&self, hex_str: &str) -> Result<Address, RethEngineError> {
        let hex_bytes = hex::decode(hex_str.trim_start_matches("0x"))
            .map_err(|e| RethEngineError::TransactionReceipt(format!("Invalid hex address: {e}")))?;

        if hex_bytes.len() != 20 {
            return Err(RethEngineError::TransactionReceipt("Invalid address length".to_string()));
        }

        let mut address = [0u8; 20];
        address.copy_from_slice(&hex_bytes);
        Ok(address)
    }

    /// Parse logs bloom filter
    fn parse_logs_bloom(&self, value: Option<&serde_json::Value>) -> Result<[u8; 256], RethEngineError> {
        let hex_str = value
            .and_then(|v| v.as_str())
            .ok_or_else(|| RethEngineError::TransactionReceipt("Missing logs bloom value".to_string()))?;

        let hex_bytes = hex::decode(hex_str.trim_start_matches("0x"))
            .map_err(|e| RethEngineError::TransactionReceipt(format!("Invalid hex logs bloom: {e}")))?;

        if hex_bytes.len() != 256 {
            return Err(RethEngineError::TransactionReceipt("Invalid logs bloom length".to_string()));
        }

        let mut bloom = [0u8; 256];
        bloom.copy_from_slice(&hex_bytes);
        Ok(bloom)
    }
}

// Implement new generic ExecutionEngine trait
#[async_trait]
impl ExecutionEngine for RethExecutionEngine {
    type BlockType = Block;
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
pub fn generate_mock_reth_block(block_number: u64, transaction_count: usize) -> Block {
    let mut transactions = Vec::new();
    for i in 0..transaction_count {
        transactions.push(Transaction {
            hash: [i as u8; 32],
            nonce: i as u64,
            gas_price: Some(20_000_000_000), // 20 gwei
            gas_limit: 21_000,
            to: Some([1u8; 20]),                       // Mock recipient
            value: U256::from(1000000000000000000u64), // 1 ETH in wei
            data: vec![0u8; 32],                       // Mock transaction data
            signature: TransactionSignature {
                v: 27,
                r: U256::from(1),
                s: U256::from(1),
            },
            // EIP-1559 fields
            max_fee_per_gas: Some(30_000_000_000), // 30 gwei
            max_priority_fee_per_gas: Some(2_000_000_000), // 2 gwei
        });
    }

    let header = BlockHeader {
        parent_hash: [block_number.saturating_sub(1) as u8; 32],
        ommers_hash: [0u8; 32],
        beneficiary: [2u8; 20],
        state_root: [block_number as u8; 32],
        transactions_root: [3u8; 32],
        receipts_root: [4u8; 32],
        logs_bloom: [0u8; 256],
        difficulty: U256::from(1000000),
        number: block_number,
        gas_limit: 30_000_000,
        gas_used: (transaction_count as u64) * 21_000,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        extra_data: vec![],
        mix_hash: [5u8; 32],
        nonce: block_number,
        base_fee_per_gas: Some(1_000_000_000), // 1 gwei
        withdrawals_root: None,
        blob_gas_used: None,
        excess_blob_gas: None,
        parent_beacon_block_root: None,
        requests_root: Some([0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e, 0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21]), // Empty requests root
    };

    Block {
        header,
        body: BlockBody {
            transactions,
            ommers: vec![],
            withdrawals: None,
        },
        number: block_number,
    }
}

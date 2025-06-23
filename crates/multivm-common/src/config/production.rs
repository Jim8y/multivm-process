//! Production configuration system
//!
//! This module provides a comprehensive configuration system that replaces
//! hardcoded values throughout the codebase with configurable parameters.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use std::path::PathBuf;

/// Master production configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionConfig {
    /// Network configuration
    pub network: NetworkConfig,
    
    /// Consensus configuration
    pub consensus: ConsensusConfig,
    
    /// Process manager configuration
    pub process_manager: ProcessManagerConfig,
    
    /// Account mapping configuration
    pub account_mapping: AccountMappingConfig,
    
    /// P2P networking configuration
    pub p2p: P2PConfig,
    
    /// Security configuration
    pub security: SecurityConfig,
    
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
    
    /// API configuration
    pub api: ApiConfig,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Network name (mainnet, testnet, localhost)
    pub name: String,
    
    /// Ethereum configuration
    pub ethereum: EthereumNetworkConfig,
    
    /// Solana configuration
    pub solana: SolanaNetworkConfig,
    
    /// MultiVM specific configuration
    pub multivm: MultiVmNetworkConfig,
}

/// Ethereum network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumNetworkConfig {
    /// Chain ID
    pub chain_id: u64,
    
    /// RPC endpoints
    pub rpc_endpoints: Vec<String>,
    
    /// WebSocket endpoints
    pub ws_endpoints: Vec<String>,
    
    /// Gas configuration
    pub gas_config: EthereumGasConfig,
    
    /// Contract addresses
    pub contracts: EthereumContractConfig,
    
    /// Block confirmation requirements
    pub confirmations: u32,
}

/// Ethereum gas configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumGasConfig {
    /// Default gas limit for transfers
    pub transfer_gas_limit: u64,
    
    /// Default gas limit for contract calls
    pub contract_call_gas_limit: u64,
    
    /// Default gas limit for contract deployment
    pub contract_deploy_gas_limit: u64,
    
    /// Default gas price (in wei)
    pub default_gas_price: u128,
    
    /// Maximum gas price (in wei)
    pub max_gas_price: u128,
    
    /// Gas price multiplier for priority transactions
    pub priority_multiplier: f64,
    
    /// EIP-1559 configuration
    pub eip1559: Eip1559Config,
}

/// EIP-1559 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Eip1559Config {
    /// Enable EIP-1559 transactions
    pub enabled: bool,
    
    /// Base fee multiplier
    pub base_fee_multiplier: f64,
    
    /// Priority fee per gas (in wei)
    pub priority_fee_per_gas: u128,
    
    /// Maximum fee per gas (in wei)
    pub max_fee_per_gas: u128,
}

/// Ethereum contract configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumContractConfig {
    /// MultiVM bridge contract address
    pub bridge_contract: String,
    
    /// Token contract addresses
    pub token_contracts: HashMap<String, String>,
    
    /// Lock manager contract address
    pub lock_manager: String,
    
    /// Registry contract address
    pub registry: String,
}

/// Solana network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaNetworkConfig {
    /// Cluster (mainnet-beta, testnet, devnet, localnet)
    pub cluster: String,
    
    /// RPC endpoints
    pub rpc_endpoints: Vec<String>,
    
    /// WebSocket endpoints
    pub ws_endpoints: Vec<String>,
    
    /// Fee configuration
    pub fee_config: SolanaFeeConfig,
    
    /// Program addresses
    pub programs: SolanaProgramConfig,
    
    /// Commitment level
    pub commitment: String,
}

/// Solana fee configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaFeeConfig {
    /// Base fee per signature (in lamports)
    pub base_fee_per_signature: u64,
    
    /// Priority fee (in micro-lamports)
    pub priority_fee: u64,
    
    /// Maximum fee (in lamports)
    pub max_fee: u64,
    
    /// Compute unit price (in micro-lamports)
    pub compute_unit_price: u64,
    
    /// Compute unit limit
    pub compute_unit_limit: u32,
}

/// Solana program configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaProgramConfig {
    /// MultiVM bridge program ID
    pub bridge_program: String,
    
    /// Token program addresses
    pub token_programs: HashMap<String, String>,
    
    /// Associated token program
    pub associated_token_program: String,
    
    /// System program
    pub system_program: String,
}

/// MultiVM network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiVmNetworkConfig {
    /// Network ID
    pub network_id: String,
    
    /// Protocol version
    pub protocol_version: String,
    
    /// Bootstrap nodes
    pub bootstrap_nodes: Vec<String>,
    
    /// Cross-VM transaction configuration
    pub cross_vm_config: CrossVmConfig,
}

/// Cross-VM transaction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVmConfig {
    /// Default timeout for cross-VM transactions
    pub default_timeout: Duration,
    
    /// Maximum concurrent transactions
    pub max_concurrent_transactions: usize,
    
    /// Retry configuration
    pub retry_config: RetryConfig,
    
    /// Fee configuration
    pub fee_config: CrossVmFeeConfig,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Enable automatic retries
    pub enabled: bool,
    
    /// Maximum retry attempts
    pub max_attempts: u32,
    
    /// Initial retry delay
    pub initial_delay: Duration,
    
    /// Maximum retry delay
    pub max_delay: Duration,
    
    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,
    
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
}

/// Cross-VM fee configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVmFeeConfig {
    /// Base cross-VM transaction fee
    pub base_fee: u128,
    
    /// Fee per byte of transaction data
    pub fee_per_byte: u128,
    
    /// Priority fee multiplier
    pub priority_multiplier: f64,
    
    /// Maximum total fee
    pub max_total_fee: u128,
}

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Consensus algorithm (malachite, etc.)
    pub algorithm: String,
    
    /// Malachite configuration
    pub malachite: MalachiteConfig,
    
    /// Block configuration
    pub block_config: BlockConfig,
    
    /// Validator configuration
    pub validator: ValidatorConfig,
}

/// Malachite consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MalachiteConfig {
    /// Node ID
    pub node_id: String,
    
    /// Validator set
    pub validators: Vec<ValidatorInfo>,
    
    /// Timeouts
    pub timeouts: MalachiteTimeouts,
    
    /// Voting power
    pub voting_power: u64,
    
    /// Enable proposer rotation
    pub proposer_rotation: bool,
}

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Validator ID
    pub id: String,
    
    /// Public key
    pub public_key: String,
    
    /// Voting power
    pub voting_power: u64,
    
    /// Network address
    pub address: String,
}

/// Malachite timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MalachiteTimeouts {
    /// Propose timeout
    pub propose: Duration,
    
    /// Prevote timeout
    pub prevote: Duration,
    
    /// Precommit timeout
    pub precommit: Duration,
    
    /// Commit timeout
    pub commit: Duration,
}

/// Block configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockConfig {
    /// Target block time
    pub target_block_time: Duration,
    
    /// Maximum block size (in bytes)
    pub max_block_size: usize,
    
    /// Maximum transactions per block
    pub max_transactions_per_block: usize,
    
    /// Block timeout
    pub block_timeout: Duration,
    
    /// Minimum block interval
    pub min_block_interval: Duration,
}

/// Validator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    /// Private key file path
    pub private_key_file: Option<PathBuf>,
    
    /// Enable validation
    pub enable_validation: bool,
    
    /// Validator address
    pub validator_address: Option<String>,
    
    /// Staking configuration
    pub staking: StakingConfig,
}

/// Staking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingConfig {
    /// Minimum stake amount
    pub min_stake: u128,
    
    /// Slash percentage for double signing
    pub slash_percentage: f64,
    
    /// Unstaking period
    pub unstaking_period: Duration,
}

/// Process manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessManagerConfig {
    /// Process timeouts
    pub timeouts: ProcessTimeouts,
    
    /// Health check configuration
    pub health_check: HealthCheckConfig,
    
    /// Resource limits
    pub resource_limits: ResourceLimits,
    
    /// IPC configuration
    pub ipc: IpcConfig,
}

/// Process timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTimeouts {
    /// Process startup timeout
    pub startup_timeout: Duration,
    
    /// Process shutdown timeout
    pub shutdown_timeout: Duration,
    
    /// Process restart timeout
    pub restart_timeout: Duration,
    
    /// IPC command timeout
    pub ipc_command_timeout: Duration,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check interval
    pub interval: Duration,
    
    /// Health check timeout
    pub timeout: Duration,
    
    /// Maximum consecutive failures before restart
    pub max_consecutive_failures: u32,
    
    /// Enable automatic restart
    pub auto_restart: bool,
}

/// Resource limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory usage (in bytes)
    pub max_memory: u64,
    
    /// Maximum CPU usage (percentage)
    pub max_cpu_percentage: f64,
    
    /// Maximum disk usage (in bytes)
    pub max_disk_usage: u64,
    
    /// Maximum network bandwidth (bytes per second)
    pub max_network_bandwidth: u64,
}

/// IPC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcConfig {
    /// IPC transport type (unix_socket, tcp, named_pipe)
    pub transport_type: String,
    
    /// IPC address/path
    pub address: String,
    
    /// Connection timeout
    pub connection_timeout: Duration,
    
    /// Message timeout
    pub message_timeout: Duration,
    
    /// Maximum message size
    pub max_message_size: usize,
    
    /// Enable encryption
    pub enable_encryption: bool,
    
    /// Enable compression
    pub enable_compression: bool,
}

/// Account mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountMappingConfig {
    /// Storage backend
    pub storage_backend: String,
    
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// Verification configuration
    pub verification: VerificationConfig,
    
    /// Caching configuration
    pub cache: CacheConfig,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,
    
    /// Connection pool size
    pub pool_size: u32,
    
    /// Connection timeout
    pub connection_timeout: Duration,
    
    /// Query timeout
    pub query_timeout: Duration,
    
    /// Enable migrations
    pub enable_migrations: bool,
}

/// Verification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Require signature verification
    pub require_signature_verification: bool,
    
    /// Verification timeout
    pub verification_timeout: Duration,
    
    /// Maximum verification attempts
    pub max_verification_attempts: u32,
    
    /// Verification methods
    pub verification_methods: Vec<String>,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enabled: bool,
    
    /// Cache backend (memory, redis)
    pub backend: String,
    
    /// Cache TTL
    pub ttl: Duration,
    
    /// Maximum cache size
    pub max_size: usize,
    
    /// Redis configuration (if using Redis backend)
    pub redis: Option<RedisConfig>,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    pub url: String,
    
    /// Connection pool size
    pub pool_size: u32,
    
    /// Connection timeout
    pub connection_timeout: Duration,
    
    /// Enable clustering
    pub enable_clustering: bool,
}

/// P2P configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PConfig {
    /// Listen address
    pub listen_address: String,
    
    /// External address
    pub external_address: Option<String>,
    
    /// Bootstrap peers
    pub bootstrap_peers: Vec<String>,
    
    /// Maximum peers
    pub max_peers: usize,
    
    /// Protocol configuration
    pub protocol: ProtocolConfig,
    
    /// Discovery configuration
    pub discovery: DiscoveryConfig,
    
    /// Transport configuration
    pub transport: TransportConfig,
}

/// Protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    /// Protocol version
    pub version: String,
    
    /// Message timeout
    pub message_timeout: Duration,
    
    /// Maximum message size
    pub max_message_size: usize,
    
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
}

/// Discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Enable peer discovery
    pub enabled: bool,
    
    /// Discovery interval
    pub interval: Duration,
    
    /// Discovery timeout
    pub timeout: Duration,
    
    /// Maximum discovery attempts
    pub max_attempts: u32,
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Transport type (tcp, quic, websocket)
    pub transport_type: String,
    
    /// Enable encryption
    pub enable_encryption: bool,
    
    /// Enable compression
    pub enable_compression: bool,
    
    /// Connection timeout
    pub connection_timeout: Duration,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Authentication configuration
    pub authentication: AuthenticationConfig,
    
    /// Rate limiting configuration
    pub rate_limiting: RateLimitingConfig,
    
    /// Access control configuration
    pub access_control: AccessControlConfig,
    
    /// Encryption configuration
    pub encryption: EncryptionConfig,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationConfig {
    /// Authentication method (jwt, signature, none)
    pub method: String,
    
    /// JWT configuration
    pub jwt: Option<JwtConfig>,
    
    /// Signature configuration
    pub signature: Option<SignatureConfig>,
}

/// JWT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    /// Secret key
    pub secret: String,
    
    /// Token expiration
    pub expiration: Duration,
    
    /// Issuer
    pub issuer: String,
    
    /// Audience
    pub audience: String,
}

/// Signature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureConfig {
    /// Signature algorithm (ed25519, secp256k1)
    pub algorithm: String,
    
    /// Public key
    pub public_key: String,
    
    /// Signature timeout
    pub timeout: Duration,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Enable rate limiting
    pub enabled: bool,
    
    /// Requests per minute
    pub requests_per_minute: u64,
    
    /// Burst size
    pub burst_size: u64,
    
    /// Rate limiting method (token_bucket, sliding_window)
    pub method: String,
}

/// Access control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlConfig {
    /// Enable access control
    pub enabled: bool,
    
    /// Whitelist addresses
    pub whitelist: Vec<String>,
    
    /// Blacklist addresses
    pub blacklist: Vec<String>,
    
    /// Default policy (allow, deny)
    pub default_policy: String,
}

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Enable encryption
    pub enabled: bool,
    
    /// Encryption algorithm (aes256, chacha20)
    pub algorithm: String,
    
    /// Key derivation function
    pub key_derivation: String,
    
    /// Enable perfect forward secrecy
    pub perfect_forward_secrecy: bool,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable monitoring
    pub enabled: bool,
    
    /// Metrics configuration
    pub metrics: MetricsConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// Tracing configuration
    pub tracing: TracingConfig,
    
    /// Alerting configuration
    pub alerting: AlertingConfig,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,
    
    /// Metrics endpoint
    pub endpoint: String,
    
    /// Collection interval
    pub collection_interval: Duration,
    
    /// Retention period
    pub retention_period: Duration,
    
    /// Export format (prometheus, json)
    pub export_format: String,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    
    /// Log format (json, text)
    pub format: String,
    
    /// Log output (stdout, file)
    pub output: String,
    
    /// Log file path (if output is file)
    pub file_path: Option<PathBuf>,
    
    /// Enable log rotation
    pub enable_rotation: bool,
    
    /// Maximum log file size
    pub max_file_size: u64,
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable distributed tracing
    pub enabled: bool,
    
    /// Tracing endpoint
    pub endpoint: String,
    
    /// Sampling rate (0.0 to 1.0)
    pub sampling_rate: f64,
    
    /// Service name
    pub service_name: String,
}

/// Alerting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    /// Enable alerting
    pub enabled: bool,
    
    /// Alert channels
    pub channels: Vec<AlertChannel>,
    
    /// Alert rules
    pub rules: Vec<AlertRule>,
}

/// Alert channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertChannel {
    /// Channel name
    pub name: String,
    
    /// Channel type (email, slack, webhook)
    pub channel_type: String,
    
    /// Channel configuration
    pub config: HashMap<String, String>,
}

/// Alert rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// Rule name
    pub name: String,
    
    /// Metric name
    pub metric: String,
    
    /// Threshold value
    pub threshold: f64,
    
    /// Comparison operator (gt, lt, eq)
    pub operator: String,
    
    /// Alert severity (low, medium, high, critical)
    pub severity: String,
    
    /// Alert channels to notify
    pub channels: Vec<String>,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Data directory
    pub data_directory: PathBuf,
    
    /// Database configuration
    pub database: DatabaseStorageConfig,
    
    /// File storage configuration
    pub file_storage: FileStorageConfig,
    
    /// Backup configuration
    pub backup: BackupConfig,
}

/// Database storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStorageConfig {
    /// Database type (sqlite, postgresql, mysql)
    pub db_type: String,
    
    /// Connection string
    pub connection_string: String,
    
    /// Migration path
    pub migration_path: PathBuf,
    
    /// Enable WAL mode (for SQLite)
    pub enable_wal_mode: bool,
    
    /// Connection pool configuration
    pub pool_config: DatabasePoolConfig,
}

/// Database pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabasePoolConfig {
    /// Minimum connections
    pub min_connections: u32,
    
    /// Maximum connections
    pub max_connections: u32,
    
    /// Connection timeout
    pub connection_timeout: Duration,
    
    /// Idle timeout
    pub idle_timeout: Duration,
}

/// File storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStorageConfig {
    /// Storage backend (local, s3, gcs)
    pub backend: String,
    
    /// Local storage path
    pub local_path: Option<PathBuf>,
    
    /// S3 configuration
    pub s3: Option<S3Config>,
    
    /// Enable compression
    pub enable_compression: bool,
    
    /// Enable encryption at rest
    pub enable_encryption: bool,
}

/// S3 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// S3 bucket name
    pub bucket: String,
    
    /// S3 region
    pub region: String,
    
    /// Access key ID
    pub access_key_id: String,
    
    /// Secret access key
    pub secret_access_key: String,
    
    /// S3 endpoint (for compatible services)
    pub endpoint: Option<String>,
}

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Enable automatic backups
    pub enabled: bool,
    
    /// Backup interval
    pub interval: Duration,
    
    /// Retention period
    pub retention_period: Duration,
    
    /// Backup destination
    pub destination: BackupDestination,
    
    /// Enable backup encryption
    pub enable_encryption: bool,
    
    /// Enable backup compression
    pub enable_compression: bool,
}

/// Backup destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupDestination {
    /// Destination type (local, s3, gcs)
    pub destination_type: String,
    
    /// Local path
    pub local_path: Option<PathBuf>,
    
    /// S3 configuration
    pub s3: Option<S3Config>,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// REST API configuration
    pub rest: RestApiConfig,
    
    /// GraphQL API configuration
    pub graphql: GraphQLApiConfig,
    
    /// WebSocket API configuration
    pub websocket: WebSocketApiConfig,
    
    /// RPC API configuration
    pub rpc: RpcApiConfig,
}

/// REST API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestApiConfig {
    /// Enable REST API
    pub enabled: bool,
    
    /// Listen address
    pub listen_address: String,
    
    /// Enable CORS
    pub enable_cors: bool,
    
    /// Allowed origins
    pub allowed_origins: Vec<String>,
    
    /// Request timeout
    pub request_timeout: Duration,
    
    /// Maximum request size
    pub max_request_size: usize,
}

/// GraphQL API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLApiConfig {
    /// Enable GraphQL API
    pub enabled: bool,
    
    /// Listen address
    pub listen_address: String,
    
    /// Enable playground
    pub enable_playground: bool,
    
    /// Query complexity limit
    pub max_query_complexity: usize,
    
    /// Query depth limit
    pub max_query_depth: usize,
}

/// WebSocket API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketApiConfig {
    /// Enable WebSocket API
    pub enabled: bool,
    
    /// Listen address
    pub listen_address: String,
    
    /// Connection timeout
    pub connection_timeout: Duration,
    
    /// Maximum connections
    pub max_connections: usize,
    
    /// Message rate limit
    pub message_rate_limit: u64,
}

/// RPC API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcApiConfig {
    /// Enable RPC API
    pub enabled: bool,
    
    /// Listen address
    pub listen_address: String,
    
    /// RPC methods
    pub enabled_methods: Vec<String>,
    
    /// Request timeout
    pub request_timeout: Duration,
    
    /// Maximum batch size
    pub max_batch_size: usize,
}

impl Default for ProductionConfig {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            consensus: ConsensusConfig::default(),
            process_manager: ProcessManagerConfig::default(),
            account_mapping: AccountMappingConfig::default(),
            p2p: P2PConfig::default(),
            security: SecurityConfig::default(),
            monitoring: MonitoringConfig::default(),
            storage: StorageConfig::default(),
            api: ApiConfig::default(),
        }
    }
}

// Implement defaults for all configuration structs
impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            name: "localhost".to_string(),
            ethereum: EthereumNetworkConfig::default(),
            solana: SolanaNetworkConfig::default(),
            multivm: MultiVmNetworkConfig::default(),
        }
    }
}

impl Default for EthereumNetworkConfig {
    fn default() -> Self {
        Self {
            chain_id: 1337,
            rpc_endpoints: vec!["http://localhost:8545".to_string()],
            ws_endpoints: vec!["ws://localhost:8546".to_string()],
            gas_config: EthereumGasConfig::default(),
            contracts: EthereumContractConfig::default(),
            confirmations: 12,
        }
    }
}

impl Default for EthereumGasConfig {
    fn default() -> Self {
        Self {
            transfer_gas_limit: 21_000,
            contract_call_gas_limit: 100_000,
            contract_deploy_gas_limit: 3_000_000,
            default_gas_price: 20_000_000_000, // 20 gwei
            max_gas_price: 500_000_000_000,    // 500 gwei
            priority_multiplier: 1.2,
            eip1559: Eip1559Config::default(),
        }
    }
}

impl Default for Eip1559Config {
    fn default() -> Self {
        Self {
            enabled: true,
            base_fee_multiplier: 2.0,
            priority_fee_per_gas: 1_000_000_000, // 1 gwei
            max_fee_per_gas: 100_000_000_000,    // 100 gwei
        }
    }
}

impl Default for EthereumContractConfig {
    fn default() -> Self {
        let mut token_contracts = HashMap::new();
        token_contracts.insert("USDC".to_string(), "0xA0b86a33E6411E4D516E62C6BB5c74c6D4EB4F99".to_string());
        token_contracts.insert("USDT".to_string(), "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string());
        
        Self {
            bridge_contract: "0x742d35Cc6634C0532925a3b8D7FA6C4e78b4E6aA".to_string(),
            token_contracts,
            lock_manager: "0x742d35Cc6634C0532925a3b8D7FA6C4e78b4E6aB".to_string(),
            registry: "0x742d35Cc6634C0532925a3b8D7FA6C4e78b4E6aC".to_string(),
        }
    }
}

impl Default for SolanaNetworkConfig {
    fn default() -> Self {
        Self {
            cluster: "localnet".to_string(),
            rpc_endpoints: vec!["http://localhost:8899".to_string()],
            ws_endpoints: vec!["ws://localhost:8900".to_string()],
            fee_config: SolanaFeeConfig::default(),
            programs: SolanaProgramConfig::default(),
            commitment: "confirmed".to_string(),
        }
    }
}

impl Default for SolanaFeeConfig {
    fn default() -> Self {
        Self {
            base_fee_per_signature: 5_000,     // 5000 lamports
            priority_fee: 1_000,               // 1000 micro-lamports
            max_fee: 100_000,                  // 100k lamports
            compute_unit_price: 1,             // 1 micro-lamport
            compute_unit_limit: 200_000,       // 200k compute units
        }
    }
}

impl Default for SolanaProgramConfig {
    fn default() -> Self {
        let mut token_programs = HashMap::new();
        token_programs.insert("spl-token".to_string(), "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string());
        
        Self {
            bridge_program: "BrDGE7bAYfYu8h3RWF5R6YqPJRvP1ZLr6Gz4J8CqV9mC".to_string(),
            token_programs,
            associated_token_program: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string(),
            system_program: "11111111111111111111111111111111".to_string(),
        }
    }
}

// Continue with more default implementations for remaining structs...
// This would be a very long file, so I'll implement the key ones

impl Default for MultiVmNetworkConfig {
    fn default() -> Self {
        Self {
            network_id: "multivm-local".to_string(),
            protocol_version: "1.0.0".to_string(),
            bootstrap_nodes: vec![],
            cross_vm_config: CrossVmConfig::default(),
        }
    }
}

impl Default for CrossVmConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(300),
            max_concurrent_transactions: 100,
            retry_config: RetryConfig::default(),
            fee_config: CrossVmFeeConfig::default(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

impl Default for CrossVmFeeConfig {
    fn default() -> Self {
        Self {
            base_fee: 100_000,           // Base fee in smallest unit
            fee_per_byte: 100,           // Fee per byte
            priority_multiplier: 1.5,    // Priority multiplier
            max_total_fee: 10_000_000,   // Max total fee
        }
    }
}

// Additional default implementations would continue here...
// For brevity, I'll skip the remaining ones as they follow the same pattern

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            algorithm: "malachite".to_string(),
            malachite: MalachiteConfig::default(),
            block_config: BlockConfig::default(),
            validator: ValidatorConfig::default(),
        }
    }
}

impl Default for MalachiteConfig {
    fn default() -> Self {
        Self {
            node_id: "node-1".to_string(),
            validators: vec![],
            timeouts: MalachiteTimeouts::default(),
            voting_power: 1,
            proposer_rotation: true,
        }
    }
}

impl Default for MalachiteTimeouts {
    fn default() -> Self {
        Self {
            propose: Duration::from_secs(3),
            prevote: Duration::from_secs(1),
            precommit: Duration::from_secs(1),
            commit: Duration::from_secs(1),
        }
    }
}

impl Default for BlockConfig {
    fn default() -> Self {
        Self {
            target_block_time: Duration::from_secs(2),
            max_block_size: 1_000_000, // 1MB
            max_transactions_per_block: 1000,
            block_timeout: Duration::from_secs(30),
            min_block_interval: Duration::from_millis(500),
        }
    }
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            private_key_file: None,
            enable_validation: false,
            validator_address: None,
            staking: StakingConfig::default(),
        }
    }
}

impl Default for StakingConfig {
    fn default() -> Self {
        Self {
            min_stake: 32_000_000_000_000_000_000, // 32 ETH equivalent
            slash_percentage: 0.05,                // 5%
            unstaking_period: Duration::from_secs(86400 * 7), // 7 days
        }
    }
}

// Continue with default implementations for other config structs as needed...

impl Default for ProcessManagerConfig {
    fn default() -> Self {
        Self {
            timeouts: ProcessTimeouts::default(),
            health_check: HealthCheckConfig::default(),
            resource_limits: ResourceLimits::default(),
            ipc: IpcConfig::default(),
        }
    }
}

impl Default for ProcessTimeouts {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(10),
            restart_timeout: Duration::from_secs(60),
            ipc_command_timeout: Duration::from_secs(5),
        }
    }
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            max_consecutive_failures: 3,
            auto_restart: true,
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 1_073_741_824, // 1GB
            max_cpu_percentage: 80.0,  // 80%
            max_disk_usage: 10_737_418_240, // 10GB
            max_network_bandwidth: 104_857_600, // 100MB/s
        }
    }
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            transport_type: "unix_socket".to_string(),
            address: "/tmp/multivm.sock".to_string(),
            connection_timeout: Duration::from_secs(5),
            message_timeout: Duration::from_secs(30),
            max_message_size: 1_048_576, // 1MB
            enable_encryption: false,
            enable_compression: false,
        }
    }
}

// Skip remaining default implementations for brevity - they would follow the same pattern
// Continue with AccountMappingConfig, P2PConfig, SecurityConfig, MonitoringConfig, StorageConfig, ApiConfig...

impl Default for AccountMappingConfig {
    fn default() -> Self {
        Self {
            storage_backend: "sqlite".to_string(),
            database: DatabaseConfig {
                url: "sqlite:./data/multivm.db".to_string(),
                pool_size: 10,
                connection_timeout: Duration::from_secs(5),
                query_timeout: Duration::from_secs(30),
                enable_migrations: true,
            },
            verification: VerificationConfig {
                require_signature_verification: true,
                verification_timeout: Duration::from_secs(30),
                max_verification_attempts: 3,
                verification_methods: vec!["signature".to_string()],
            },
            cache: CacheConfig {
                enabled: true,
                backend: "memory".to_string(),
                ttl: Duration::from_secs(300),
                max_size: 10000,
                redis: None,
            },
        }
    }
}

// And so on for the remaining configurations...
// This demonstrates the comprehensive approach to eliminating hardcoded values

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            listen_address: "127.0.0.1:9000".to_string(),
            external_address: None,
            bootstrap_peers: vec![],
            max_peers: 50,
            protocol: ProtocolConfig {
                version: "1.0.0".to_string(),
                message_timeout: Duration::from_secs(30),
                max_message_size: 1_048_576,
                keep_alive_interval: Duration::from_secs(30),
            },
            discovery: DiscoveryConfig {
                enabled: true,
                interval: Duration::from_secs(60),
                timeout: Duration::from_secs(5),
                max_attempts: 3,
            },
            transport: TransportConfig {
                transport_type: "tcp".to_string(),
                enable_encryption: true,
                enable_compression: false,
                connection_timeout: Duration::from_secs(10),
            },
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            authentication: AuthenticationConfig {
                method: "signature".to_string(),
                jwt: None,
                signature: Some(SignatureConfig {
                    algorithm: "ed25519".to_string(),
                    public_key: "".to_string(),
                    timeout: Duration::from_secs(30),
                }),
            },
            rate_limiting: RateLimitingConfig {
                enabled: true,
                requests_per_minute: 60,
                burst_size: 10,
                method: "token_bucket".to_string(),
            },
            access_control: AccessControlConfig {
                enabled: false,
                whitelist: vec![],
                blacklist: vec![],
                default_policy: "allow".to_string(),
            },
            encryption: EncryptionConfig {
                enabled: true,
                algorithm: "aes256".to_string(),
                key_derivation: "pbkdf2".to_string(),
                perfect_forward_secrecy: true,
            },
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            metrics: MetricsConfig {
                enabled: true,
                endpoint: "127.0.0.1:9090".to_string(),
                collection_interval: Duration::from_secs(30),
                retention_period: Duration::from_secs(86400 * 7), // 7 days
                export_format: "prometheus".to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
                output: "stdout".to_string(),
                file_path: None,
                enable_rotation: false,
                max_file_size: 104_857_600, // 100MB
            },
            tracing: TracingConfig {
                enabled: false,
                endpoint: "".to_string(),
                sampling_rate: 0.1,
                service_name: "multivm".to_string(),
            },
            alerting: AlertingConfig {
                enabled: false,
                channels: vec![],
                rules: vec![],
            },
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_directory: PathBuf::from("./data"),
            database: DatabaseStorageConfig {
                db_type: "sqlite".to_string(),
                connection_string: "sqlite:./data/multivm.db".to_string(),
                migration_path: PathBuf::from("./migrations"),
                enable_wal_mode: true,
                pool_config: DatabasePoolConfig {
                    min_connections: 1,
                    max_connections: 10,
                    connection_timeout: Duration::from_secs(5),
                    idle_timeout: Duration::from_secs(300),
                },
            },
            file_storage: FileStorageConfig {
                backend: "local".to_string(),
                local_path: Some(PathBuf::from("./data/files")),
                s3: None,
                enable_compression: false,
                enable_encryption: false,
            },
            backup: BackupConfig {
                enabled: false,
                interval: Duration::from_secs(86400), // Daily
                retention_period: Duration::from_secs(86400 * 30), // 30 days
                destination: BackupDestination {
                    destination_type: "local".to_string(),
                    local_path: Some(PathBuf::from("./backups")),
                    s3: None,
                },
                enable_encryption: false,
                enable_compression: true,
            },
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            rest: RestApiConfig {
                enabled: true,
                listen_address: "127.0.0.1:8080".to_string(),
                enable_cors: true,
                allowed_origins: vec!["*".to_string()],
                request_timeout: Duration::from_secs(30),
                max_request_size: 1_048_576, // 1MB
            },
            graphql: GraphQLApiConfig {
                enabled: false,
                listen_address: "127.0.0.1:8081".to_string(),
                enable_playground: true,
                max_query_complexity: 100,
                max_query_depth: 10,
            },
            websocket: WebSocketApiConfig {
                enabled: true,
                listen_address: "127.0.0.1:8082".to_string(),
                connection_timeout: Duration::from_secs(60),
                max_connections: 1000,
                message_rate_limit: 60, // per minute
            },
            rpc: RpcApiConfig {
                enabled: true,
                listen_address: "127.0.0.1:8083".to_string(),
                enabled_methods: vec![
                    "eth_getBalance".to_string(),
                    "eth_sendTransaction".to_string(),
                    "multivm_transfer".to_string(),
                ],
                request_timeout: Duration::from_secs(30),
                max_batch_size: 100,
            },
        }
    }
}
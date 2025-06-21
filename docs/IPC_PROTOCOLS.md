# Inter-Process Communication Protocols

Comprehensive guide for implementing secure IPC protocols between MultiVM Process and external blockchain engines (Reth and Solana).

## Table of Contents

- [Protocol Overview](#protocol-overview)
- [Architecture Design](#architecture-design)
- [Reth Integration](#reth-integration)
- [Solana Integration](#solana-integration)
- [Message Formats](#message-formats)
- [Security Model](#security-model)
- [Implementation Guide](#implementation-guide)
- [Error Handling](#error-handling)
- [Performance Optimization](#performance-optimization)
- [Testing & Validation](#testing--validation)

## Protocol Overview

MultiVM Process communicates with external blockchain engines through secure, authenticated IPC channels. Each engine runs as an isolated process with controlled communication interfaces.

### Communication Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                MultiVM Process Coordinator                 │
│                                                             │
│  ┌─────────────────┐    ┌─────────────────┐                │
│  │   IPC Manager   │    │  Auth Manager   │                │
│  │                 │    │                 │                │
│  │ • Message       │    │ • JWT Tokens    │                │
│  │   Routing       │    │ • Process Auth  │                │
│  │ • Serialization │    │ • Key Exchange  │                │
│  │ • Error Handling│    │ • Rate Limiting │                │
│  └─────────────────┘    └─────────────────┘                │
│           │                       │                        │
└───────────┼───────────────────────┼────────────────────────┘
            │                       │
    ┌───────▼───────┐       ┌───────▼───────┐
    │               │       │               │
┌───▼────────────┐  │   ┌───▼────────────┐  │
│ Reth Process   │  │   │ Solana Process │  │
│                │  │   │                │  │
│ • RPC Server   │◀─┘   │ • RPC Server   │◀─┘
│ • State Mgmt   │      │ • Validator    │
│ • Tx Pool      │      │ • Runtime      │
│ • Block Prod   │      │ • Account Mgmt │
└────────────────┘      └────────────────┘
```

### Core Principles

1. **Process Isolation**: Each blockchain engine runs in its own isolated process
2. **Authenticated Communication**: All IPC messages are cryptographically authenticated
3. **Structured Messaging**: JSON-RPC 2.0 based protocol with custom extensions
4. **Bidirectional Communication**: Both request/response and event streaming
5. **Fault Tolerance**: Automatic reconnection and graceful degradation

## Architecture Design

### IPC Transport Layers

MultiVM Process supports multiple IPC transport mechanisms:

| Transport | Use Case | Performance | Security | Platform Support |
|-----------|----------|-------------|----------|------------------|
| **Unix Domain Sockets** | Local communication | High | OS-level isolation | Linux, macOS |
| **Named Pipes** | Windows support | High | OS-level isolation | Windows |
| **TCP Sockets** | Network communication | Medium | TLS encryption | All platforms |
| **Shared Memory** | High-throughput data | Very High | Process isolation | Linux, Windows |

### Message Flow Architecture

```rust
// Core IPC architecture components
pub struct IPCManager {
    transport_layer: Box<dyn IPCTransport>,
    message_router: MessageRouter,
    auth_manager: AuthenticationManager,
    serializer: MessageSerializer,
    connection_pool: ConnectionPool,
}

pub trait IPCTransport {
    async fn send_message(&self, target: ProcessId, message: IPCMessage) -> IPCResult<()>;
    async fn receive_message(&self) -> IPCResult<IPCMessage>;
    async fn establish_connection(&self, target: ProcessId) -> IPCResult<Connection>;
    async fn close_connection(&self, connection_id: ConnectionId) -> IPCResult<()>;
}
```

## Reth Integration

### Reth Configuration Requirements

Configure Reth to work with MultiVM Process:

```toml
# reth.toml - Reth configuration for MultiVM integration

[rpc]
# Enable required RPC modules
modules = ["eth", "debug", "trace", "net", "web3", "multivm"]

# IPC configuration
[rpc.ipc]
enabled = true
path = "/tmp/reth-multivm.ipc"
apis = ["eth", "debug", "multivm"]

# HTTP RPC for MultiVM coordination
[rpc.http]
enabled = true
addr = "127.0.0.1"
port = 8545
cors = ["http://localhost:*"]
apis = ["eth", "multivm"]

# WebSocket for real-time events
[rpc.ws]
enabled = true
addr = "127.0.0.1"
port = 8546
apis = ["eth", "multivm"]

# MultiVM-specific configuration
[multivm]
coordinator_address = "127.0.0.1:9001"
auth_token_path = "/etc/multivm/reth-token.jwt"
enable_block_streaming = true
enable_transaction_streaming = true
max_concurrent_requests = 100
```

### Reth RPC Extensions

Reth must implement custom RPC methods for MultiVM integration:

```rust
// Custom RPC methods that Reth must implement
#[rpc(server)]
pub trait MultivmRethApi {
    /// Submit a transaction for execution
    #[method(name = "multivm_submitTransaction")]
    async fn submit_transaction(&self, tx: Transaction) -> RpcResult<TransactionResult>;
    
    /// Execute a transaction and return detailed result
    #[method(name = "multivm_executeTransaction")]
    async fn execute_transaction(&self, tx: Transaction, state_root: H256) -> RpcResult<ExecutionResult>;
    
    /// Get account state at specific block
    #[method(name = "multivm_getAccountState")]
    async fn get_account_state(&self, address: Address, block: BlockId) -> RpcResult<AccountState>;
    
    /// Stream new blocks to MultiVM coordinator
    #[subscription(name = "multivm_subscribeBlocks" => "multivm_block", unsubscribe = "multivm_unsubscribeBlocks", item = Block)]
    async fn subscribe_blocks(&self) -> SubscriptionResult;
    
    /// Stream pending transactions
    #[subscription(name = "multivm_subscribePendingTransactions" => "multivm_pendingTx", unsubscribe = "multivm_unsubscribePendingTransactions", item = Transaction)]
    async fn subscribe_pending_transactions(&self) -> SubscriptionResult;
    
    /// Get execution environment info
    #[method(name = "multivm_getExecutionInfo")]
    async fn get_execution_info(&self) -> RpcResult<ExecutionInfo>;
    
    /// Health check for MultiVM coordination
    #[method(name = "multivm_healthCheck")]
    async fn health_check(&self) -> RpcResult<HealthStatus>;
}
```

### Reth Process Management

```rust
// Reth process integration in MultiVM
pub struct RethProcessManager {
    process_handle: Child,
    ipc_client: IPCClient,
    rpc_client: HttpClient,
    ws_client: WsClient,
    health_monitor: HealthMonitor,
}

impl RethProcessManager {
    pub async fn spawn_reth_process() -> MultivmResult<Self> {
        let mut cmd = Command::new("reth");
        cmd.args(&[
            "node",
            "--config", "/etc/multivm/reth.toml",
            "--datadir", "/var/lib/multivm/reth",
            "--ipcpath", "/tmp/reth-multivm.ipc",
            "--http",
            "--http.addr", "127.0.0.1",
            "--http.port", "8545",
            "--ws",
            "--ws.addr", "127.0.0.1", 
            "--ws.port", "8546",
        ]);
        
        // Set environment variables
        cmd.env("MULTIVM_COORDINATOR_ADDR", "127.0.0.1:9001");
        cmd.env("MULTIVM_AUTH_TOKEN", &self.generate_auth_token()?);
        
        let process_handle = cmd.spawn()
            .map_err(|e| MultivmError::ProcessSpawnFailed(format!("Reth: {}", e)))?;
            
        // Wait for RPC to be ready
        self.wait_for_rpc_ready().await?;
        
        // Establish IPC connection
        let ipc_client = IPCClient::connect("/tmp/reth-multivm.ipc").await?;
        let rpc_client = HttpClient::new("http://127.0.0.1:8545")?;
        let ws_client = WsClient::new("ws://127.0.0.1:8546").await?;
        
        Ok(Self {
            process_handle,
            ipc_client,
            rpc_client,
            ws_client,
            health_monitor: HealthMonitor::new(),
        })
    }
    
    pub async fn submit_block(&self, block: MultivmBlock) -> MultivmResult<BlockResult> {
        let reth_block = self.convert_multivm_block_to_reth(block)?;
        
        let result = self.rpc_client
            .call("multivm_submitTransaction", &reth_block)
            .await?;
            
        Ok(self.convert_reth_result_to_multivm(result)?)
    }
}
```

## Solana Integration

### Solana Validator Configuration

Configure Solana validator for MultiVM integration:

```toml
# solana-validator.toml - Solana configuration for MultiVM

[rpc]
# Enable JSON RPC server
enable_rpc = true
rpc_bind_address = "127.0.0.1:8899"
rpc_max_connections = 1000

# Enable WebSocket for subscriptions
enable_rpc_pubsub = true
rpc_pubsub_bind_address = "127.0.0.1:8900"

# MultiVM-specific settings
[multivm]
coordinator_address = "127.0.0.1:9001"
ipc_socket_path = "/tmp/solana-multivm.sock"
auth_token_path = "/etc/multivm/solana-token.jwt"
enable_block_streaming = true
enable_account_streaming = true
block_commitment = "confirmed"

# Account data streaming configuration
[multivm.account_streaming]
max_accounts_per_request = 1000
stream_buffer_size = 10000
compression_enabled = true

# Transaction streaming
[multivm.transaction_streaming]
include_failed_transactions = false
max_transactions_per_batch = 500
```

### Solana RPC Extensions

Solana validator must implement custom RPC methods:

```rust
// Custom RPC methods for Solana-MultiVM integration
#[derive(Debug)]
pub struct MultivmSolanaRpc {
    bank_forks: Arc<RwLock<BankForks>>,
    cluster_info: Arc<ClusterInfo>,
    multivm_coordinator: MultivmCoordinatorClient,
}

impl MultivmSolanaRpc {
    /// Submit a transaction to Solana for execution
    pub async fn multivm_submit_transaction(
        &self,
        transaction: VersionedTransaction,
        options: MultivmSubmitOptions,
    ) -> RpcResult<MultivmTransactionResult> {
        // Validate transaction
        let sanitized_tx = self.validate_transaction(&transaction)?;
        
        // Execute transaction
        let result = self.process_transaction(sanitized_tx, options).await?;
        
        Ok(MultivmTransactionResult {
            signature: result.signature,
            status: result.status,
            compute_units_used: result.compute_units_used,
            logs: result.logs,
            account_changes: result.account_changes,
        })
    }
    
    /// Get account state for MultiVM coordination
    pub async fn multivm_get_account_state(
        &self,
        pubkey: Pubkey,
        commitment: Option<CommitmentConfig>,
    ) -> RpcResult<MultivmAccountState> {
        let bank = self.get_bank_with_commitment(commitment)?;
        let account = bank.get_account(&pubkey);
        
        Ok(MultivmAccountState {
            pubkey,
            account: account.map(|acc| MultivmAccount {
                lamports: acc.lamports,
                data: acc.data,
                owner: acc.owner,
                executable: acc.executable,
                rent_epoch: acc.rent_epoch,
            }),
            slot: bank.slot(),
            block_height: bank.block_height(),
        })
    }
    
    /// Stream block updates to MultiVM
    pub async fn multivm_subscribe_blocks(
        &self,
        sink: SubscriptionSink,
        commitment: Option<CommitmentConfig>,
    ) -> SubscriptionResult {
        let mut block_subscriber = self.create_block_subscriber(commitment).await?;
        
        tokio::spawn(async move {
            while let Some(block) = block_subscriber.next().await {
                let multivm_block = MultivmSolanaBlock {
                    slot: block.slot,
                    hash: block.blockhash.to_string(),
                    parent_hash: block.previous_blockhash.to_string(),
                    transactions: block.transactions.into_iter()
                        .map(|tx| self.convert_transaction_to_multivm(tx))
                        .collect(),
                    block_time: block.block_time,
                    block_height: block.block_height,
                };
                
                if sink.send(&multivm_block).is_err() {
                    break;
                }
            }
        });
        
        Ok(())
    }
}
```

### Solana Process Management

```rust
pub struct SolanaProcessManager {
    validator_process: Child,
    rpc_client: RpcClient,
    ws_client: PubsubClient,
    ipc_client: UnixStreamClient,
    bank_notification_receiver: Receiver<BankNotification>,
}

impl SolanaProcessManager {
    pub async fn spawn_solana_validator() -> MultivmResult<Self> {
        // Generate Solana keypair for validator
        let keypair = Keypair::new();
        let keypair_path = "/tmp/solana-validator-keypair.json";
        write_keypair_file(&keypair, keypair_path)?;
        
        let mut cmd = Command::new("solana-validator");
        cmd.args(&[
            "--identity", keypair_path,
            "--ledger", "/var/lib/multivm/solana/ledger",
            "--accounts", "/var/lib/multivm/solana/accounts",
            "--rpc-bind-address", "127.0.0.1:8899",
            "--rpc-pubsub-bind-address", "127.0.0.1:8900",
            "--enable-rpc-transaction-history",
            "--enable-extended-tx-metadata-storage",
            "--no-voting",  // MultiVM controls consensus
            "--no-check-vote-account",
        ]);
        
        // Environment variables for MultiVM integration
        cmd.env("MULTIVM_COORDINATOR_ADDR", "127.0.0.1:9001");
        cmd.env("MULTIVM_AUTH_TOKEN", &self.generate_auth_token()?);
        cmd.env("MULTIVM_IPC_SOCKET", "/tmp/solana-multivm.sock");
        
        let validator_process = cmd.spawn()
            .map_err(|e| MultivmError::ProcessSpawnFailed(format!("Solana: {}", e)))?;
            
        // Wait for RPC to be ready
        self.wait_for_rpc_ready().await?;
        
        // Establish connections
        let rpc_client = RpcClient::new("http://127.0.0.1:8899");
        let ws_client = PubsubClient::new("ws://127.0.0.1:8900").await?;
        let ipc_client = UnixStreamClient::connect("/tmp/solana-multivm.sock").await?;
        
        Ok(Self {
            validator_process,
            rpc_client,
            ws_client,
            ipc_client,
            bank_notification_receiver: self.setup_bank_notifications().await?,
        })
    }
    
    pub async fn execute_transaction(
        &self, 
        transaction: MultivmTransaction
    ) -> MultivmResult<TransactionResult> {
        // Convert MultiVM transaction to Solana format
        let solana_tx = self.convert_multivm_transaction_to_solana(transaction)?;
        
        // Submit via RPC
        let result = self.rpc_client
            .send_and_confirm_transaction(&solana_tx)
            .await?;
            
        Ok(self.convert_solana_result_to_multivm(result)?)
    }
}
```

## Message Formats

### Core IPC Message Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IPCMessage {
    pub header: MessageHeader,
    pub payload: MessagePayload,
    pub signature: Option<MessageSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    pub message_id: MessageId,
    pub correlation_id: Option<CorrelationId>,
    pub source: ProcessId,
    pub target: ProcessId,
    pub message_type: MessageType,
    pub timestamp: SystemTime,
    pub auth_token: Option<String>,
    pub compression: CompressionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    Request(RequestPayload),
    Response(ResponsePayload),
    Event(EventPayload),
    Error(ErrorPayload),
}
```

### Request/Response Messages

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestPayload {
    pub method: String,
    pub params: Value,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsePayload {
    pub result: Option<Value>,
    pub error: Option<ErrorInfo>,
    pub metadata: HashMap<String, String>,
}

// Example: Block execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteBlockRequest {
    pub block: MultivmBlock,
    pub parent_state_root: H256,
    pub execution_options: ExecutionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteBlockResponse {
    pub state_root: H256,
    pub receipts: Vec<TransactionReceipt>,
    pub gas_used: U256,
    pub logs: Vec<Log>,
    pub state_changes: Vec<StateChange>,
}
```

### Event Streaming Messages

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub event_type: EventType,
    pub data: Value,
    pub sequence_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    BlockProduced,
    TransactionExecuted,
    StateChanged,
    ConsensusUpdate,
    HealthStatus,
}

// Example: Block produced event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockProducedEvent {
    pub block: MultivmBlock,
    pub execution_result: BlockExecutionResult,
    pub vm_type: VmType,
    pub timestamp: SystemTime,
}
```

## Security Model

### Authentication Protocol

```rust
pub struct IPCAuthenticationManager {
    jwt_secret: SecretKey,
    active_tokens: Arc<RwLock<HashMap<ProcessId, AuthToken>>>,
    token_rotation_interval: Duration,
}

impl IPCAuthenticationManager {
    pub async fn authenticate_process(&self, process_id: ProcessId) -> MultivmResult<AuthToken> {
        // Generate JWT token for process
        let claims = Claims {
            sub: process_id.to_string(),
            exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            aud: "multivm-ipc".to_string(),
            permissions: self.get_process_permissions(&process_id),
        };
        
        let token = encode(&Header::new(Algorithm::HS256), &claims, &self.jwt_secret)?;
        
        // Store token for validation
        self.active_tokens.write().await.insert(process_id, AuthToken {
            token: token.clone(),
            issued_at: SystemTime::now(),
            expires_at: SystemTime::now() + Duration::from_secs(3600),
        });
        
        Ok(AuthToken { token, issued_at: SystemTime::now(), expires_at: SystemTime::now() + Duration::from_secs(3600) })
    }
    
    pub async fn validate_message(&self, message: &IPCMessage) -> MultivmResult<bool> {
        let token = message.header.auth_token
            .as_ref()
            .ok_or(MultivmError::MissingAuthToken)?;
            
        // Decode and validate JWT
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_ref()),
            &Validation::new(Algorithm::HS256),
        )?;
        
        // Check if token is still active
        let active_tokens = self.active_tokens.read().await;
        if let Some(stored_token) = active_tokens.get(&message.header.source) {
            if stored_token.token == *token && stored_token.expires_at > SystemTime::now() {
                return Ok(true);
            }
        }
        
        Err(MultivmError::InvalidAuthToken)
    }
}
```

### Message Encryption

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};

pub struct MessageEncryption {
    cipher: Aes256Gcm,
    nonce_counter: AtomicU64,
}

impl MessageEncryption {
    pub fn encrypt_message(&self, plaintext: &[u8]) -> MultivmResult<EncryptedMessage> {
        let nonce_bytes = self.nonce_counter.fetch_add(1, Ordering::SeqCst).to_le_bytes();
        let mut nonce_array = [0u8; 12];
        nonce_array[..8].copy_from_slice(&nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_array);
        
        let ciphertext = self.cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| MultivmError::EncryptionFailed(e.to_string()))?;
            
        Ok(EncryptedMessage {
            nonce: nonce_array.to_vec(),
            ciphertext,
        })
    }
    
    pub fn decrypt_message(&self, encrypted: &EncryptedMessage) -> MultivmResult<Vec<u8>> {
        let nonce = Nonce::from_slice(&encrypted.nonce);
        
        self.cipher
            .decrypt(nonce, encrypted.ciphertext.as_slice())
            .map_err(|e| MultivmError::DecryptionFailed(e.to_string()))
    }
}
```

## Implementation Guide

### MultiVM Coordinator Implementation

```rust
pub struct MultivmCoordinator {
    reth_manager: RethProcessManager,
    solana_manager: SolanaProcessManager,
    ipc_manager: IPCManager,
    message_router: MessageRouter,
    consensus_engine: ConsensusEngine,
}

impl MultivmCoordinator {
    pub async fn initialize() -> MultivmResult<Self> {
        // Start external processes
        let reth_manager = RethProcessManager::spawn_reth_process().await?;
        let solana_manager = SolanaProcessManager::spawn_solana_validator().await?;
        
        // Initialize IPC
        let ipc_manager = IPCManager::new().await?;
        
        // Set up message routing
        let message_router = MessageRouter::new()
            .add_route("reth", reth_manager.get_ipc_client())
            .add_route("solana", solana_manager.get_ipc_client());
            
        Ok(Self {
            reth_manager,
            solana_manager,
            ipc_manager,
            message_router,
            consensus_engine: ConsensusEngine::new(),
        })
    }
    
    pub async fn execute_cross_vm_transaction(
        &self,
        transaction: CrossVmTransaction,
    ) -> MultivmResult<CrossVmExecutionResult> {
        match transaction.vm_type {
            VmType::Ethereum => {
                let request = ExecuteBlockRequest {
                    block: transaction.into_ethereum_block(),
                    parent_state_root: transaction.parent_state,
                    execution_options: ExecutionOptions::default(),
                };
                
                let response = self.reth_manager
                    .send_request("multivm_executeTransaction", request)
                    .await?;
                    
                Ok(CrossVmExecutionResult::Ethereum(response))
            },
            VmType::Solana => {
                let request = MultivmSubmitOptions {
                    transaction: transaction.into_solana_transaction(),
                    commitment: CommitmentConfig::confirmed(),
                    options: SolanaExecutionOptions::default(),
                };
                
                let response = self.solana_manager
                    .send_request("multivm_submitTransaction", request)
                    .await?;
                    
                Ok(CrossVmExecutionResult::Solana(response))
            },
        }
    }
}
```

### Process Lifecycle Management

```rust
pub struct ProcessLifecycleManager {
    processes: HashMap<ProcessId, ProcessHandle>,
    health_monitors: HashMap<ProcessId, HealthMonitor>,
    restart_policies: HashMap<ProcessId, RestartPolicy>,
}

impl ProcessLifecycleManager {
    pub async fn start_process(&mut self, config: ProcessConfig) -> MultivmResult<ProcessId> {
        let process_id = ProcessId::new();
        
        // Spawn process based on type
        let handle = match config.process_type {
            ProcessType::Reth => self.spawn_reth_process(config).await?,
            ProcessType::Solana => self.spawn_solana_process(config).await?,
        };
        
        // Set up health monitoring
        let health_monitor = HealthMonitor::new(process_id.clone(), config.health_check_config);
        
        self.processes.insert(process_id.clone(), handle);
        self.health_monitors.insert(process_id.clone(), health_monitor);
        
        Ok(process_id)
    }
    
    pub async fn handle_process_failure(&mut self, process_id: ProcessId) -> MultivmResult<()> {
        if let Some(restart_policy) = self.restart_policies.get(&process_id) {
            match restart_policy.strategy {
                RestartStrategy::Always => {
                    self.restart_process(process_id).await?;
                },
                RestartStrategy::OnFailure => {
                    if self.is_process_failed(&process_id) {
                        self.restart_process(process_id).await?;
                    }
                },
                RestartStrategy::Never => {
                    // Mark process as failed and continue
                    self.mark_process_failed(process_id);
                },
            }
        }
        
        Ok(())
    }
}
```

## Error Handling

### Error Types and Recovery

```rust
#[derive(Debug, Error)]
pub enum IPCError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Message serialization failed: {0}")]
    SerializationFailed(String),
    
    #[error("Process not responding: {process_id}")]
    ProcessNotResponding { process_id: ProcessId },
    
    #[error("Invalid message format: {0}")]
    InvalidMessageFormat(String),
    
    #[error("Rate limit exceeded for process: {process_id}")]
    RateLimitExceeded { process_id: ProcessId },
}

pub struct ErrorRecoveryHandler {
    retry_policies: HashMap<IPCError, RetryPolicy>,
    circuit_breakers: HashMap<ProcessId, CircuitBreaker>,
}

impl ErrorRecoveryHandler {
    pub async fn handle_error(&self, error: IPCError, context: ErrorContext) -> RecoveryAction {
        match error {
            IPCError::ConnectionFailed(_) => {
                // Attempt to reconnect with exponential backoff
                RecoveryAction::Retry {
                    delay: Duration::from_millis(100),
                    max_attempts: 5,
                    backoff_multiplier: 2.0,
                }
            },
            IPCError::ProcessNotResponding { process_id } => {
                // Check if process needs restart
                if self.should_restart_process(&process_id).await {
                    RecoveryAction::RestartProcess(process_id)
                } else {
                    RecoveryAction::MarkProcessUnhealthy(process_id)
                }
            },
            IPCError::RateLimitExceeded { process_id } => {
                // Back off and retry with delay
                RecoveryAction::BackoffAndRetry {
                    delay: Duration::from_secs(1),
                    process_id,
                }
            },
            _ => RecoveryAction::Fail,
        }
    }
}
```

### Circuit Breaker Pattern

```rust
pub struct CircuitBreaker {
    state: CircuitBreakerState,
    failure_count: AtomicU32,
    last_failure_time: AtomicU64,
    failure_threshold: u32,
    recovery_timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing fast
    HalfOpen, // Testing recovery
}

impl CircuitBreaker {
    pub async fn call<F, R>(&self, operation: F) -> Result<R, CircuitBreakerError>
    where
        F: Future<Output = Result<R, IPCError>>,
    {
        match self.state {
            CircuitBreakerState::Open => {
                if self.should_attempt_reset() {
                    self.transition_to_half_open();
                } else {
                    return Err(CircuitBreakerError::Open);
                }
            },
            CircuitBreakerState::HalfOpen => {
                // Limited calls allowed to test recovery
            },
            CircuitBreakerState::Closed => {
                // Normal operation
            },
        }
        
        match operation.await {
            Ok(result) => {
                self.on_success();
                Ok(result)
            },
            Err(error) => {
                self.on_failure();
                Err(CircuitBreakerError::OperationFailed(error))
            },
        }
    }
}
```

## Performance Optimization

### Message Batching

```rust
pub struct MessageBatcher {
    pending_messages: Vec<IPCMessage>,
    batch_size: usize,
    batch_timeout: Duration,
    last_flush: Instant,
}

impl MessageBatcher {
    pub async fn add_message(&mut self, message: IPCMessage) -> Option<Vec<IPCMessage>> {
        self.pending_messages.push(message);
        
        // Check if we should flush
        if self.should_flush() {
            Some(self.flush())
        } else {
            None
        }
    }
    
    fn should_flush(&self) -> bool {
        self.pending_messages.len() >= self.batch_size ||
        self.last_flush.elapsed() >= self.batch_timeout
    }
    
    fn flush(&mut self) -> Vec<IPCMessage> {
        let messages = std::mem::take(&mut self.pending_messages);
        self.last_flush = Instant::now();
        messages
    }
}
```

### Connection Pooling

```rust
pub struct ConnectionPool {
    pools: HashMap<ProcessId, Pool<Connection>>,
    pool_config: PoolConfig,
}

impl ConnectionPool {
    pub async fn get_connection(&self, process_id: &ProcessId) -> Result<PooledConnection, PoolError> {
        let pool = self.pools.get(process_id)
            .ok_or(PoolError::PoolNotFound)?;
            
        pool.get().await
    }
    
    pub async fn create_pool_for_process(&mut self, process_id: ProcessId, endpoint: Endpoint) -> Result<(), PoolError> {
        let pool = Pool::builder()
            .max_size(self.pool_config.max_connections)
            .min_idle(self.pool_config.min_idle)
            .connection_timeout(self.pool_config.connection_timeout)
            .build(ConnectionManager::new(endpoint))
            .await?;
            
        self.pools.insert(process_id, pool);
        Ok(())
    }
}
```

## Testing & Validation

### Integration Test Framework

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestEnvironment {
        coordinator: MultivmCoordinator,
        mock_reth: MockRethProcess,
        mock_solana: MockSolanaProcess,
    }
    
    impl TestEnvironment {
        async fn setup() -> Self {
            let mock_reth = MockRethProcess::new().await;
            let mock_solana = MockSolanaProcess::new().await;
            
            let coordinator = MultivmCoordinator::new_with_mocks(
                mock_reth.clone(),
                mock_solana.clone(),
            ).await;
            
            Self {
                coordinator,
                mock_reth,
                mock_solana,
            }
        }
    }
    
    #[tokio::test]
    async fn test_cross_vm_transaction_execution() {
        let env = TestEnvironment::setup().await;
        
        // Create a cross-VM transaction
        let transaction = CrossVmTransaction {
            from_vm: VmType::Ethereum,
            to_vm: VmType::Solana,
            amount: U256::from(1000),
            data: vec![1, 2, 3, 4],
        };
        
        // Set up mock responses
        env.mock_reth
            .expect_execute_transaction()
            .returning(|_| Ok(TransactionResult::success()));
            
        env.mock_solana
            .expect_submit_transaction()
            .returning(|_| Ok(SolanaTransactionResult::success()));
        
        // Execute transaction
        let result = env.coordinator
            .execute_cross_vm_transaction(transaction)
            .await
            .expect("Transaction should succeed");
            
        assert!(result.is_success());
    }
    
    #[tokio::test]
    async fn test_process_failure_recovery() {
        let env = TestEnvironment::setup().await;
        
        // Simulate Reth process failure
        env.mock_reth.simulate_failure().await;
        
        // Coordinator should detect failure and restart
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        assert!(env.coordinator.is_process_healthy("reth").await);
    }
}
```

### Load Testing

```rust
pub struct IPCLoadTester {
    coordinator: MultivmCoordinator,
    metrics_collector: MetricsCollector,
}

impl IPCLoadTester {
    pub async fn run_load_test(&self, config: LoadTestConfig) -> LoadTestResult {
        let start_time = Instant::now();
        let mut handles = Vec::new();
        
        // Spawn concurrent transactions
        for i in 0..config.concurrent_transactions {
            let coordinator = self.coordinator.clone();
            let transaction = self.generate_test_transaction(i);
            
            let handle = tokio::spawn(async move {
                coordinator.execute_cross_vm_transaction(transaction).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all transactions to complete
        let results = futures::future::join_all(handles).await;
        
        let duration = start_time.elapsed();
        let successful_transactions = results.iter()
            .filter(|r| r.is_ok())
            .count();
            
        LoadTestResult {
            total_transactions: config.concurrent_transactions,
            successful_transactions,
            duration,
            throughput: successful_transactions as f64 / duration.as_secs_f64(),
            error_rate: (results.len() - successful_transactions) as f64 / results.len() as f64,
        }
    }
}
```

---

Previous: [API Reference](API_REFERENCE.md) | Next: [Developer Guide](DEVELOPER_GUIDE.md)
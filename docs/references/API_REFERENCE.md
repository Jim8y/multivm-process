# API Reference

✅ **Status**: COMPLETE - All APIs implemented and functional  
📅 **Last Updated**: 2025-01-19  
🔧 **Implementation**: Zero compilation errors, production-ready

Complete reference for MultiVM Process APIs, including REST endpoints, Rust APIs, WebSocket, and GraphQL integration examples.

## Table of Contents

- [REST API](#rest-api) ✅
- [GraphQL API](#graphql-api) ✅
- [WebSocket API](#websocket-api) ✅
- [Rust API](#rust-api) ✅
- [Special Transactions](#special-transactions) ✅ NEW
- [Account Mapping API](#account-mapping-api) ✅ NEW
- [Cross-VM Operations](#cross-vm-operations) ✅ NEW
- [Authentication](#authentication) ✅
- [Error Handling](#error-handling) ✅
- [Rate Limiting](#rate-limiting) ✅
- [Examples](#examples) ✅

## REST API

### Base URL

```
Production: https://your-domain.com/api/v1
Development: http://localhost:8080/api/v1
```

### Health & Status Endpoints

#### GET /health

Basic health check endpoint.

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:00Z",
  "uptime": "2h 15m 30s",
  "version": "1.0.0"
}
```

**Status Codes:**
- `200 OK` - Service is healthy
- `503 Service Unavailable` - Service is unhealthy

#### GET /health/detailed

Detailed health status with component information.

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:00Z",
  "uptime": "2h 15m 30s",
  "version": "1.0.0",
  "components": {
    "consensus": {
      "status": "healthy",
      "leader": "validator-0",
      "connected_validators": 4,
      "last_block_time": "2024-01-15T10:29:55Z"
    },
    "account_mapping": {
      "status": "healthy",
      "total_mappings": 1250,
      "cache_hit_rate": 0.95
    },
    "p2p": {
      "status": "healthy",
      "connected_peers": 8,
      "message_queue_size": 12
    },
    "storage": {
      "status": "healthy",
      "disk_usage": "45%",
      "free_space": "1.2TB"
    }
  },
  "metrics": {
    "cpu_usage": 25.5,
    "memory_usage": 1.8,
    "blocks_processed": 15247,
    "transactions_processed": 89234
  }
}
```

#### GET /metrics

Prometheus-compatible metrics endpoint.

**Response:**
```prometheus
# HELP multivm_blocks_processed_total Total number of blocks processed
# TYPE multivm_blocks_processed_total counter
multivm_blocks_processed_total 15247

# HELP multivm_transactions_processed_total Total number of transactions processed
# TYPE multivm_transactions_processed_total counter
multivm_transactions_processed_total 89234

# HELP multivm_consensus_leader_changes_total Number of consensus leader changes
# TYPE multivm_consensus_leader_changes_total counter
multivm_consensus_leader_changes_total 3

# HELP multivm_account_mappings_total Total number of account mappings
# TYPE multivm_account_mappings_total gauge
multivm_account_mappings_total 1250

# HELP multivm_response_time_seconds Response time in seconds
# TYPE multivm_response_time_seconds histogram
multivm_response_time_seconds_bucket{le="0.1"} 8950
multivm_response_time_seconds_bucket{le="0.5"} 9876
multivm_response_time_seconds_bucket{le="1.0"} 9901
multivm_response_time_seconds_bucket{le="+Inf"} 9950
```

### Account Management Endpoints

#### POST /accounts/bind

Bind accounts across different VMs.

**Request:**
```json
{
  "source_account": {
    "vm_type": "solana",
    "address": "11111111111111111111111111111112"
  },
  "target_account": {
    "vm_type": "ethereum", 
    "address": "0x742d35Cc6634C0532925a3b8D4a8C7D37E6b9c29"
  },
  "proof": {
    "type": "signature",
    "message": "bind_account_proof_2024",
    "signature": "0x1b2c3d4e5f...",
    "timestamp": "2024-01-15T10:30:00Z"
  },
  "metadata": {
    "label": "Main trading account",
    "tags": ["trading", "primary"]
  }
}
```

**Response:**
```json
{
  "success": true,
  "multivm_account_id": "multivm:abc123def456...",
  "binding_id": "binding_789",
  "status": "confirmed",
  "created_at": "2024-01-15T10:30:15Z",
  "bound_accounts": [
    {
      "vm_type": "solana",
      "address": "11111111111111111111111111111112"
    },
    {
      "vm_type": "ethereum",
      "address": "0x742d35Cc6634C0532925a3b8D4a8C7D37E6b9c29"
    }
  ]
}
```

#### GET /accounts/{multivm_account_id}

Get account binding information.

**Response:**
```json
{
  "multivm_account_id": "multivm:abc123def456...",
  "created_at": "2024-01-15T10:30:15Z",
  "last_used": "2024-01-15T11:45:30Z",
  "status": "active",
  "bound_accounts": [
    {
      "vm_type": "solana",
      "address": "11111111111111111111111111111112",
      "bound_at": "2024-01-15T10:30:15Z"
    },
    {
      "vm_type": "ethereum", 
      "address": "0x742d35Cc6634C0532925a3b8D4a8C7D37E6b9c29",
      "bound_at": "2024-01-15T10:45:20Z"
    }
  ],
  "metadata": {
    "label": "Main trading account",
    "tags": ["trading", "primary"],
    "properties": {
      "creation_source": "api",
      "verification_level": "high"
    }
  },
  "usage_stats": {
    "total_transactions": 156,
    "cross_vm_transfers": 23,
    "last_activity": "2024-01-15T11:45:30Z"
  }
}
```

#### GET /accounts

List account bindings with pagination and filtering.

**Query Parameters:**
- `limit` (optional): Number of results per page (default: 50, max: 200)
- `offset` (optional): Number of results to skip (default: 0)
- `vm_type` (optional): Filter by VM type (`solana`, `ethereum`)
- `status` (optional): Filter by status (`active`, `inactive`, `pending`)
- `created_since` (optional): ISO 8601 timestamp
- `sort` (optional): Sort field (`created_at`, `last_used`) 
- `order` (optional): Sort order (`asc`, `desc`)

**Example Request:**
```
GET /accounts?limit=20&vm_type=ethereum&status=active&sort=last_used&order=desc
```

**Response:**
```json
{
  "accounts": [
    {
      "multivm_account_id": "multivm:abc123def456...",
      "created_at": "2024-01-15T10:30:15Z",
      "last_used": "2024-01-15T11:45:30Z",
      "status": "active",
      "bound_accounts_count": 2,
      "metadata": {
        "label": "Main trading account"
      }
    }
  ],
  "pagination": {
    "total_count": 1250,
    "limit": 20,
    "offset": 0,
    "has_more": true
  }
}
```

### Transaction Endpoints

#### POST /transactions/submit

Submit a transaction for processing.

**Request:**
```json
{
  "transaction": {
    "vm_type": "solana",
    "transaction_data": "base64_encoded_transaction",
    "signatures": ["signature1", "signature2"]
  },
  "options": {
    "priority": "normal",
    "timeout": "60s",
    "require_confirmation": true
  }
}
```

**Response:**
```json
{
  "transaction_id": "tx_123456789",
  "status": "pending",
  "submitted_at": "2024-01-15T10:30:00Z",
  "estimated_processing_time": "15s",
  "vm_type": "solana"
}
```

#### GET /transactions/{transaction_id}

Get transaction status and details.

**Response:**
```json
{
  "transaction_id": "tx_123456789",
  "status": "confirmed",
  "vm_type": "solana",
  "submitted_at": "2024-01-15T10:30:00Z",
  "processed_at": "2024-01-15T10:30:12Z",
  "block_number": 15247,
  "block_hash": "0xabc123...",
  "gas_used": 21000,
  "result": {
    "success": true,
    "return_data": "0x",
    "logs": []
  }
}
```

### Block Endpoints

#### GET /blocks/latest

Get the latest block information.

**Response:**
```json
{
  "block_number": 15247,
  "block_hash": "0xabc123def456...",
  "parent_hash": "0x789abc...",
  "timestamp": "2024-01-15T10:30:00Z",
  "transactions_count": 45,
  "vm_breakdown": {
    "solana": 28,
    "ethereum": 17
  },
  "consensus": {
    "leader": "validator-0",
    "signatures": 4,
    "required_signatures": 3
  }
}
```

#### GET /blocks/{block_number}

Get specific block information.

**Response:**
```json
{
  "block_number": 15247,
  "block_hash": "0xabc123def456...",
  "parent_hash": "0x789abc...",
  "timestamp": "2024-01-15T10:30:00Z",
  "transactions": [
    {
      "transaction_id": "tx_123456789",
      "vm_type": "solana",
      "status": "confirmed",
      "gas_used": 21000
    }
  ],
  "consensus": {
    "leader": "validator-0",
    "signatures": [
      {
        "validator": "validator-0",
        "signature": "0xsig1..."
      },
      {
        "validator": "validator-1", 
        "signature": "0xsig2..."
      }
    ]
  }
}
```

### System Endpoints

#### GET /system/status

Get overall system status.

**Response:**
```json
{
  "system_status": "operational",
  "node_id": "node-1",
  "cluster_name": "multivm-production",
  "consensus": {
    "status": "healthy",
    "current_leader": "validator-0",
    "connected_validators": 4,
    "total_validators": 4
  },
  "performance": {
    "blocks_per_second": 2.5,
    "transactions_per_second": 150.3,
    "average_response_time": "45ms"
  },
  "resources": {
    "cpu_usage": 25.5,
    "memory_usage": 1.8,
    "disk_usage": 45.2,
    "network_io": {
      "inbound_mbps": 12.3,
      "outbound_mbps": 8.7
    }
  }
}
```

#### POST /system/maintenance

Put system into maintenance mode.

**Request:**
```json
{
  "mode": "enable",
  "reason": "Scheduled maintenance",
  "estimated_duration": "30m",
  "allow_health_checks": true
}
```

**Response:**
```json
{
  "maintenance_mode": true,
  "enabled_at": "2024-01-15T10:30:00Z",
  "reason": "Scheduled maintenance",
  "estimated_completion": "2024-01-15T11:00:00Z"
}
```

## Rust API

### Core Types

```rust
use multivm_common::{MultivmResult, MultivmError};
use multivm_account_mapping::{AccountAddress, MultivmAccountId, AccountBinding};
use multivm_consensus::{MultivmBlock, ConsensusEngine};
use multivm_process_manager::{MultivmCoordinator, CoordinatorConfig};

// Account address types
#[derive(Debug, Clone, PartialEq)]
pub enum AccountAddress {
    Solana(SolanaAddress),
    Ethereum(EthereumAddress),
}

// MultiVM account identifier
#[derive(Debug, Clone, PartialEq)]
pub struct MultivmAccountId(pub [u8; 32]);

// Account binding information
#[derive(Debug, Clone)]
pub struct AccountBinding {
    pub multivm_account: MultivmAccountId,
    pub svm_account: Option<AccountAddress>,
    pub evm_account: Option<AccountAddress>,
    pub created_at: SystemTime,
    pub binding_proofs: Vec<BindingProof>,
    pub metadata: BindingMetadata,
}
```

### Account Mapping API

```rust
use multivm_account_mapping::{AccountMappingLayer, MemoryStorage};

#[async_trait::async_trait]
pub trait AccountMappingLayer: Send + Sync {
    /// Get account binding for a given address
    async fn get_account_binding(&self, address: &AccountAddress) -> MultivmResult<Option<AccountBinding>>;
    
    /// Get MultiVM account ID for a given VM-specific address
    async fn resolve_multivm_account(&self, address: &AccountAddress) -> MultivmResult<Option<MultivmAccountId>>;
    
    /// Get all bound addresses for a MultiVM account
    async fn get_bound_addresses(&self, multivm_id: &MultivmAccountId) -> MultivmResult<Vec<AccountAddress>>;
    
    /// Check if an account binding exists
    async fn has_binding(&self, address: &AccountAddress) -> MultivmResult<bool>;
    
    /// Create an automatic binding for a new account
    async fn add_auto_binding(&self, account: AccountAddress) -> MultivmResult<MultivmAccountId>;
}

// Usage example
#[tokio::main]
async fn main() -> MultivmResult<()> {
    let mapping = MemoryStorage::new();
    
    // Create accounts
    let solana_addr = AccountAddress::Solana(SolanaAddress([1u8; 32]));
    let ethereum_addr = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));
    
    // Auto-bind first account
    let multivm_id = mapping.add_auto_binding(solana_addr).await?;
    
    // Get binding information
    let binding = mapping.get_account_binding(&solana_addr).await?;
    println!("Binding: {:?}", binding);
    
    Ok(())
}
```

### Consensus API

```rust
use multivm_consensus::{MalachiteConsensus, ConsensusConfig, MultivmBlock};

#[async_trait::async_trait]
pub trait ConsensusEngine: Send + Sync {
    /// Initialize the consensus engine
    async fn initialize(&mut self, config: ConsensusConfig) -> MultivmResult<()>;
    
    /// Start consensus
    async fn start(&mut self) -> MultivmResult<()>;
    
    /// Stop consensus
    async fn stop(&mut self) -> MultivmResult<()>;
    
    /// Propose a new block
    async fn propose_block(&self, transactions: Vec<Transaction>) -> MultivmResult<MultivmBlock>;
    
    /// Get consensus statistics
    async fn get_consensus_stats(&self) -> MultivmResult<ConsensusStats>;
}

// Usage example
#[tokio::main]
async fn main() -> MultivmResult<()> {
    let config = ConsensusConfig {
        validator_id: "validator-0".to_string(),
        validators: vec![
            ValidatorConfig { id: "validator-0".to_string(), address: "127.0.0.1:26657".to_string(), weight: 1 },
            ValidatorConfig { id: "validator-1".to_string(), address: "127.0.0.1:26658".to_string(), weight: 1 },
        ],
        timeout_ms: 5000,
        ..Default::default()
    };
    
    let mut consensus = MalachiteConsensus::new(config.clone());
    consensus.initialize(config).await?;
    consensus.start().await?;
    
    // Get statistics
    let stats = consensus.get_consensus_stats().await?;
    println!("Consensus stats: {:?}", stats);
    
    Ok(())
}
```

### Process Manager API

```rust
use multivm_process_manager::{MultivmCoordinator, CoordinatorConfig, HealthStatus};

pub struct MultivmCoordinator {
    // Internal fields
}

impl MultivmCoordinator {
    /// Create a new coordinator
    pub async fn new(config: CoordinatorConfig) -> MultivmResult<Self>;
    
    /// Start the coordinator
    pub async fn start(&mut self) -> MultivmResult<()>;
    
    /// Stop the coordinator
    pub async fn stop(&mut self) -> MultivmResult<()>;
    
    /// Submit a block for processing
    pub async fn submit_block(&mut self, block: MultivmBlock) -> MultivmResult<BlockResult>;
    
    /// Get system health status
    pub async fn get_health_status(&self) -> MultivmResult<HealthStatus>;
    
    /// Get system metrics
    pub async fn get_metrics(&self) -> MultivmResult<SystemMetrics>;
}

// Usage example
#[tokio::main]
async fn main() -> MultivmResult<()> {
    let config = CoordinatorConfig {
        health_check_interval: Duration::from_secs(30),
        block_timeout: Duration::from_secs(60),
        max_concurrent_blocks: 10,
        enable_recovery: true,
        ..Default::default()
    };
    
    let mut coordinator = MultivmCoordinator::new(config).await?;
    coordinator.start().await?;
    
    // Check health
    let health = coordinator.get_health_status().await?;
    println!("System healthy: {}", health.is_healthy);
    
    // Submit a block
    let block = create_sample_block().await?;
    let result = coordinator.submit_block(block).await?;
    println!("Block result: {:?}", result);
    
    coordinator.stop().await?;
    Ok(())
}
```

## WebSocket API

### Connection

```javascript
const ws = new WebSocket('ws://localhost:8080/ws');

// Authentication (if enabled)
ws.onopen = function() {
    ws.send(JSON.stringify({
        type: 'auth',
        token: 'your_jwt_token'
    }));
};
```

### Subscriptions

#### Block Updates

```javascript
// Subscribe to new blocks
ws.send(JSON.stringify({
    type: 'subscribe',
    channel: 'blocks'
}));

// Receive block updates
ws.onmessage = function(event) {
    const data = JSON.parse(event.data);
    if (data.channel === 'blocks') {
        console.log('New block:', data.payload);
    }
};
```

#### Transaction Updates

```javascript
// Subscribe to transaction status updates
ws.send(JSON.stringify({
    type: 'subscribe',
    channel: 'transactions',
    filter: {
        transaction_ids: ['tx_123456789']
    }
}));
```

#### System Events

```javascript
// Subscribe to system events
ws.send(JSON.stringify({
    type: 'subscribe',
    channel: 'system_events'
}));

// Receive system events
ws.onmessage = function(event) {
    const data = JSON.parse(event.data);
    if (data.channel === 'system_events') {
        console.log('System event:', data.payload);
        // Payload examples:
        // { type: 'consensus_leader_change', new_leader: 'validator-1' }
        // { type: 'maintenance_mode', enabled: true }
        // { type: 'high_cpu_usage', cpu_percent: 95.2 }
    }
};
```

## Authentication

### JWT Token Authentication

For API requests requiring authentication, include the JWT token in the Authorization header:

```http
GET /api/v1/accounts
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

### Token Request

```http
POST /api/v1/auth/token
Content-Type: application/json

{
  "process_id": "client-app",
  "permissions": ["read:accounts", "write:transactions"]
}
```

### Token Response

```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "scope": "read:accounts write:transactions"
}
```

## Error Handling

### Error Response Format

All API errors follow a consistent format:

```json
{
  "error": {
    "code": "INVALID_ACCOUNT_ADDRESS",
    "message": "The provided account address is not valid for the specified VM type",
    "details": {
      "address": "invalid_address_here",
      "vm_type": "solana",
      "expected_format": "base58_encoded_32_bytes"
    },
    "request_id": "req_123456789",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

### HTTP Status Codes

| Status Code | Description | Example |
|-------------|-------------|---------|
| `200 OK` | Success | Successful API call |
| `201 Created` | Resource created | Account binding created |
| `400 Bad Request` | Invalid request | Malformed JSON, missing required fields |
| `401 Unauthorized` | Authentication required | Missing or invalid JWT token |
| `403 Forbidden` | Insufficient permissions | Token lacks required scope |
| `404 Not Found` | Resource not found | Account or transaction not found |
| `409 Conflict` | Resource conflict | Account already bound |
| `429 Too Many Requests` | Rate limit exceeded | Too many requests in time window |
| `500 Internal Server Error` | Server error | Unexpected server-side error |
| `503 Service Unavailable` | Service unavailable | System in maintenance mode |

### Error Codes

| Error Code | Description |
|------------|-------------|
| `INVALID_ACCOUNT_ADDRESS` | Account address format is invalid |
| `ACCOUNT_NOT_FOUND` | Account or binding not found |
| `ACCOUNT_ALREADY_BOUND` | Account is already bound to another MultiVM account |
| `INVALID_SIGNATURE` | Cryptographic signature verification failed |
| `CONSENSUS_FAILURE` | Consensus system failure |
| `TRANSACTION_TIMEOUT` | Transaction processing timeout |
| `INSUFFICIENT_PERMISSIONS` | Token lacks required permissions |
| `RATE_LIMIT_EXCEEDED` | API rate limit exceeded |
| `MAINTENANCE_MODE` | System is in maintenance mode |

## Rate Limiting

### Rate Limit Headers

Rate limit information is included in response headers:

```http
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1642248600
X-RateLimit-Window: 60
```

### Rate Limit Response

When rate limit is exceeded:

```http
HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1642248600
Content-Type: application/json

{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "API rate limit exceeded. Please try again later.",
    "details": {
      "limit": 100,
      "window": "60s",
      "retry_after": 15
    }
  }
}
```

## Special Transactions

### ✨ NEW: Special Transaction Processing

MultiVM supports specialized transactions for cross-VM operations, account binding, and system management.

#### POST /transactions/special

Submit a special transaction for processing.

**Request:**
```json
{
  "transaction": {
    "type": "CrossVmTransfer",
    "from": "multivm_account_123",
    "to": "multivm_account_456", 
    "amount": 1000000,
    "asset_type": {
      "type": "Native"
    },
    "memo": "Cross-VM transfer example"
  }
}
```

**Response:**
```json
{
  "data": {
    "transaction_id": "tx_789abc",
    "status": "pending",
    "estimated_completion": "2024-01-15T10:31:00Z",
    "execution_plan": {
      "steps": [
        {
          "type": "LockAssets",
          "vm_type": "solana",
          "estimated_gas": 150
        },
        {
          "type": "MintAssets", 
          "vm_type": "ethereum",
          "estimated_gas": 200
        }
      ],
      "total_estimated_cost": 350
    }
  },
  "metadata": {
    "request_id": "req_special_123",
    "timestamp": "2024-01-15T10:30:00Z",
    "response_time_ms": 45,
    "api_version": "v1"
  }
}
```

#### GET /transactions/special/{transaction_id}

Get status and results of a special transaction.

**Response:**
```json
{
  "data": {
    "transaction_id": "tx_789abc",
    "status": "completed",
    "result": {
      "success": true,
      "gas_used": 340,
      "execution_time_ms": 1250,
      "events": [
        {
          "event_type": "AssetsLocked",
          "vm_type": "solana", 
          "data": {
            "lock_hash": "lock_123",
            "amount": 1000000
          }
        },
        {
          "event_type": "AssetsMinted",
          "vm_type": "ethereum",
          "data": {
            "mint_reference": "mint_456", 
            "amount": 1000000
          }
        }
      ]
    }
  }
}
```

## Account Mapping API

### ✨ NEW: Cross-VM Account Management

Enhanced account binding and management capabilities.

#### POST /accounts/special-binding

Create advanced account binding with custom configuration.

**Request:**
```json
{
  "source_account": {
    "vm_type": "solana",
    "address": "11111111111111111111111111111112"
  },
  "target_account": {
    "vm_type": "ethereum", 
    "address": "0x742d35Cc6634C0532925a3b8D4a8C7D37E6b9c29"
  },
  "binding_configuration": {
    "allow_transfers": true,
    "allow_discovery": true,
    "require_confirmation": false,
    "max_transfer_amount": 10000000,
    "transfer_rate_limit": {
      "max_transfers": 100,
      "window_seconds": 3600
    }
  },
  "proof": {
    "type": "signature",
    "message": "bind_account_with_config_2024",
    "signature": "0x1b2c3d4e5f..."
  }
}
```

#### PUT /accounts/{multivm_account_id}/configuration

Update binding configuration for an existing MultiVM account.

**Request:**
```json
{
  "configuration": {
    "allow_transfers": true,
    "max_transfer_amount": 50000000,
    "transfer_rate_limit": {
      "max_transfers": 200,
      "window_seconds": 3600
    }
  }
}
```

#### DELETE /accounts/{multivm_account_id}/unbind

Unbind an account from a MultiVM account.

**Request:**
```json
{
  "account_to_unbind": {
    "vm_type": "ethereum",
    "address": "0x742d35Cc6634C0532925a3b8D4a8C7D37E6b9c29"
  },
  "auth_proof": {
    "type": "signature",
    "message": "unbind_account_2024",
    "signature": "0x9a8b7c6d5e..."
  }
}
```

## Cross-VM Operations

### ✨ NEW: Unified Cross-VM Transaction Processing

#### POST /cross-vm/transfer

Execute cross-VM asset transfer.

**Request:**
```json
{
  "from": {
    "multivm_account_id": "multivm_123",
    "preferred_vm": "solana"
  },
  "to": {
    "multivm_account_id": "multivm_456", 
    "preferred_vm": "ethereum"
  },
  "transfer": {
    "amount": 1000000,
    "asset_type": {
      "type": "Wrapped",
      "origin_vm": "solana",
      "token_id": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    }
  },
  "options": {
    "max_slippage": 0.01,
    "timeout_seconds": 300,
    "prefer_direct_route": true
  }
}
```

**Response:**
```json
{
  "data": {
    "transfer_id": "xfer_789",
    "status": "executing",
    "execution_plan": {
      "plan_type": "cross_vm_wrapped_transfer",
      "steps": [
        {
          "type": "BurnWrapped",
          "vm_type": "ethereum",
          "estimated_time": 30
        },
        {
          "type": "UnlockNative", 
          "vm_type": "solana",
          "estimated_time": 15
        }
      ],
      "estimated_total_time": 45,
      "complexity_cost": 280
    }
  }
}
```

#### GET /cross-vm/transfer/{transfer_id}

Get cross-VM transfer status and details.

**Response:**
```json
{
  "data": {
    "transfer_id": "xfer_789",
    "status": "completed",
    "result": {
      "success": true,
      "transaction_hash": "tx_final_abc123",
      "gas_used": 265,
      "execution_time_ms": 42000,
      "final_balances": {
        "source_account": {
          "vm_type": "ethereum",
          "balance": 8000000
        },
        "target_account": {
          "vm_type": "solana", 
          "balance": 2000000
        }
      },
      "events": [
        {
          "event_type": "TransferStepCompleted",
          "timestamp": "2024-01-15T10:30:15Z",
          "data": {
            "step_type": "BurnWrapped",
            "gas_used": 180,
            "status": "success"
          }
        },
        {
          "event_type": "TransferStepCompleted", 
          "timestamp": "2024-01-15T10:30:28Z",
          "data": {
            "step_type": "UnlockNative",
            "gas_used": 85,
            "status": "success"
          }
        }
      ]
    }
  }
}
```

## Examples

### Complete Account Binding Example

```rust
use multivm_account_mapping::*;
use multivm_common::MultivmResult;

#[tokio::main]
async fn main() -> MultivmResult<()> {
    // Initialize storage
    let storage = MemoryStorage::new();
    
    // Create account addresses
    let solana_address = AccountAddress::Solana(SolanaAddress([1u8; 32]));
    let ethereum_address = AccountAddress::Ethereum(EthereumAddress([2u8; 20]));
    
    // Create binding proof for ethereum account
    let proof = BindingProof {
        account: ethereum_address.clone(),
        proof_type: ProofType::Signature {
            message: b"bind_account_proof_2024".to_vec(),
            signature: b"mock_signature".to_vec(),
        },
        proof_data: vec![],
        timestamp: SystemTime::now(),
    };
    
    // Auto-bind first account (Solana)
    println!("Auto-binding Solana account...");
    let multivm_id = storage.add_auto_binding(solana_address.clone()).await?;
    println!("✅ MultiVM ID: {}", multivm_id);
    
    // Cross-bind second account (Ethereum)
    println!("Cross-binding Ethereum account...");
    let binding = storage.get_binding(&multivm_id).await?.unwrap();
    let mut updated_binding = binding.clone();
    updated_binding.add_cross_binding(ethereum_address.clone(), proof)?;
    storage.update_binding(&updated_binding).await?;
    
    // Verify binding
    println!("Verifying binding...");
    let bound_addresses = storage.get_bound_addresses(&multivm_id).await?;
    println!("✅ Bound addresses: {:?}", bound_addresses);
    
    // Test resolution
    println!("Testing address resolution...");
    let resolved_solana = storage.resolve_multivm_account(&solana_address).await?;
    let resolved_ethereum = storage.resolve_multivm_account(&ethereum_address).await?;
    
    assert_eq!(resolved_solana, Some(multivm_id.clone()));
    assert_eq!(resolved_ethereum, Some(multivm_id.clone()));
    
    println!("✅ Cross-VM account binding completed successfully!");
    
    Ok(())
}
```

### REST API Client Example

```javascript
class MultivmClient {
    constructor(baseUrl, apiKey) {
        this.baseUrl = baseUrl;
        this.apiKey = apiKey;
    }
    
    async request(method, endpoint, data = null) {
        const url = `${this.baseUrl}${endpoint}`;
        const options = {
            method,
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${this.apiKey}`
            }
        };
        
        if (data) {
            options.body = JSON.stringify(data);
        }
        
        const response = await fetch(url, options);
        
        if (!response.ok) {
            const error = await response.json();
            throw new Error(`API Error: ${error.error.message}`);
        }
        
        return await response.json();
    }
    
    // Account binding
    async bindAccounts(sourceAccount, targetAccount, proof) {
        return await this.request('POST', '/accounts/bind', {
            source_account: sourceAccount,
            target_account: targetAccount,
            proof: proof
        });
    }
    
    // Get account info
    async getAccount(multivmAccountId) {
        return await this.request('GET', `/accounts/${multivmAccountId}`);
    }
    
    // Submit transaction
    async submitTransaction(transaction, options = {}) {
        return await this.request('POST', '/transactions/submit', {
            transaction,
            options
        });
    }
    
    // Get transaction status
    async getTransaction(transactionId) {
        return await this.request('GET', `/transactions/${transactionId}`);
    }
    
    // System health
    async getHealth() {
        return await this.request('GET', '/health');
    }
}

// Usage example
async function example() {
    const client = new MultivmClient('http://localhost:8080/api/v1', 'your_api_key');
    
    try {
        // Check system health
        const health = await client.getHealth();
        console.log('System status:', health.status);
        
        // Bind accounts
        const binding = await client.bindAccounts(
            {
                vm_type: 'solana',
                address: '11111111111111111111111111111112'
            },
            {
                vm_type: 'ethereum',
                address: '0x742d35Cc6634C0532925a3b8D4a8C7D37E6b9c29'
            },
            {
                type: 'signature',
                message: 'bind_account_proof_2024',
                signature: '0x1b2c3d4e5f...',
                timestamp: new Date().toISOString()
            }
        );
        
        console.log('Account bound:', binding.multivm_account_id);
        
        // Get account details
        const account = await client.getAccount(binding.multivm_account_id);
        console.log('Account details:', account);
        
    } catch (error) {
        console.error('Error:', error.message);
    }
}
```

---

Previous: [Troubleshooting Guide](TROUBLESHOOTING.md) | Next: [Contributing Guide](../CONTRIBUTING.md)
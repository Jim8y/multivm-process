# MultiVM API Reference

This document provides comprehensive API reference for the MultiVM platform.

## Base URLs

- **REST API**: `http://localhost:8080/api/v1`
- **GraphQL**: `http://localhost:8081/graphql`
- **WebSocket**: `ws://localhost:8082/ws`
- **Admin**: `http://localhost:8083/admin`
- **Health**: `http://localhost:8090/health`
- **Metrics**: `http://localhost:9090/metrics`

## Authentication

All API endpoints require authentication via JWT tokens or API keys.

### JWT Authentication
```http
Authorization: Bearer <jwt_token>
```

### API Key Authentication
```http
X-API-Key: <api_key>
```

## REST API Endpoints

### Health and Status

#### GET /health
Returns the health status of the platform.

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2025-01-01T00:00:00Z",
  "components": {
    "database": "healthy",
    "cache": "healthy",
    "consensus": "healthy"
  }
}
```

#### GET /status
Returns detailed system status and metrics.

**Response:**
```json
{
  "uptime": 3600,
  "version": "0.1.0",
  "node_id": "node-123",
  "consensus": {
    "current_height": 1000,
    "validator_count": 4
  },
  "vm_engines": {
    "solana": "active",
    "ethereum": "active"
  }
}
```

### Account Management

#### POST /accounts/bind
Bind accounts across different VMs.

**Request:**
```json
{
  "solana_address": "11111111111111111111111111111112",
  "ethereum_address": "0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db5C",
  "proof": "binding_proof_data"
}
```

**Response:**
```json
{
  "binding_id": "binding-123",
  "status": "confirmed",
  "created_at": "2025-01-01T00:00:00Z"
}
```

#### GET /accounts/{address}/bindings
Get all bindings for an account.

**Response:**
```json
{
  "address": "11111111111111111111111111111112",
  "bindings": [
    {
      "target_address": "0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db5C",
      "target_vm": "ethereum",
      "status": "active",
      "created_at": "2025-01-01T00:00:00Z"
    }
  ]
}
```

### Transaction Processing

#### POST /transactions/cross-vm
Submit a cross-VM transaction.

**Request:**
```json
{
  "source_vm": "solana",
  "target_vm": "ethereum", 
  "source_address": "11111111111111111111111111111112",
  "target_address": "0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db5C",
  "amount": "1000000",
  "data": "transaction_data"
}
```

**Response:**
```json
{
  "transaction_id": "tx-123",
  "status": "pending",
  "estimated_confirmation_time": 30
}
```

#### GET /transactions/{id}
Get transaction status and details.

**Response:**
```json
{
  "transaction_id": "tx-123",
  "status": "confirmed",
  "source_vm": "solana",
  "target_vm": "ethereum",
  "confirmations": 6,
  "created_at": "2025-01-01T00:00:00Z",
  "confirmed_at": "2025-01-01T00:01:00Z"
}
```

### VM Operations

#### GET /vm/solana/status
Get Solana VM status.

**Response:**
```json
{
  "status": "active",
  "current_slot": 1000,
  "epoch": 100,
  "health": "healthy"
}
```

#### GET /vm/ethereum/status  
Get Ethereum VM status.

**Response:**
```json
{
  "status": "active",
  "current_block": 2000,
  "gas_price": "20000000000",
  "health": "healthy"
}
```

## GraphQL API

### Schema Overview

```graphql
type Query {
  account(address: String!): Account
  transaction(id: String!): Transaction
  vmStatus(vm: VmType!): VmStatus
  systemStatus: SystemStatus
}

type Mutation {
  bindAccounts(input: BindAccountsInput!): BindingResult
  submitTransaction(input: TransactionInput!): TransactionResult
}

type Subscription {
  transactionUpdates(id: String!): Transaction
  systemEvents: SystemEvent
}
```

### Example Queries

#### Get Account Information
```graphql
query GetAccount($address: String!) {
  account(address: $address) {
    address
    vm_type
    bindings {
      target_address
      target_vm
      status
    }
    balance
  }
}
```

#### Submit Cross-VM Transaction
```graphql
mutation SubmitCrossVmTransaction($input: TransactionInput!) {
  submitTransaction(input: $input) {
    transaction_id
    status
    estimated_confirmation_time
  }
}
```

#### Subscribe to Transaction Updates
```graphql
subscription TransactionUpdates($id: String!) {
  transactionUpdates(id: $id) {
    transaction_id
    status
    confirmations
    updated_at
  }
}
```

## WebSocket API

### Connection
```javascript
const ws = new WebSocket('ws://localhost:8082/ws');
```

### Message Format
```json
{
  "type": "message_type",
  "data": { ... },
  "timestamp": "2025-01-01T00:00:00Z"
}
```

### Message Types

#### Transaction Updates
```json
{
  "type": "transaction_update",
  "data": {
    "transaction_id": "tx-123",
    "status": "confirmed",
    "confirmations": 6
  }
}
```

#### System Events
```json
{
  "type": "system_event",
  "data": {
    "event": "consensus_height_update",
    "height": 1001
  }
}
```

#### Account Notifications
```json
{
  "type": "account_notification",
  "data": {
    "address": "11111111111111111111111111111112",
    "event": "balance_update",
    "new_balance": "2000000"
  }
}
```

## Error Responses

All APIs use consistent error response format:

```json
{
  "error": {
    "code": "INVALID_ADDRESS",
    "message": "The provided address is not valid",
    "details": {
      "field": "ethereum_address",
      "value": "invalid_address"
    }
  },
  "request_id": "req-123",
  "timestamp": "2025-01-01T00:00:00Z"
}
```

### Common Error Codes

- `INVALID_ADDRESS`: Invalid blockchain address
- `INSUFFICIENT_BALANCE`: Insufficient account balance
- `BINDING_NOT_FOUND`: Account binding not found
- `TRANSACTION_FAILED`: Transaction execution failed
- `RATE_LIMIT_EXCEEDED`: API rate limit exceeded
- `UNAUTHORIZED`: Authentication required
- `FORBIDDEN`: Insufficient permissions
- `INTERNAL_ERROR`: Internal server error

## Rate Limits

- **Default**: 1000 requests per minute per API key
- **Burst**: Up to 100 requests in 10 seconds
- **WebSocket**: 10 connections per IP

Rate limit headers:
```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1640995200
```

## SDK Examples

### JavaScript/TypeScript
```typescript
import { MultivmClient } from '@multivm/sdk';

const client = new MultivmClient({
  baseUrl: 'http://localhost:8080',
  apiKey: 'your-api-key'
});

// Bind accounts
const binding = await client.accounts.bind({
  solana_address: '11111111111111111111111111111112',
  ethereum_address: '0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db5C',
  proof: 'binding_proof'
});

// Submit cross-VM transaction
const transaction = await client.transactions.submitCrossVm({
  source_vm: 'solana',
  target_vm: 'ethereum',
  source_address: '11111111111111111111111111111112',
  target_address: '0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db5C',
  amount: '1000000'
});
```

### Rust
```rust
use multivm_client::MultivmClient;

let client = MultivmClient::new("http://localhost:8080", "your-api-key");

// Bind accounts
let binding = client.accounts().bind(BindAccountsRequest {
    solana_address: "11111111111111111111111111111112".to_string(),
    ethereum_address: "0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db5C".to_string(),
    proof: "binding_proof".to_string(),
}).await?;

// Submit cross-VM transaction  
let transaction = client.transactions().submit_cross_vm(CrossVmTransactionRequest {
    source_vm: VmType::Solana,
    target_vm: VmType::Ethereum,
    source_address: "11111111111111111111111111111112".to_string(),
    target_address: "0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db5C".to_string(),
    amount: "1000000".to_string(),
    data: None,
}).await?;
```

## Monitoring and Metrics

### Prometheus Metrics

Available at `http://localhost:9090/metrics`:

- `multivm_transactions_total`: Total number of transactions
- `multivm_transactions_duration_seconds`: Transaction processing time
- `multivm_accounts_bound_total`: Total number of bound accounts
- `multivm_vm_status`: VM engine status (0=inactive, 1=active)
- `multivm_consensus_height`: Current consensus height
- `multivm_http_requests_total`: HTTP request count by endpoint
- `multivm_websocket_connections`: Active WebSocket connections

### Health Check Endpoints

- `GET /health`: Basic health check
- `GET /health/detailed`: Detailed component health
- `GET /health/ready`: Readiness probe for Kubernetes
- `GET /health/live`: Liveness probe for Kubernetes
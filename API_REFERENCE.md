# MultiVM API Reference

## Overview

The MultiVM API provides comprehensive access to cross-VM blockchain operations through REST, GraphQL, and WebSocket interfaces.

## Base URLs

- **REST API**: `https://api.multivm.org/v1`
- **GraphQL**: `https://api.multivm.org/graphql`
- **WebSocket**: `wss://api.multivm.org/ws`

## Authentication

All API requests require authentication using either JWT tokens or API keys.

### JWT Authentication

```bash
# Login
curl -X POST https://api.multivm.org/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "user", "password": "pass"}'

# Use token
curl -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  https://api.multivm.org/v1/accounts
```

### API Key Authentication

```bash
curl -H "X-API-Key: YOUR_API_KEY" \
  https://api.multivm.org/v1/accounts
```

## REST API Endpoints

### Account Management

#### Create MultiVM Account

```http
POST /v1/accounts
Content-Type: application/json

{
  "name": "My MultiVM Account",
  "description": "Primary trading account"
}
```

**Response:**
```json
{
  "id": "multivm:7f8a9b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f",
  "name": "My MultiVM Account",
  "created_at": "2024-01-15T10:30:00Z"
}
```

#### Bind Account

```http
POST /v1/accounts/{id}/bind
Content-Type: application/json

{
  "target_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD7E",
  "vm_type": "evm",
  "proof": {
    "type": "signature",
    "data": "0x...",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

#### List Bound Addresses

```http
GET /v1/accounts/{id}/addresses
```

**Response:**
```json
{
  "addresses": [
    {
      "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD7E",
      "vm_type": "evm",
      "bound_at": "2024-01-15T10:30:00Z"
    },
    {
      "address": "7S3P4HxJpyyDjpcJKJGDYLgEBHyYcCjED2QTtzhRpQmU",
      "vm_type": "svm",
      "bound_at": "2024-01-15T10:35:00Z"
    }
  ]
}
```

### Cross-VM Operations

#### Transfer Assets

```http
POST /v1/transfers
Content-Type: application/json

{
  "from": "multivm:7f8a9b1c...",
  "to": "multivm:8a9b2c3d...",
  "amount": "1000000000",
  "asset": {
    "type": "native",
    "vm": "evm"
  },
  "memo": "Payment for services"
}
```

**Response:**
```json
{
  "transaction_id": "tx_123456789",
  "status": "pending",
  "estimated_completion": "2024-01-15T10:32:00Z",
  "source_tx": "0x...",
  "target_tx": null
}
```

#### Atomic Swap

```http
POST /v1/swaps
Content-Type: application/json

{
  "party_a": {
    "account": "multivm:7f8a9b1c...",
    "asset": "ETH",
    "amount": "1000000000000000000"
  },
  "party_b": {
    "account": "multivm:8a9b2c3d...",
    "asset": "SOL",
    "amount": "50000000000"
  },
  "expires_at": "2024-01-15T11:00:00Z"
}
```

### Transaction Queries

#### Get Transaction Status

```http
GET /v1/transactions/{id}
```

**Response:**
```json
{
  "id": "tx_123456789",
  "type": "cross_vm_transfer",
  "status": "completed",
  "source": {
    "vm": "evm",
    "tx_hash": "0x...",
    "confirmations": 12,
    "gas_used": "21000"
  },
  "target": {
    "vm": "svm",
    "tx_hash": "5xY3p...",
    "confirmations": 32,
    "compute_units": "5000"
  },
  "created_at": "2024-01-15T10:30:00Z",
  "completed_at": "2024-01-15T10:31:30Z"
}
```

#### List Transactions

```http
GET /v1/transactions?account={id}&limit=10&offset=0
```

### Blockchain State

#### EVM Block Info

```http
GET /v1/evm/blocks/latest
```

**Response:**
```json
{
  "number": 18500000,
  "hash": "0x...",
  "parent_hash": "0x...",
  "timestamp": 1700000000,
  "transactions": 250,
  "gas_used": "15000000",
  "gas_limit": "30000000"
}
```

#### SVM Slot Info

```http
GET /v1/svm/slots/latest
```

**Response:**
```json
{
  "slot": 180000000,
  "block_hash": "...",
  "parent_slot": 179999999,
  "transactions": 1500,
  "compute_units_used": 48000000
}
```

## GraphQL API

### Schema Overview

```graphql
type Query {
  account(id: ID!): Account
  accounts(limit: Int, offset: Int): [Account!]!
  transaction(id: ID!): Transaction
  transactions(filter: TransactionFilter): [Transaction!]!
  crossVmStats: CrossVmStatistics
}

type Mutation {
  createAccount(input: CreateAccountInput!): Account!
  bindAccount(input: BindAccountInput!): AccountBinding!
  transfer(input: TransferInput!): Transaction!
  atomicSwap(input: SwapInput!): SwapTransaction!
}

type Subscription {
  accountUpdates(accountId: ID!): AccountUpdate!
  transactionStatus(transactionId: ID!): TransactionStatus!
  blockHeaders(vmType: VmType!): BlockHeader!
}
```

### Example Queries

#### Get Account Details

```graphql
query GetAccount($id: ID!) {
  account(id: $id) {
    id
    name
    addresses {
      address
      vmType
      balance
    }
    transactions(last: 10) {
      id
      type
      status
      amount
      timestamp
    }
  }
}
```

#### Cross-VM Transfer

```graphql
mutation CrossVmTransfer($input: TransferInput!) {
  transfer(input: $input) {
    id
    status
    source {
      vm
      txHash
    }
    target {
      vm
      txHash
    }
  }
}
```

### Real-time Subscriptions

```graphql
subscription WatchTransaction($id: ID!) {
  transactionStatus(transactionId: $id) {
    status
    confirmations
    error
  }
}
```

## WebSocket API

### Connection

```javascript
const ws = new WebSocket('wss://api.multivm.org/ws');

ws.on('open', () => {
  // Authenticate
  ws.send(JSON.stringify({
    type: 'auth',
    token: 'YOUR_JWT_TOKEN'
  }));
});
```

### Subscribe to Events

```javascript
// Subscribe to account updates
ws.send(JSON.stringify({
  type: 'subscribe',
  channel: 'account',
  params: {
    account_id: 'multivm:7f8a9b1c...'
  }
}));

// Subscribe to block headers
ws.send(JSON.stringify({
  type: 'subscribe',
  channel: 'blocks',
  params: {
    vm_type: 'evm'
  }
}));
```

### Message Types

#### Account Update

```json
{
  "type": "account_update",
  "data": {
    "account_id": "multivm:7f8a9b1c...",
    "event": "balance_change",
    "details": {
      "vm": "evm",
      "address": "0x...",
      "old_balance": "1000000000000000000",
      "new_balance": "2000000000000000000"
    }
  }
}
```

#### Transaction Update

```json
{
  "type": "transaction_update",
  "data": {
    "transaction_id": "tx_123456789",
    "status": "confirmed",
    "confirmations": 12
  }
}
```

## Error Handling

### Error Response Format

```json
{
  "error": {
    "code": "INVALID_ACCOUNT",
    "message": "Account not found",
    "details": {
      "account_id": "multivm:invalid..."
    }
  }
}
```

### Common Error Codes

| Code | Description | HTTP Status |
|------|-------------|-------------|
| `AUTH_REQUIRED` | Authentication required | 401 |
| `INVALID_TOKEN` | Invalid or expired token | 401 |
| `PERMISSION_DENIED` | Insufficient permissions | 403 |
| `INVALID_ACCOUNT` | Account not found | 404 |
| `INVALID_BINDING` | Invalid binding proof | 400 |
| `INSUFFICIENT_BALANCE` | Not enough funds | 400 |
| `RATE_LIMITED` | Too many requests | 429 |
| `INTERNAL_ERROR` | Server error | 500 |

## Rate Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| Authentication | 10 | 1 hour |
| Account Creation | 5 | 1 hour |
| Transfers | 100 | 1 minute |
| Queries | 1000 | 1 minute |
| WebSocket Messages | 100 | 1 second |

## SDK Examples

### JavaScript/TypeScript

```typescript
import { MultiVMClient } from '@multivm/sdk';

const client = new MultiVMClient({
  apiKey: 'YOUR_API_KEY',
  network: 'mainnet'
});

// Create account
const account = await client.accounts.create({
  name: 'My Account'
});

// Bind Ethereum address
await client.accounts.bind(account.id, {
  address: '0x...',
  vmType: 'evm',
  proof: signedMessage
});

// Transfer assets
const tx = await client.transfers.create({
  from: account.id,
  to: 'multivm:8a9b2c3d...',
  amount: '1000000000',
  asset: 'ETH'
});
```

### Python

```python
from multivm import Client

client = Client(api_key='YOUR_API_KEY')

# Get account
account = client.accounts.get('multivm:7f8a9b1c...')

# List transactions
transactions = client.transactions.list(
    account_id=account.id,
    limit=10
)

# Subscribe to updates
async def handle_update(update):
    print(f"New update: {update}")

await client.subscribe_account(account.id, handle_update)
```

## Webhooks

Configure webhooks to receive real-time notifications:

```http
POST /v1/webhooks
Content-Type: application/json

{
  "url": "https://your-server.com/webhook",
  "events": ["transfer.completed", "swap.executed"],
  "secret": "your-webhook-secret"
}
```

## Testing

Use the testnet API for development:

- Base URL: `https://testnet-api.multivm.org/v1`
- Faucet: `https://testnet-faucet.multivm.org`

---

© 2024 MultiVM Project. Licensed under MIT/Apache-2.0.
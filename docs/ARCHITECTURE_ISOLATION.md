# MultiVM Architecture: Process Isolation and Communication

## Overview

MultiVM implements a strict separation of concerns between consensus and execution:

- **MultiVM Process**: Handles ALL P2P networking, consensus (Malachite BFT), and transaction routing
- **Execution Engines**: Reth (Ethereum) and Solana are isolated processes that ONLY execute transactions

## Key Architecture Principles

### 1. Network Isolation

```
┌─────────────────────────────────────────────────────────────┐
│                     External Network                         │
│                                                              │
│  Clients ──► MultiVM RPC (8080-8086) ◄── Explorer (3001)   │
│              MultiVM P2P (9000-9006)                         │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│                  Docker Network: multivm-testnet             │
│                                                              │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐      ┌─────────┐     │
│  │ Node 1  │ │ Node 2  │ │ Node 3  │ ···  │ Node 7  │     │
│  └────┬────┘ └────┬────┘ └────┬────┘      └────┬────┘     │
│       │           │           │                  │          │
├───────┼───────────┼───────────┼──────────────────┼──────────┤
│       │           │           │                  │          │
│  Docker Network: execution-net (internal=true, isolated)    │
│       │           │           │                  │          │
│  ┌────┴──────────┴───────────┴──────────────────┴────┐     │
│  │                                                     │     │
│  │  ┌──────────────┐           ┌───────────────┐     │     │
│  │  │     Reth     │           │    Solana     │     │     │
│  │  │  (No P2P)    │           │   (No P2P)    │     │     │
│  │  │  Engine API  │           │   RPC Only    │     │     │
│  │  └──────────────┘           └───────────────┘     │     │
│  │                                                     │     │
│  └─────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

### 2. Process Responsibilities

#### MultiVM Process
- **P2P Networking**: Peer discovery, message propagation
- **Consensus**: Malachite BFT consensus protocol
- **Transaction Pool**: Collecting and validating transactions
- **Block Production**: Creating new blocks
- **State Management**: Coordinating state across execution engines
- **RPC Interface**: External API for clients

#### Reth Process (Ethereum Execution)
- **NO P2P**: All P2P functionality disabled (`--disable-discovery --port 0 --max-peers 0`)
- **Engine API Only**: Receives blocks via Engine API from MultiVM
- **State Execution**: Executes Ethereum transactions
- **State Storage**: Maintains Ethereum world state
- **No External Access**: Only accessible via internal docker network

#### Solana Process (SVM Execution)
- **NO Gossip**: Gossip port disabled (`--gossip-port 0`)
- **No Voting**: Validator voting disabled (`--no-voting`)
- **RPC Interface**: Internal RPC for MultiVM communication
- **State Execution**: Executes Solana programs
- **No External Access**: Only accessible via internal docker network

### 3. Communication Flow

```
1. Client submits transaction to MultiVM RPC
   └─> MultiVM validates and adds to mempool
   
2. MultiVM consensus creates new block
   └─> Block contains both EVM and SVM transactions
   
3. MultiVM sends block to execution engines:
   ├─> Reth: via Engine API (newPayload)
   └─> Solana: via RPC (sendTransaction)
   
4. Execution engines process transactions:
   ├─> Reth: Updates Ethereum state
   └─> Solana: Updates Solana state
   
5. MultiVM queries state from engines:
   ├─> Reth: via JSON-RPC (eth_getBalance, etc.)
   └─> Solana: via JSON-RPC (getAccountInfo, etc.)
```

### 4. Security Boundaries

1. **Network Isolation**:
   - `execution-net` is marked as `internal: true` in Docker
   - No port mappings for Reth or Solana containers
   - Only MultiVM nodes can access execution engines

2. **Authentication**:
   - JWT authentication between MultiVM and Reth Engine API
   - Secure IPC channels for local communication

3. **Process Isolation**:
   - Each execution engine runs in its own container
   - No shared volumes between execution engines
   - Resource limits can be applied per container

### 5. Configuration Examples

#### Reth Configuration (P2P Disabled)
```bash
reth node \
  --disable-discovery \     # No peer discovery
  --port 0 \               # No P2P port
  --max-peers 0 \          # No peers allowed
  --nodiscover \           # No discovery protocol
  --no-persist-peers \     # Don't save peer info
  --nat none               # No NAT traversal
```

#### Solana Configuration (Isolated)
```bash
solana-validator \
  --gossip-port 0 \        # No gossip
  --no-voting \            # No consensus participation
  --no-snapshot-fetch \    # No snapshot downloads
  --no-genesis-fetch       # No genesis downloads
```

### 6. Deployment Checklist

- [ ] Reth and Solana containers have NO external port mappings
- [ ] execution-net is configured with `internal: true`
- [ ] MultiVM nodes connect to both networks (multivm-testnet + execution-net)
- [ ] P2P is disabled in Reth configuration
- [ ] Gossip is disabled in Solana configuration
- [ ] JWT secret is shared between MultiVM and Reth
- [ ] Architecture validation script passes all checks

### 7. Monitoring and Validation

Run the architecture validation script to ensure proper setup:
```bash
./scripts/validate-architecture.sh
```

This script verifies:
- Network isolation is enforced
- No external ports are exposed for execution engines
- P2P/Gossip is properly disabled
- Communication paths are correctly configured
- MultiVM is the only external-facing component

### 8. Common Misconfigurations

❌ **Wrong**: Exposing Reth/Solana ports externally
```yaml
reth:
  ports:
    - "8545:8545"  # DO NOT DO THIS
```

✅ **Correct**: Using internal exposure only
```yaml
reth:
  expose:
    - "8545"  # Internal only
```

❌ **Wrong**: Connecting Explorer directly to Reth/Solana
```yaml
explorer:
  environment:
    - RETH_RPC_URL=http://reth:8545  # DO NOT DO THIS
```

✅ **Correct**: Explorer connects only to MultiVM
```yaml
explorer:
  environment:
    - MULTIVM_RPC_URLS=http://multivm-node-1:8080
    - RETH_RPC_URL=  # Empty - no direct access
```
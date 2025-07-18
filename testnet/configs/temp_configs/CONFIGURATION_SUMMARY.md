# MultiVM Testnet Configuration Summary

This directory contains configuration files for a 7-node MultiVM testnet. Each node is configured as a validator with both Ethereum (Reth) and Solana execution engines.

## Node Configuration Overview

All 7 nodes are configured as validators with the following characteristics:

### Network Architecture
- **Chain ID**: 1337 (testnet)
- **Consensus**: Malachite BFT with 2-second block time
- **Validators**: All 7 nodes participate in consensus with equal voting power (100)
- **Network**: Each node has unique P2P, RPC, and consensus ports to avoid conflicts

### Port Assignments

| Node | P2P Port | RPC Port | WS Port | Consensus Port | Metrics Port | Health Port |
|------|----------|----------|---------|----------------|--------------|-------------|
| node1 | 9000    | 8000     | 8100    | 7000          | 9090         | 8090        |
| node2 | 9001    | 8001     | 8101    | 7001          | 9092         | 8091        |
| node3 | 9002    | 8002     | 8102    | 7002          | 9094         | 8092        |
| node4 | 9003    | 8003     | 8103    | 7003          | 9096         | 8093        |
| node5 | 9004    | 8004     | 8104    | 7004          | 9098         | 8094        |
| node6 | 9005    | 8005     | 8105    | 7005          | 9100         | 8095        |
| node7 | 9006    | 8006     | 8106    | 7006          | 9102         | 8096        |

### Ethereum/Reth Configuration

Each node runs a Reth instance with:
- **P2P disabled**: MultiVM handles consensus, Reth P2P is disabled
- **Engine API enabled**: For consensus layer communication
- **Archive mode**: Full state preservation
- **JWT authentication**: Secure Engine API communication

| Node | HTTP RPC | WS RPC | Engine API |
|------|----------|--------|------------|
| node1 | 8545    | 8546   | 8551       |
| node2 | 8555    | 8556   | 8561       |
| node3 | 8565    | 8566   | 8571       |
| node4 | 8575    | 8576   | 8581       |
| node5 | 8585    | 8586   | 8591       |
| node6 | 8595    | 8596   | 8601       |
| node7 | 8605    | 8606   | 8611       |

### Solana Configuration

Each node runs a Solana private validator with:
- **RPC and WebSocket endpoints**: For client connections
- **Gossip port**: For Solana network communication
- **64 ticks per slot**: Standard Solana timing
- **8192 slots per epoch**: Standard epoch length

| Node | RPC Port | WS Port | Gossip Port |
|------|----------|---------|-------------|
| node1 | 8899    | 8900    | 1024        |
| node2 | 8909    | 8910    | 1025        |
| node3 | 8919    | 8920    | 1026        |
| node4 | 8929    | 8930    | 1027        |
| node5 | 8939    | 8940    | 1028        |
| node6 | 8949    | 8950    | 1029        |
| node7 | 8959    | 8960    | 1030        |

### Key Features

1. **BFT Consensus**: 2/3+ majority required for consensus
2. **IPC Communication**: Unix sockets with encryption and authentication
3. **Storage**: RocksDB backend with compression and auto-compaction
4. **Security**: JWT authentication, rate limiting, DDoS protection
5. **Monitoring**: Prometheus metrics, health checks, structured logging
6. **APIs**: REST, GraphQL, and WebSocket interfaces

### Deployment Notes

1. Each node requires:
   - A unique `validator.json` key file
   - A shared `jwt.hex` file for Engine API authentication
   - Proper genesis configuration

2. Bootstrap peers are configured in a mesh topology where each node connects to 3 others

3. All nodes have development features enabled for testnet:
   - Debug endpoints
   - Unsafe RPC methods
   - GraphQL playground

4. Resource requirements per node:
   - 1GB cache for RocksDB
   - Auto-detected worker threads
   - 10MB max message size for IPC

### Usage

To deploy these configurations:

1. Copy the config files to their respective node directories:
   ```bash
   sudo cp node1_multivm.toml /path/to/testnet/configs/node1/multivm.toml
   # Repeat for all 7 nodes
   ```

2. Ensure each node has:
   - Valid `genesis.json` file
   - Shared `jwt.hex` secret
   - Unique `validator.json` key

3. Start the nodes using the MultiVM process manager or Docker Compose

### Environment Variables

The following environment variables are used:
- `${MULTIVM_JWT_SECRET}`: Shared JWT secret for API authentication
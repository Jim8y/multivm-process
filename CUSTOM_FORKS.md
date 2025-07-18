# MultiVM Custom Fork Configuration

## Overview

MultiVM uses custom forks of Reth and Solana that have been modified to work as isolated execution engines without P2P or consensus capabilities.

## Custom Forks

### Reth (Ethereum Execution Engine)
- **Repository**: `git@github.com:vm-multiverse/reth.git`
- **Branch**: `dev`
- **Purpose**: Modified Reth that operates as a pure execution engine
- **Key Modifications**:
  - P2P networking disabled
  - Consensus mechanisms removed
  - Only accepts blocks/transactions from MultiVM
  - Enhanced Engine API for MultiVM integration

### Solana (SVM Execution Engine)
- **Repository**: `git@github.com:vm-multiverse/multivm-agave.git`
- **Branch**: `master`
- **Purpose**: Modified Solana validator that operates as a pure execution engine
- **Key Modifications**:
  - P2P networking disabled
  - Consensus/voting disabled
  - Only accepts transactions from MultiVM
  - Custom RPC endpoints for MultiVM integration

## Building from Source

### Automated Setup
```bash
# This script will clone and build both custom forks
./scripts/setup-test-binaries.sh
```

### Manual Build - Reth
```bash
git clone -b dev git@github.com:vm-multiverse/reth.git
cd reth
cargo build --release --bin reth
```

### Manual Build - Solana
```bash
git clone -b master git@github.com:vm-multiverse/multivm-agave.git
cd multivm-agave
cargo build --release --bin solana-test-validator
```

## Docker Images

Docker images are built from these custom forks:

```bash
# Build Reth image
docker build -t multivm-reth ./reth

# Build Solana image
docker build -t multivm-solana ./solana
```

## Integration Testing

All integration tests are configured to use these specific custom forks:

```bash
# Run integration tests with custom binaries
./scripts/run-integration-tests-custom.sh
```

## Important Notes

1. **Never use official Reth or Solana binaries** - They include P2P and consensus functionality that violates the MultiVM architecture
2. **Always verify fork URLs** - Ensure you're using the vm-multiverse organization forks
3. **Branch selection** - Reth uses `dev` branch, Solana uses `master` branch
4. **Network isolation** - These forks are designed to work in isolated networks without external connectivity
#!/bin/bash
# Run MultiVM node locally without Docker

set -e

echo "======================================"
echo "Starting MultiVM Single Node Locally"
echo "======================================"
echo ""

# Set environment variables
export NODE_ID=single-node
export NODE_TYPE=bootstrap
export CONSENSUS_ROLE=validator
export VALIDATOR_KEY=single-validator-key
export BLOCK_GENERATION_ENABLED=true
export BLOCK_INTERVAL_MS=2000
export SVM_TX_PER_BLOCK=3
export EVM_TX_PER_BLOCK=3
export RUST_LOG=info,multivm=debug,multivm_consensus=debug

echo "Configuration:"
echo "- Node ID: $NODE_ID"
echo "- Node Type: $NODE_TYPE"
echo "- Consensus Role: $CONSENSUS_ROLE"
echo "- Block Generation: $BLOCK_GENERATION_ENABLED"
echo "- Block Interval: ${BLOCK_INTERVAL_MS}ms"
echo "- Log Level: $RUST_LOG"
echo ""

# Check if binary exists
if [ ! -f "./target/release/multivm-node" ]; then
    echo "Binary not found. Building..."
    cargo build --release --bin multivm-node
fi

# Create required directories
mkdir -p logs
mkdir -p data

echo "Starting MultiVM node..."
echo "Press Ctrl+C to stop"
echo ""

# Run the node and capture output
./target/release/multivm-node 2>&1 | tee logs/multivm-local-$(date +%Y%m%d_%H%M%S).log
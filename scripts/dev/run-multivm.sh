#!/bin/bash
# Direct execution of MultiVM single node

# Set environment for block generation
export NODE_ID="single-node"
export BLOCK_GENERATION_ENABLED="true"
export BLOCK_INTERVAL_MS="2000"
export SVM_TX_PER_BLOCK="3"
export EVM_TX_PER_BLOCK="3"
export RUST_LOG="info,multivm=trace"

echo "Starting Real MultiVM Blockchain Node"
echo "===================================="
echo "Block Generation: Every 2 seconds"
echo "Transactions: 3 SVM + 3 EVM per block"
echo ""

# Create necessary directories
mkdir -p data logs config

# Run the node
cd crates/multivm-cli && \
cargo run --release 2>&1 | tee ../../multivm-output.log
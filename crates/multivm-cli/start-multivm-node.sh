#!/bin/bash
# Start Real MultiVM Single Node Blockchain

echo "🚀 Starting Real MultiVM Blockchain Node"
echo "======================================="
echo ""
echo "Configuration:"
echo "- Node Type: Single Bootstrap Validator"
echo "- Block Generation: Enabled (2-second intervals)"
echo "- Transactions: 3 SVM + 3 EVM per block"
echo "- Consensus: Malachite BFT"
echo ""

# Set environment variables
export NODE_ID="single-node"
export BLOCK_GENERATION_ENABLED="true"
export BLOCK_INTERVAL_MS="2000"
export SVM_TX_PER_BLOCK="3"
export EVM_TX_PER_BLOCK="3"
export RUST_LOG="info,multivm=debug,multivm_process_manager=info,multivm_consensus=info"

# Create directories
mkdir -p data logs config

# Create timestamp for this run
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_FILE="logs/multivm-node-$TIMESTAMP.log"

echo "Log file: $LOG_FILE"
echo ""
echo "Starting node..."
echo "================"

# Function to highlight important log lines
highlight_logs() {
    while IFS= read -r line; do
        echo "$line" >> "$LOG_FILE"
        
        if echo "$line" | grep -q "Generated mock block"; then
            echo -e "\033[0;32m✓ $line\033[0m"  # Green for blocks
        elif echo "$line" | grep -q "started successfully\|initialized\|running"; then
            echo -e "\033[0;34mℹ $line\033[0m"  # Blue for info
        elif echo "$line" | grep -q "ERROR\|failed"; then
            echo -e "\033[0;31m✗ $line\033[0m"  # Red for errors
        else
            echo "$line"
        fi
    done
}

# Run the node
cd crates/multivm-cli
exec cargo run --release 2>&1 | highlight_logs
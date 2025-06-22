#!/bin/bash
# Run MultiVM single node natively (without Docker)

set -e

# Configuration
LOG_FILE="multivm-native-$(date +%Y%m%d_%H%M%S).log"
DATA_DIR="./multivm-data"
CONFIG_FILE="./multivm-config.toml"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}MultiVM Native Single Node Runner${NC}"
echo "================================="
echo "Log file: $LOG_FILE"
echo ""

# Check if binary exists
if [ ! -f "target/release/multivm-node" ]; then
    echo -e "${YELLOW}MultiVM binary not found. Building...${NC}"
    cargo build --release --bin multivm-node 2>&1 | tee -a "$LOG_FILE"
    echo -e "${GREEN}✓ Build completed${NC}"
fi

# Create data directory
mkdir -p "$DATA_DIR"

# Create minimal config file if it doesn't exist
if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${YELLOW}Creating configuration file...${NC}"
    cat > "$CONFIG_FILE" << 'EOF'
[node]
id = "single-node"
type = "bootstrap"
role = "validator"

[network]
p2p_listen_addr = "0.0.0.0:26656"
api_listen_addr = "0.0.0.0:8080"
bootstrap_nodes = []

[consensus]
algorithm = "malachite"
validator_key = "single-validator-key"

[logging]
level = "debug"
file = "./multivm-node.log"

[storage]
data_dir = "./multivm-data"

[execution]
solana_mode = "mock"
reth_mode = "mock"
EOF
    echo -e "${GREEN}✓ Configuration created${NC}"
fi

# Set environment variables for block generation
export NODE_ID="single-node"
export BLOCK_GENERATION_ENABLED="true"
export BLOCK_INTERVAL_MS="2000"
export SVM_TX_PER_BLOCK="3"
export EVM_TX_PER_BLOCK="3"
export RUST_LOG="info,multivm=debug,multivm_process_manager=debug"

echo -e "${BLUE}Starting MultiVM node with environment:${NC}"
echo "  NODE_ID=$NODE_ID"
echo "  BLOCK_GENERATION_ENABLED=$BLOCK_GENERATION_ENABLED"
echo "  BLOCK_INTERVAL_MS=$BLOCK_INTERVAL_MS"
echo "  SVM_TX_PER_BLOCK=$SVM_TX_PER_BLOCK"
echo "  EVM_TX_PER_BLOCK=$EVM_TX_PER_BLOCK"
echo ""

# Function to cleanup on exit
cleanup() {
    echo -e "\n${YELLOW}Shutting down MultiVM node...${NC}"
    # Kill the multivm-node process if it's running
    pkill -f "multivm-node" 2>/dev/null || true
    echo -e "${GREEN}✓ Cleanup completed${NC}"
}

trap cleanup EXIT INT TERM

# Start the node
echo -e "${BLUE}Starting MultiVM node...${NC}"
echo "==============================" | tee -a "$LOG_FILE"

# Run the node and capture output
./target/release/multivm-node \
    --config "$CONFIG_FILE" \
    --data-dir "$DATA_DIR" \
    --log-level debug \
    2>&1 | tee -a "$LOG_FILE" &

NODE_PID=$!

# Wait a moment for startup
sleep 5

# Check if process is still running
if ps -p $NODE_PID > /dev/null; then
    echo -e "${GREEN}✓ MultiVM node is running (PID: $NODE_PID)${NC}"
    echo ""
    echo "Monitoring block generation..."
    echo "Press Ctrl+C to stop"
    echo ""
    
    # Monitor the log file for block generation
    tail -f "$LOG_FILE" | grep --line-buffered -E "(Generated mock block|Block generator|started successfully|Processing block)" &
    
    # Wait for the node process
    wait $NODE_PID
else
    echo -e "${RED}✗ MultiVM node failed to start${NC}"
    echo "Check the log file for errors: $LOG_FILE"
    exit 1
fi
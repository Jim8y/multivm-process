#!/bin/bash
# Start a real MultiVM single node blockchain network

set -e

# Configuration
LOG_FILE="multivm-real-node-$(date +%Y%m%d_%H%M%S).log"
DATA_DIR="./data/single-node"
CONFIG_DIR="./config"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}Starting Real MultiVM Single Node Blockchain${NC}"
echo "==========================================="
echo "Log file: $LOG_FILE"
echo ""

# Create necessary directories
mkdir -p "$DATA_DIR" "$CONFIG_DIR" logs

# Create configuration file
echo -e "${YELLOW}Creating configuration...${NC}"
cat > "$CONFIG_DIR/single-node.toml" << 'EOF'
# MultiVM Single Node Configuration

[system]
max_processes = 10
process_restart_delay_ms = 5000
shutdown_timeout_ms = 30000
health_check_interval_ms = 30000
data_dir = "./data/single-node"
enable_metrics = true

[system.resource_limits]
max_memory_mb = 4096
max_cpu_percent = 80.0
max_open_files = 1024
max_disk_usage_gb = 100

[solana]
mode = "mock"
rpc_endpoint = "http://localhost:8899"
ws_endpoint = "ws://localhost:8900"
commitment_level = "confirmed"
transaction_timeout_ms = 30000
max_retries = 3

[ethereum]
mode = "mock"
rpc_endpoint = "http://localhost:8545"
ws_endpoint = "ws://localhost:8546"
chain_id = 1337
block_time_ms = 2000
gas_price_wei = "20000000000"

[ipc]
socket_dir = "/tmp/multivm"
message_timeout_ms = 30000
max_message_size = 16777216
buffer_size = 1048576

[ipc.transport]
mode = "socket"
encryption_enabled = false

[logging]
level = "debug"
file_path = "./logs/multivm.log"
enable_json = false
enable_colors = true
EOF

# Set environment variables
export NODE_ID="single-node"
export NODE_TYPE="bootstrap"
export CONSENSUS_ROLE="validator"
export BLOCK_GENERATION_ENABLED="true"
export BLOCK_INTERVAL_MS="2000"
export SVM_TX_PER_BLOCK="3"
export EVM_TX_PER_BLOCK="3"
export RUST_LOG="info,multivm=debug,multivm_process_manager=debug,multivm_consensus=debug"
export RUST_BACKTRACE="1"

echo -e "${BLUE}Environment Configuration:${NC}"
echo "  NODE_ID: $NODE_ID"
echo "  Block Generation: Enabled"
echo "  Block Interval: 2 seconds"
echo "  Transactions per block: 3 SVM + 3 EVM"
echo ""

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}Stopping MultiVM node...${NC}"
    pkill -f "multivm-node" 2>/dev/null || true
    pkill -f "cargo run" 2>/dev/null || true
    echo -e "${GREEN}✓ Node stopped${NC}"
}

trap cleanup EXIT INT TERM

# Start the node using cargo run
echo -e "${BLUE}Starting MultiVM node...${NC}"
echo "========================" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Run with cargo in the background
cd crates/multivm-cli
cargo run --release -- \
    --config "../../$CONFIG_DIR/single-node.toml" \
    --data-dir "../../$DATA_DIR" \
    --log-level debug \
    2>&1 | while IFS= read -r line; do
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] $line" | tee -a "../../$LOG_FILE"
        
        # Highlight important messages
        if echo "$line" | grep -q "Generated mock block"; then
            echo -e "${GREEN}>>> Block Generated <<<${NC}"
        elif echo "$line" | grep -q "started successfully"; then
            echo -e "${GREEN}>>> System Started <<<${NC}"
        elif echo "$line" | grep -q "ERROR"; then
            echo -e "${RED}>>> Error Detected <<<${NC}"
        fi
    done &

NODE_PID=$!
cd ../..

# Wait for node to start
echo -e "\n${YELLOW}Waiting for node to start...${NC}"
sleep 10

# Check if node is running
if ps -p $NODE_PID > /dev/null 2>&1; then
    echo -e "${GREEN}✓ MultiVM node is running!${NC}"
    echo ""
    echo -e "${BLUE}Node Status:${NC}"
    echo "- Process ID: $NODE_PID"
    echo "- Log file: $LOG_FILE"
    echo "- Data directory: $DATA_DIR"
    echo ""
    echo -e "${YELLOW}Monitoring block generation (Press Ctrl+C to stop)...${NC}"
    echo "=================================================="
    
    # Monitor for block generation
    tail -f "$LOG_FILE" | grep --line-buffered -E "(Generated mock block|Block|height|Processing)" | while IFS= read -r line; do
        if echo "$line" | grep -q "Generated mock block"; then
            echo -e "${GREEN}$line${NC}"
        else
            echo "$line"
        fi
    done
    
    wait $NODE_PID
else
    echo -e "${RED}✗ Failed to start MultiVM node${NC}"
    echo "Last 20 lines of log:"
    tail -20 "$LOG_FILE"
    exit 1
fi
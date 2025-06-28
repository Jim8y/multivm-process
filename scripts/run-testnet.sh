#!/bin/bash

# Simple MultiVM Testnet Runner
# This script starts the testnet and shows clean real-time logs

set -e

# Configuration
TESTNET_DIR="/tmp/multivm-testnet"
CONFIG_FILE="$TESTNET_DIR/config/testnet.toml"
DATA_DIR="$TESTNET_DIR/data"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MULTIVM_BINARY="$PROJECT_ROOT/target/release/multivm-node"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

# Kill any existing processes on our ports first
echo -e "${YELLOW}🧹 Cleaning up existing processes...${NC}"
PORTS="9090 8080 8081 8082 8083 8090"
for PORT in $PORTS; do
    PID=$(lsof -ti:$PORT 2>/dev/null || true)
    if [ ! -z "$PID" ]; then
        echo -e "   Killing process on port $PORT (PID: $PID)"
        kill -9 $PID 2>/dev/null || true
    fi
done
pkill -f multivm-node 2>/dev/null || true
sleep 2

echo -e "${CYAN}🚀 Starting MultiVM Solo Testnet...${NC}"
echo -e "${GREEN}📍 API Endpoints:${NC}"
echo "   REST API: http://localhost:8080"
echo "   GraphQL:  http://localhost:8081" 
echo "   WebSocket: ws://localhost:8082"
echo "   Metrics:  http://localhost:9090"
echo "   Health:   http://localhost:8090"
echo ""
echo -e "${YELLOW}💡 Press Ctrl+C to stop${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Function to cleanup
cleanup() {
    echo -e "\n${YELLOW}🛑 Stopping testnet...${NC}"
    jobs -p | xargs -r kill 2>/dev/null || true
    # Clean up ports
    for PORT in $PORTS; do
        PID=$(lsof -ti:$PORT 2>/dev/null || true)
        if [ ! -z "$PID" ]; then
            kill -9 $PID 2>/dev/null || true
        fi
    done
    echo -e "${GREEN}✅ Testnet stopped${NC}"
}

trap cleanup EXIT INT TERM

# Check if binary exists
if [ ! -f "$MULTIVM_BINARY" ]; then
    echo -e "${RED}❌ Binary not found. Building...${NC}"
    cd "$PROJECT_ROOT"
    cargo build --release
fi

# Run MultiVM with colored output
"$MULTIVM_BINARY" -c "$CONFIG_FILE" -d "$DATA_DIR" 2>&1 | sed -u \
    -e "s/.*ERROR.*/$(printf "${RED}&${NC}")/" \
    -e "s/.*WARN.*/$(printf "${YELLOW}&${NC}")/" \
    -e "s/.*Block.*generated.*/$(printf "${GREEN}&${NC}")/" \
    -e "s/.*Added transaction.*/$(printf "${BLUE}&${NC}")/" \
    -e "s/.*started\|.*initialized\|.*listening.*/$(printf "${CYAN}&${NC}")/"
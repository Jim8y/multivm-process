#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo -e "${GREEN}Starting MultiVM Testnet with Reth Integration${NC}"
echo "================================================"

# Check if docker-compose is available
if ! command -v docker-compose &> /dev/null && ! command -v docker &> /dev/null; then
    echo -e "${RED}Error: Neither docker-compose nor docker compose is available${NC}"
    exit 1
fi

# Determine docker compose command
# Prefer docker compose v2 over deprecated docker-compose v1
if docker compose version &> /dev/null; then
    COMPOSE_CMD="docker compose"
elif command -v docker-compose &> /dev/null; then
    COMPOSE_CMD="docker-compose"
else
    echo -e "${RED}Error: Neither docker compose nor docker-compose is available${NC}"
    exit 1
fi

# Function to check if a service is healthy
check_service_health() {
    local service=$1
    local max_attempts=${2:-30}
    local attempt=0
    
    echo -n "Waiting for $service to be healthy..."
    while [ $attempt -lt $max_attempts ]; do
        if $COMPOSE_CMD -f "$PROJECT_ROOT/docker-compose.testnet-reth.yml" ps | grep -q "$service.*Up.*healthy"; then
            echo -e " ${GREEN}OK${NC}"
            return 0
        fi
        echo -n "."
        sleep 2
        ((attempt++))
    done
    echo -e " ${RED}FAILED${NC}"
    return 1
}

# Function to check if a port is open
check_port() {
    local host=$1
    local port=$2
    nc -z "$host" "$port" 2>/dev/null
}

# Ensure we're in the project root
cd "$PROJECT_ROOT"

# Stop any existing testnet
echo "Stopping any existing testnet..."
$COMPOSE_CMD -f docker-compose.testnet-reth.yml down -v 2>/dev/null || true

# Clean up old data
echo "Cleaning up old data..."
rm -rf testnet/data/*
mkdir -p testnet/data/{reth,multivm-{1..7}}

# Generate new JWT secret if it doesn't exist
if [ ! -f testnet/configs/jwt.hex ]; then
    echo "Generating JWT secret..."
    ./scripts/generate-jwt-token.sh testnet/configs/jwt.hex --generate-only
fi

# Build images if needed
echo "Checking if images need to be built..."
if ! docker images | grep -q "multivm-testnet" || [ "${FORCE_BUILD:-}" = "true" ]; then
    echo "Building images..."
    $COMPOSE_CMD -f docker-compose.testnet-reth.yml build
else
    echo "Images already exist. Skipping build (use FORCE_BUILD=true to rebuild)"
fi

# Start the testnet
echo -e "\n${YELLOW}Starting testnet services...${NC}"
$COMPOSE_CMD -f docker-compose.testnet-reth.yml up -d

# Wait for services to be ready
echo -e "\n${YELLOW}Waiting for services to be ready...${NC}"

# Check Reth
echo -n "Checking Reth RPC..."
attempt=0
while ! check_port localhost 8545 && [ $attempt -lt 30 ]; do
    echo -n "."
    sleep 2
    ((attempt++))
done
if check_port localhost 8545; then
    echo -e " ${GREEN}OK${NC}"
else
    echo -e " ${RED}FAILED${NC}"
    echo "Reth logs:"
    $COMPOSE_CMD -f docker-compose.testnet-reth.yml logs reth | tail -20
fi

# Check MultiVM nodes
for i in {1..7}; do
    port=$((8080 + i - 1))
    echo -n "Checking MultiVM node $i (port $port)..."
    if check_port localhost $port; then
        echo -e " ${GREEN}OK${NC}"
    else
        echo -e " ${RED}FAILED${NC}"
    fi
done

# Wait a bit for consensus to establish
echo -e "\n${YELLOW}Waiting for consensus to establish...${NC}"
sleep 10

# Check if blocks are being produced
echo -e "\n${YELLOW}Checking block production...${NC}"
./scripts/verify-testnet.sh --quick

# Show status
echo -e "\n${GREEN}Testnet Status:${NC}"
echo "==============="
$COMPOSE_CMD -f docker-compose.testnet-reth.yml ps

echo -e "\n${GREEN}Testnet started successfully!${NC}"
echo ""
echo "Services:"
echo "  - Reth RPC: http://localhost:8545"
echo "  - Reth Engine API: http://localhost:8551"
echo "  - MultiVM nodes: http://localhost:8080-8086"
echo "  - Prometheus: http://localhost:9090"
echo "  - Grafana: http://localhost:3000 (admin/admin)"
echo ""
echo "Commands:"
echo "  - View logs: $COMPOSE_CMD -f docker-compose.testnet-reth.yml logs -f"
echo "  - Stop testnet: $COMPOSE_CMD -f docker-compose.testnet-reth.yml down"
echo "  - Monitor: ./scripts/monitor-testnet.sh"
echo "  - Verify: ./scripts/verify-testnet.sh"
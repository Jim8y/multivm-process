#!/bin/bash
# Start MultiVM testnet with proper architecture isolation
# MultiVM handles all P2P/consensus, Reth/Solana are isolated execution engines

set -e

echo "======================================"
echo "Starting MultiVM Isolated Testnet"
echo "======================================"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "\n${YELLOW}Architecture Overview:${NC}"
echo "- MultiVM: Handles all P2P networking and consensus"
echo "- Reth: Isolated Ethereum execution engine (no P2P)"
echo "- Solana: Isolated SVM execution engine (no P2P)"
echo "- Communication: Only via internal docker network"

# Check if docker-compose file exists
if [ ! -f "docker-compose.testnet-reth.yml" ]; then
    echo "Error: docker-compose.testnet-reth.yml not found!"
    exit 1
fi

# Create necessary directories
echo -e "\n${GREEN}Creating data directories...${NC}"
mkdir -p testnet/data/multivm-{1..7}
mkdir -p testnet/configs
mkdir -p reth-data
mkdir -p solana-data

# Generate JWT secret if not exists
if [ ! -f "testnet/configs/jwt.hex" ]; then
    echo -e "\n${GREEN}Generating JWT secret...${NC}"
    openssl rand -hex 32 > testnet/configs/jwt.hex
    echo "JWT secret generated"
fi

# Clone custom forks if not present
if [ ! -d "reth" ]; then
    echo -e "\n${GREEN}Cloning vm-multiverse/reth...${NC}"
    git clone -b dev git@github.com:vm-multiverse/reth.git reth
fi

if [ ! -d "multivm-agave" ]; then
    echo -e "\n${GREEN}Cloning vm-multiverse/multivm-agave...${NC}"
    git clone -b master git@github.com:vm-multiverse/multivm-agave.git multivm-agave
fi

# Stop any existing containers
echo -e "\n${GREEN}Stopping existing containers...${NC}"
docker-compose -f docker-compose.testnet-reth.yml down

# Remove old networks
echo -e "\n${GREEN}Cleaning up networks...${NC}"
docker network rm multivm-testnet execution-net 2>/dev/null || true

# Start the testnet
echo -e "\n${GREEN}Starting testnet services...${NC}"
docker-compose -f docker-compose.testnet-reth.yml up -d

# Wait for services to start
echo -e "\n${GREEN}Waiting for services to initialize...${NC}"
sleep 10

# Run architecture validation
echo -e "\n${GREEN}Running architecture validation...${NC}"
./scripts/validate-architecture.sh

echo -e "\n${GREEN}Testnet started successfully!${NC}"
echo ""
echo "Services:"
echo "- MultiVM RPC endpoints: http://localhost:8080-8086"
echo "- Prometheus: http://localhost:9090"
echo "- Grafana: http://localhost:3000 (admin/admin)"
echo "- Explorer: http://localhost:3001"
echo ""
echo "Monitoring:"
echo "- Logs: docker-compose -f docker-compose.testnet-reth.yml logs -f"
echo "- Status: ./scripts/monitor-complete-system.sh"
echo "- Verify: ./scripts/verify-complete-testnet.sh"
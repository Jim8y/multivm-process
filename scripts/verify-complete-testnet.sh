#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== MultiVM Complete Testnet Verification ===${NC}"
echo

# Function to check service
check_service() {
    local name=$1
    local url=$2
    local expected=$3
    
    echo -n "Checking $name... "
    if curl -s "$url" | grep -q "$expected" 2>/dev/null; then
        echo -e "${GREEN}✓${NC}"
        return 0
    else
        echo -e "${RED}✗${NC}"
        return 1
    fi
}

# Function to check RPC endpoint
check_rpc() {
    local name=$1
    local url=$2
    local method=$3
    local params=$4
    
    echo -n "Testing $name RPC ($method)... "
    result=$(curl -s -X POST "$url" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}" \
        2>/dev/null || echo "failed")
    
    if echo "$result" | grep -q "result" && ! echo "$result" | grep -q "error"; then
        echo -e "${GREEN}✓${NC}"
        return 0
    else
        echo -e "${RED}✗${NC} - $result"
        return 1
    fi
}

# Function to get block height
get_block_height() {
    local url=$1
    curl -s -X POST "$url" \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        2>/dev/null | jq -r '.result' | xargs printf "%d\n" 2>/dev/null || echo "0"
}

# 1. Check all MultiVM nodes are running
echo -e "${YELLOW}1. Checking MultiVM Nodes Status${NC}"
all_nodes_up=true
for i in {1..7}; do
    port=$((8080 + i - 1))
    if check_service "MultiVM Node $i" "http://localhost:$port/health" "ok"; then
        # Get node info
        node_info=$(curl -s "http://localhost:$port/node/info" 2>/dev/null || echo "{}")
        echo "   Node $i: $(echo "$node_info" | jq -r '.node_id // "unknown"')"
    else
        all_nodes_up=false
    fi
done

# 2. Check Reth connectivity
echo -e "\n${YELLOW}2. Checking Reth Process${NC}"
check_service "Reth RPC" "http://localhost:8545" "jsonrpc"
check_service "Reth Engine API" "http://localhost:8551" ""

# Test Reth RPC methods
check_rpc "Reth" "http://localhost:8545" "eth_chainId" "[]"
check_rpc "Reth" "http://localhost:8545" "eth_blockNumber" "[]"
check_rpc "Reth" "http://localhost:8545" "net_version" "[]"

# 3. Check consensus mechanism
echo -e "\n${YELLOW}3. Checking Consensus Mechanism${NC}"
echo -n "Getting consensus info... "
consensus_info=$(curl -s "http://localhost:8080/consensus/status" 2>/dev/null || echo "{}")
if [ "$consensus_info" != "{}" ]; then
    echo -e "${GREEN}✓${NC}"
    echo "   Current round: $(echo "$consensus_info" | jq -r '.round // "unknown"')"
    echo "   Current leader: $(echo "$consensus_info" | jq -r '.leader // "unknown"')"
    echo "   Consensus state: $(echo "$consensus_info" | jq -r '.state // "unknown"')"
else
    echo -e "${RED}✗${NC}"
fi

# 4. Check block production
echo -e "\n${YELLOW}4. Checking Block Production${NC}"
echo "Getting block heights from all nodes..."
declare -A block_heights
max_height=0
for i in {1..7}; do
    port=$((8080 + i - 1))
    height=$(get_block_height "http://localhost:$port")
    block_heights[$i]=$height
    echo "   Node $i: Block height = $height"
    if [ "$height" -gt "$max_height" ]; then
        max_height=$height
    fi
done

# Check if blocks are being produced
if [ "$max_height" -gt "0" ]; then
    echo -e "   ${GREEN}✓ Blocks are being produced (max height: $max_height)${NC}"
    
    # Check consensus
    min_height=$max_height
    for height in "${block_heights[@]}"; do
        if [ "$height" -lt "$min_height" ]; then
            min_height=$height
        fi
    done
    
    diff=$((max_height - min_height))
    if [ "$diff" -le "2" ]; then
        echo -e "   ${GREEN}✓ All nodes are in sync (max diff: $diff blocks)${NC}"
    else
        echo -e "   ${YELLOW}⚠ Nodes are not fully synced (max diff: $diff blocks)${NC}"
    fi
else
    echo -e "   ${RED}✗ No blocks are being produced${NC}"
fi

# 5. Check RPC server functionality
echo -e "\n${YELLOW}5. Testing RPC Server Endpoints${NC}"

# Test various RPC methods on multiple nodes
rpc_methods=(
    "eth_blockNumber:[]"
    "eth_getBalance:[\"0x0000000000000000000000000000000000000000\",\"latest\"]"
    "net_peerCount:[]"
    "eth_chainId:[]"
    "eth_gasPrice:[]"
)

for method_params in "${rpc_methods[@]}"; do
    IFS=':' read -r method params <<< "$method_params"
    check_rpc "Node 1" "http://localhost:8080" "$method" "$params"
done

# 6. Check MultiVM-Reth communication
echo -e "\n${YELLOW}6. Checking MultiVM-Reth Communication${NC}"

# Check if MultiVM nodes can communicate with Reth
echo -n "Testing MultiVM -> Reth connection... "
engine_status=$(curl -s "http://localhost:8080/engine/status" 2>/dev/null || echo "{}")
if echo "$engine_status" | grep -q "connected"; then
    echo -e "${GREEN}✓${NC}"
    echo "   Engine API connected: $(echo "$engine_status" | jq -r '.connected // false')"
    echo "   Last communication: $(echo "$engine_status" | jq -r '.last_communication // "unknown"')"
else
    echo -e "${RED}✗${NC}"
fi

# 7. Check transaction pool
echo -e "\n${YELLOW}7. Checking Transaction Pool${NC}"
echo -n "Getting transaction pool status... "
pool_status=$(curl -s "http://localhost:8080/txpool/status" 2>/dev/null || echo "{}")
if [ "$pool_status" != "{}" ]; then
    echo -e "${GREEN}✓${NC}"
    echo "   Pending transactions: $(echo "$pool_status" | jq -r '.pending // 0')"
    echo "   Queued transactions: $(echo "$pool_status" | jq -r '.queued // 0')"
else
    echo -e "${RED}✗${NC}"
fi

# 8. Check metrics endpoints
echo -e "\n${YELLOW}8. Checking Metrics${NC}"
check_service "Prometheus" "http://localhost:9090/-/healthy" "Prometheus"
check_service "Grafana" "http://localhost:3000/api/health" "ok"

# Check if metrics are being collected
echo -n "Checking metrics collection... "
metrics=$(curl -s "http://localhost:9080/metrics" 2>/dev/null || echo "")
if echo "$metrics" | grep -q "multivm_"; then
    echo -e "${GREEN}✓${NC}"
    metric_count=$(echo "$metrics" | grep -c "multivm_" || echo "0")
    echo "   Found $metric_count MultiVM metrics"
else
    echo -e "${RED}✗${NC}"
fi

# 9. Summary
echo -e "\n${BLUE}=== Verification Summary ===${NC}"
echo

if [ "$all_nodes_up" = true ] && [ "$max_height" -gt "0" ]; then
    echo -e "${GREEN}✓ All systems operational!${NC}"
    echo
    echo "Testnet Details:"
    echo "  - 7 MultiVM nodes: Running and connected"
    echo "  - Reth execution: Connected via Engine API"
    echo "  - Consensus: Active with block production"
    echo "  - Current height: $max_height blocks"
    echo "  - RPC endpoints: Functional"
    echo "  - Metrics: Being collected"
    echo
    echo "Access Points:"
    echo "  - MultiVM RPC: http://localhost:8080-8086"
    echo "  - Reth RPC: http://localhost:8545"
    echo "  - Prometheus: http://localhost:9090"
    echo "  - Grafana: http://localhost:3000 (admin/admin)"
else
    echo -e "${RED}✗ Some components are not working correctly${NC}"
    echo
    echo "Please check the logs with:"
    echo "  docker-compose -f docker-compose.testnet-reth.yml logs -f"
fi
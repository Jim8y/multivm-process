#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Parse arguments
QUICK_MODE=false
VERBOSE=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--quick] [--verbose]"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}MultiVM Testnet Verification${NC}"
echo "============================="

# Function to make JSON-RPC calls
json_rpc() {
    local url=$1
    local method=$2
    local params=${3:-[]}
    
    curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}" \
        "$url" 2>/dev/null
}

# Function to check MultiVM node status
check_multivm_node() {
    local node_num=$1
    local port=$((8080 + node_num - 1))
    local url="http://localhost:$port"
    
    echo -e "\n${YELLOW}Checking MultiVM Node $node_num (port $port)${NC}"
    
    # Check if node is responding
    if ! curl -s "$url/health" > /dev/null 2>&1; then
        echo -e "  Health: ${RED}Not responding${NC}"
        return 1
    fi
    
    echo -e "  Health: ${GREEN}OK${NC}"
    
    # Get node info
    local info=$(curl -s "$url/api/v1/node/info" 2>/dev/null || echo "{}")
    if [ "$info" != "{}" ] && [ "$info" != "" ]; then
        local node_id=$(echo "$info" | jq -r '.node_id // "unknown"' 2>/dev/null || echo "unknown")
        local block_height=$(echo "$info" | jq -r '.block_height // "0"' 2>/dev/null || echo "0")
        local is_validator=$(echo "$info" | jq -r '.is_validator // false' 2>/dev/null || echo "false")
        
        echo "  Node ID: $node_id"
        echo "  Block Height: $block_height"
        echo "  Is Validator: $is_validator"
    fi
    
    # Check consensus status
    local consensus=$(curl -s "$url/api/v1/consensus/status" 2>/dev/null || echo "{}")
    if [ "$consensus" != "{}" ] && [ "$consensus" != "" ]; then
        local view=$(echo "$consensus" | jq -r '.current_view // "0"' 2>/dev/null || echo "0")
        local leader=$(echo "$consensus" | jq -r '.current_leader // "unknown"' 2>/dev/null || echo "unknown")
        
        echo "  Current View: $view"
        echo "  Current Leader: $leader"
    fi
}

# Check Reth status
echo -e "\n${YELLOW}Checking Reth Node${NC}"
echo "=================="

# Check if Reth is running
if ! nc -z localhost 8545 2>/dev/null; then
    echo -e "Reth RPC: ${RED}Not responding${NC}"
else
    echo -e "Reth RPC: ${GREEN}OK${NC}"
    
    # Get block number
    block_response=$(json_rpc "http://localhost:8545" "eth_blockNumber")
    if [ "$VERBOSE" = true ]; then
        echo "Raw response: $block_response"
    fi
    block_number=$(echo "$block_response" | jq -r '.result // "0x0"' 2>/dev/null || echo "0x0")
    block_decimal=$((16#${block_number#0x}))
    echo "Latest Block: $block_decimal"
    
    # Get syncing status
    sync_response=$(json_rpc "http://localhost:8545" "eth_syncing")
    is_syncing=$(echo "$sync_response" | jq -r '.result' 2>/dev/null || echo "unknown")
    echo "Syncing: $is_syncing"
    
    # Get peer count
    peer_response=$(json_rpc "http://localhost:8545" "net_peerCount")
    peer_count=$(echo "$peer_response" | jq -r '.result // "0x0"' 2>/dev/null || echo "0x0")
    peer_decimal=$((16#${peer_count#0x}))
    echo "Peer Count: $peer_decimal"
fi

# Check Engine API with JWT
echo -e "\n${YELLOW}Checking Reth Engine API${NC}"
if [ -f "$PROJECT_ROOT/testnet/configs/jwt.hex" ]; then
    JWT_TOKEN=$("$PROJECT_ROOT/scripts/generate-jwt-token.sh" "$PROJECT_ROOT/testnet/configs/jwt.hex" --quiet 2>/dev/null || echo "")
    if [ -n "$JWT_TOKEN" ]; then
        capabilities_response=$(curl -s -X POST -H "Content-Type: application/json" \
            -H "Authorization: Bearer $JWT_TOKEN" \
            -d '{"jsonrpc":"2.0","method":"engine_exchangeCapabilities","params":[["engine_newPayloadV3","engine_forkchoiceUpdatedV3","engine_getPayloadV3"]],"id":1}' \
            http://localhost:8551 2>/dev/null || echo "{}")
        
        if echo "$capabilities_response" | jq -e '.result' > /dev/null 2>&1; then
            echo -e "Engine API: ${GREEN}OK${NC}"
            capabilities_count=$(echo "$capabilities_response" | jq '.result | length' 2>/dev/null || echo "0")
            echo "Supported Capabilities: $capabilities_count"
        else
            echo -e "Engine API: ${RED}Not working${NC}"
            if [ "$VERBOSE" = true ]; then
                echo "Response: $capabilities_response"
            fi
        fi
    else
        echo -e "Engine API: ${RED}JWT generation failed${NC}"
    fi
else
    echo -e "Engine API: ${RED}JWT secret not found${NC}"
fi

# Check MultiVM nodes
if [ "$QUICK_MODE" = false ]; then
    echo -e "\n${BLUE}Checking MultiVM Nodes${NC}"
    echo "====================="
    
    for i in {1..7}; do
        check_multivm_node $i
    done
fi

# Check transaction generator
echo -e "\n${YELLOW}Checking Transaction Generator${NC}"
tx_gen_logs=$(docker-compose -f "$PROJECT_ROOT/docker-compose.testnet-reth.yml" logs --tail=10 tx-generator 2>/dev/null || echo "")
if echo "$tx_gen_logs" | grep -q "Sent.*transactions"; then
    echo -e "Transaction Generator: ${GREEN}Running${NC}"
    tx_count=$(echo "$tx_gen_logs" | grep -oE "Sent [0-9]+ transactions" | tail -1 | grep -oE "[0-9]+" || echo "0")
    echo "Transactions Sent: $tx_count"
else
    echo -e "Transaction Generator: ${RED}Not running or no transactions sent${NC}"
fi

# Check for block production
echo -e "\n${YELLOW}Checking Block Production${NC}"
echo "========================"

# Wait and check block height change
if [ "$QUICK_MODE" = false ]; then
    echo "Monitoring block production for 10 seconds..."
    
    # Get initial block heights
    initial_reth_block=$(json_rpc "http://localhost:8545" "eth_blockNumber" | jq -r '.result // "0x0"' 2>/dev/null || echo "0x0")
    initial_reth_decimal=$((16#${initial_reth_block#0x}))
    
    # Get initial MultiVM block height from node 1
    initial_multivm_block=$(curl -s "http://localhost:8080/api/v1/node/info" 2>/dev/null | jq -r '.block_height // "0"' 2>/dev/null || echo "0")
    
    # Wait 10 seconds
    sleep 10
    
    # Get final block heights
    final_reth_block=$(json_rpc "http://localhost:8545" "eth_blockNumber" | jq -r '.result // "0x0"' 2>/dev/null || echo "0x0")
    final_reth_decimal=$((16#${final_reth_block#0x}))
    
    final_multivm_block=$(curl -s "http://localhost:8080/api/v1/node/info" 2>/dev/null | jq -r '.block_height // "0"' 2>/dev/null || echo "0")
    
    # Calculate block production
    reth_blocks_produced=$((final_reth_decimal - initial_reth_decimal))
    multivm_blocks_produced=$((final_multivm_block - initial_multivm_block))
    
    echo ""
    echo "Reth blocks produced: $reth_blocks_produced"
    echo "MultiVM blocks produced: $multivm_blocks_produced"
    
    if [ $reth_blocks_produced -gt 0 ] || [ $multivm_blocks_produced -gt 0 ]; then
        echo -e "\n${GREEN}✓ Block production is working!${NC}"
    else
        echo -e "\n${RED}✗ No blocks produced in 10 seconds${NC}"
    fi
fi

# Summary
echo -e "\n${BLUE}Summary${NC}"
echo "======="

# Count healthy services
healthy_count=0
total_count=9  # Reth + 7 MultiVM nodes + TX generator

if nc -z localhost 8545 2>/dev/null; then
    ((healthy_count++))
fi

for i in {1..7}; do
    if nc -z localhost $((8080 + i - 1)) 2>/dev/null; then
        ((healthy_count++))
    fi
done

if echo "$tx_gen_logs" | grep -q "Sent.*transactions"; then
    ((healthy_count++))
fi

echo "Healthy Services: $healthy_count/$total_count"

if [ $healthy_count -eq $total_count ]; then
    echo -e "\n${GREEN}✓ All services are healthy!${NC}"
    exit 0
else
    echo -e "\n${YELLOW}⚠ Some services may not be fully operational${NC}"
    exit 1
fi
#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Clear screen function
clear_screen() {
    printf "\033c"
}

# Function to make JSON-RPC calls
json_rpc() {
    local url=$1
    local method=$2
    local params=${3:-[]}
    
    curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}" \
        "$url" 2>/dev/null
}

# Function to format number with commas
format_number() {
    printf "%'d" "$1" 2>/dev/null || echo "$1"
}

# Function to get Reth stats
get_reth_stats() {
    local block_response=$(json_rpc "http://localhost:8545" "eth_blockNumber")
    local block_hex=$(echo "$block_response" | jq -r '.result // "0x0"' 2>/dev/null || echo "0x0")
    local block_number=$((16#${block_hex#0x}))
    
    local peer_response=$(json_rpc "http://localhost:8545" "net_peerCount")
    local peer_hex=$(echo "$peer_response" | jq -r '.result // "0x0"' 2>/dev/null || echo "0x0")
    local peer_count=$((16#${peer_hex#0x}))
    
    echo "$block_number|$peer_count"
}

# Function to get MultiVM node stats
get_multivm_stats() {
    local node_num=$1
    local port=$((8080 + node_num - 1))
    
    local info=$(curl -s "http://localhost:$port/api/v1/node/info" 2>/dev/null || echo "{}")
    local consensus=$(curl -s "http://localhost:$port/api/v1/consensus/status" 2>/dev/null || echo "{}")
    
    local block_height=$(echo "$info" | jq -r '.block_height // "0"' 2>/dev/null || echo "0")
    local is_validator=$(echo "$info" | jq -r '.is_validator // false' 2>/dev/null || echo "false")
    local current_view=$(echo "$consensus" | jq -r '.current_view // "0"' 2>/dev/null || echo "0")
    local tx_pool_size=$(echo "$info" | jq -r '.tx_pool_size // "0"' 2>/dev/null || echo "0")
    
    echo "$block_height|$is_validator|$current_view|$tx_pool_size"
}

# Function to get transaction generator stats
get_tx_gen_stats() {
    local logs=$(docker-compose -f "$PROJECT_ROOT/docker-compose.testnet-reth.yml" logs --tail=50 tx-generator 2>/dev/null || echo "")
    local tx_count=$(echo "$logs" | grep -oE "Sent [0-9]+ transactions" | tail -1 | grep -oE "[0-9]+" || echo "0")
    local evm_count=$(echo "$logs" | grep -c "Sent EVM transaction" || echo "0")
    local svm_count=$(echo "$logs" | grep -c "Sent SVM transaction" || echo "0")
    local cross_count=$(echo "$logs" | grep -c "Sent Cross-VM transaction" || echo "0")
    
    echo "$tx_count|$evm_count|$svm_count|$cross_count"
}

# Main monitoring loop
echo -e "${BLUE}MultiVM Testnet Monitor${NC}"
echo "Press Ctrl+C to exit"
echo ""

# Initialize counters for rate calculation
declare -A prev_blocks
declare -A prev_txs
prev_reth_block=0
prev_tx_count=0
prev_time=$(date +%s)

while true; do
    clear_screen
    
    # Header
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║                    MultiVM Testnet Monitor                         ║${NC}"
    echo -e "${BLUE}║                 $(date '+%Y-%m-%d %H:%M:%S')                     ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    
    # Get current time for rate calculations
    current_time=$(date +%s)
    time_diff=$((current_time - prev_time))
    if [ $time_diff -eq 0 ]; then time_diff=1; fi
    
    # Reth Status
    echo -e "${YELLOW}Reth Execution Engine${NC}"
    echo "─────────────────────"
    reth_stats=$(get_reth_stats)
    IFS='|' read -r reth_block reth_peers <<< "$reth_stats"
    
    if [ $reth_block -gt 0 ]; then
        echo -e "Status: ${GREEN}Running${NC}"
        echo "Block Height: $(format_number "$reth_block")"
        echo "Peer Count: $reth_peers"
        
        # Calculate block rate
        if [ $prev_reth_block -gt 0 ]; then
            block_rate=$(( (reth_block - prev_reth_block) * 60 / time_diff ))
            echo "Block Rate: ~$block_rate blocks/min"
        fi
        prev_reth_block=$reth_block
    else
        echo -e "Status: ${RED}Not Available${NC}"
    fi
    echo ""
    
    # MultiVM Nodes Status
    echo -e "${YELLOW}MultiVM Nodes${NC}"
    echo "─────────────"
    printf "%-8s %-8s %-10s %-8s %-8s %-10s\n" "Node" "Type" "Height" "View" "TxPool" "Status"
    echo "────────────────────────────────────────────────────────────"
    
    total_validators=0
    active_nodes=0
    
    for i in {1..7}; do
        stats=$(get_multivm_stats $i)
        IFS='|' read -r height is_validator view tx_pool <<< "$stats"
        
        if [ "$height" != "0" ]; then
            active_nodes=$((active_nodes + 1))
            if [ "$is_validator" = "true" ]; then
                total_validators=$((total_validators + 1))
                node_type="${GREEN}Validator${NC}"
            else
                node_type="${CYAN}Full${NC}"
            fi
            status="${GREEN}Active${NC}"
            
            # Calculate block rate
            prev_height=${prev_blocks[$i]:-0}
            if [ $prev_height -gt 0 ] && [ $height -gt $prev_height ]; then
                node_block_rate=$(( (height - prev_height) * 60 / time_diff ))
            else
                node_block_rate=0
            fi
            prev_blocks[$i]=$height
        else
            node_type="${RED}Unknown${NC}"
            status="${RED}Down${NC}"
            node_block_rate=0
        fi
        
        printf "%-8s %-18s %-10s %-8s %-8s %-18s\n" \
            "Node-$i" "$node_type" "$(format_number "$height")" "$view" "$tx_pool" "$status"
    done
    
    echo ""
    echo "Active Nodes: $active_nodes/7 (Validators: $total_validators)"
    echo ""
    
    # Transaction Generator Status
    echo -e "${YELLOW}Transaction Generator${NC}"
    echo "────────────────────"
    tx_stats=$(get_tx_gen_stats)
    IFS='|' read -r total_tx evm_tx svm_tx cross_tx <<< "$tx_stats"
    
    if [ "$total_tx" != "0" ]; then
        echo -e "Status: ${GREEN}Running${NC}"
        echo "Total Transactions: $(format_number "$total_tx")"
        echo "  - EVM: $(format_number "$evm_tx")"
        echo "  - SVM: $(format_number "$svm_tx")"
        echo "  - Cross-VM: $(format_number "$cross_tx")"
        
        # Calculate tx rate
        if [ $prev_tx_count -gt 0 ] && [ $total_tx -gt $prev_tx_count ]; then
            tx_rate=$(( (total_tx - prev_tx_count) * 60 / time_diff ))
            echo "TX Rate: ~$tx_rate tx/min"
        fi
        prev_tx_count=$total_tx
    else
        echo -e "Status: ${RED}Not Running${NC}"
    fi
    echo ""
    
    # System Resources
    echo -e "${YELLOW}System Resources${NC}"
    echo "───────────────"
    
    # Get Docker stats
    docker_stats=$(docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}" 2>/dev/null | grep -E "(reth|multivm|tx-generator)" || echo "")
    
    if [ -n "$docker_stats" ]; then
        echo "$docker_stats"
    else
        echo "Unable to fetch Docker statistics"
    fi
    echo ""
    
    # Footer
    echo -e "${BLUE}────────────────────────────────────────────────────────────────────${NC}"
    echo "Services:"
    echo "  • Reth RPC: http://localhost:8545"
    echo "  • MultiVM API: http://localhost:8080-8086"
    echo "  • Prometheus: http://localhost:9090"
    echo "  • Grafana: http://localhost:3000"
    echo ""
    echo "Press Ctrl+C to exit"
    
    # Update previous time
    prev_time=$current_time
    
    # Wait before next update
    sleep 5
done
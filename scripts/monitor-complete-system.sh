#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to format numbers with commas
format_number() {
    printf "%'d" "$1"
}

# Function to get JSON value safely
get_json_value() {
    echo "$1" | jq -r "$2" 2>/dev/null || echo "N/A"
}

# Clear screen and show header
clear
echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           MultiVM Complete System Monitor v1.0                 ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
echo

while true; do
    # Move cursor to line 5
    tput cup 4 0
    
    # Get current timestamp
    echo -e "${CYAN}Last Update: $(date '+%Y-%m-%d %H:%M:%S')${NC}"
    echo
    
    # 1. Node Status
    echo -e "${YELLOW}📊 Node Status:${NC}"
    echo "┌────────┬──────────────┬─────────┬─────────┬──────────┬─────────┐"
    echo "│ Node   │ Status       │ Height  │ Peers   │ TPS      │ Latency │"
    echo "├────────┼──────────────┼─────────┼─────────┼──────────┼─────────┤"
    
    for i in {1..7}; do
        port=$((8080 + i - 1))
        url="http://localhost:$port"
        
        # Check if node is up
        if curl -s --max-time 1 "$url/health" > /dev/null 2>&1; then
            status="${GREEN}✓ Online${NC}"
            
            # Get node info
            node_info=$(curl -s --max-time 1 "$url/node/info" 2>/dev/null || echo "{}")
            block_height=$(get_json_value "$node_info" '.block_height // 0')
            peer_count=$(get_json_value "$node_info" '.peer_count // 0')
            tps=$(get_json_value "$node_info" '.tps // 0')
            
            # Measure latency
            start_time=$(date +%s%N)
            curl -s --max-time 1 "$url/health" > /dev/null 2>&1
            end_time=$(date +%s%N)
            latency=$(( (end_time - start_time) / 1000000 ))
            
            printf "│ Node %d │ %-12s │ %7s │ %7s │ %8s │ %5dms │\n" \
                "$i" "$status" "$block_height" "$peer_count" "$tps" "$latency"
        else
            printf "│ Node %d │ ${RED}✗ Offline${NC}    │    -    │    -    │     -    │    -    │\n" "$i"
        fi
    done
    echo "└────────┴──────────────┴─────────┴─────────┴──────────┴─────────┘"
    echo
    
    # 2. Reth Status
    echo -e "${YELLOW}⚡ Reth Execution Engine:${NC}"
    reth_status=$(curl -s --max-time 1 -X POST http://localhost:8545 \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' 2>/dev/null || echo "{}")
    
    if echo "$reth_status" | grep -q "result"; then
        reth_height=$(echo "$reth_status" | jq -r '.result' | xargs printf "%d\n" 2>/dev/null || echo "0")
        echo -e "  Status: ${GREEN}Connected${NC}"
        echo -e "  Block Height: $(format_number $reth_height)"
        
        # Get sync status
        sync_status=$(curl -s --max-time 1 -X POST http://localhost:8545 \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' 2>/dev/null || echo "{}")
        
        if echo "$sync_status" | grep -q "false"; then
            echo -e "  Sync Status: ${GREEN}Synced${NC}"
        else
            echo -e "  Sync Status: ${YELLOW}Syncing${NC}"
        fi
    else
        echo -e "  Status: ${RED}Disconnected${NC}"
    fi
    echo
    
    # 3. Consensus Status
    echo -e "${YELLOW}🤝 Consensus Status:${NC}"
    consensus_info=$(curl -s --max-time 1 "http://localhost:8080/consensus/status" 2>/dev/null || echo "{}")
    
    if [ "$consensus_info" != "{}" ]; then
        round=$(get_json_value "$consensus_info" '.round // "N/A"')
        phase=$(get_json_value "$consensus_info" '.phase // "N/A"')
        leader=$(get_json_value "$consensus_info" '.leader // "N/A"')
        
        echo "  Current Round: $round"
        echo "  Phase: $phase"
        echo "  Current Leader: $leader"
    else
        echo -e "  Status: ${RED}Not Available${NC}"
    fi
    echo
    
    # 4. Transaction Pool
    echo -e "${YELLOW}💱 Transaction Pool:${NC}"
    txpool_status=$(curl -s --max-time 1 "http://localhost:8080/txpool/status" 2>/dev/null || echo "{}")
    
    if [ "$txpool_status" != "{}" ]; then
        pending=$(get_json_value "$txpool_status" '.pending // 0')
        queued=$(get_json_value "$txpool_status" '.queued // 0')
        
        echo "  Pending: $pending"
        echo "  Queued: $queued"
    else
        echo -e "  Status: ${RED}Not Available${NC}"
    fi
    echo
    
    # 5. Network Stats
    echo -e "${YELLOW}🌐 Network Statistics:${NC}"
    
    # Calculate average block time
    block_times=$(curl -s --max-time 1 "http://localhost:8080/stats/block_times" 2>/dev/null || echo "{}")
    if [ "$block_times" != "{}" ]; then
        avg_block_time=$(get_json_value "$block_times" '.average // "N/A"')
        echo "  Average Block Time: ${avg_block_time}s"
    fi
    
    # Get network TPS
    network_tps=$(curl -s --max-time 1 "http://localhost:8080/stats/tps" 2>/dev/null || echo "{}")
    if [ "$network_tps" != "{}" ]; then
        current_tps=$(get_json_value "$network_tps" '.current // 0')
        peak_tps=$(get_json_value "$network_tps" '.peak // 0')
        echo "  Current TPS: $current_tps"
        echo "  Peak TPS: $peak_tps"
    fi
    echo
    
    # 6. Engine API Status
    echo -e "${YELLOW}🔧 Engine API Status:${NC}"
    engine_status=$(curl -s --max-time 1 "http://localhost:8080/engine/status" 2>/dev/null || echo "{}")
    
    if [ "$engine_status" != "{}" ]; then
        connected=$(get_json_value "$engine_status" '.connected // false')
        last_comm=$(get_json_value "$engine_status" '.last_communication // "N/A"')
        payload_count=$(get_json_value "$engine_status" '.payload_count // 0')
        
        if [ "$connected" = "true" ]; then
            echo -e "  Connection: ${GREEN}Active${NC}"
        else
            echo -e "  Connection: ${RED}Inactive${NC}"
        fi
        echo "  Last Communication: $last_comm"
        echo "  Payloads Processed: $payload_count"
    else
        echo -e "  Status: ${RED}Not Available${NC}"
    fi
    echo
    
    # 7. System Resources
    echo -e "${YELLOW}💻 System Resources:${NC}"
    
    # Get Docker stats for MultiVM containers
    docker_stats=$(docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}" 2>/dev/null | grep multivm || echo "")
    
    if [ -n "$docker_stats" ]; then
        echo "$docker_stats" | column -t -s $'\t'
    else
        echo "  Docker stats not available"
    fi
    
    echo
    echo -e "${CYAN}Press Ctrl+C to exit${NC}"
    
    # Refresh every 2 seconds
    sleep 2
done
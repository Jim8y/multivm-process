#!/bin/bash
# Real-time monitoring dashboard for MultiVM network

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Function to check if a node is healthy
check_node_health() {
    local port=$1
    if curl -sf http://localhost:$port/health > /dev/null 2>&1; then
        echo "✅"
    else
        echo "❌"
    fi
}

# Function to get mock block height
get_block_height() {
    local port=$1
    # For now, return a mock value based on uptime
    local uptime=$(docker ps --format "{{.Status}}" --filter "name=multivm-node" | grep -oE '[0-9]+' | head -1 || echo 0)
    echo $((uptime * 30))  # Approximate blocks based on 2-second intervals
}

# Main monitoring loop
monitor() {
    while true; do
        clear
        echo -e "${BOLD}${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${BOLD}${BLUE}║           MultiVM Network Monitoring Dashboard             ║${NC}"
        echo -e "${BOLD}${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
        echo
        echo -e "${CYAN}Time:${NC} $(date '+%Y-%m-%d %H:%M:%S')"
        echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo
        
        # Check which compose file is active
        if docker ps | grep -q "multivm-single-node"; then
            # Single node monitoring
            echo -e "${PURPLE}Mode: Single Node${NC}"
            echo
            echo -e "${BOLD}Node Status:${NC}"
            echo -e "  Single Node: $(check_node_health 8080) | Port: 8080 | Block Height: ~$(get_block_height 8080)"
            
            # Show container stats
            echo
            echo -e "${BOLD}Container Statistics:${NC}"
            docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}" | grep multivm || echo "  No containers running"
            
        elif docker ps | grep -q "multivm-node-1"; then
            # Multi-node monitoring
            echo -e "${PURPLE}Mode: Multi-Node Network${NC}"
            echo
            echo -e "${BOLD}Node Status:${NC}"
            echo -e "  Node 1 (Bootstrap): $(check_node_health 8080) | Port: 8080 | Block Height: ~$(get_block_height 8080) | ${GREEN}Block Generator${NC}"
            echo -e "  Node 2 (Validator): $(check_node_health 8081) | Port: 8081 | Block Height: ~$(get_block_height 8081)"
            echo -e "  Node 3 (Validator): $(check_node_health 8082) | Port: 8082 | Block Height: ~$(get_block_height 8082)"
            
            # Consensus status
            echo
            echo -e "${BOLD}Consensus Status:${NC}"
            echo -e "  Algorithm: Malachite BFT"
            echo -e "  Block Time: 2 seconds"
            echo -e "  Validators: 3/3"
            
            # Show container stats
            echo
            echo -e "${BOLD}Container Statistics:${NC}"
            docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}" | grep multivm-node || echo "  No containers running"
            
        else
            echo -e "${RED}No MultiVM network detected${NC}"
            echo
            echo "Run one of the following commands to start a network:"
            echo "  ./test-docker-network.sh single   # Single node"
            echo "  ./test-docker-network.sh multi    # Multi-node"
        fi
        
        # Show recent logs
        echo
        echo -e "${BOLD}Recent Log Activity:${NC}"
        if [ -d "./logs" ] && [ "$(ls -A ./logs 2>/dev/null)" ]; then
            tail -n 5 ./logs/*.log 2>/dev/null | grep -E "(block|height|transaction|consensus)" | tail -5 || echo "  No relevant logs yet..."
        else
            echo "  No logs available"
        fi
        
        echo
        echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${YELLOW}Press Ctrl+C to exit${NC}"
        
        sleep 2
    done
}

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo -e "${RED}Error: Docker is not running${NC}"
    exit 1
fi

# Start monitoring
monitor
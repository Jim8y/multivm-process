#!/bin/bash
# Health check script for MultiVM system

set -e

# Configuration
API_HOST="${MULTIVM_API_HOST:-localhost}"
API_PORT="${MULTIVM_API_PORT:-8080}"
HEALTH_ENDPOINT="http://${API_HOST}:${API_PORT}/health"
DETAILED_ENDPOINT="http://${API_HOST}:${API_PORT}/health/detail"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to check endpoint
check_endpoint() {
    local endpoint=$1
    local description=$2
    
    echo -e "${BLUE}Checking ${description}...${NC}"
    
    response=$(curl -s -w "\n%{http_code}" "$endpoint" 2>/dev/null || echo "000")
    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | head -n-1)
    
    if [ "$http_code" = "200" ]; then
        echo -e "${GREEN}✓ ${description}: OK${NC}"
        return 0
    else
        echo -e "${RED}✗ ${description}: Failed (HTTP $http_code)${NC}"
        return 1
    fi
}

# Function to get detailed health
get_detailed_health() {
    echo -e "\n${BLUE}Fetching detailed health information...${NC}"
    
    response=$(curl -s -X POST "$DETAILED_ENDPOINT" \
        -H "Content-Type: application/json" \
        -d '{
            "include_components": true,
            "include_metrics": true,
            "include_dependencies": true
        }' 2>/dev/null || echo "{}")
    
    # Parse JSON response (requires jq)
    if command -v jq &> /dev/null; then
        status=$(echo "$response" | jq -r '.status // "unknown"')
        message=$(echo "$response" | jq -r '.message // "No message"')
        uptime=$(echo "$response" | jq -r '.uptime_seconds // 0')
        
        # Convert uptime to human readable
        days=$((${uptime%.*} / 86400))
        hours=$(( (${uptime%.*} % 86400) / 3600))
        minutes=$(( (${uptime%.*} % 3600) / 60))
        
        echo -e "\n${BLUE}=== System Status ===${NC}"
        echo -e "Status: $(format_status "$status")"
        echo -e "Message: $message"
        echo -e "Uptime: ${days}d ${hours}h ${minutes}m"
        
        # Component health
        if [ "$(echo "$response" | jq -r '.components // null')" != "null" ]; then
            echo -e "\n${BLUE}=== Component Health ===${NC}"
            echo "$response" | jq -r '.components.components[] | "\(.name): \(.status) - \(.message)"' | while read line; do
                if [[ $line == *"healthy"* ]]; then
                    echo -e "${GREEN}✓ $line${NC}"
                elif [[ $line == *"degraded"* ]]; then
                    echo -e "${YELLOW}⚠ $line${NC}"
                else
                    echo -e "${RED}✗ $line${NC}"
                fi
            done
            
            healthy=$(echo "$response" | jq -r '.components.healthy_count // 0')
            degraded=$(echo "$response" | jq -r '.components.degraded_count // 0')
            unhealthy=$(echo "$response" | jq -r '.components.unhealthy_count // 0')
            
            echo -e "\nSummary: ${GREEN}$healthy healthy${NC}, ${YELLOW}$degraded degraded${NC}, ${RED}$unhealthy unhealthy${NC}"
        fi
        
        # System metrics
        if [ "$(echo "$response" | jq -r '.metrics // null')" != "null" ]; then
            echo -e "\n${BLUE}=== System Metrics ===${NC}"
            cpu=$(echo "$response" | jq -r '.metrics.cpu_usage_percent // 0')
            mem_percent=$(echo "$response" | jq -r '.metrics.memory.usage_percent // 0')
            connections=$(echo "$response" | jq -r '.metrics.network.active_connections // 0')
            rps=$(echo "$response" | jq -r '.metrics.requests.requests_per_second // 0')
            
            echo "CPU Usage: $(format_metric "$cpu" 80 90)%"
            echo "Memory Usage: $(format_metric "$mem_percent" 80 90)%"
            echo "Active Connections: $connections"
            echo "Requests/sec: $rps"
        fi
        
        # Dependencies
        if [ "$(echo "$response" | jq -r '.dependencies // null')" != "null" ]; then
            echo -e "\n${BLUE}=== Dependencies ===${NC}"
            
            # Database
            db_available=$(echo "$response" | jq -r '.dependencies.database.available // false')
            db_info=$(echo "$response" | jq -r '.dependencies.database.info // "Unknown"')
            echo -e "Database: $(format_bool "$db_available") - $db_info"
            
            # Cache
            cache_available=$(echo "$response" | jq -r '.dependencies.cache.available // false')
            cache_info=$(echo "$response" | jq -r '.dependencies.cache.info // "Unknown"')
            echo -e "Cache: $(format_bool "$cache_available") - $cache_info"
            
            # Consensus
            consensus_available=$(echo "$response" | jq -r '.dependencies.consensus_network.available // false')
            consensus_info=$(echo "$response" | jq -r '.dependencies.consensus_network.info // "Unknown"')
            echo -e "Consensus: $(format_bool "$consensus_available") - $consensus_info"
            
            # VMs
            evm_available=$(echo "$response" | jq -r '.dependencies.vm_processes.evm.available // false')
            svm_available=$(echo "$response" | jq -r '.dependencies.vm_processes.svm.available // false')
            echo -e "EVM Process: $(format_bool "$evm_available")"
            echo -e "SVM Process: $(format_bool "$svm_available")"
        fi
        
    else
        echo -e "${YELLOW}Warning: jq not installed, showing raw response${NC}"
        echo "$response" | python -m json.tool 2>/dev/null || echo "$response"
    fi
}

# Format status with color
format_status() {
    case "$1" in
        "healthy")
            echo -e "${GREEN}HEALTHY${NC}"
            ;;
        "degraded")
            echo -e "${YELLOW}DEGRADED${NC}"
            ;;
        "unhealthy")
            echo -e "${RED}UNHEALTHY${NC}"
            ;;
        *)
            echo -e "${RED}UNKNOWN${NC}"
            ;;
    esac
}

# Format boolean with color
format_bool() {
    if [ "$1" = "true" ]; then
        echo -e "${GREEN}✓ Available${NC}"
    else
        echo -e "${RED}✗ Unavailable${NC}"
    fi
}

# Format metric with thresholds
format_metric() {
    local value=$1
    local warn_threshold=$2
    local crit_threshold=$3
    
    if (( $(echo "$value >= $crit_threshold" | bc -l) )); then
        echo -e "${RED}$value${NC}"
    elif (( $(echo "$value >= $warn_threshold" | bc -l) )); then
        echo -e "${YELLOW}$value${NC}"
    else
        echo -e "${GREEN}$value${NC}"
    fi
}

# Main execution
echo -e "${BLUE}=== MultiVM Health Check ===${NC}"
echo -e "API Endpoint: ${API_HOST}:${API_PORT}\n"

# Check if API is reachable
if ! nc -z "$API_HOST" "$API_PORT" 2>/dev/null; then
    echo -e "${RED}Error: Cannot connect to API at ${API_HOST}:${API_PORT}${NC}"
    exit 1
fi

# Basic health check
check_endpoint "$HEALTH_ENDPOINT" "Basic Health"

# Live health check (Kubernetes style)
check_endpoint "${HEALTH_ENDPOINT}/live" "Liveness"

# Detailed health check
get_detailed_health

# Exit code based on overall health
if [[ "$status" == "healthy" ]]; then
    echo -e "\n${GREEN}✓ System is healthy${NC}"
    exit 0
elif [[ "$status" == "degraded" ]]; then
    echo -e "\n${YELLOW}⚠ System is degraded but operational${NC}"
    exit 1
else
    echo -e "\n${RED}✗ System is unhealthy${NC}"
    exit 2
fi
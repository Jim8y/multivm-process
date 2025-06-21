#!/bin/bash
# MultiVM Node Health Check Script

NODE_ID=${NODE_ID:-"unknown"}
API_ADDR=${API_LISTEN_ADDR:-"localhost:8080"}

# Extract host and port
HOST=$(echo $API_ADDR | cut -d':' -f1)
PORT=$(echo $API_ADDR | cut -d':' -f2)

# Health check function
check_health() {
    curl -f "http://$HOST:$PORT/health" > /dev/null 2>&1
    return $?
}

# API endpoint check
check_api() {
    curl -f "http://$HOST:$PORT/api/v1/system/info" > /dev/null 2>&1
    return $?
}

# P2P connectivity check
check_p2p() {
    # Simple port check for P2P
    local p2p_port=${P2P_LISTEN_ADDR##*:}
    netstat -ln | grep ":$p2p_port " > /dev/null 2>&1
    return $?
}

# Main health check
main() {
    echo "[$NODE_ID] Running health check..."
    
    # Check if the main health endpoint is available
    if check_health; then
        echo "[$NODE_ID] ✅ Health endpoint: HEALTHY"
    else
        echo "[$NODE_ID] ❌ Health endpoint: UNHEALTHY"
        exit 1
    fi
    
    # Check API availability
    if check_api; then
        echo "[$NODE_ID] ✅ API endpoint: AVAILABLE"
    else
        echo "[$NODE_ID] ⚠️  API endpoint: LIMITED"
    fi
    
    # Check P2P port
    if check_p2p; then
        echo "[$NODE_ID] ✅ P2P port: LISTENING"
    else
        echo "[$NODE_ID] ⚠️  P2P port: NOT LISTENING"
    fi
    
    echo "[$NODE_ID] Health check completed successfully"
}

main "$@"
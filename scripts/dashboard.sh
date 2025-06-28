#!/bin/bash

# Real-time MultiVM Node Dashboard

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
NC='\033[0m'

clear

while true; do
    # Move cursor to top
    tput cup 0 0
    
    echo -e "${CYAN}╔════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║                      🚀 MultiVM Node Dashboard                         ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    
    # Get stats
    STATS=$(curl -s http://localhost:8080/api/v1/explorer/stats 2>/dev/null)
    HEALTH=$(curl -s http://localhost:8090/health 2>/dev/null)
    
    if [ ! -z "$STATS" ]; then
        # Extract data
        POOL=$(echo $STATS | jq -r '.data.transaction_pool')
        PENDING=$(echo $POOL | jq -r '.pending')
        TOTAL=$(echo $POOL | jq -r '.total_submitted')
        INCLUDED=$(echo $POOL | jq -r '.total_included')
        EVM_PENDING=$(echo $POOL | jq -r '.evm_pending')
        SVM_PENDING=$(echo $POOL | jq -r '.svm_pending')
        
        # Network stats
        HEIGHT=$(echo $STATS | jq -r '.data.current_height')
        TPS=$(echo $STATS | jq -r '.data.tps_current')
        VALIDATORS=$(echo $STATS | jq -r '.data.validators_online')
        
        echo -e "${PURPLE}🔗 Network Status${NC}"
        echo -e "├─ Block Height:    ${GREEN}#$HEIGHT${NC}"
        echo -e "├─ Current TPS:     ${YELLOW}$TPS${NC}"
        echo -e "└─ Validators:      ${BLUE}$VALIDATORS online${NC}"
        echo ""
        
        echo -e "${PURPLE}📊 Transaction Pool${NC}"
        echo -e "├─ Pending:         ${YELLOW}$PENDING${NC} transactions"
        echo -e "├─ Total Submitted: ${BLUE}$TOTAL${NC}"
        echo -e "├─ Total Included:  ${GREEN}$INCLUDED${NC}"
        echo -e "├─ EVM Pending:     ${CYAN}$EVM_PENDING${NC}"
        echo -e "└─ SVM Pending:     ${CYAN}$SVM_PENDING${NC}"
        echo ""
        
        # API endpoints
        echo -e "${PURPLE}🌐 API Endpoints${NC}"
        echo -e "├─ REST API:        ${GREEN}✓${NC} http://localhost:8080"
        echo -e "├─ GraphQL:         ${GREEN}✓${NC} http://localhost:8081"
        echo -e "├─ WebSocket:       ${GREEN}✓${NC} ws://localhost:8082"
        echo -e "├─ Metrics:         ${GREEN}✓${NC} http://localhost:9090/metrics"
        echo -e "└─ Health:          ${GREEN}✓${NC} http://localhost:8090/health"
        echo ""
        
        # Recent block from logs
        RECENT_BLOCKS=$(tail -5 /tmp/multivm-testnet/logs/node.log 2>/dev/null | grep "Block.*generated" | tail -3)
        if [ ! -z "$RECENT_BLOCKS" ]; then
            echo -e "${PURPLE}📦 Recent Blocks${NC}"
            echo "$RECENT_BLOCKS" | while read -r line; do
                BLOCK_NUM=$(echo $line | grep -oP 'Block \K\d+')
                TX_COUNT=$(echo $line | grep -oP 'generated with \K\d+')
                echo -e "└─ Block #$BLOCK_NUM: ${GREEN}$TX_COUNT${NC} transactions"
            done
        fi
        
    else
        echo -e "${RED}⚠️  Unable to connect to node${NC}"
    fi
    
    echo ""
    echo -e "${CYAN}Press Ctrl+C to exit${NC}"
    
    sleep 2
done
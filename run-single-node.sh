#!/bin/bash
# Run single node MultiVM network with continuous logging

set -e

# Configuration
LOG_FILE="multivm-single-node-$(date +%Y%m%d_%H%M%S).log"
COMPOSE_FILE="docker-compose.single.yml"
HEALTH_CHECK_INTERVAL=10
BLOCK_CHECK_INTERVAL=30

# Colors for terminal output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m'

# Logging functions
log() {
    local message="[$(date '+%Y-%m-%d %H:%M:%S')] $*"
    echo -e "${BLUE}${message}${NC}"
    echo "${message}" >> "$LOG_FILE"
}

success() {
    local message="✓ $*"
    echo -e "${GREEN}${message}${NC}"
    echo "[SUCCESS] ${message}" >> "$LOG_FILE"
}

warning() {
    local message="⚠ $*"
    echo -e "${YELLOW}${message}${NC}"
    echo "[WARNING] ${message}" >> "$LOG_FILE"
}

error() {
    local message="✗ $*"
    echo -e "${RED}${message}${NC}"
    echo "[ERROR] ${message}" >> "$LOG_FILE"
}

info() {
    local message="ℹ $*"
    echo -e "${PURPLE}${message}${NC}"
    echo "[INFO] ${message}" >> "$LOG_FILE"
}

# Cleanup function
cleanup() {
    log "Cleaning up..."
    docker-compose -f $COMPOSE_FILE down -v 2>&1 | tee -a "$LOG_FILE"
    if [ -f multivm-node-output.log ]; then
        cat multivm-node-output.log >> "$LOG_FILE"
        rm multivm-node-output.log
    fi
    success "Cleanup completed"
}

# Set trap for cleanup on exit
trap cleanup EXIT INT TERM

# Main execution
main() {
    echo "======================================"
    echo "MultiVM Single Node Network Runner"
    echo "======================================"
    echo "Log file: $LOG_FILE"
    echo ""
    
    # Initialize log file
    echo "MultiVM Single Node Network Log" > "$LOG_FILE"
    echo "Started at: $(date)" >> "$LOG_FILE"
    echo "======================================" >> "$LOG_FILE"
    
    # Check Docker
    log "Checking Docker availability..."
    if ! docker info > /dev/null 2>&1; then
        error "Docker is not running. Please start Docker first."
        exit 1
    fi
    success "Docker is running"
    
    # Clean up any existing containers
    log "Cleaning up any existing containers..."
    docker-compose -f $COMPOSE_FILE down -v 2>&1 >> "$LOG_FILE" 2>&1 || true
    
    # Create logs directory
    mkdir -p logs
    
    # Build the image
    log "Building MultiVM Docker image..."
    docker-compose -f $COMPOSE_FILE build 2>&1 | tee -a "$LOG_FILE"
    success "Docker image built successfully"
    
    # Start the single node
    log "Starting MultiVM single node..."
    docker-compose -f $COMPOSE_FILE up -d 2>&1 | tee -a "$LOG_FILE"
    
    # Wait for node to be healthy
    log "Waiting for node to become healthy..."
    local retries=60
    local healthy=false
    
    while [ $retries -gt 0 ]; do
        if docker-compose -f $COMPOSE_FILE ps 2>/dev/null | grep -q "healthy"; then
            healthy=true
            break
        fi
        echo -n "."
        sleep 2
        retries=$((retries - 1))
    done
    echo ""
    
    if [ "$healthy" = true ]; then
        success "Node is healthy!"
    else
        error "Node failed to become healthy within timeout"
        docker-compose -f $COMPOSE_FILE logs >> "$LOG_FILE" 2>&1
        exit 1
    fi
    
    # Verify health endpoint
    log "Verifying health endpoint..."
    if curl -sf http://localhost:8080/health > /dev/null; then
        success "Health endpoint is responding"
        echo "Health check response:" >> "$LOG_FILE"
        curl -s http://localhost:8080/health 2>/dev/null | tee -a "$LOG_FILE" || true
        echo "" >> "$LOG_FILE"
    else
        error "Health endpoint is not responding"
    fi
    
    # Start continuous logging in background
    log "Starting continuous log collection..."
    docker-compose -f $COMPOSE_FILE logs -f > multivm-node-output.log 2>&1 &
    LOGS_PID=$!
    
    # Monitor block generation
    info "Monitoring block generation..."
    echo ""
    echo "Block Generation Status:" | tee -a "$LOG_FILE"
    echo "========================" | tee -a "$LOG_FILE"
    
    local block_count=0
    local last_block_check=$(date +%s)
    local monitoring_start=$(date +%s)
    
    while true; do
        # Check if container is still running
        if ! docker-compose -f $COMPOSE_FILE ps 2>/dev/null | grep -q "Up"; then
            error "Container has stopped unexpectedly"
            break
        fi
        
        # Get current timestamp
        local current_time=$(date +%s)
        local elapsed=$((current_time - monitoring_start))
        
        # Check for new blocks in logs
        if [ -f multivm-node-output.log ]; then
            local new_blocks=$(grep -c -E "(Generated consensus block|Successfully proposed consensus block|Generated mock block)" multivm-node-output.log 2>/dev/null || echo 0)
            if [ $new_blocks -gt $block_count ]; then
                local blocks_generated=$((new_blocks - block_count))
                block_count=$new_blocks
                
                # Extract last few block generation logs
                local recent_blocks=$(tail -n 100 multivm-node-output.log | grep -E "(Generated consensus block|Successfully proposed|Generated mock block)" | tail -5)
                
                success "Blocks generated: $block_count (Total) | +$blocks_generated (New)"
                echo "Recent block generation logs:" | tee -a "$LOG_FILE"
                echo "$recent_blocks" | tee -a "$LOG_FILE"
                echo "" | tee -a "$LOG_FILE"
                
                # Calculate block generation rate
                if [ $elapsed -gt 0 ]; then
                    local blocks_per_minute=$(echo "scale=2; $block_count * 60 / $elapsed" | bc 2>/dev/null || echo "N/A")
                    info "Block generation rate: ~$blocks_per_minute blocks/minute"
                    echo "Block generation rate: ~$blocks_per_minute blocks/minute" >> "$LOG_FILE"
                fi
            fi
        fi
        
        # Show container stats periodically
        if [ $((current_time - last_block_check)) -ge $BLOCK_CHECK_INTERVAL ]; then
            echo "" | tee -a "$LOG_FILE"
            info "Container Statistics:"
            docker stats --no-stream multivm-single-node 2>/dev/null | tee -a "$LOG_FILE" || true
            
            # Show recent consensus/transaction logs
            if [ -f multivm-node-output.log ]; then
                echo "" | tee -a "$LOG_FILE"
                info "Recent activity:"
                tail -n 20 multivm-node-output.log | grep -E "(consensus|transaction|block|height)" | tail -5 | tee -a "$LOG_FILE" || true
            fi
            
            last_block_check=$current_time
            
            # Summary
            echo "" | tee -a "$LOG_FILE"
            echo "Summary after $elapsed seconds:" | tee -a "$LOG_FILE"
            echo "- Total blocks generated: $block_count" | tee -a "$LOG_FILE"
            echo "- Expected blocks (2s interval): $((elapsed / 2))" | tee -a "$LOG_FILE"
            echo "- Node status: $(docker-compose -f $COMPOSE_FILE ps | grep multivm | awk '{print $NF}')" | tee -a "$LOG_FILE"
            echo "================================" | tee -a "$LOG_FILE"
        fi
        
        # Check every 5 seconds
        sleep 5
        
        # Show we're still monitoring
        echo -ne "\rMonitoring... Elapsed: ${elapsed}s | Blocks: ${block_count} | Press Ctrl+C to stop"
    done
    
    # Kill background logs process
    if [ ! -z "$LOGS_PID" ]; then
        kill $LOGS_PID 2>/dev/null || true
    fi
}

# Start the single node network
log "Starting MultiVM single node network test"
main

# Final summary
echo ""
echo ""
log "Test completed. Full logs saved to: $LOG_FILE"
info "To view the log file: cat $LOG_FILE"
info "To follow the log file: tail -f $LOG_FILE"
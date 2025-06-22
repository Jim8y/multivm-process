#!/bin/bash
# Simple script to start single node and log output

# Create logs directory
mkdir -p logs

# Set log file with timestamp
LOG_FILE="logs/multivm-single-node-$(date +%Y%m%d_%H%M%S).log"

echo "Starting MultiVM single node network..."
echo "Log file: $LOG_FILE"
echo "======================================="

# Clean up any existing containers
echo "Cleaning up existing containers..."
docker-compose -f docker-compose.single.yml down -v 2>/dev/null || true

# Build and start
echo "Building and starting node..."
docker-compose -f docker-compose.single.yml build 2>&1 | tee -a "$LOG_FILE"
docker-compose -f docker-compose.single.yml up 2>&1 | tee -a "$LOG_FILE"
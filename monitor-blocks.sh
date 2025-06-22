#!/bin/bash
# Monitor block generation from running container

# Check if container is running
if ! docker ps | grep -q "multivm-single-node"; then
    echo "Error: multivm-single-node container is not running"
    echo "Start it with: ./run-single-node.sh or ./start-and-log.sh"
    exit 1
fi

echo "Monitoring block generation..."
echo "=============================="
echo ""

# Follow logs and highlight block generation
docker logs -f multivm-single-node 2>&1 | grep --line-buffered -E "(Generated mock block|Block generator|block.*height|Starting block generation)" | while IFS= read -r line; do
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $line"
done
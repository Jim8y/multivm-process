#!/bin/bash
# Monitor GitHub Actions build progress

RUN_ID=16337500915
REPO="vm-multiverse/multivm"

echo "Monitoring build progress for run $RUN_ID..."
echo "Started at: $(date)"
echo "---"

while true; do
    STATUS=$(gh run view $RUN_ID --repo $REPO --json status,conclusion -q '.status')
    CONCLUSION=$(gh run view $RUN_ID --repo $REPO --json status,conclusion -q '.conclusion')
    
    if [ "$STATUS" = "completed" ]; then
        echo "Build completed with result: $CONCLUSION"
        
        # Get job details
        gh run view $RUN_ID --repo $REPO
        
        # If failed, try to get logs
        if [ "$CONCLUSION" != "success" ]; then
            echo "Attempting to get failure logs..."
            gh run view $RUN_ID --repo $REPO --log-failed || echo "Logs not available"
        fi
        
        break
    else
        echo "$(date '+%H:%M:%S') - Status: $STATUS (still running...)"
        sleep 60
    fi
done

echo "---"
echo "Finished at: $(date)"
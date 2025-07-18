#!/bin/bash
# Script to copy MultiVM configuration files to their correct locations

echo "This script will copy the MultiVM configuration files to their respective node directories."
echo "Since the node directories are owned by root, you'll need to run this with sudo."
echo ""
echo "Run the following commands to copy the files:"
echo ""

for i in {1..7}; do
    echo "sudo cp /home/neo/git/multivm-process/testnet/configs/temp_configs/node${i}_multivm.toml /home/neo/git/multivm-process/testnet/configs/node${i}/multivm.toml"
done

echo ""
echo "After copying, you can verify the files with:"
echo "ls -la /home/neo/git/multivm-process/testnet/configs/node*/multivm.toml"
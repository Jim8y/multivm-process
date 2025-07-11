#!/bin/bash

echo "=== Running MultiVM Test Suite ==="
echo ""

total_passed=0
total_failed=0

# Test each package and capture results
packages=(
    "multivm-common"
    "multivm-consensus"
    "multivm-process-manager"
    "multivm-account-mapping"
    "multivm-p2p"
    "multivm-application"
    "reth-execution-engine"
    "solana-execution-engine"
)

for package in "${packages[@]}"; do
    echo "Testing $package..."
    output=$(cargo test -p "$package" --lib 2>&1)
    
    # Extract test results
    if echo "$output" | grep -q "test result: ok"; then
        passed=$(echo "$output" | grep "test result: ok" | tail -1 | sed -E 's/test result: ok\. ([0-9]+) passed; ([0-9]+) failed.*/\1/')
        failed=$(echo "$output" | grep "test result: ok" | tail -1 | sed -E 's/test result: ok\. ([0-9]+) passed; ([0-9]+) failed.*/\2/')
        echo "  ✓ $package: $passed passed, $failed failed"
        total_passed=$((total_passed + passed))
        total_failed=$((total_failed + failed))
    else
        echo "  ✗ $package: Build or test failed"
    fi
done

echo ""
echo "=== Integration Tests ==="
output=$(cargo test --test '*' 2>&1)
if echo "$output" | grep -q "test result: ok"; then
    int_passed=$(echo "$output" | grep "test result: ok" | awk '{sum += $3} END {print sum}')
    echo "  ✓ Integration tests: $int_passed passed"
    total_passed=$((total_passed + int_passed))
fi

echo ""
echo "=== Test Summary ==="
echo "Total tests passed: $total_passed"
echo "Total tests failed: $total_failed"

if [ $total_failed -eq 0 ]; then
    echo "✅ All tests passed!"
else
    echo "❌ Some tests failed!"
    exit 1
fi
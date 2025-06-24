#!/bin/bash

# MultiVM Consistency Check Script

echo "🔍 MultiVM Consistency Check"
echo "============================"

# Check 1: Ensure all Cargo.toml files have consistent metadata
echo ""
echo "📦 Checking Cargo.toml consistency..."
for toml in $(find crates -name "Cargo.toml"); do
    if ! grep -q "version.workspace = true" "$toml"; then
        echo "❌ $toml: Missing workspace version"
    fi
    if ! grep -q "edition.workspace = true" "$toml"; then
        echo "❌ $toml: Missing workspace edition"
    fi
    if ! grep -q "license.workspace = true" "$toml"; then
        echo "❌ $toml: Missing workspace license"
    fi
done

# Check 2: Ensure all error types implement std::error::Error
echo ""
echo "🚨 Checking error implementations..."
for error_file in $(find crates -path "*/src/*" -name "error.rs" -o -name "errors.rs"); do
    if grep -q "pub enum.*Error" "$error_file"; then
        if ! grep -q "impl.*std::error::Error" "$error_file" && ! grep -q "thiserror::Error" "$error_file"; then
            echo "❌ $error_file: Error enum doesn't implement std::error::Error"
        fi
    fi
done

# Check 3: Ensure consistent use of async runtime
echo ""
echo "⚡ Checking async runtime consistency..."
for rs_file in $(find crates -name "*.rs" -type f); do
    if grep -q "async fn main" "$rs_file"; then
        if ! grep -q "#\[tokio::main\]" "$rs_file"; then
            echo "❌ $rs_file: async main without tokio::main attribute"
        fi
    fi
done

# Check 4: Check for TODO/FIXME comments
echo ""
echo "📝 Checking for TODO/FIXME comments..."
TODO_COUNT=$(grep -r "TODO\|FIXME" crates --include="*.rs" | wc -l)
if [ "$TODO_COUNT" -gt 0 ]; then
    echo "⚠️  Found $TODO_COUNT TODO/FIXME comments:"
    grep -r "TODO\|FIXME" crates --include="*.rs" | head -5
    echo "..."
fi

# Check 5: Ensure all public types have documentation
echo ""
echo "📚 Checking documentation coverage..."
for rs_file in $(find crates -name "lib.rs" -o -name "mod.rs"); do
    PUB_ITEMS=$(grep -E "^pub (struct|enum|fn|trait|type)" "$rs_file" | wc -l)
    DOC_ITEMS=$(grep -B1 -E "^pub (struct|enum|fn|trait|type)" "$rs_file" | grep "///" | wc -l)
    if [ "$PUB_ITEMS" -gt 0 ] && [ "$DOC_ITEMS" -lt "$PUB_ITEMS" ]; then
        COVERAGE=$((DOC_ITEMS * 100 / PUB_ITEMS))
        if [ "$COVERAGE" -lt 80 ]; then
            echo "⚠️  $rs_file: Documentation coverage is only $COVERAGE% ($DOC_ITEMS/$PUB_ITEMS)"
        fi
    fi
done

# Check 6: Verify all tests pass
echo ""
echo "🧪 Running tests..."
if cargo test --workspace --quiet 2>/dev/null; then
    echo "✅ All tests pass"
else
    echo "❌ Some tests are failing"
fi

# Check 7: Check for clippy warnings
echo ""
echo "📎 Running clippy..."
CLIPPY_WARNINGS=$(cargo clippy --workspace --all-features 2>&1 | grep -c "warning:")
if [ "$CLIPPY_WARNINGS" -eq 0 ]; then
    echo "✅ No clippy warnings"
else
    echo "⚠️  Found $CLIPPY_WARNINGS clippy warnings"
fi

# Check 8: Verify consistent dependency versions
echo ""
echo "🔗 Checking dependency consistency..."
cargo tree --workspace --duplicates 2>/dev/null | head -10

# Check 9: Security audit
echo ""
echo "🔒 Running security audit..."
if command -v cargo-audit >/dev/null 2>&1; then
    cargo audit 2>/dev/null || echo "⚠️  Some vulnerabilities found"
else
    echo "ℹ️  cargo-audit not installed, skipping security check"
fi

echo ""
echo "✅ Consistency check complete!"
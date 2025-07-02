#!/bin/bash
# Script to find compatible Rust version for the project

echo "=== Finding Compatible Rust Version ==="
echo

# Check if we need edition2024
if grep -r "edition.*2024" . --include="*.toml" 2>/dev/null | grep -v "target/"; then
    echo "⚠️  Found crates using edition2024 - nightly Rust required"
    echo
    echo "To use nightly:"
    echo "  rustup install nightly"
    echo "  rustup default nightly"
    echo
    echo "Or to test with nightly:"
    echo "  cargo +nightly build"
else
    echo "✓ No edition2024 dependencies found"
fi

# Check current Rust version
echo
echo "Current Rust version:"
rustc --version

# Test with stable
echo
echo "Testing with stable Rust..."
if command -v rustup >/dev/null 2>&1; then
    rustup run stable rustc --version 2>/dev/null || echo "Stable not installed"
    
    echo
    echo "Testing with nightly Rust..."
    rustup run nightly rustc --version 2>/dev/null || echo "Nightly not installed"
    
    # List available toolchains
    echo
    echo "Available toolchains:"
    rustup toolchain list
fi

# Check Cargo.toml for rust-version
echo
echo "Project rust-version requirement:"
grep -h "rust-version" Cargo.toml 2>/dev/null || echo "No rust-version specified"

# Recommendations
echo
echo "=== Recommendations ==="
echo
echo "1. For immediate testing:"
echo "   ./test-ci-locally.sh"
echo
echo "2. If edition2024 is required:"
echo "   rustup install nightly"
echo "   cargo +nightly build --workspace --all-features"
echo
echo "3. To update CI to use nightly:"
echo "   Change 'dtolnay/rust-toolchain@1.79' to 'dtolnay/rust-toolchain@nightly'"
echo "   in .github/workflows/ci.yml"
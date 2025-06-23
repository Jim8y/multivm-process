#!/bin/bash

echo "🔧 Fixing Cargo.toml consistency issues..."

# Function to add workspace inheritance to a Cargo.toml
add_workspace_inheritance() {
    local file=$1
    echo "  Updating $file to use workspace inheritance..."
    
    # Replace version = "0.1.0" with version.workspace = true
    sed -i 's/version = "0.1.0"/version.workspace = true/g' "$file"
    
    # Replace edition = "2021" with edition.workspace = true
    sed -i 's/edition = "2021"/edition.workspace = true/g' "$file"
    
    # Replace license = "MIT OR Apache-2.0" with license.workspace = true
    sed -i 's/license = "MIT OR Apache-2.0"/license.workspace = true/g' "$file"
}

# Fix multivm-cli to use workspace versions
echo "📦 Fixing multivm-cli dependencies..."
add_workspace_inheritance "crates/multivm-cli/Cargo.toml"

# Update tempfile dependency to use workspace version
sed -i 's/tempfile = "3.0"/tempfile.workspace = true/g' "crates/multivm-cli/Cargo.toml"
sed -i 's/tempfile = "3.0"/tempfile.workspace = true/g' "crates/reth-execution-engine/Cargo.toml"

# Update hex dependency to use workspace version
sed -i 's/hex = "0.4.3"/hex.workspace = true/g' "crates/multivm-mock-processes/Cargo.toml"
sed -i 's/hex = "0.4"/hex.workspace = true/g' "crates/multivm-p2p/Cargo.toml"

echo "📝 Adding missing metadata to crates..."

# Add authors to all crates missing it
AUTHOR_LINE='authors = ["MultiVM Contributors"]'

# Function to add authors after version line
add_authors() {
    local file=$1
    if ! grep -q "authors = " "$file"; then
        echo "  Adding authors to $file"
        # Add authors after the version line
        sed -i "/^version/a $AUTHOR_LINE" "$file"
    fi
}

# Add authors to crates missing them
add_authors "crates/multivm-common/Cargo.toml"
add_authors "crates/multivm-consensus/Cargo.toml"
add_authors "crates/multivm-account-mapping/Cargo.toml"
add_authors "crates/multivm-p2p/Cargo.toml"
add_authors "crates/multivm-process-manager/Cargo.toml"
add_authors "crates/multivm-mock-processes/Cargo.toml"
add_authors "crates/solana-execution-engine/Cargo.toml"
add_authors "crates/reth-execution-engine/Cargo.toml"

# Add descriptions to crates missing them
add_description() {
    local file=$1
    local desc=$2
    if ! grep -q "description = " "$file"; then
        echo "  Adding description to $file"
        sed -i "/^authors/a description = \"$desc\"" "$file"
    fi
}

add_description "crates/multivm-consensus/Cargo.toml" "Malachite consensus integration for MultiVM"
add_description "crates/multivm-mock-processes/Cargo.toml" "Mock blockchain processes for MultiVM testing"
add_description "crates/solana-execution-engine/Cargo.toml" "Solana execution engine integration for MultiVM"
add_description "crates/reth-execution-engine/Cargo.toml" "Reth execution engine integration for MultiVM"

echo "🔄 Consolidating duplicate dependencies to workspace..."

# Add ed25519-dalek to workspace dependencies if not already there
if ! grep -q "ed25519-dalek" "Cargo.toml"; then
    echo "  Adding ed25519-dalek to workspace dependencies"
    # This would need manual editing as it's complex to insert in the right place
fi

echo "✅ Cargo.toml consistency fixes applied!"
echo ""
echo "⚠️  Manual steps required:"
echo "1. Add ed25519-dalek = \"2.1\" to workspace dependencies in root Cargo.toml"
echo "2. Update all crates using ed25519-dalek to use workspace version"
echo "3. Consider updating outdated dependencies:"
echo "   - sysinfo 0.29 → 0.31+"
echo "   - which 4.4 → 6.0+"
echo "   - testcontainers 0.15 → 0.23+"
echo "   - wiremock 0.5 → 0.6+"
echo "4. Fix tests/Cargo.toml to have proper package metadata"
echo "5. Consolidate governor and nonzero_ext to workspace dependencies"
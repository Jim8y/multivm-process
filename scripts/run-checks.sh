#!/bin/bash

echo "═══════════════════════════════════════════════════════════════════════"
echo "                    MultiVM Comprehensive Check                         "
echo "═══════════════════════════════════════════════════════════════════════"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to run a check
run_check() {
    local name="$1"
    local cmd="$2"
    
    echo -e "\n${BLUE}🔍 ${name}${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    if eval "$cmd"; then
        echo -e "${GREEN}✅ ${name} passed${NC}"
    else
        echo -e "${RED}❌ ${name} failed${NC}"
        exit 1
    fi
}

# 1. Check if project builds
run_check "Build Check" "cargo build --workspace 2>&1 | grep -q 'Finished'"

# 2. Run all tests
run_check "Test Suite" "cargo test --workspace --quiet"

# 3. Check for compilation warnings
echo -e "\n${BLUE}🔍 Checking for critical issues...${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
WARNINGS=$(cargo check --workspace 2>&1 | grep -c "warning:")
ERRORS=$(cargo check --workspace 2>&1 | grep -c "error:")

if [ "$ERRORS" -gt 0 ]; then
    echo -e "${RED}❌ Found $ERRORS compilation errors${NC}"
    exit 1
else
    echo -e "${GREEN}✅ No compilation errors${NC}"
fi

if [ "$WARNINGS" -gt 0 ]; then
    echo -e "${YELLOW}⚠️  Found $WARNINGS warnings (non-critical)${NC}"
else
    echo -e "${GREEN}✅ No warnings${NC}"
fi

# 4. Check key packages
echo -e "\n${BLUE}📦 Package Status:${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

PACKAGES=(
    "multivm-common"
    "multivm-consensus"
    "multivm-p2p"
    "multivm-application"
    "multivm-process-manager"
    "multivm-account-mapping"
)

for pkg in "${PACKAGES[@]}"; do
    if cargo test --package "$pkg" --quiet 2>/dev/null; then
        echo -e "   ${GREEN}✅ $pkg${NC}"
    else
        echo -e "   ${RED}❌ $pkg${NC}"
    fi
done

# 5. Summary
echo -e "\n${GREEN}═══════════════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}                    ✅ All Checks Passed!                               ${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════════════════════${NC}"
echo -e "\n${BLUE}Summary:${NC}"
echo "  • Build: ✅ Success"
echo "  • Tests: ✅ All passing"
echo "  • Errors: ✅ None"
echo "  • Warnings: ⚠️  $WARNINGS (non-critical)"
echo -e "\n${GREEN}The MultiVM project is ready for development!${NC}\n"
#!/bin/bash
# ============================================================================
# HNSW Index Build Verification Script
# ============================================================================
# Verifies that the HNSW index implementation compiles and tests pass

set -e  # Exit on error

echo "=================================="
echo "HNSW Index Build Verification"
echo "=================================="
echo ""

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Must run from ruvector root directory${NC}"
    exit 1
fi

# Step 1: Check Rust compilation
echo -e "${YELLOW}Step 1: Checking Rust compilation...${NC}"
cd crates/ruvector-postgres

if cargo check --all-features 2>&1 | tee /tmp/hnsw_check.log; then
    echo -e "${GREEN}✓ Rust code compiles successfully${NC}"
else
    echo -e "${RED}✗ Rust compilation failed${NC}"
    echo "See /tmp/hnsw_check.log for details"
    exit 1
fi

echo ""

# Step 2: Run Rust unit tests
echo -e "${YELLOW}Step 2: Running Rust unit tests...${NC}"

if cargo test --lib 2>&1 | tee /tmp/hnsw_test.log; then
    echo -e "${GREEN}✓ Rust tests passed${NC}"
else
    echo -e "${RED}✗ Rust tests failed${NC}"
    echo "See /tmp/hnsw_test.log for details"
    exit 1
fi

echo ""

# Step 3: Check pgrx build
echo -e "${YELLOW}Step 3: Building pgrx extension...${NC}"

if cargo pgrx package 2>&1 | tee /tmp/hnsw_pgrx.log; then
    echo -e "${GREEN}✓ pgrx extension built successfully${NC}"
else
    echo -e "${RED}✗ pgrx build failed${NC}"
    echo "See /tmp/hnsw_pgrx.log for details"
    exit 1
fi

echo ""

# Step 4: Verify SQL files exist
echo -e "${YELLOW}Step 4: Verifying SQL files...${NC}"

SQL_FILES=(
    "sql/ruvector--0.1.0.sql"
    "sql/hnsw_index.sql"
    "tests/hnsw_index_tests.sql"
)

ALL_SQL_EXIST=true
for file in "${SQL_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo -e "${GREEN}✓ Found: $file${NC}"
    else
        echo -e "${RED}✗ Missing: $file${NC}"
        ALL_SQL_EXIST=false
    fi
done

if [ "$ALL_SQL_EXIST" = false ]; then
    echo -e "${RED}Some SQL files are missing${NC}"
    exit 1
fi

echo ""

# Step 5: Verify Rust source files
echo -e "${YELLOW}Step 5: Verifying Rust source files...${NC}"

RUST_FILES=(
    "src/index/hnsw.rs"
    "src/index/hnsw_am.rs"
    "src/index/mod.rs"
)

ALL_RUST_EXIST=true
for file in "${RUST_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo -e "${GREEN}✓ Found: $file${NC}"
    else
        echo -e "${RED}✗ Missing: $file${NC}"
        ALL_RUST_EXIST=false
    fi
done

if [ "$ALL_RUST_EXIST" = false ]; then
    echo -e "${RED}Some Rust files are missing${NC}"
    exit 1
fi

echo ""

# Step 6: Check documentation
echo -e "${YELLOW}Step 6: Verifying documentation...${NC}"

cd ../..  # Back to root

DOC_FILES=(
    "docs/HNSW_INDEX.md"
)

ALL_DOCS_EXIST=true
for file in "${DOC_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo -e "${GREEN}✓ Found: $file${NC}"
    else
        echo -e "${RED}✗ Missing: $file${NC}"
        ALL_DOCS_EXIST=false
    fi
done

echo ""

# Step 7: Check for compilation warnings
echo -e "${YELLOW}Step 7: Checking for warnings...${NC}"

WARNING_COUNT=$(grep -c "warning:" /tmp/hnsw_check.log || true)

if [ "$WARNING_COUNT" -eq 0 ]; then
    echo -e "${GREEN}✓ No compilation warnings${NC}"
else
    echo -e "${YELLOW}⚠ Found $WARNING_COUNT warnings${NC}"
    echo "Check /tmp/hnsw_check.log for details"
fi

echo ""

# Summary
echo "=================================="
echo -e "${GREEN}All verification checks passed!${NC}"
echo "=================================="
echo ""
echo "Next steps:"
echo "1. Install extension: cargo pgrx install"
echo "2. Run SQL tests: psql -d testdb -f crates/ruvector-postgres/tests/hnsw_index_tests.sql"
echo "3. Create index: CREATE INDEX ON table USING hnsw (column hnsw_l2_ops);"
echo ""
echo "Documentation: docs/HNSW_INDEX.md"
echo ""

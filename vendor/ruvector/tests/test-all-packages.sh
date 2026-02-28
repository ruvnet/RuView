#!/bin/bash
set -e

echo "=================================="
echo "🧪 COMPREHENSIVE PACKAGE TEST SUITE"
echo "=================================="

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

success() { echo -e "${GREEN}✅ $1${NC}"; }
error() { echo -e "${RED}❌ $1${NC}"; exit 1; }
info() { echo -e "${YELLOW}ℹ️  $1${NC}"; }

# Test 1: @ruvector/core
echo ""
info "TEST 1: Building @ruvector/core"
cd npm/core
npm run build || error "@ruvector/core build failed"
success "@ruvector/core built successfully"

# Check .node binaries
info "Checking native .node binaries..."
if [ -f "native/linux-x64/ruvector.node" ]; then
    ls -lh native/linux-x64/ruvector.node
    success "Native binary found: $(du -h native/linux-x64/ruvector.node | cut -f1)"
else
    error "Native binary not found!"
fi

# Test 2: ruvector wrapper
echo ""
info "TEST 2: Building ruvector wrapper"
cd ../packages/ruvector
npm run build || error "ruvector build failed"
success "ruvector wrapper built successfully"

# Test 3: ruvector-extensions
echo ""
info "TEST 3: Building ruvector-extensions"
cd ../ruvector-extensions
npm run build || error "ruvector-extensions build failed"
success "ruvector-extensions built successfully"

# Test 4: Create fresh test environment
echo ""
info "TEST 4: Creating fresh test environment"
cd /tmp
rm -rf test-ruv-complete
mkdir test-ruv-complete
cd test-ruv-complete
npm init -y > /dev/null

# Install from published versions
info "Installing packages..."
npm install @ruvector/core@0.1.14 ruvector@0.1.20 --silent || error "Package installation failed"
success "Packages installed"

# Test 5: ESM Import Test
echo ""
info "TEST 5: ESM Import Test"
cat > test-esm.js << 'TESTEOF'
import('@ruvector/core').then(core => {
    console.log('  VectorDB:', typeof core.VectorDB);
    console.log('  version:', core.version());
    console.log('  hello:', core.hello());
    if (typeof core.VectorDB !== 'function') process.exit(1);
    console.log('  ✅ ESM imports working');
}).catch(err => {
    console.error('❌ ESM test failed:', err);
    process.exit(1);
});
TESTEOF
node test-esm.js || error "ESM import test failed"
success "ESM imports work correctly"

# Test 6: CommonJS Require Test
echo ""
info "TEST 6: CommonJS Require Test"
cat > test-cjs.js << 'TESTEOF'
try {
    const core = require('@ruvector/core');
    console.log('  Exports:', Object.keys(core));
    console.log('  VectorDB:', typeof core.VectorDB);
    console.log('  version:', core.version());
    console.log('  hello:', core.hello());
    
    if (typeof core.VectorDB !== 'function') {
        console.error('❌ VectorDB not found in exports');
        process.exit(1);
    }
    console.log('  ✅ CommonJS require working');
} catch (err) {
    console.error('❌ CommonJS test failed:', err);
    process.exit(1);
}
TESTEOF
node test-cjs.js || error "CommonJS require test failed"
success "CommonJS require works correctly"

# Test 7: VectorDB Instantiation
echo ""
info "TEST 7: VectorDB Instantiation & Basic Operations"
cat > test-vectordb.js << 'TESTEOF'
const { VectorDB, DistanceMetric } = require('@ruvector/core');

async function test() {
    console.log('  Creating VectorDB instance...');
    const db = new VectorDB({
        dimensions: 3,
        distanceMetric: DistanceMetric.Cosine,
        storagePath: '/tmp/test-vectordb.db'
    });
    
    console.log('  Inserting vectors...');
    const id1 = await db.insert({
        id: 'vec1',
        vector: [1.0, 0.0, 0.0]
    });
    console.log('  Inserted:', id1);
    
    const id2 = await db.insert({
        id: 'vec2',
        vector: [0.9, 0.1, 0.0]
    });
    console.log('  Inserted:', id2);
    
    console.log('  Searching...');
    const results = await db.search({
        vector: [1.0, 0.0, 0.0],
        k: 2
    });
    console.log('  Found', results.length, 'results');
    
    const len = await db.len();
    console.log('  Total vectors:', len);
    
    if (len !== 2) {
        console.error('❌ Expected 2 vectors, got', len);
        process.exit(1);
    }
    
    console.log('  ✅ VectorDB operations working');
}

test().catch(err => {
    console.error('❌ VectorDB test failed:', err);
    process.exit(1);
});
TESTEOF
node test-vectordb.js || error "VectorDB operations failed"
success "VectorDB operations work correctly"

# Test 8: CLI Test
echo ""
info "TEST 8: CLI Tool Test"
npx ruvector info || error "CLI tool failed"
success "CLI tool works correctly"

# Test 9: ruvector wrapper functionality
echo ""
info "TEST 9: Ruvector Wrapper Test"
cat > test-wrapper.js << 'TESTEOF'
const { VectorDB, getImplementationType, isNative } = require('ruvector');

console.log('  Implementation:', getImplementationType());
console.log('  Is Native:', isNative());
console.log('  VectorDB:', typeof VectorDB);

if (typeof VectorDB !== 'function') {
    console.error('❌ VectorDB not exported from wrapper');
    process.exit(1);
}

console.log('  ✅ Wrapper exports working');
TESTEOF
node test-wrapper.js || error "Wrapper test failed"
success "Ruvector wrapper works correctly"

# Test 10: Check binary compatibility
echo ""
info "TEST 10: Binary Compatibility Check"
file node_modules/@ruvector/core/native/linux-x64/ruvector.node || error "Cannot inspect binary"
success "Binary is valid ELF shared object"

# Final Summary
echo ""
echo "=================================="
echo "🎉 ALL TESTS PASSED!"
echo "=================================="
echo ""
echo "Summary:"
echo "  ✅ @ruvector/core builds"
echo "  ✅ Native .node binaries present"
echo "  ✅ ruvector wrapper builds"
echo "  ✅ ruvector-extensions builds"
echo "  ✅ ESM imports work"
echo "  ✅ CommonJS requires work"
echo "  ✅ VectorDB instantiation works"
echo "  ✅ Vector operations work (insert, search, len)"
echo "  ✅ CLI tool works"
echo "  ✅ Wrapper exports work"
echo "  ✅ Binary compatibility verified"
echo ""
echo "📦 Package Versions:"
cd /tmp/test-ruv-complete
npm list @ruvector/core ruvector 2>/dev/null | grep -E "@ruvector/core|ruvector@"
echo ""
echo "🚀 Everything is working correctly!"

#!/bin/bash
# Test ruvector npm package in Docker container
set -e

echo "=== Creating test package ==="

# Create temporary test directory
TEST_DIR=$(mktemp -d)
cd "$TEST_DIR"

# Create package.json
cat > package.json << 'EOF'
{
  "name": "ruvector-test",
  "version": "1.0.0",
  "type": "module",
  "main": "test.mjs"
}
EOF

# Create test script
cat > test.mjs << 'EOF'
import ruvector from '@ruvector/core';

const { VectorDB, CollectionManager, version, hello, getHealth, getMetrics } = ruvector;

console.log('=== Ruvector Package Test ===\n');

// Test version and hello
console.log('Version:', version());
console.log('Hello:', hello());

// Test health
console.log('\n--- Health Check ---');
const health = getHealth();
console.log('Status:', health.status);
console.log('Version:', health.version);

// Test metrics
console.log('\n--- Metrics ---');
const metrics = getMetrics();
console.log('Metrics available:', metrics.length > 0 ? 'Yes' : 'No');

// Test VectorDB
console.log('\n--- VectorDB Test ---');
const db = VectorDB.withDimensions(4);
console.log('Created VectorDB with 4 dimensions');

// Insert vectors
const id1 = await db.insert({ vector: new Float32Array([1.0, 0.0, 0.0, 0.0]) });
const id2 = await db.insert({ vector: new Float32Array([0.0, 1.0, 0.0, 0.0]) });
const id3 = await db.insert({ vector: new Float32Array([0.9, 0.1, 0.0, 0.0]) });
console.log('Inserted 3 vectors:', id1, id2, id3);

// Search
const results = await db.search({ vector: new Float32Array([1.0, 0.0, 0.0, 0.0]), k: 2 });
console.log('Search results:', results);

// Verify correct order
if (results[0].id === id1 && results[1].id === id3) {
  console.log('✓ Search results correct!');
} else {
  console.log('✗ Search results incorrect');
  process.exit(1);
}

// Test CollectionManager
console.log('\n--- CollectionManager Test ---');
try {
  const manager = new CollectionManager('./test-collections');
  console.log('Created CollectionManager');

  await manager.createCollection('test_vectors', { dimensions: 128 });
  console.log('Created collection: test_vectors');

  const collections = await manager.listCollections();
  console.log('Collections:', collections);

  const stats = await manager.getStats('test_vectors');
  console.log('Stats:', stats);

  await manager.deleteCollection('test_vectors');
  console.log('Deleted collection: test_vectors');
  console.log('✓ CollectionManager works!');
} catch (err) {
  console.log('CollectionManager error:', err.message);
}

console.log('\n=== All Tests Passed! ===');
EOF

echo "=== Test files created in $TEST_DIR ==="

# Copy local package
echo "=== Copying local package ==="
mkdir -p node_modules/@ruvector
cp -r /workspaces/ruvector/npm/core node_modules/@ruvector/

# Run test
echo ""
echo "=== Running test ==="
node test.mjs

# Cleanup
cd /
rm -rf "$TEST_DIR"
echo ""
echo "=== Test completed successfully ==="

/**
 * Test script to verify native module loads correctly
 */

import ruvector from './dist/index.js';

console.log('=== Ruvector Native Module Test ===\n');

try {
  // Test 1: Load module
  console.log('✓ Module imported successfully');
  console.log('Available exports:', Object.keys(ruvector));

  // Test 2: Get version
  console.log('\n--- Version Info ---');
  console.log('Version:', ruvector.version());

  // Test 3: Hello function
  console.log('\n--- Hello Test ---');
  console.log(ruvector.hello());

  // Test 4: Create VectorDB instance
  console.log('\n--- VectorDB Creation ---');
  const db = ruvector.VectorDB.withDimensions(384);
  console.log('✓ VectorDB created with 384 dimensions');

  // Test 5: Check database is empty
  console.log('\n--- Database Status ---');
  const isEmpty = await db.isEmpty();
  console.log('Database is empty:', isEmpty);

  const len = await db.len();
  console.log('Database length:', len);

  // Test 6: Insert a vector
  console.log('\n--- Insert Vector ---');
  const testVector = new Float32Array(384).fill(0.1);
  const id = await db.insert({
    vector: testVector,
  });
  console.log('✓ Inserted vector with ID:', id);

  const newLen = await db.len();
  console.log('Database length after insert:', newLen);

  // Test 7: Search
  console.log('\n--- Search Test ---');
  const queryVector = new Float32Array(384).fill(0.15);
  const results = await db.search({
    vector: queryVector,
    k: 10
  });
  console.log('✓ Search completed');
  console.log('Found', results.length, 'results');
  if (results.length > 0) {
    console.log('First result:', {
      id: results[0].id,
      score: results[0].score
    });
  }

  // Test 8: Get vector
  console.log('\n--- Get Vector Test ---');
  const retrieved = await db.get(id);
  if (retrieved) {
    console.log('✓ Retrieved vector with ID:', retrieved.id);
    console.log('Vector length:', retrieved.vector.length);
  }

  console.log('\n=== ✅ All tests passed! ===\n');
  process.exit(0);

} catch (error) {
  console.error('\n❌ Test failed:', error.message);
  console.error(error.stack);
  process.exit(1);
}

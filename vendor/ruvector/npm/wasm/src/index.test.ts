/**
 * Tests for @ruvector/wasm
 */

import { VectorDB, detectSIMD, version } from './node';

async function testBasicOperations() {
  console.log('Testing basic VectorDB operations...');

  // Create database
  const db = new VectorDB({ dimensions: 3 });
  await db.init();

  // Test insert
  const vector1 = new Float32Array([1.0, 0.0, 0.0]);
  const id1 = db.insert(vector1, 'vec1', { label: 'test1' });
  console.log('✓ Insert single vector:', id1);

  // Test batch insert
  const entries = [
    { vector: [0.0, 1.0, 0.0], id: 'vec2', metadata: { label: 'test2' } },
    { vector: [0.0, 0.0, 1.0], id: 'vec3', metadata: { label: 'test3' } },
  ];
  const ids = db.insertBatch(entries);
  console.log('✓ Batch insert:', ids);

  // Test len
  const count = db.len();
  console.log('✓ Vector count:', count);
  if (count !== 3) throw new Error('Expected 3 vectors');

  // Test search
  const query = new Float32Array([1.0, 0.1, 0.0]);
  const results = db.search(query, 2);
  console.log('✓ Search results:', results.length);
  if (results.length !== 2) throw new Error('Expected 2 results');

  // Test get
  const entry = db.get('vec1');
  console.log('✓ Get by ID:', entry?.id);
  if (!entry || entry.id !== 'vec1') throw new Error('Expected vec1');

  // Test delete
  const deleted = db.delete('vec1');
  console.log('✓ Delete:', deleted);
  if (!deleted) throw new Error('Expected delete to succeed');

  // Test isEmpty
  const isEmpty = db.isEmpty();
  console.log('✓ Is empty:', isEmpty);
  if (isEmpty) throw new Error('Expected database to not be empty');

  // Test getDimensions
  const dims = db.getDimensions();
  console.log('✓ Dimensions:', dims);
  if (dims !== 3) throw new Error('Expected 3 dimensions');

  console.log('✓ All basic operations passed!\n');
}

async function testUtilities() {
  console.log('Testing utility functions...');

  // Test version
  const ver = await version();
  console.log('✓ Version:', ver);

  // Test SIMD detection
  const hasSIMD = await detectSIMD();
  console.log('✓ SIMD support:', hasSIMD);

  console.log('✓ All utility tests passed!\n');
}

async function testErrorHandling() {
  console.log('Testing error handling...');

  try {
    const db = new VectorDB({ dimensions: 3 });
    // Should throw error if not initialized
    db.insert(new Float32Array([1, 2, 3]));
    throw new Error('Expected error when using uninitialized database');
  } catch (err: any) {
    if (err.message.includes('not initialized')) {
      console.log('✓ Uninitialized database error');
    } else {
      throw err;
    }
  }

  try {
    const db = new VectorDB({ dimensions: 3 });
    await db.init();
    // Should handle dimension mismatch
    const wrongVector = new Float32Array([1, 2, 3, 4, 5]);
    db.search(wrongVector, 5);
    throw new Error('Expected dimension mismatch error');
  } catch (err: any) {
    if (err.message.includes('dimension')) {
      console.log('✓ Dimension mismatch error');
    } else {
      throw err;
    }
  }

  console.log('✓ All error handling tests passed!\n');
}

async function runAllTests() {
  console.log('Starting @ruvector/wasm tests...\n');

  try {
    await testUtilities();
    await testBasicOperations();
    await testErrorHandling();

    console.log('✅ ALL TESTS PASSED!');
    process.exit(0);
  } catch (error) {
    console.error('❌ TEST FAILED:', error);
    process.exit(1);
  }
}

runAllTests();

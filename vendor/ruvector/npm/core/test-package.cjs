#!/usr/bin/env node
/**
 * Test script for @ruvector/core-linux-x64-gnu package
 * Verifies that the native binary loads correctly
 */

const fs = require('fs');
const path = require('path');

console.log('🧪 Testing @ruvector/core-linux-x64-gnu package...\n');

// Test 1: Check files exist
console.log('📁 Test 1: Checking file structure...');
const platformDir = path.join(__dirname, 'platforms/linux-x64-gnu');
const requiredFiles = [
  'index.js',
  'ruvector.node',
  'package.json',
  'README.md'
];

let filesOk = true;
for (const file of requiredFiles) {
  const filePath = path.join(platformDir, file);
  if (fs.existsSync(filePath)) {
    const stats = fs.statSync(filePath);
    const size = stats.size > 1024 * 1024
      ? `${(stats.size / (1024 * 1024)).toFixed(2)} MB`
      : `${(stats.size / 1024).toFixed(2)} KB`;
    console.log(`  ✅ ${file} (${size})`);
  } else {
    console.log(`  ❌ ${file} - MISSING`);
    filesOk = false;
  }
}

if (!filesOk) {
  console.error('\n❌ File structure test FAILED');
  process.exit(1);
}

console.log('\n✅ File structure test PASSED\n');

// Test 2: Load native module
console.log('📦 Test 2: Loading native module...');
try {
  const nativeModule = require(path.join(platformDir, 'index.js'));
  console.log('  ✅ Native module loaded successfully');
  console.log('  ℹ️  Module exports:', Object.keys(nativeModule).join(', '));
  console.log('\n✅ Native module test PASSED\n');
} catch (error) {
  console.error('  ❌ Failed to load native module:', error.message);
  console.error('\n❌ Native module test FAILED');
  process.exit(1);
}

// Test 3: Create database instance
console.log('🗄️  Test 3: Creating database instance...');
try {
  const { VectorDb } = require(path.join(platformDir, 'index.js'));

  const db = new VectorDb({
    dimensions: 128,
    maxElements: 1000,
    storagePath: `/tmp/ruvector-test-${Date.now()}-1.db`
  });

  console.log('  ✅ Database instance created successfully');
  console.log('\n✅ Database creation test PASSED\n');
} catch (error) {
  console.error('  ❌ Failed to create database:', error.message);
  console.error('\n❌ Database creation test FAILED');
  process.exit(1);
}

// Test 4: Basic operations
console.log('🔧 Test 4: Testing basic operations...');
(async () => {
  try {
    const { VectorDb } = require(path.join(platformDir, 'index.js'));

    const db = new VectorDb({
      dimensions: 3,
      maxElements: 100,
      storagePath: `/tmp/ruvector-test-${Date.now()}-2.db`
    });

    // Insert vector
    const vector = new Float32Array([0.1, 0.2, 0.3]);
    const id = await db.insert({
      id: 'test_vector',
      vector: vector
    });
    console.log(`  ✅ Inserted vector with ID: ${id}`);

    // Count vectors
    const count = await db.len();
    console.log(`  ✅ Vector count: ${count}`);

    // Search
    const queryVector = new Float32Array([0.1, 0.2, 0.3]);
    const results = await db.search({
      vector: queryVector,
      k: 1
    });
    console.log(`  ✅ Search returned ${results.length} result(s)`);
    if (results.length > 0) {
      console.log(`    - ID: ${results[0].id}, Score: ${results[0].score.toFixed(6)}`);
    }

    // Delete
    const deleted = await db.delete('test_vector');
    console.log(`  ✅ Deleted vector: ${deleted}`);

    console.log('\n✅ Basic operations test PASSED\n');
    console.log('🎉 All tests PASSED!\n');
    console.log('Package is ready for publishing.');
  } catch (error) {
    console.error('  ❌ Basic operations failed:', error.message);
    console.error(error.stack);
    console.error('\n❌ Basic operations test FAILED');
    process.exit(1);
  }
})();

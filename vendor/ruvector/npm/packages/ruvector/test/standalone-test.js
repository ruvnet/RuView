#!/usr/bin/env node

/**
 * Standalone test using mock implementation
 * This demonstrates the package structure and API without requiring native/WASM modules
 */

const assert = require('assert');
const path = require('path');
const fs = require('fs');

console.log('ruvector Standalone Test (with mock implementation)\n');
console.log('='.repeat(60));

// Test 1: Package structure
console.log('\n1. Testing package structure...');
try {
  const packageJson = require('../package.json');
  assert(packageJson.name === 'ruvector', 'Package name should be ruvector');
  assert(packageJson.version === '0.1.1', 'Version should be 0.1.1');
  assert(packageJson.main === 'dist/index.js', 'Main entry correct');
  assert(packageJson.types === 'dist/index.d.ts', 'Types entry correct');
  console.log('   ✓ package.json structure valid');

  const distExists = fs.existsSync(path.join(__dirname, '../dist'));
  assert(distExists, 'dist directory should exist');
  console.log('   ✓ dist directory exists');

  const indexExists = fs.existsSync(path.join(__dirname, '../dist/index.js'));
  assert(indexExists, 'dist/index.js should exist');
  console.log('   ✓ dist/index.js compiled');

  const typesExist = fs.existsSync(path.join(__dirname, '../dist/types.d.ts'));
  assert(typesExist, 'Type definitions should exist');
  console.log('   ✓ TypeScript definitions compiled');

  const cliExists = fs.existsSync(path.join(__dirname, '../bin/cli.js'));
  assert(cliExists, 'CLI script should exist');
  console.log('   ✓ CLI script exists');
} catch (error) {
  console.error('   ✗ Package structure test failed:', error.message);
  process.exit(1);
}

// Test 2: Type definitions
console.log('\n2. Testing TypeScript type definitions...');
try {
  const typeDefs = fs.readFileSync(path.join(__dirname, '../dist/types.d.ts'), 'utf8');

  const requiredTypes = [
    'VectorEntry',
    'SearchQuery',
    'SearchResult',
    'DbOptions',
    'DbStats',
    'VectorDB'
  ];

  for (const type of requiredTypes) {
    assert(typeDefs.includes(type), `Should include ${type}`);
    console.log(`   ✓ ${type} interface defined`);
  }

  const indexDefs = fs.readFileSync(path.join(__dirname, '../dist/index.d.ts'), 'utf8');
  // Check for type re-exports (TypeScript may compile to different formats)
  const hasTypeExports = indexDefs.includes('VectorEntry') ||
                         indexDefs.includes('from "./types"') ||
                         indexDefs.includes('export *');
  assert(hasTypeExports, 'Should export types');
  assert(indexDefs.includes('getImplementationType'), 'Should export getImplementationType');
  assert(indexDefs.includes('VectorDB'), 'Should export VectorDB');
  console.log('   ✓ Index exports all types and functions');
} catch (error) {
  console.error('   ✗ Type definitions test failed:', error.message);
  process.exit(1);
}

// Test 3: Mock VectorDB functionality
console.log('\n3. Testing VectorDB API (with mock)...');
try {
  const { VectorDB } = require('./mock-implementation.js');

  // Create database
  const db = new VectorDB({
    dimension: 3,
    metric: 'cosine'
  });
  console.log('   ✓ Database created');

  // Insert vectors
  db.insert({
    id: 'vec1',
    vector: [1, 0, 0],
    metadata: { label: 'first' }
  });

  db.insertBatch([
    { id: 'vec2', vector: [0, 1, 0], metadata: { label: 'second' } },
    { id: 'vec3', vector: [0, 0, 1], metadata: { label: 'third' } },
    { id: 'vec4', vector: [0.7, 0.7, 0], metadata: { label: 'fourth' } }
  ]);
  console.log('   ✓ Vectors inserted');

  // Get stats
  const stats = db.stats();
  assert(stats.count === 4, 'Should have 4 vectors');
  assert(stats.dimension === 3, 'Dimension should be 3');
  console.log(`   ✓ Stats: ${stats.count} vectors, dim=${stats.dimension}`);

  // Search
  const results = db.search({
    vector: [1, 0, 0],
    k: 3
  });
  assert(results.length === 3, 'Should return 3 results');
  assert(results[0].id === 'vec1', 'First result should be vec1');
  console.log(`   ✓ Search returned ${results.length} results`);
  console.log(`     Top result: ${results[0].id} (score: ${results[0].score.toFixed(4)})`);

  // Get by ID
  const vec = db.get('vec2');
  assert(vec !== null, 'Should find vector');
  assert(vec.id === 'vec2', 'Should have correct ID');
  console.log('   ✓ Get by ID works');

  // Update metadata
  db.updateMetadata('vec1', { updated: true });
  const updated = db.get('vec1');
  assert(updated.metadata.updated === true, 'Metadata should be updated');
  console.log('   ✓ Update metadata works');

  // Delete
  const deleted = db.delete('vec3');
  assert(deleted === true, 'Should delete successfully');
  assert(db.stats().count === 3, 'Should have 3 vectors after delete');
  console.log('   ✓ Delete works');

} catch (error) {
  console.error('   ✗ VectorDB API test failed:', error.message);
  process.exit(1);
}

// Test 4: CLI structure
console.log('\n4. Testing CLI structure...');
try {
  const cliContent = fs.readFileSync(path.join(__dirname, '../bin/cli.js'), 'utf8');

  const cliFeatures = [
    'create',
    'insert',
    'search',
    'stats',
    'benchmark',
    'info'
  ];

  for (const feature of cliFeatures) {
    assert(cliContent.includes(feature), `CLI should include ${feature} command`);
    console.log(`   ✓ ${feature} command present`);
  }

  assert(cliContent.includes('#!/usr/bin/env node'), 'Should have shebang');
  assert(cliContent.includes('commander'), 'Should use commander');
  assert(cliContent.includes('chalk'), 'Should use chalk');
  assert(cliContent.includes('ora'), 'Should use ora');
  console.log('   ✓ CLI dependencies correct');

} catch (error) {
  console.error('   ✗ CLI structure test failed:', error.message);
  process.exit(1);
}

// Test 5: Smart loader logic
console.log('\n5. Testing smart loader logic...');
try {
  const loaderContent = fs.readFileSync(path.join(__dirname, '../dist/index.js'), 'utf8');

  assert(loaderContent.includes('@ruvector/core'), 'Should try to load native');
  assert(loaderContent.includes('@ruvector/wasm'), 'Should fallback to WASM');
  assert(loaderContent.includes('getImplementationType'), 'Should export implementation type');
  assert(loaderContent.includes('isNative'), 'Should export isNative');
  assert(loaderContent.includes('isWasm'), 'Should export isWasm');
  console.log('   ✓ Smart loader has platform detection');
  console.log('   ✓ Exports implementation detection functions');

} catch (error) {
  console.error('   ✗ Smart loader test failed:', error.message);
  process.exit(1);
}

// Summary
console.log('\n' + '='.repeat(60));
console.log('\n✓ All package structure tests passed!');
console.log('\nPackage features:');
console.log('  ✓ Smart native/WASM loader with automatic fallback');
console.log('  ✓ Complete TypeScript type definitions');
console.log('  ✓ VectorDB API (insert, search, delete, stats)');
console.log('  ✓ CLI tools (create, insert, search, stats, benchmark, info)');
console.log('  ✓ Platform detection (isNative, isWasm, getImplementationType)');
console.log('\nPackage structure:');
console.log('  📦 /workspaces/ruvector/npm/packages/ruvector');
console.log('    ├── dist/          (compiled JavaScript and types)');
console.log('    ├── src/           (TypeScript source)');
console.log('    ├── bin/           (CLI script)');
console.log('    ├── test/          (integration tests)');
console.log('    └── package.json   (npm package config)');
console.log('\nReady for integration with:');
console.log('  - @ruvector/core  (native Rust bindings)');
console.log('  - @ruvector/wasm  (WebAssembly module)');
console.log('\nNext steps:');
console.log('  1. Create @ruvector/core package (native bindings)');
console.log('  2. Create @ruvector/wasm package (WASM module)');
console.log('  3. Update package.json to include them as dependencies');
console.log('  4. Test full integration');

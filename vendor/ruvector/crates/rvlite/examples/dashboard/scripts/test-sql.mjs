#!/usr/bin/env node
/**
 * SQL Test Script - Test CREATE TABLE and SQL operations in WASM
 */

import { readFile } from 'fs/promises';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

async function main() {
  console.log('╔════════════════════════════════════════════════════════════╗');
  console.log('║     RvLite WASM SQL Test Suite                             ║');
  console.log('╚════════════════════════════════════════════════════════════╝\n');

  // Load WASM
  const wasmPath = join(__dirname, '../public/pkg/rvlite_bg.wasm');
  const wasmBytes = await readFile(wasmPath);

  const { default: initRvLite, RvLite, RvLiteConfig } = await import('../public/pkg/rvlite.js');
  await initRvLite(wasmBytes);
  console.log('WASM loaded successfully!\n');

  const config = new RvLiteConfig(384);
  const db = new RvLite(config);

  const tests = [];

  function test(name, fn) {
    try {
      fn();
      tests.push({ name, passed: true });
      console.log(`  \x1b[32m✓\x1b[0m ${name}`);
    } catch (error) {
      tests.push({ name, passed: false, error: error.message });
      console.log(`  \x1b[31m✗\x1b[0m ${name}`);
      console.log(`    Error: ${error.message}`);
    }
  }

  console.log('═══════════════════════════════════════════════════════════════');
  console.log('SQL Parser Tests');
  console.log('═══════════════════════════════════════════════════════════════');

  // Test 1: CREATE TABLE with VECTOR column (3-dimensional for test)
  test('CREATE TABLE with VECTOR', () => {
    const result = db.sql("CREATE TABLE docs (id TEXT, content TEXT, embedding VECTOR(3))");
    console.log('    Result:', JSON.stringify(result));
  });

  // Test 2: INSERT with correct vector dimensions
  test('INSERT INTO table', () => {
    const result = db.sql("INSERT INTO docs (id, content, embedding) VALUES ('doc1', 'hello world', [1.0, 2.0, 3.0])");
    console.log('    Result:', JSON.stringify(result));
  });

  // Test 3: INSERT another vector
  test('INSERT second vector', () => {
    const result = db.sql("INSERT INTO docs (id, content, embedding) VALUES ('doc2', 'test content', [4.0, 5.0, 6.0])");
    console.log('    Result:', JSON.stringify(result));
  });

  // Test 4: Vector search with L2 distance
  test('Vector search with L2 distance', () => {
    const result = db.sql("SELECT * FROM docs ORDER BY embedding <-> [1.0, 2.0, 3.0] LIMIT 5");
    console.log('    Result:', JSON.stringify(result));
  });

  // Test 5: Vector search with cosine distance
  test('Vector search with cosine distance', () => {
    const result = db.sql("SELECT * FROM docs ORDER BY embedding <=> [0.5, 0.5, 0.5] LIMIT 5");
    console.log('    Result:', JSON.stringify(result));
  });

  // Test 6: Vector search with filter
  test('Vector search with filter', () => {
    const result = db.sql("SELECT * FROM docs WHERE id = 'doc1' ORDER BY embedding <-> [1.0, 2.0, 3.0] LIMIT 5");
    console.log('    Result:', JSON.stringify(result));
  });

  // Test 7: DROP TABLE
  test('DROP TABLE', () => {
    const result = db.sql("DROP TABLE docs");
    console.log('    Result:', JSON.stringify(result));
  });

  // Cleanup
  db.free();

  // Summary
  console.log('\n╔════════════════════════════════════════════════════════════╗');
  console.log('║                    TEST SUMMARY                             ║');
  console.log('╚════════════════════════════════════════════════════════════╝\n');

  const passed = tests.filter(t => t.passed).length;
  const failed = tests.filter(t => !t.passed).length;

  console.log(`  Total:  ${tests.length} tests`);
  console.log(`  \x1b[32mPassed: ${passed}\x1b[0m`);
  console.log(`  \x1b[31mFailed: ${failed}\x1b[0m`);

  if (failed > 0) {
    console.log('\n  Failed tests:');
    tests.filter(t => !t.passed).forEach(t => {
      console.log(`    - ${t.name}: ${t.error}`);
    });
    process.exit(1);
  }
}

main().catch(console.error);

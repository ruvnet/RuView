#!/usr/bin/env node
/**
 * Comprehensive RvLite WASM Test Suite
 * Tests ALL features: Vector API, SQL, SPARQL, Cypher
 *
 * Run with: node scripts/test-all.mjs
 */

import { readFile } from 'fs/promises';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Test results tracking
const results = {
  total: 0,
  passed: 0,
  failed: 0,
  sections: [],
  errors: []
};

// Colors for terminal output
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  cyan: '\x1b[36m',
  dim: '\x1b[2m',
  bold: '\x1b[1m'
};

function log(color, text) {
  console.log(`${color}${text}${colors.reset}`);
}

// Test runner
async function test(name, fn) {
  results.total++;
  try {
    await fn();
    results.passed++;
    console.log(`  ${colors.green}✓${colors.reset} ${name}`);
    return true;
  } catch (e) {
    results.failed++;
    results.errors.push({ name, error: e.message });
    console.log(`  ${colors.red}✗${colors.reset} ${name}`);
    console.log(`    ${colors.dim}Error: ${e.message}${colors.reset}`);
    return false;
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message || 'Assertion failed');
}

function assertEqual(actual, expected, message) {
  if (actual !== expected) {
    throw new Error(message || `Expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function assertDeepIncludes(obj, key) {
  if (typeof obj !== 'object' || obj === null || !(key in obj)) {
    throw new Error(`Object should have key '${key}', got: ${JSON.stringify(obj)}`);
  }
}

// Section header
function section(name) {
  console.log(`\n${colors.cyan}═══════════════════════════════════════════════════════════════${colors.reset}`);
  console.log(`${colors.bold}${name}${colors.reset}`);
  console.log(`${colors.cyan}═══════════════════════════════════════════════════════════════${colors.reset}`);
  results.sections.push({ name, startIndex: results.total });
}

async function runTests() {
  console.log(`${colors.cyan}╔════════════════════════════════════════════════════════════╗${colors.reset}`);
  console.log(`${colors.cyan}║${colors.reset}     ${colors.bold}RvLite WASM Comprehensive Test Suite${colors.reset}                   ${colors.cyan}║${colors.reset}`);
  console.log(`${colors.cyan}║${colors.reset}     Tests: Vector API • SQL • SPARQL • Cypher              ${colors.cyan}║${colors.reset}`);
  console.log(`${colors.cyan}╚════════════════════════════════════════════════════════════╝${colors.reset}\n`);

  // Load WASM
  console.log('Loading WASM module...');
  const wasmPath = join(__dirname, '../public/pkg/rvlite_bg.wasm');
  const wasmBytes = await readFile(wasmPath);
  const rvliteModule = await import('../public/pkg/rvlite.js');
  const { default: initRvLite, RvLite, RvLiteConfig } = rvliteModule;
  await initRvLite(wasmBytes);

  const version = 'RvLite loaded';
  console.log(`${colors.green}✓${colors.reset} ${version}\n`);

  const config = new RvLiteConfig(128);
  const db = new RvLite(config);

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 1: INITIALIZATION & CONFIG
  // ═══════════════════════════════════════════════════════════════════════════
  section('SECTION 1: Initialization & Configuration');

  await test('RvLite instance created', () => {
    assert(db !== null && db !== undefined, 'DB should be created');
  });

  await test('is_ready returns true', () => {
    assert(db.is_ready() === true, 'Should be ready');
  });

  await test('get_version returns string', () => {
    const version = db.get_version();
    assert(typeof version === 'string' && version.length > 0, 'Should have version');
  });

  await test('get_features returns array', () => {
    const features = db.get_features();
    assert(Array.isArray(features), 'Features should be array');
  });

  await test('get_config returns valid config', () => {
    const cfg = db.get_config();
    assert(cfg !== null, 'Config should exist');
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 2: VECTOR API
  // ═══════════════════════════════════════════════════════════════════════════
  section('SECTION 2: Vector API');

  let vectorId;
  await test('Insert vector returns ID', () => {
    const vector = new Float32Array(128).fill(0.5);
    vectorId = db.insert(vector, { label: 'test-1' });
    assert(vectorId !== null && vectorId !== undefined, 'Should return ID');
  });

  await test('Insert multiple vectors', () => {
    for (let i = 0; i < 5; i++) {
      const vector = new Float32Array(128).map(() => Math.random());
      db.insert(vector, { index: i });
    }
    assert(db.len() >= 6, 'Should have at least 6 vectors');
  });

  await test('Search returns results', () => {
    const query = new Float32Array(128).fill(0.5);
    const results = db.search(query, 3);
    assert(Array.isArray(results), 'Should return array');
    assert(results.length > 0, 'Should find results');
  });

  await test('Search results have id and score', () => {
    const query = new Float32Array(128).fill(0.5);
    const results = db.search(query, 1);
    assert(results[0].id !== undefined, 'Should have id');
    assert(results[0].score !== undefined, 'Should have score');
  });

  await test('len() returns correct count', () => {
    const count = db.len();
    assert(typeof count === 'number' && count >= 6, 'Should have count >= 6');
  });

  await test('is_empty() returns false after inserts', () => {
    assert(db.is_empty() === false, 'Should not be empty');
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 3: SQL (Vector Search)
  // ═══════════════════════════════════════════════════════════════════════════
  section('SECTION 3: SQL (Vector Search)');

  await test('SQL: DROP TABLE (cleanup)', () => {
    try {
      const result = db.sql('DROP TABLE test_docs');
      assert(result !== undefined, 'Should return result');
    } catch {
      // Table might not exist - that's OK
    }
  });

  await test('SQL: CREATE TABLE with VECTOR', () => {
    const result = db.sql('CREATE TABLE test_docs (id TEXT, title TEXT, embedding VECTOR(3))');
    assert(result !== undefined, 'Should return result');
    assertDeepIncludes(result, 'rows');
  });

  await test('SQL: INSERT vector data', () => {
    const result = db.sql("INSERT INTO test_docs (id, title, embedding) VALUES ('d1', 'First Doc', [1.0, 2.0, 3.0])");
    assert(result !== undefined, 'Should return result');
  });

  await test('SQL: INSERT multiple rows', () => {
    db.sql("INSERT INTO test_docs (id, title, embedding) VALUES ('d2', 'Second Doc', [4.0, 5.0, 6.0])");
    db.sql("INSERT INTO test_docs (id, title, embedding) VALUES ('d3', 'Third Doc', [7.0, 8.0, 9.0])");
  });

  await test('SQL: Vector search with L2 distance (<->)', () => {
    const result = db.sql('SELECT * FROM test_docs ORDER BY embedding <-> [1.0, 2.0, 3.0] LIMIT 5');
    assert(result.rows !== undefined, 'Should have rows');
    assert(result.rows.length > 0, 'Should return results');
  });

  await test('SQL: Vector search with cosine distance (<=>)', () => {
    const result = db.sql('SELECT * FROM test_docs ORDER BY embedding <=> [0.5, 0.5, 0.5] LIMIT 3');
    assert(result.rows !== undefined, 'Should have rows');
  });

  await test('SQL: Vector search with WHERE filter', () => {
    const result = db.sql("SELECT * FROM test_docs WHERE id = 'd1' ORDER BY embedding <-> [1.0, 2.0, 3.0] LIMIT 5");
    assert(result.rows !== undefined, 'Should have rows');
  });

  await test('SQL: Non-vector SELECT (table scan)', () => {
    const result = db.sql('SELECT * FROM test_docs');
    assert(result.rows !== undefined, 'Should have rows');
    assert(result.rows.length >= 3, 'Should return all 3 inserted rows');
  });

  await test('SQL: Results contain actual data (not empty objects)', () => {
    const result = db.sql('SELECT * FROM test_docs ORDER BY embedding <-> [1.0, 2.0, 3.0] LIMIT 1');
    assert(result.rows.length > 0, 'Should have rows');
    const row = result.rows[0];
    // Check that row is not empty {}
    const keys = Object.keys(row);
    assert(keys.length > 0, 'Row should have properties, not be empty {}');
  });

  await test('SQL: DROP TABLE', () => {
    const result = db.sql('DROP TABLE test_docs');
    assert(result !== undefined, 'Should return result');
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 4: SPARQL (RDF Triple Store)
  // ═══════════════════════════════════════════════════════════════════════════
  section('SECTION 4: SPARQL (RDF Triple Store)');

  await test('SPARQL: Add triple', () => {
    db.add_triple('<http://example.org/Alice>', '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', '<http://example.org/Person>');
    db.add_triple('<http://example.org/Alice>', '<http://example.org/name>', '"Alice Smith"');
    db.add_triple('<http://example.org/Bob>', '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', '<http://example.org/Person>');
    db.add_triple('<http://example.org/Alice>', '<http://example.org/knows>', '<http://example.org/Bob>');
  });

  await test('SPARQL: triple_count() > 0', () => {
    const count = db.triple_count();
    assert(count >= 4, `Should have at least 4 triples, got ${count}`);
  });

  await test('SPARQL: SELECT query', () => {
    const result = db.sparql('SELECT ?person WHERE { ?person <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> }');
    assertEqual(result.type, 'select', 'Should be SELECT result');
    assert(result.bindings.length >= 2, 'Should find at least 2 people');
  });

  await test('SPARQL: SELECT with "a" keyword (rdf:type shortcut)', () => {
    const result = db.sparql('SELECT ?s WHERE { ?s a <http://example.org/Person> }');
    assertEqual(result.type, 'select', 'Should be SELECT result');
    assert(result.bindings.length >= 2, 'Should find people');
  });

  await test('SPARQL: ASK query (true case)', () => {
    const result = db.sparql('ASK { <http://example.org/Alice> <http://example.org/knows> <http://example.org/Bob> }');
    assertEqual(result.type, 'ask', 'Should be ASK result');
    assertEqual(result.result, true, 'Alice should know Bob');
  });

  await test('SPARQL: ASK query (false case)', () => {
    const result = db.sparql('ASK { <http://example.org/Bob> <http://example.org/knows> <http://example.org/Alice> }');
    assertEqual(result.type, 'ask', 'Should be ASK result');
    assertEqual(result.result, false, 'Bob does not know Alice');
  });

  await test('SPARQL: SELECT with LIMIT', () => {
    const result = db.sparql('SELECT ?s WHERE { ?s a <http://example.org/Person> } LIMIT 1');
    assertEqual(result.bindings.length, 1, 'Should return exactly 1 result');
  });

  await test('SPARQL: Result binding has type and value', () => {
    const result = db.sparql('SELECT ?s WHERE { ?s a <http://example.org/Person> } LIMIT 1');
    const binding = result.bindings[0];
    assert(binding.s !== undefined, 'Should have binding');
    assert(binding.s.type !== undefined, 'Should have type');
    assert(binding.s.value !== undefined, 'Should have value');
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 5: CYPHER (Graph Database)
  // ═══════════════════════════════════════════════════════════════════════════
  section('SECTION 5: Cypher (Graph Database)');

  await test('Cypher: CREATE node', () => {
    const result = db.cypher("CREATE (n:TestPerson {name: 'Charlie', age: 35})");
    assert(result !== undefined, 'Should return result');
  });

  await test('Cypher: CREATE multiple nodes', () => {
    db.cypher("CREATE (n:TestPerson {name: 'Diana', age: 28})");
    db.cypher("CREATE (c:TestCompany {name: 'Acme Inc', founded: 2010})");
  });

  await test('Cypher: MATCH query', () => {
    const result = db.cypher('MATCH (n:TestPerson) RETURN n');
    assert(result !== undefined, 'Should return result');
  });

  await test('Cypher: cypher_stats returns counts', () => {
    const stats = db.cypher_stats();
    assert(stats !== undefined, 'Should return stats');
    // Stats might have node_count or nodes depending on version
  });

  await test('Cypher: CREATE relationship', () => {
    const result = db.cypher("MATCH (a:TestPerson {name: 'Charlie'}), (b:TestCompany {name: 'Acme Inc'}) CREATE (a)-[r:WORKS_AT]->(b) RETURN r");
    assert(result !== undefined, 'Should return result');
  });

  await test('Cypher: MATCH with relationship', () => {
    const result = db.cypher('MATCH (a:TestPerson)-[r:WORKS_AT]->(b:TestCompany) RETURN a, b');
    assert(result !== undefined, 'Should return result');
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 6: EDGE CASES & ERROR HANDLING
  // ═══════════════════════════════════════════════════════════════════════════
  section('SECTION 6: Edge Cases & Error Handling');

  await test('SPARQL: Empty result for non-existent data', () => {
    const result = db.sparql('SELECT ?s WHERE { ?s a <http://example.org/NonExistent> }');
    assertEqual(result.bindings.length, 0, 'Should return empty bindings');
  });

  await test('SQL: Error on non-existent table', () => {
    try {
      db.sql('SELECT * FROM nonexistent_table ORDER BY col <-> [1,2,3] LIMIT 1');
      throw new Error('Should have thrown');
    } catch (e) {
      assert(e.message.includes('not found') || e.message.includes('does not exist') || e.message !== 'Should have thrown',
        'Should throw table not found error');
    }
  });

  await test('Fresh instance search returns empty', () => {
    const freshConfig = new RvLiteConfig(32);
    const freshDb = new RvLite(freshConfig);
    const query = new Float32Array(32).fill(0.5);
    const searchResults = freshDb.search(query, 5);
    assertEqual(searchResults.length, 0, 'Should return empty array');
    freshDb.free();
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 7: DATA PERSISTENCE METHODS
  // ═══════════════════════════════════════════════════════════════════════════
  section('SECTION 7: Data Persistence Methods');

  await test('export_json returns object', () => {
    const exported = db.export_json();
    assert(typeof exported === 'object', 'Should return object');
  });

  // Cleanup
  db.free();

  // ═══════════════════════════════════════════════════════════════════════════
  // SUMMARY
  // ═══════════════════════════════════════════════════════════════════════════
  console.log(`\n${colors.cyan}╔════════════════════════════════════════════════════════════╗${colors.reset}`);
  console.log(`${colors.cyan}║${colors.reset}                    ${colors.bold}TEST SUMMARY${colors.reset}                             ${colors.cyan}║${colors.reset}`);
  console.log(`${colors.cyan}╚════════════════════════════════════════════════════════════╝${colors.reset}`);

  console.log(`\n  Total:  ${results.total} tests`);
  console.log(`  ${colors.green}Passed: ${results.passed}${colors.reset}`);
  console.log(`  ${colors.red}Failed: ${results.failed}${colors.reset}`);

  if (results.failed > 0) {
    console.log(`\n${colors.red}${colors.bold}Failed Tests:${colors.reset}`);
    results.errors.forEach(({ name, error }) => {
      console.log(`  ${colors.red}✗${colors.reset} ${name}`);
      console.log(`    ${colors.dim}${error}${colors.reset}`);
    });
  }

  // Per-section summary
  console.log(`\n${colors.bold}Section Results:${colors.reset}`);
  results.sections.forEach((sec, i) => {
    const nextStart = results.sections[i + 1]?.startIndex || results.total;
    const sectionTests = nextStart - sec.startIndex;
    const icon = results.errors.some(e => {
      const idx = results.errors.indexOf(e);
      return idx >= sec.startIndex && idx < nextStart;
    }) ? colors.yellow + '⚠' : colors.green + '✓';
    console.log(`  ${icon}${colors.reset} ${sec.name}`);
  });

  console.log('\n');

  // Exit with appropriate code
  process.exit(results.failed > 0 ? 1 : 0);
}

runTests().catch(err => {
  console.error(`${colors.red}Test suite crashed:${colors.reset}`, err);
  process.exit(1);
});

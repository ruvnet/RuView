#!/usr/bin/env node
/**
 * Comprehensive E2E Test for RvLite WASM
 * Tests: Vector API, SPARQL, Cypher, SQL
 */

import { readFile } from 'fs/promises';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Test results tracking
const results = {
  passed: 0,
  failed: 0,
  tests: []
};

function test(name, fn) {
  return async () => {
    try {
      await fn();
      results.passed++;
      results.tests.push({ name, status: 'PASS' });
      console.log(`  \x1b[32m✓\x1b[0m ${name}`);
    } catch (e) {
      results.failed++;
      results.tests.push({ name, status: 'FAIL', error: e.message });
      console.log(`  \x1b[31m✗\x1b[0m ${name}`);
      console.log(`    Error: ${e.message}`);
    }
  };
}

function assert(condition, message) {
  if (!condition) throw new Error(message || 'Assertion failed');
}

function assertEqual(actual, expected, message) {
  if (actual !== expected) {
    throw new Error(message || `Expected ${expected}, got ${actual}`);
  }
}

async function runTests() {
  console.log('╔════════════════════════════════════════════════════════════╗');
  console.log('║     RvLite WASM Comprehensive E2E Test Suite               ║');
  console.log('╚════════════════════════════════════════════════════════════╝\n');

  // Load WASM
  console.log('Loading WASM module...');
  const wasmPath = join(__dirname, '../public/pkg/rvlite_bg.wasm');
  const wasmBytes = await readFile(wasmPath);
  const rvliteModule = await import('../public/pkg/rvlite.js');
  const { default: initRvLite, RvLite, RvLiteConfig } = rvliteModule;
  await initRvLite(wasmBytes);
  console.log('WASM module loaded successfully!\n');

  const config = new RvLiteConfig(384);
  const db = new RvLite(config);

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 1: VECTOR API TESTS
  // ═══════════════════════════════════════════════════════════════════════════
  console.log('═══════════════════════════════════════════════════════════════');
  console.log('SECTION 1: Vector API Tests');
  console.log('═══════════════════════════════════════════════════════════════');

  await test('Vector insert works', async () => {
    const vector = Array.from({ length: 384 }, () => Math.random());
    const id = db.insert(vector, { label: 'test-vector-1' });
    assert(id !== undefined && id !== null, 'Insert should return an ID');
  })();

  await test('Vector multiple inserts work', async () => {
    const insertedIds = [];
    for (let i = 0; i < 10; i++) {
      const vector = Array.from({ length: 384 }, () => Math.random());
      const id = db.insert(vector, { index: i, batch: 'batch-1' });
      insertedIds.push(id);
    }
    assertEqual(insertedIds.length, 10, 'Should insert 10 vectors');
  })();

  await test('Vector search returns results', async () => {
    const query = Array.from({ length: 384 }, () => Math.random());
    const results = db.search(query, 5);
    assert(Array.isArray(results), 'Search should return array');
    assert(results.length > 0, 'Should find vectors');
    assert(results[0].id !== undefined, 'Results should have IDs');
    assert(results[0].score !== undefined, 'Results should have scores');
  })();

  await test('Vector count is correct', async () => {
    const count = db.len();
    assert(count >= 11, 'Should have at least 11 vectors');
  })();

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 2: SPARQL TESTS
  // ═══════════════════════════════════════════════════════════════════════════
  console.log('\n═══════════════════════════════════════════════════════════════');
  console.log('SECTION 2: SPARQL Tests');
  console.log('═══════════════════════════════════════════════════════════════');

  // Add RDF triples for testing
  const rdfTriples = [
    // People
    ['<http://example.org/Alice>', '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', '<http://example.org/Person>'],
    ['<http://example.org/Alice>', '<http://example.org/name>', '"Alice Smith"'],
    ['<http://example.org/Alice>', '<http://example.org/age>', '"30"'],
    ['<http://example.org/Alice>', '<http://example.org/email>', '"alice@example.org"'],

    ['<http://example.org/Bob>', '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', '<http://example.org/Person>'],
    ['<http://example.org/Bob>', '<http://example.org/name>', '"Bob Jones"'],
    ['<http://example.org/Bob>', '<http://example.org/age>', '"25"'],

    ['<http://example.org/Carol>', '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', '<http://example.org/Person>'],
    ['<http://example.org/Carol>', '<http://example.org/name>', '"Carol White"'],

    // Relationships
    ['<http://example.org/Alice>', '<http://example.org/knows>', '<http://example.org/Bob>'],
    ['<http://example.org/Alice>', '<http://example.org/knows>', '<http://example.org/Carol>'],
    ['<http://example.org/Bob>', '<http://example.org/knows>', '<http://example.org/Carol>'],

    // Projects
    ['<http://example.org/ProjectX>', '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', '<http://example.org/Project>'],
    ['<http://example.org/ProjectX>', '<http://example.org/name>', '"Project X"'],
    ['<http://example.org/Alice>', '<http://example.org/worksOn>', '<http://example.org/ProjectX>'],
    ['<http://example.org/Bob>', '<http://example.org/worksOn>', '<http://example.org/ProjectX>'],
  ];

  await test('SPARQL: Add triples', async () => {
    for (const [s, p, o] of rdfTriples) {
      db.add_triple(s, p, o);
    }
    const count = db.triple_count();
    assert(count >= rdfTriples.length, `Should have at least ${rdfTriples.length} triples`);
  })();

  await test('SPARQL: SELECT with rdf:type', async () => {
    const result = db.sparql('SELECT ?person WHERE { ?person <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> }');
    assertEqual(result.type, 'select', 'Should be SELECT result');
    assert(result.bindings.length >= 3, 'Should find at least 3 people');
    assert(result.bindings.some(b => b.person.value === 'http://example.org/Alice'), 'Should find Alice');
  })();

  await test('SPARQL: SELECT with "a" keyword (rdf:type shortcut)', async () => {
    const result = db.sparql('SELECT ?project WHERE { ?project a <http://example.org/Project> }');
    assertEqual(result.type, 'select', 'Should be SELECT result');
    assert(result.bindings.length >= 1, 'Should find at least 1 project');
  })();

  await test('SPARQL: SELECT with specific predicate', async () => {
    const result = db.sparql('SELECT ?who WHERE { <http://example.org/Alice> <http://example.org/knows> ?who }');
    assertEqual(result.type, 'select', 'Should be SELECT result');
    assertEqual(result.bindings.length, 2, 'Alice knows 2 people');
  })();

  await test('SPARQL: ASK query (true case)', async () => {
    const result = db.sparql('ASK { <http://example.org/Alice> <http://example.org/knows> <http://example.org/Bob> }');
    assertEqual(result.type, 'ask', 'Should be ASK result');
    assertEqual(result.result, true, 'Alice should know Bob');
  })();

  await test('SPARQL: ASK query (false case)', async () => {
    const result = db.sparql('ASK { <http://example.org/Carol> <http://example.org/knows> <http://example.org/Alice> }');
    assertEqual(result.type, 'ask', 'Should be ASK result');
    assertEqual(result.result, false, 'Carol does not know Alice');
  })();

  await test('SPARQL: SELECT with LIMIT', async () => {
    const result = db.sparql('SELECT ?s WHERE { ?s a <http://example.org/Person> } LIMIT 2');
    assertEqual(result.type, 'select', 'Should be SELECT result');
    assertEqual(result.bindings.length, 2, 'Should return exactly 2 results');
  })();

  await test('SPARQL: SELECT with literal values', async () => {
    const result = db.sparql('SELECT ?name WHERE { <http://example.org/Alice> <http://example.org/name> ?name }');
    assertEqual(result.type, 'select', 'Should be SELECT result');
    assertEqual(result.bindings.length, 1, 'Should find Alice\'s name');
    assertEqual(result.bindings[0].name.type, 'literal', 'Name should be literal');
  })();

  await test('SPARQL: Result binding format (IRI)', async () => {
    const result = db.sparql('SELECT ?s WHERE { ?s a <http://example.org/Person> } LIMIT 1');
    const binding = result.bindings[0];
    assertEqual(binding.s.type, 'iri', 'Should have type=iri');
    assert(binding.s.value.startsWith('http://'), 'Value should be clean IRI');
  })();

  await test('SPARQL: Result binding format (Literal)', async () => {
    const result = db.sparql('SELECT ?name WHERE { <http://example.org/Bob> <http://example.org/name> ?name }');
    const binding = result.bindings[0];
    assertEqual(binding.name.type, 'literal', 'Should have type=literal');
    assert(binding.name.datatype, 'Should have datatype');
  })();

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 3: CYPHER TESTS
  // ═══════════════════════════════════════════════════════════════════════════
  console.log('\n═══════════════════════════════════════════════════════════════');
  console.log('SECTION 3: Cypher Tests');
  console.log('═══════════════════════════════════════════════════════════════');

  await test('Cypher: CREATE node', async () => {
    const result = db.cypher('CREATE (n:TestNode {name: "TestCypher"}) RETURN n');
    assert(result !== undefined, 'Should return result');
  })();

  await test('Cypher: MATCH query', async () => {
    const result = db.cypher('MATCH (n:TestNode) RETURN n.name');
    assert(result !== undefined, 'Should return result');
  })();

  await test('Cypher: CREATE relationship', async () => {
    db.cypher('CREATE (a:CypherPerson {name: "Dave"})');
    db.cypher('CREATE (b:CypherPerson {name: "Eve"})');
    const result = db.cypher('MATCH (a:CypherPerson {name: "Dave"}), (b:CypherPerson {name: "Eve"}) CREATE (a)-[r:KNOWS]->(b) RETURN r');
    assert(result !== undefined, 'Should create relationship');
  })();

  await test('Cypher: MATCH with relationship', async () => {
    const result = db.cypher('MATCH (a:CypherPerson)-[r:KNOWS]->(b:CypherPerson) RETURN a.name, b.name');
    assert(result !== undefined, 'Should match relationships');
  })();

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 4: DATABASE INFO & STATS
  // ═══════════════════════════════════════════════════════════════════════════
  console.log('\n═══════════════════════════════════════════════════════════════');
  console.log('SECTION 4: Database Statistics');
  console.log('═══════════════════════════════════════════════════════════════');

  await test('Get vector count', async () => {
    const count = db.len();
    console.log(`    Vector count: ${count}`);
    assert(count >= 0, 'Should return valid count');
  })();

  await test('Get triple count', async () => {
    const count = db.triple_count();
    console.log(`    Triple count: ${count}`);
    assert(count >= rdfTriples.length, 'Should return valid count');
  })();

  await test('Get database config', async () => {
    const config = db.get_config();
    assert(config.dimensions, 'Should have dimensions');
    const version = db.get_version();
    assert(version, 'Should have version');
    const features = db.get_features();
    assert(features, 'Should have features');
    console.log(`    Version: ${version}`);
    console.log(`    Dimensions: ${config.dimensions}`);
    console.log(`    Distance metric: ${config.distance_metric}`);
  })();

  // ═══════════════════════════════════════════════════════════════════════════
  // SECTION 5: EDGE CASES & ERROR HANDLING
  // ═══════════════════════════════════════════════════════════════════════════
  console.log('\n═══════════════════════════════════════════════════════════════');
  console.log('SECTION 5: Edge Cases & Error Handling');
  console.log('═══════════════════════════════════════════════════════════════');

  await test('SPARQL: Empty result for non-existent data', async () => {
    const result = db.sparql('SELECT ?s WHERE { ?s a <http://example.org/NonExistent> }');
    assertEqual(result.bindings.length, 0, 'Should return empty bindings');
  })();

  await test('SPARQL: Handle special characters in IRIs', async () => {
    db.add_triple('<http://example.org/item#1>', '<http://example.org/type>', '<http://example.org/Thing>');
    const result = db.sparql('SELECT ?s WHERE { ?s <http://example.org/type> <http://example.org/Thing> }');
    assert(result.bindings.length >= 1, 'Should handle # in IRIs');
  })();

  await test('Vector: Search with empty database returns empty array', async () => {
    // Create fresh instance for this test
    const freshConfig = new RvLiteConfig(64);
    const freshDb = new RvLite(freshConfig);
    const query = Array.from({ length: 64 }, () => Math.random());
    const searchResults = freshDb.search(query, 5);
    assert(Array.isArray(searchResults), 'Should return array');
    assertEqual(searchResults.length, 0, 'Should return empty array');
    freshDb.free();
  })();

  // ═══════════════════════════════════════════════════════════════════════════
  // SUMMARY
  // ═══════════════════════════════════════════════════════════════════════════
  console.log('\n╔════════════════════════════════════════════════════════════╗');
  console.log('║                    TEST SUMMARY                             ║');
  console.log('╚════════════════════════════════════════════════════════════╝');
  console.log(`\n  Total:  ${results.passed + results.failed} tests`);
  console.log(`  \x1b[32mPassed: ${results.passed}\x1b[0m`);
  console.log(`  \x1b[31mFailed: ${results.failed}\x1b[0m`);

  if (results.failed > 0) {
    console.log('\n\x1b[31mFailed Tests:\x1b[0m');
    results.tests.filter(t => t.status === 'FAIL').forEach(t => {
      console.log(`  - ${t.name}: ${t.error}`);
    });
  }

  console.log('\n');

  // Cleanup
  db.free();

  // Exit with appropriate code
  process.exit(results.failed > 0 ? 1 : 0);
}

runTests().catch(err => {
  console.error('Test suite crashed:', err);
  process.exit(1);
});

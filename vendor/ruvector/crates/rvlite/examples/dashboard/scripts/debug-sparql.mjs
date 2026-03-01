#!/usr/bin/env node
/**
 * Debug SPARQL execution to understand why results are empty
 */

import { readFile } from 'fs/promises';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));

async function test() {
  console.log('=== Debug SPARQL Execution ===\n');

  // Load WASM
  const wasmPath = join(__dirname, '../public/pkg/rvlite_bg.wasm');
  const wasmBytes = await readFile(wasmPath);
  const rvliteModule = await import('../public/pkg/rvlite.js');
  const { default: initRvLite, RvLite, RvLiteConfig } = rvliteModule;
  await initRvLite(wasmBytes);

  const config = new RvLiteConfig(384);
  const db = new RvLite(config);
  console.log('✓ WASM initialized');

  // Add triples
  console.log('\n=== Adding Triples ===');
  const triples = [
    ['<http://example.org/Alice>', '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', '<http://example.org/Person>'],
    ['<http://example.org/Alice>', '<http://example.org/name>', '"Alice"'],
    ['<http://example.org/Bob>', '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', '<http://example.org/Person>'],
    ['<http://example.org/Bob>', '<http://example.org/name>', '"Bob"'],
    ['<http://example.org/Alice>', '<http://example.org/knows>', '<http://example.org/Bob>'],
  ];

  for (const [s, p, o] of triples) {
    try {
      db.add_triple(s, p, o);
      console.log(`  Added: ${s} ${p} ${o}`);
    } catch (e) {
      console.log(`  ERROR adding triple: ${e.message}`);
    }
  }
  console.log(`Triple count: ${db.triple_count()}`);

  // Test queries with full debug output
  console.log('\n=== Testing SPARQL Queries ===');

  const queries = [
    // Simple SELECT with variable predicate
    "SELECT ?s ?p ?o WHERE { ?s ?p ?o }",
    // SELECT with specific predicate
    "SELECT ?s WHERE { ?s <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> }",
    // SELECT with specific predicate (no angle brackets in predicate)
    "SELECT ?s WHERE { ?s <http://example.org/knows> ?o }",
    // ASK query
    "ASK { <http://example.org/Alice> <http://example.org/knows> <http://example.org/Bob> }",
  ];

  for (const query of queries) {
    console.log(`\nQuery: ${query}`);
    try {
      const result = db.sparql(query);
      console.log('Result type:', typeof result);
      console.log('Result:', JSON.stringify(result, null, 2));
    } catch (e) {
      console.log('ERROR:', e.message || e);
    }
  }

  db.free();
}

test().catch(console.error);

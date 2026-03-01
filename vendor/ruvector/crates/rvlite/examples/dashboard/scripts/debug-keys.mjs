#!/usr/bin/env node
/**
 * Debug key format mismatch in SPARQL triple store
 */

import { readFile } from 'fs/promises';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));

async function test() {
  console.log('=== Debug Key Format Mismatch ===\n');

  // Load WASM
  const wasmPath = join(__dirname, '../public/pkg/rvlite_bg.wasm');
  const wasmBytes = await readFile(wasmPath);
  const rvliteModule = await import('../public/pkg/rvlite.js');
  const { default: initRvLite, RvLite, RvLiteConfig } = rvliteModule;
  await initRvLite(wasmBytes);

  const config = new RvLiteConfig(384);
  const db = new RvLite(config);
  console.log('✓ WASM initialized');

  // Test 1: Check what format the add_triple expects
  console.log('\n=== Test 1: Adding triples with different formats ===');

  // Format A: With angle brackets
  try {
    db.add_triple('<http://ex.org/a1>', '<http://ex.org/p1>', '<http://ex.org/o1>');
    console.log('✓ Format A (with <>) accepted');
  } catch (e) {
    console.log('✗ Format A (with <>) rejected:', e.message);
  }

  // Format B: Without angle brackets
  try {
    db.add_triple('http://ex.org/a2', 'http://ex.org/p2', 'http://ex.org/o2');
    console.log('✓ Format B (without <>) accepted');
  } catch (e) {
    console.log('✗ Format B (without <>) rejected:', e.message);
  }

  console.log(`Total triples: ${db.triple_count()}`);

  // Test 2: Try SPARQL queries that match each format
  console.log('\n=== Test 2: SPARQL queries with different predicate formats ===');

  const testQueries = [
    // Query for triples added with format A
    'SELECT ?s WHERE { ?s <http://ex.org/p1> ?o }',
    // Query for triples added with format B
    'SELECT ?s WHERE { ?s <http://ex.org/p2> ?o }',
    // Wildcard predicate (variable)
    // 'SELECT ?s ?p WHERE { ?s ?p ?o }', // This fails with "Complex property paths not yet supported"
  ];

  for (const query of testQueries) {
    console.log(`\nQuery: ${query}`);
    try {
      const result = db.sparql(query);
      console.log('Result:', JSON.stringify(result, null, 2));
    } catch (e) {
      console.log('ERROR:', e.message || e);
    }
  }

  // Test 3: Add rdf:type triple and test with actual RDF type query
  console.log('\n=== Test 3: RDF type query ===');

  db.add_triple(
    '<http://example.org/Alice>',
    '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>',
    '<http://example.org/Person>'
  );
  console.log(`Triple count after adding rdf:type: ${db.triple_count()}`);

  const typeQuery = 'SELECT ?s WHERE { ?s <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> }';
  console.log(`Query: ${typeQuery}`);
  try {
    const result = db.sparql(typeQuery);
    console.log('Result:', JSON.stringify(result, null, 2));

    if (result.bindings && result.bindings.length > 0) {
      console.log('✓ SPARQL is working!');
    } else {
      console.log('✗ No bindings returned - key mismatch suspected');
    }
  } catch (e) {
    console.log('ERROR:', e.message || e);
  }

  // Test 4: Simple triple with known data
  console.log('\n=== Test 4: Minimal test case ===');

  db.add_triple('<http://a>', '<http://b>', '<http://c>');
  console.log(`Triple count: ${db.triple_count()}`);

  const minimalQuery = 'SELECT ?s WHERE { ?s <http://b> ?o }';
  console.log(`Query: ${minimalQuery}`);
  try {
    const result = db.sparql(minimalQuery);
    console.log('Result:', JSON.stringify(result, null, 2));
  } catch (e) {
    console.log('ERROR:', e.message || e);
  }

  // Test 5: Get all triples using 'a' keyword (rdf:type shortcut)
  console.log('\n=== Test 5: Using "a" keyword ===');
  const aQuery = 'SELECT ?s WHERE { ?s a <http://example.org/Person> }';
  console.log(`Query: ${aQuery}`);
  try {
    const result = db.sparql(aQuery);
    console.log('Result:', JSON.stringify(result, null, 2));
  } catch (e) {
    console.log('ERROR:', e.message || e);
  }

  db.free();
}

test().catch(console.error);

#!/usr/bin/env node
// rvf-node-example.mjs — Node.js RVF quick start
// Usage: node scripts/rvf-node-example.mjs
// Prerequisites: npm install @ruvector/rvf-node

import { createRequire } from 'module';
const require = createRequire(import.meta.url);

let RvfDatabase;
try {
  ({ RvfDatabase } = require('@ruvector/rvf-node'));
} catch {
  console.error('Install first: npm install @ruvector/rvf-node');
  process.exit(1);
}

const path = '/tmp/rvf_node_demo.rvf';

console.log('=== RVF Node.js Quick Start ===\n');

// 1. Create store
console.log('[1/6] Creating vector store...');
const db = RvfDatabase.create(path, {
  dimension: 128,
  metric: 'cosine',
  m: 16,
  efConstruction: 200,
});
console.log(`  Created: ${path} (128-dim, cosine)`);

// 2. Ingest vectors
console.log('[2/6] Ingesting vectors...');
const count = 100;
const vectors = new Float32Array(count * 128);
const ids = [];
for (let i = 0; i < count; i++) {
  for (let d = 0; d < 128; d++) {
    vectors[i * 128 + d] = Math.sin(i * 0.1 + d * 0.01);
  }
  ids.push(i + 1);
}
const result = db.ingestBatch(vectors, ids);
console.log(`  Ingested: ${result.accepted} vectors, epoch ${result.epoch}`);

// 3. Query nearest neighbors
console.log('[3/6] Querying nearest neighbors...');
const query = new Float32Array(128);
query.fill(0.1);
const matches = db.query(query, 5, { efSearch: 200 });
console.log('  Top-5 results:');
for (const m of matches) {
  console.log(`    id=${m.id}, distance=${m.distance.toFixed(4)}`);
}

// 4. Filtered query
console.log('[4/6] Querying with metadata filter...');
const filtered = db.query(query, 3, {
  filter: JSON.stringify({ op: 'gt', fieldId: 0, valueType: 'u64', value: '50' }),
});
console.log(`  Filtered results: ${filtered.length} matches`);

// 5. Lineage
console.log('[5/6] Checking lineage...');
console.log(`  fileId: ${db.fileId()}`);
console.log(`  parentId: ${db.parentId()}`);
console.log(`  lineageDepth: ${db.lineageDepth()}`);

// 6. Status
console.log('[6/6] Store status...');
const status = db.status();
console.log(`  vectors: ${status.totalVectors}`);
console.log(`  segments: ${status.totalSegments}`);
console.log(`  file size: ${status.fileSize} bytes`);

db.close();
console.log('\n=== Done ===');

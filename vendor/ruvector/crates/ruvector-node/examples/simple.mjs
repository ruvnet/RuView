#!/usr/bin/env node

/**
 * Simple example demonstrating basic Ruvector operations
 */

import { VectorDB } from '../index.js';

async function main() {
  console.log('🚀 Ruvector Simple Example\n');

  // Create a vector database
  const db = new VectorDB({
    dimensions: 3,
    distanceMetric: 'Cosine',
    storagePath: './simple-example.db',
  });

  console.log('✅ Created vector database');

  // Insert vectors
  console.log('\n📝 Inserting vectors...');

  const id1 = await db.insert({
    id: 'vec1',
    vector: new Float32Array([1.0, 0.0, 0.0]),
    metadata: { text: 'First vector' },
  });

  const id2 = await db.insert({
    id: 'vec2',
    vector: new Float32Array([0.0, 1.0, 0.0]),
    metadata: { text: 'Second vector' },
  });

  const id3 = await db.insert({
    id: 'vec3',
    vector: new Float32Array([0.5, 0.5, 0.0]),
    metadata: { text: 'Third vector' },
  });

  console.log(`  Inserted: ${id1}, ${id2}, ${id3}`);

  // Get database stats
  const count = await db.len();
  console.log(`\n📊 Database contains ${count} vectors`);

  // Search for similar vectors
  console.log('\n🔍 Searching for similar vectors...');

  const results = await db.search({
    vector: new Float32Array([1.0, 0.0, 0.0]),
    k: 3,
  });

  console.log(`  Found ${results.length} results:`);
  results.forEach((result, i) => {
    console.log(`    ${i + 1}. ID: ${result.id}, Score: ${result.score.toFixed(4)}`);
    console.log(`       Metadata: ${JSON.stringify(result.metadata)}`);
  });

  // Get a specific vector
  console.log('\n🎯 Getting vector by ID...');
  const entry = await db.get('vec2');
  if (entry) {
    console.log(`  Found: ${entry.id}`);
    console.log(`  Vector: [${Array.from(entry.vector).join(', ')}]`);
    console.log(`  Metadata: ${JSON.stringify(entry.metadata)}`);
  }

  // Delete a vector
  console.log('\n🗑️  Deleting vector...');
  const deleted = await db.delete('vec1');
  console.log(`  Deleted: ${deleted}`);

  const newCount = await db.len();
  console.log(`  Database now contains ${newCount} vectors`);

  console.log('\n✨ Example complete!');
}

main().catch((err) => {
  console.error('Error:', err);
  process.exit(1);
});

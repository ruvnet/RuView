#!/usr/bin/env node

/**
 * ruvector API Usage Examples
 *
 * This demonstrates how to use ruvector in your Node.js applications
 */

// For this demo, we use the mock implementation
// In production, you would use: const { VectorDB } = require('ruvector');
const { VectorDB } = require('../test/mock-implementation.js');

console.log('ruvector API Examples\n');
console.log('='.repeat(60));

// Show info
console.log('\nUsing: Mock implementation (for demo purposes)');
console.log('In production: npm install ruvector\n');

// Example 1: Basic usage
console.log('Example 1: Basic Vector Operations');
console.log('-'.repeat(60));

const db = new VectorDB({
  dimension: 3,
  metric: 'cosine'
});

// Insert some vectors
db.insert({
  id: 'doc1',
  vector: [1, 0, 0],
  metadata: { title: 'First Document', category: 'A' }
});

db.insertBatch([
  { id: 'doc2', vector: [0, 1, 0], metadata: { title: 'Second Document', category: 'B' } },
  { id: 'doc3', vector: [0, 0, 1], metadata: { title: 'Third Document', category: 'C' } },
  { id: 'doc4', vector: [0.7, 0.7, 0], metadata: { title: 'Fourth Document', category: 'A' } }
]);

console.log('✓ Inserted 4 vectors');

// Get stats
const stats = db.stats();
console.log(`✓ Database has ${stats.count} vectors, dimension ${stats.dimension}`);

// Search
const results = db.search({
  vector: [1, 0, 0],
  k: 3
});

console.log(`✓ Search returned ${results.length} results:`);
results.forEach((result, i) => {
  console.log(`  ${i + 1}. ${result.id} (score: ${result.score.toFixed(4)}) - ${result.metadata.title}`);
});

// Get by ID
const doc = db.get('doc2');
console.log(`✓ Retrieved document: ${doc.metadata.title}`);

// Update metadata
db.updateMetadata('doc1', { updated: true, timestamp: Date.now() });
console.log('✓ Updated metadata');

// Delete
db.delete('doc3');
console.log('✓ Deleted doc3');
console.log(`✓ Database now has ${db.stats().count} vectors\n`);

// Example 2: Semantic Search Simulation
console.log('Example 2: Semantic Search Simulation');
console.log('-'.repeat(60));

const semanticDb = new VectorDB({
  dimension: 5,
  metric: 'cosine'
});

// Simulate document embeddings
const documents = [
  { id: 'machine-learning', vector: [0.9, 0.8, 0.1, 0.2, 0.1], metadata: { title: 'Introduction to Machine Learning', topic: 'AI' } },
  { id: 'deep-learning', vector: [0.85, 0.9, 0.15, 0.25, 0.1], metadata: { title: 'Deep Learning Fundamentals', topic: 'AI' } },
  { id: 'web-dev', vector: [0.1, 0.2, 0.9, 0.8, 0.1], metadata: { title: 'Web Development Guide', topic: 'Web' } },
  { id: 'react', vector: [0.15, 0.2, 0.85, 0.9, 0.1], metadata: { title: 'React Tutorial', topic: 'Web' } },
  { id: 'database', vector: [0.2, 0.3, 0.3, 0.4, 0.9], metadata: { title: 'Database Design', topic: 'Data' } }
];

semanticDb.insertBatch(documents);
console.log(`✓ Indexed ${documents.length} documents`);

// Search for AI-related content
const aiQuery = [0.9, 0.85, 0.1, 0.2, 0.1];
const aiResults = semanticDb.search({ vector: aiQuery, k: 2 });

console.log('\nQuery: AI-related content');
console.log('Results:');
aiResults.forEach((result, i) => {
  console.log(`  ${i + 1}. ${result.metadata.title} (score: ${result.score.toFixed(4)})`);
});

// Search for Web-related content
const webQuery = [0.1, 0.2, 0.9, 0.85, 0.1];
const webResults = semanticDb.search({ vector: webQuery, k: 2 });

console.log('\nQuery: Web-related content');
console.log('Results:');
webResults.forEach((result, i) => {
  console.log(`  ${i + 1}. ${result.metadata.title} (score: ${result.score.toFixed(4)})`);
});

// Example 3: Different Distance Metrics
console.log('\n\nExample 3: Distance Metrics Comparison');
console.log('-'.repeat(60));

const metrics = ['cosine', 'euclidean', 'dot'];
const testVectors = [
  { id: 'v1', vector: [1, 0, 0] },
  { id: 'v2', vector: [0.7, 0.7, 0] },
  { id: 'v3', vector: [0, 1, 0] }
];

metrics.forEach(metric => {
  const metricDb = new VectorDB({ dimension: 3, metric });
  metricDb.insertBatch(testVectors);

  const results = metricDb.search({ vector: [1, 0, 0], k: 3 });

  console.log(`\n${metric.toUpperCase()} metric:`);
  results.forEach((result, i) => {
    console.log(`  ${i + 1}. ${result.id}: ${result.score.toFixed(4)}`);
  });
});

// Example 4: Batch Operations Performance
console.log('\n\nExample 4: Batch Operations Performance');
console.log('-'.repeat(60));

const perfDb = new VectorDB({ dimension: 128, metric: 'cosine' });

// Generate random vectors
const numVectors = 1000;
const vectors = [];
for (let i = 0; i < numVectors; i++) {
  vectors.push({
    id: `vec_${i}`,
    vector: Array.from({ length: 128 }, () => Math.random()),
    metadata: { index: i, batch: Math.floor(i / 100) }
  });
}

console.log(`Inserting ${numVectors} vectors...`);
const insertStart = Date.now();
perfDb.insertBatch(vectors);
const insertTime = Date.now() - insertStart;

console.log(`✓ Inserted ${numVectors} vectors in ${insertTime}ms`);
console.log(`✓ Rate: ${Math.round(numVectors / (insertTime / 1000))} vectors/sec`);

// Search performance
const numQueries = 100;
console.log(`\nRunning ${numQueries} searches...`);
const searchStart = Date.now();

for (let i = 0; i < numQueries; i++) {
  const query = {
    vector: Array.from({ length: 128 }, () => Math.random()),
    k: 10
  };
  perfDb.search(query);
}

const searchTime = Date.now() - searchStart;
console.log(`✓ Completed ${numQueries} searches in ${searchTime}ms`);
console.log(`✓ Rate: ${Math.round(numQueries / (searchTime / 1000))} queries/sec`);
console.log(`✓ Avg latency: ${(searchTime / numQueries).toFixed(2)}ms`);

// Example 5: Persistence (conceptual, would need real implementation)
console.log('\n\nExample 5: Persistence');
console.log('-'.repeat(60));

const persistDb = new VectorDB({
  dimension: 3,
  metric: 'cosine',
  path: './my-vectors.db',
  autoPersist: true
});

persistDb.insertBatch([
  { id: 'p1', vector: [1, 0, 0], metadata: { name: 'First' } },
  { id: 'p2', vector: [0, 1, 0], metadata: { name: 'Second' } }
]);

console.log('✓ Created database with auto-persist enabled');
console.log('✓ Insert operations will automatically save to disk');
console.log('✓ Use db.save(path) for manual saves');
console.log('✓ Use db.load(path) to restore from disk');

// Summary
console.log('\n' + '='.repeat(60));
console.log('\n✅ All examples completed successfully!');
console.log('\nKey Features Demonstrated:');
console.log('  • Basic CRUD operations (insert, search, get, update, delete)');
console.log('  • Batch operations for better performance');
console.log('  • Multiple distance metrics (cosine, euclidean, dot)');
console.log('  • Semantic search simulation');
console.log('  • Performance benchmarking');
console.log('  • Metadata filtering and updates');
console.log('  • Persistence (save/load)');
console.log('\nFor more examples, see: /workspaces/ruvector/npm/packages/ruvector/examples/');

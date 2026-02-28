#!/usr/bin/env node

/**
 * Advanced example demonstrating HNSW indexing and batch operations
 */

import { VectorDB } from '../index.js';

// Generate random vector
function randomVector(dim) {
  return new Float32Array(dim).fill(0).map(() => Math.random());
}

async function main() {
  console.log('🚀 Ruvector Advanced Example\n');

  // Create database with HNSW indexing
  const db = new VectorDB({
    dimensions: 128,
    distanceMetric: 'Cosine',
    storagePath: './advanced-example.db',
    hnswConfig: {
      m: 32, // Number of connections per node
      efConstruction: 200, // Construction quality
      efSearch: 100, // Search quality
      maxElements: 100000,
    },
    quantization: {
      type: 'scalar', // 4x compression
    },
  });

  console.log('✅ Created database with HNSW indexing');

  // Batch insert
  console.log('\n📝 Inserting 10,000 vectors in batches...');

  const batchSize = 1000;
  const totalVectors = 10000;
  const startTime = Date.now();

  for (let i = 0; i < totalVectors / batchSize; i++) {
    const batch = Array.from({ length: batchSize }, (_, j) => ({
      vector: randomVector(128),
      metadata: {
        batch: i,
        index: i * batchSize + j,
        category: ['A', 'B', 'C'][j % 3],
      },
    }));

    await db.insertBatch(batch);

    const progress = ((i + 1) / (totalVectors / batchSize)) * 100;
    process.stdout.write(`\r  Progress: ${progress.toFixed(0)}%`);
  }

  const insertTime = Date.now() - startTime;
  console.log(`\n  Inserted ${totalVectors} vectors in ${insertTime}ms`);
  console.log(`  Throughput: ${((totalVectors / insertTime) * 1000).toFixed(0)} vectors/sec`);

  // Verify database size
  const count = await db.len();
  console.log(`\n📊 Database contains ${count} vectors`);

  // Benchmark search performance
  console.log('\n🔍 Benchmarking search performance...');

  const numQueries = 100;
  const searchStart = Date.now();

  for (let i = 0; i < numQueries; i++) {
    const results = await db.search({
      vector: randomVector(128),
      k: 10,
    });

    if (i === 0) {
      console.log(`\n  First query results:`);
      results.slice(0, 3).forEach((r, idx) => {
        console.log(`    ${idx + 1}. Score: ${r.score.toFixed(6)}, Category: ${r.metadata?.category}`);
      });
    }
  }

  const searchTime = Date.now() - searchStart;
  const avgLatency = searchTime / numQueries;
  const qps = (numQueries / searchTime) * 1000;

  console.log(`\n  Completed ${numQueries} queries in ${searchTime}ms`);
  console.log(`  Average latency: ${avgLatency.toFixed(2)}ms`);
  console.log(`  QPS: ${qps.toFixed(0)} queries/sec`);

  // Search with metadata filter
  console.log('\n🎯 Searching with metadata filter...');

  const filteredResults = await db.search({
    vector: randomVector(128),
    k: 20,
    filter: { category: 'A' },
  });

  console.log(`  Found ${filteredResults.length} results in category 'A'`);
  filteredResults.slice(0, 3).forEach((r, i) => {
    console.log(`    ${i + 1}. Score: ${r.score.toFixed(6)}, Index: ${r.metadata?.index}`);
  });

  // Concurrent operations
  console.log('\n⚡ Testing concurrent operations...');

  const concurrentStart = Date.now();

  const promises = [
    // Concurrent searches
    ...Array.from({ length: 50 }, () =>
      db.search({
        vector: randomVector(128),
        k: 10,
      })
    ),
    // Concurrent inserts
    ...Array.from({ length: 50 }, (_, i) =>
      db.insert({
        vector: randomVector(128),
        metadata: { concurrent: true, index: i },
      })
    ),
  ];

  await Promise.all(promises);

  const concurrentTime = Date.now() - concurrentStart;
  console.log(`  Completed 100 concurrent operations in ${concurrentTime}ms`);

  // Final stats
  const finalCount = await db.len();
  console.log(`\n📊 Final database size: ${finalCount} vectors`);

  console.log('\n✨ Advanced example complete!');
}

main().catch((err) => {
  console.error('Error:', err);
  process.exit(1);
});

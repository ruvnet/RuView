#!/usr/bin/env node

/**
 * Semantic search example with text embeddings
 *
 * Note: This example assumes you have a way to generate embeddings.
 * In practice, you would use an embedding model like sentence-transformers
 * or OpenAI's API to generate actual embeddings.
 */

import { VectorDB } from '../index.js';

// Mock embedding function (in practice, use a real embedding model)
function mockEmbedding(text, dim = 384) {
  // Simple deterministic "embedding" based on text
  const hash = text.split('').reduce((acc, char) => {
    return ((acc << 5) - acc) + char.charCodeAt(0);
  }, 0);

  const vector = new Float32Array(dim);
  for (let i = 0; i < dim; i++) {
    vector[i] = Math.sin(hash * (i + 1) * 0.1);
  }

  // Normalize
  const norm = Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
  for (let i = 0; i < dim; i++) {
    vector[i] /= norm;
  }

  return vector;
}

async function main() {
  console.log('🚀 Ruvector Semantic Search Example\n');

  // Sample documents
  const documents = [
    { id: 'doc1', text: 'The cat sat on the mat', category: 'animals' },
    { id: 'doc2', text: 'The dog played in the park', category: 'animals' },
    { id: 'doc3', text: 'Python is a programming language', category: 'tech' },
    { id: 'doc4', text: 'JavaScript is used for web development', category: 'tech' },
    { id: 'doc5', text: 'Machine learning models learn from data', category: 'tech' },
    { id: 'doc6', text: 'The bird flew over the tree', category: 'animals' },
    { id: 'doc7', text: 'Rust is a systems programming language', category: 'tech' },
    { id: 'doc8', text: 'The fish swam in the ocean', category: 'animals' },
    { id: 'doc9', text: 'Neural networks are inspired by the brain', category: 'tech' },
    { id: 'doc10', text: 'The horse galloped across the field', category: 'animals' },
  ];

  // Create database
  const db = new VectorDB({
    dimensions: 384,
    distanceMetric: 'Cosine',
    storagePath: './semantic-search.db',
  });

  console.log('✅ Created vector database');

  // Index documents
  console.log('\n📝 Indexing documents...');

  const entries = documents.map((doc) => ({
    id: doc.id,
    vector: mockEmbedding(doc.text),
    metadata: {
      text: doc.text,
      category: doc.category,
    },
  }));

  await db.insertBatch(entries);
  console.log(`  Indexed ${documents.length} documents`);

  // Search queries
  const queries = [
    'animals in nature',
    'programming languages',
    'artificial intelligence',
    'pets and animals',
  ];

  console.log('\n🔍 Running semantic searches...\n');

  for (const query of queries) {
    console.log(`Query: "${query}"`);

    const results = await db.search({
      vector: mockEmbedding(query),
      k: 3,
    });

    console.log('  Top results:');
    results.forEach((result, i) => {
      console.log(`    ${i + 1}. [${result.metadata?.category}] ${result.metadata?.text}`);
      console.log(`       Score: ${result.score.toFixed(4)}`);
    });
    console.log();
  }

  // Category-filtered search
  console.log('🎯 Filtered search (tech category only)...\n');

  const techQuery = 'coding and software';
  console.log(`Query: "${techQuery}"`);

  const techResults = await db.search({
    vector: mockEmbedding(techQuery),
    k: 3,
    filter: { category: 'tech' },
  });

  console.log('  Top results:');
  techResults.forEach((result, i) => {
    console.log(`    ${i + 1}. [${result.metadata?.category}] ${result.metadata?.text}`);
    console.log(`       Score: ${result.score.toFixed(4)}`);
  });

  // Update a document
  console.log('\n📝 Updating a document...');

  await db.delete('doc3');
  await db.insert({
    id: 'doc3',
    vector: mockEmbedding('Python is great for machine learning and AI'),
    metadata: {
      text: 'Python is great for machine learning and AI',
      category: 'tech',
    },
  });

  console.log('  Updated doc3');

  // Search again to see the change
  const updatedResults = await db.search({
    vector: mockEmbedding('artificial intelligence'),
    k: 3,
  });

  console.log('\n  Results after update:');
  updatedResults.forEach((result, i) => {
    console.log(`    ${i + 1}. [${result.metadata?.category}] ${result.metadata?.text}`);
    console.log(`       Score: ${result.score.toFixed(4)}`);
  });

  console.log('\n✨ Semantic search example complete!');
  console.log('\n💡 Tip: In production, use real embeddings from models like:');
  console.log('   - sentence-transformers (e.g., all-MiniLM-L6-v2)');
  console.log('   - OpenAI embeddings (text-embedding-ada-002)');
  console.log('   - Cohere embeddings');
}

main().catch((err) => {
  console.error('Error:', err);
  process.exit(1);
});

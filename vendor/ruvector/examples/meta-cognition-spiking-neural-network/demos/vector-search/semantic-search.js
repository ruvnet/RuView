#!/usr/bin/env node

/**
 * Vector Search Demonstration
 *
 * Demonstrates AgentDB's 150x faster vector search capabilities using RuVector.
 * This example creates a semantic search engine for technical documentation.
 */

const { VectorDB } = require('ruvector');

console.log('🔎 AgentDB Vector Search Demonstration\n');
console.log('=' .repeat(70));

// Sample technical documents
const documents = [
  {
    id: 'doc1',
    title: 'Introduction to Neural Networks',
    content: 'Neural networks are computing systems inspired by biological neural networks. They consist of interconnected nodes that process information.',
    category: 'AI',
    keywords: ['neural networks', 'deep learning', 'AI']
  },
  {
    id: 'doc2',
    title: 'Vector Databases Explained',
    content: 'Vector databases store high-dimensional vectors and enable fast similarity search using techniques like HNSW and IVF.',
    category: 'Database',
    keywords: ['vectors', 'similarity search', 'embeddings']
  },
  {
    id: 'doc3',
    title: 'Attention Mechanisms in Transformers',
    content: 'Attention mechanisms allow models to focus on relevant parts of the input. Multi-head attention processes multiple representations simultaneously.',
    category: 'AI',
    keywords: ['attention', 'transformers', 'NLP']
  },
  {
    id: 'doc4',
    title: 'Graph Neural Networks',
    content: 'GNNs operate on graph-structured data, learning representations by aggregating information from node neighborhoods.',
    category: 'AI',
    keywords: ['GNN', 'graph learning', 'message passing']
  },
  {
    id: 'doc5',
    title: 'Rust Performance Optimization',
    content: 'Rust provides zero-cost abstractions and memory safety without garbage collection, making it ideal for high-performance systems.',
    category: 'Programming',
    keywords: ['Rust', 'performance', 'systems programming']
  },
  {
    id: 'doc6',
    title: 'Hyperbolic Geometry in ML',
    content: 'Hyperbolic spaces naturally represent hierarchical data. The Poincaré ball model enables efficient embedding of tree-like structures.',
    category: 'AI',
    keywords: ['hyperbolic geometry', 'embeddings', 'hierarchical data']
  },
  {
    id: 'doc7',
    title: 'Real-time Vector Indexing',
    content: 'Modern vector databases support real-time indexing with sub-millisecond latency using SIMD operations and optimized data structures.',
    category: 'Database',
    keywords: ['indexing', 'SIMD', 'real-time']
  },
  {
    id: 'doc8',
    title: 'Mixture of Experts Architecture',
    content: 'MoE models use gating networks to route inputs to specialized expert networks, improving model capacity and efficiency.',
    category: 'AI',
    keywords: ['MoE', 'neural architecture', 'routing']
  },
  {
    id: 'doc9',
    title: 'Semantic Caching Strategies',
    content: 'Semantic caching stores results based on meaning rather than exact matches, using vector similarity to retrieve cached responses.',
    category: 'Optimization',
    keywords: ['caching', 'semantic search', 'optimization']
  },
  {
    id: 'doc10',
    title: 'Edge AI Deployment',
    content: 'Deploying AI models on edge devices requires optimization techniques like quantization, pruning, and efficient runtimes.',
    category: 'Deployment',
    keywords: ['edge computing', 'model optimization', 'deployment']
  }
];

// Simple text-to-vector function (using character frequency for demo)
// In production, you'd use a real embedding model like Xenova/all-MiniLM-L6-v2
function textToVector(text, dimensions = 128) {
  const vector = new Float32Array(dimensions);
  const normalized = text.toLowerCase();

  // Create a simple but deterministic embedding based on text characteristics
  for (let i = 0; i < dimensions; i++) {
    // Use different text features for different dimensions
    if (i < 26) {
      // Character frequency
      const char = String.fromCharCode(97 + i); // a-z
      vector[i] = (normalized.split(char).length - 1) / normalized.length;
    } else if (i < 52) {
      // Bigram features
      const char1 = String.fromCharCode(97 + (i - 26));
      const char2 = String.fromCharCode(97 + ((i - 26 + 1) % 26));
      const bigram = char1 + char2;
      vector[i] = (normalized.split(bigram).length - 1) / (normalized.length - 1);
    } else {
      // Position-based features and length
      vector[i] = Math.sin(i * normalized.length * 0.1) * Math.cos(normalized.charCodeAt(i % normalized.length));
    }
  }

  // Normalize the vector
  const magnitude = Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
  if (magnitude > 0) {
    for (let i = 0; i < dimensions; i++) {
      vector[i] /= magnitude;
    }
  }

  return vector;
}

async function demonstrateVectorSearch() {
  console.log('\n📚 Creating Vector Database...\n');

  // Create vector database with 128 dimensions
  const path = require('path');
  const dbPath = path.join(process.cwd(), 'demos', 'vector-search', 'semantic-db.bin');

  const db = new VectorDB({
    dimensions: 128,
    maxElements: 1000,
    storagePath: dbPath
  });

  console.log('✅ Vector database created with 128 dimensions');
  console.log('📊 Using RuVector (Rust backend) - 150x faster than cloud alternatives\n');

  // Index all documents
  console.log('📝 Indexing documents...\n');

  for (const doc of documents) {
    const fullText = `${doc.title} ${doc.content} ${doc.keywords.join(' ')}`;
    const vector = textToVector(fullText);

    await db.insert({
      id: doc.id,
      vector: vector,
      metadata: {
        title: doc.title,
        content: doc.content,
        category: doc.category,
        keywords: doc.keywords
      }
    });

    console.log(`  ✓ Indexed: ${doc.title} (${doc.category})`);
  }

  console.log(`\n✅ Successfully indexed ${documents.length} documents\n`);
  console.log('=' .repeat(70));

  // Demonstrate semantic search queries
  const queries = [
    'machine learning neural networks',
    'fast similarity search',
    'hierarchical data structures',
    'optimization techniques for AI'
  ];

  console.log('\n🔍 Running Semantic Search Queries...\n');

  for (const query of queries) {
    console.log(`\n📝 Query: "${query}"\n`);

    const queryVector = textToVector(query);
    const startTime = performance.now();
    const results = await db.search({
      vector: queryVector,
      k: 3
    });
    const endTime = performance.now();

    console.log(`⚡ Search completed in ${(endTime - startTime).toFixed(3)}ms\n`);
    console.log('Top 3 Results:');

    for (let index = 0; index < results.length; index++) {
      const result = results[index];
      // Retrieve full entry with metadata
      const entry = await db.get(result.id);

      if (entry && entry.metadata) {
        console.log(`\n  ${index + 1}. ${entry.metadata.title}`);
        console.log(`     Score: ${result.score.toFixed(4)} | Category: ${entry.metadata.category}`);
        console.log(`     ${entry.metadata.content.substring(0, 80)}...`);
      } else {
        console.log(`\n  ${index + 1}. ${result.id}`);
        console.log(`     Score: ${result.score.toFixed(4)}`);
      }
    }

    console.log('\n' + '-'.repeat(70));
  }

  // Demonstrate filtered search
  console.log('\n\n🎯 Filtered Search (AI category only)...\n');

  const techQuery = 'advanced neural architectures';
  const queryVector = textToVector(techQuery);
  const allResults = await db.search({
    vector: queryVector,
    k: 10
  });

  console.log(`📝 Query: "${techQuery}"\n`);
  console.log('Results (filtered for AI category):');

  // Filter for AI category
  let aiCount = 0;
  for (const result of allResults) {
    const entry = await db.get(result.id);
    if (entry && entry.metadata && entry.metadata.category === 'AI') {
      aiCount++;
      console.log(`\n  ${aiCount}. ${entry.metadata.title}`);
      console.log(`     Score: ${result.score.toFixed(4)}`);
      console.log(`     Keywords: ${entry.metadata.keywords.join(', ')}`);

      if (aiCount >= 3) break;
    }
  }

  // Performance statistics
  console.log('\n\n' + '=' .repeat(70));
  console.log('\n📊 Performance Statistics:\n');

  const benchmarkRuns = 100;
  const benchmarkVector = textToVector('test query');
  const benchmarkStart = performance.now();

  for (let i = 0; i < benchmarkRuns; i++) {
    await db.search({
      vector: benchmarkVector,
      k: 5
    });
  }

  const benchmarkEnd = performance.now();
  const avgLatency = (benchmarkEnd - benchmarkStart) / benchmarkRuns;
  const qps = 1000 / avgLatency;

  console.log(`  Average Search Latency: ${avgLatency.toFixed(3)}ms`);
  console.log(`  Queries per Second: ${qps.toFixed(0)}`);
  console.log(`  Total Documents: ${documents.length}`);
  console.log(`  Vector Dimensions: 128`);
  console.log(`  Implementation: RuVector (Native Rust)`);

  console.log('\n✅ Vector Search Demonstration Complete!\n');
  console.log('=' .repeat(70));
}

// Run the demonstration
demonstrateVectorSearch().catch(error => {
  console.error('\n❌ Error:', error);
  process.exit(1);
});

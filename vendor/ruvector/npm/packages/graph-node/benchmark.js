/**
 * RuVector Graph Node Benchmark
 *
 * Tests performance of graph operations including:
 * - Node creation
 * - Edge creation
 * - Hyperedge creation
 * - Batch inserts
 * - Vector similarity search
 * - k-hop neighbor traversal
 * - Cypher queries
 */

const { GraphDatabase, version } = require('./index.js');

const DIMENSIONS = 384;
const NUM_NODES = 10000;
const NUM_EDGES = 50000;
const NUM_HYPEREDGES = 5000;
const SEARCH_K = 10;

function randomEmbedding(dims) {
  const arr = new Float32Array(dims);
  for (let i = 0; i < dims; i++) {
    arr[i] = Math.random();
  }
  return arr;
}

function formatTime(ms) {
  if (ms < 1) return `${(ms * 1000).toFixed(2)}μs`;
  if (ms < 1000) return `${ms.toFixed(2)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

function formatOps(count, ms) {
  const ops = (count / ms) * 1000;
  if (ops >= 1000000) return `${(ops / 1000000).toFixed(2)}M ops/sec`;
  if (ops >= 1000) return `${(ops / 1000).toFixed(2)}K ops/sec`;
  return `${ops.toFixed(2)} ops/sec`;
}

async function benchmark() {
  console.log('╔════════════════════════════════════════════════════════════════╗');
  console.log('║          RuVector Graph Node Benchmark Suite                   ║');
  console.log('╠════════════════════════════════════════════════════════════════╣');
  console.log(`║ Version: ${version().padEnd(54)}║`);
  console.log(`║ Dimensions: ${DIMENSIONS.toString().padEnd(51)}║`);
  console.log(`║ Nodes: ${NUM_NODES.toLocaleString().padEnd(56)}║`);
  console.log(`║ Edges: ${NUM_EDGES.toLocaleString().padEnd(56)}║`);
  console.log(`║ Hyperedges: ${NUM_HYPEREDGES.toLocaleString().padEnd(51)}║`);
  console.log('╚════════════════════════════════════════════════════════════════╝\n');

  const db = new GraphDatabase({
    distanceMetric: 'Cosine',
    dimensions: DIMENSIONS
  });

  const results = [];

  // Benchmark 1: Node Creation
  console.log('📌 Benchmark 1: Individual Node Creation');
  const nodeCount = 1000;
  const nodeStart = performance.now();
  for (let i = 0; i < nodeCount; i++) {
    await db.createNode({
      id: `node_${i}`,
      embedding: randomEmbedding(DIMENSIONS),
      labels: ['TestNode'],
      properties: { index: String(i) }
    });
  }
  const nodeEnd = performance.now();
  const nodeTime = nodeEnd - nodeStart;
  console.log(`   Created ${nodeCount} nodes in ${formatTime(nodeTime)}`);
  console.log(`   Throughput: ${formatOps(nodeCount, nodeTime)}\n`);
  results.push({ name: 'Node Creation', count: nodeCount, time: nodeTime });

  // Benchmark 2: Batch Node Creation
  console.log('📌 Benchmark 2: Batch Node Creation');
  const batchSize = 1000;
  const batchNodes = [];
  for (let i = 0; i < batchSize; i++) {
    batchNodes.push({
      id: `batch_node_${i}`,
      embedding: randomEmbedding(DIMENSIONS),
      labels: ['BatchNode']
    });
  }
  const batchNodeStart = performance.now();
  await db.batchInsert({ nodes: batchNodes, edges: [] });
  const batchNodeEnd = performance.now();
  const batchNodeTime = batchNodeEnd - batchNodeStart;
  console.log(`   Inserted ${batchSize} nodes in ${formatTime(batchNodeTime)}`);
  console.log(`   Throughput: ${formatOps(batchSize, batchNodeTime)}\n`);
  results.push({ name: 'Batch Node Creation', count: batchSize, time: batchNodeTime });

  // Benchmark 3: Edge Creation
  console.log('📌 Benchmark 3: Edge Creation');
  const edgeCount = 1000;
  const edgeStart = performance.now();
  for (let i = 0; i < edgeCount; i++) {
    const from = `node_${i % nodeCount}`;
    const to = `node_${(i + 1) % nodeCount}`;
    await db.createEdge({
      from,
      to,
      description: 'CONNECTED_TO',
      embedding: randomEmbedding(DIMENSIONS),
      confidence: Math.random()
    });
  }
  const edgeEnd = performance.now();
  const edgeTime = edgeEnd - edgeStart;
  console.log(`   Created ${edgeCount} edges in ${formatTime(edgeTime)}`);
  console.log(`   Throughput: ${formatOps(edgeCount, edgeTime)}\n`);
  results.push({ name: 'Edge Creation', count: edgeCount, time: edgeTime });

  // Benchmark 4: Hyperedge Creation
  console.log('📌 Benchmark 4: Hyperedge Creation');
  const hyperedgeCount = 500;
  const hyperedgeStart = performance.now();
  for (let i = 0; i < hyperedgeCount; i++) {
    const nodes = [];
    const numNodes = 3 + Math.floor(Math.random() * 5); // 3-7 nodes per hyperedge
    for (let j = 0; j < numNodes; j++) {
      nodes.push(`node_${(i + j) % nodeCount}`);
    }
    await db.createHyperedge({
      nodes,
      description: `RELATIONSHIP_${i}`,
      embedding: randomEmbedding(DIMENSIONS),
      confidence: Math.random()
    });
  }
  const hyperedgeEnd = performance.now();
  const hyperedgeTime = hyperedgeEnd - hyperedgeStart;
  console.log(`   Created ${hyperedgeCount} hyperedges in ${formatTime(hyperedgeTime)}`);
  console.log(`   Throughput: ${formatOps(hyperedgeCount, hyperedgeTime)}\n`);
  results.push({ name: 'Hyperedge Creation', count: hyperedgeCount, time: hyperedgeTime });

  // Benchmark 5: Vector Similarity Search
  console.log('📌 Benchmark 5: Vector Similarity Search');
  const searchCount = 100;
  const searchStart = performance.now();
  for (let i = 0; i < searchCount; i++) {
    await db.searchHyperedges({
      embedding: randomEmbedding(DIMENSIONS),
      k: SEARCH_K
    });
  }
  const searchEnd = performance.now();
  const searchTime = searchEnd - searchStart;
  console.log(`   Performed ${searchCount} searches (k=${SEARCH_K}) in ${formatTime(searchTime)}`);
  console.log(`   Throughput: ${formatOps(searchCount, searchTime)}\n`);
  results.push({ name: 'Vector Search', count: searchCount, time: searchTime });

  // Benchmark 6: k-hop Neighbor Traversal
  console.log('📌 Benchmark 6: k-hop Neighbor Traversal');
  const traversalCount = 100;
  const traversalStart = performance.now();
  for (let i = 0; i < traversalCount; i++) {
    await db.kHopNeighbors(`node_${i % nodeCount}`, 2);
  }
  const traversalEnd = performance.now();
  const traversalTime = traversalEnd - traversalStart;
  console.log(`   Performed ${traversalCount} 2-hop traversals in ${formatTime(traversalTime)}`);
  console.log(`   Throughput: ${formatOps(traversalCount, traversalTime)}\n`);
  results.push({ name: 'k-hop Traversal', count: traversalCount, time: traversalTime });

  // Benchmark 7: Statistics Query
  console.log('📌 Benchmark 7: Statistics Query');
  const statsCount = 1000;
  const statsStart = performance.now();
  for (let i = 0; i < statsCount; i++) {
    await db.stats();
  }
  const statsEnd = performance.now();
  const statsTime = statsEnd - statsStart;
  console.log(`   Performed ${statsCount} stats queries in ${formatTime(statsTime)}`);
  console.log(`   Throughput: ${formatOps(statsCount, statsTime)}\n`);
  results.push({ name: 'Stats Query', count: statsCount, time: statsTime });

  // Benchmark 8: Transaction Overhead
  console.log('📌 Benchmark 8: Transaction Overhead');
  const txCount = 100;
  const txStart = performance.now();
  for (let i = 0; i < txCount; i++) {
    const txId = await db.begin();
    await db.commit(txId);
  }
  const txEnd = performance.now();
  const txTime = txEnd - txStart;
  console.log(`   Performed ${txCount} transactions in ${formatTime(txTime)}`);
  console.log(`   Throughput: ${formatOps(txCount, txTime)}\n`);
  results.push({ name: 'Transaction', count: txCount, time: txTime });

  // Summary
  console.log('╔════════════════════════════════════════════════════════════════╗');
  console.log('║                      BENCHMARK SUMMARY                         ║');
  console.log('╠════════════════════════════════════════════════════════════════╣');
  for (const r of results) {
    const ops = formatOps(r.count, r.time);
    console.log(`║ ${r.name.padEnd(25)} ${ops.padStart(20)}  ${formatTime(r.time).padStart(12)} ║`);
  }
  console.log('╚════════════════════════════════════════════════════════════════╝');

  // Final stats
  const finalStats = await db.stats();
  console.log(`\n📊 Final Database State:`);
  console.log(`   Total Nodes: ${finalStats.totalNodes.toLocaleString()}`);
  console.log(`   Total Edges: ${finalStats.totalEdges.toLocaleString()}`);
  console.log(`   Avg Degree: ${finalStats.avgDegree.toFixed(4)}`);
}

benchmark().catch(console.error);

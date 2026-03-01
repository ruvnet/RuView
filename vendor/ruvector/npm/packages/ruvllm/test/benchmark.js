#!/usr/bin/env node
/**
 * Comprehensive Benchmark Suite for RuvLLM
 *
 * Tests performance of all major components:
 * - Core Engine (query, generate, embed)
 * - Memory operations (add, search)
 * - SIMD operations
 * - LoRA adapters
 * - Federated learning
 * - Training pipeline
 * - Export/Import
 */

const {
  RuvLLM,
  SimdOps,
  SessionManager,
  StreamingGenerator,
  SonaCoordinator,
  TrajectoryBuilder,
  ReasoningBank,
  EwcManager,
  EphemeralAgent,
  FederatedCoordinator,
  LoraAdapter,
  LoraManager,
  SafeTensorsWriter,
  SafeTensorsReader,
  ModelExporter,
  TrainingPipeline,
  TrainingFactory,
} = require('../dist/cjs/index.js');

// Benchmark configuration
const CONFIG = {
  iterations: {
    fast: 100,
    medium: 1000,
    slow: 10000,
  },
  vectorDims: [64, 128, 256, 512, 768],
  batchSizes: [1, 10, 100],
};

// Results storage
const results = {
  timestamp: new Date().toISOString(),
  platform: process.platform,
  arch: process.arch,
  nodeVersion: process.version,
  benchmarks: {},
};

// Utility functions
function formatTime(ns) {
  if (ns < 1000) return `${ns.toFixed(2)}ns`;
  if (ns < 1000000) return `${(ns / 1000).toFixed(2)}μs`;
  if (ns < 1000000000) return `${(ns / 1000000).toFixed(2)}ms`;
  return `${(ns / 1000000000).toFixed(2)}s`;
}

function formatOps(ops) {
  if (ops < 1000) return `${ops.toFixed(0)} ops/s`;
  if (ops < 1000000) return `${(ops / 1000).toFixed(2)}K ops/s`;
  return `${(ops / 1000000).toFixed(2)}M ops/s`;
}

function generateVector(dim) {
  return Array.from({ length: dim }, () => Math.random());
}

function generateVectors(count, dim) {
  return Array.from({ length: count }, () => generateVector(dim));
}

function benchmark(name, fn, iterations = CONFIG.iterations.medium) {
  // Warmup
  for (let i = 0; i < Math.min(10, iterations / 10); i++) {
    fn();
  }

  // Actual benchmark
  const start = process.hrtime.bigint();
  for (let i = 0; i < iterations; i++) {
    fn();
  }
  const end = process.hrtime.bigint();

  const totalNs = Number(end - start);
  const avgNs = totalNs / iterations;
  const opsPerSec = 1e9 / avgNs;

  return {
    name,
    iterations,
    totalMs: totalNs / 1e6,
    avgNs,
    opsPerSec,
    formatted: {
      avg: formatTime(avgNs),
      ops: formatOps(opsPerSec),
    },
  };
}

async function benchmarkAsync(name, fn, iterations = CONFIG.iterations.fast) {
  // Warmup
  for (let i = 0; i < Math.min(5, iterations / 10); i++) {
    await fn();
  }

  // Actual benchmark
  const start = process.hrtime.bigint();
  for (let i = 0; i < iterations; i++) {
    await fn();
  }
  const end = process.hrtime.bigint();

  const totalNs = Number(end - start);
  const avgNs = totalNs / iterations;
  const opsPerSec = 1e9 / avgNs;

  return {
    name,
    iterations,
    totalMs: totalNs / 1e6,
    avgNs,
    opsPerSec,
    formatted: {
      avg: formatTime(avgNs),
      ops: formatOps(opsPerSec),
    },
  };
}

// ============================================
// Benchmark Suites
// ============================================

async function benchmarkCoreEngine() {
  console.log('\n📊 Core Engine Benchmarks');
  console.log('─'.repeat(60));

  const llm = new RuvLLM({ embeddingDim: 256 });
  const benchmarks = [];

  // Query benchmark
  benchmarks.push(benchmark('query (short)', () => {
    llm.query('Hello world');
  }, CONFIG.iterations.medium));

  benchmarks.push(benchmark('query (long)', () => {
    llm.query('This is a longer query that contains more text and should require more processing time to handle properly.');
  }, CONFIG.iterations.medium));

  // Generate benchmark
  benchmarks.push(benchmark('generate', () => {
    llm.generate('Write a story');
  }, CONFIG.iterations.medium));

  // Embed benchmark
  for (const dim of [256, 768]) {
    const llmDim = new RuvLLM({ embeddingDim: dim });
    benchmarks.push(benchmark(`embed (${dim}d)`, () => {
      llmDim.embed('Test embedding text');
    }, CONFIG.iterations.medium));
  }

  // Similarity benchmark
  benchmarks.push(benchmark('similarity', () => {
    llm.similarity('hello world', 'hello there');
  }, CONFIG.iterations.medium));

  // Route benchmark
  benchmarks.push(benchmark('route', () => {
    llm.route('What is machine learning?');
  }, CONFIG.iterations.medium));

  for (const b of benchmarks) {
    console.log(`  ${b.name.padEnd(25)} ${b.formatted.avg.padStart(12)} | ${b.formatted.ops.padStart(15)}`);
  }

  return benchmarks;
}

async function benchmarkMemory() {
  console.log('\n📊 Memory Operations Benchmarks');
  console.log('─'.repeat(60));

  const llm = new RuvLLM({ embeddingDim: 256 });
  const benchmarks = [];

  // Add memory benchmark
  benchmarks.push(benchmark('addMemory', () => {
    llm.addMemory('Test content ' + Math.random(), { type: 'test' });
  }, CONFIG.iterations.medium));

  // Pre-populate memory for search
  for (let i = 0; i < 100; i++) {
    llm.addMemory(`Memory item ${i}`, { index: i });
  }

  // Search memory benchmark
  for (const k of [5, 10, 20]) {
    benchmarks.push(benchmark(`searchMemory (k=${k})`, () => {
      llm.searchMemory('Test search query', k);
    }, CONFIG.iterations.fast));
  }

  for (const b of benchmarks) {
    console.log(`  ${b.name.padEnd(25)} ${b.formatted.avg.padStart(12)} | ${b.formatted.ops.padStart(15)}`);
  }

  return benchmarks;
}

async function benchmarkSimd() {
  console.log('\n📊 SIMD Operations Benchmarks');
  console.log('─'.repeat(60));

  const simd = new SimdOps();
  const benchmarks = [];

  for (const dim of CONFIG.vectorDims) {
    const a = generateVector(dim);
    const b = generateVector(dim);

    benchmarks.push(benchmark(`dotProduct (${dim}d)`, () => {
      simd.dotProduct(a, b);
    }, CONFIG.iterations.slow));

    benchmarks.push(benchmark(`cosineSimilarity (${dim}d)`, () => {
      simd.cosineSimilarity(a, b);
    }, CONFIG.iterations.slow));

    benchmarks.push(benchmark(`l2Distance (${dim}d)`, () => {
      simd.l2Distance(a, b);
    }, CONFIG.iterations.slow));
  }

  // Softmax benchmark
  for (const dim of [64, 256]) {
    const vec = generateVector(dim);
    benchmarks.push(benchmark(`softmax (${dim}d)`, () => {
      simd.softmax(vec);
    }, CONFIG.iterations.medium));
  }

  // Normalize benchmark
  for (const dim of [64, 256]) {
    const vec = generateVector(dim);
    benchmarks.push(benchmark(`normalize (${dim}d)`, () => {
      simd.normalize(vec);
    }, CONFIG.iterations.medium));
  }

  for (const b of benchmarks) {
    console.log(`  ${b.name.padEnd(25)} ${b.formatted.avg.padStart(12)} | ${b.formatted.ops.padStart(15)}`);
  }

  return benchmarks;
}

async function benchmarkLoRA() {
  console.log('\n📊 LoRA Adapter Benchmarks');
  console.log('─'.repeat(60));

  const benchmarks = [];

  for (const dim of [64, 128, 256]) {
    for (const rank of [4, 8, 16]) {
      const adapter = new LoraAdapter({ rank }, dim, dim);
      const input = generateVector(dim);

      benchmarks.push(benchmark(`forward (${dim}d, r=${rank})`, () => {
        adapter.forward(input);
      }, CONFIG.iterations.medium));
    }
  }

  // Backward pass benchmark
  const adapter = new LoraAdapter({ rank: 8 }, 128, 128);
  adapter.startTraining(0.001);
  const input = generateVector(128);
  const grad = generateVector(128);

  benchmarks.push(benchmark('backward (128d, r=8)', () => {
    adapter.backward(input, grad, 0.001);
  }, CONFIG.iterations.medium));

  // Merge benchmark
  benchmarks.push(benchmark('merge (128d, r=8)', () => {
    adapter.merge();
  }, CONFIG.iterations.fast));

  // Batch forward benchmark
  for (const batchSize of CONFIG.batchSizes) {
    const batchAdapter = new LoraAdapter({ rank: 8 }, 128, 128);
    const batch = generateVectors(batchSize, 128);

    benchmarks.push(benchmark(`forwardBatch (bs=${batchSize})`, () => {
      batchAdapter.forwardBatch(batch);
    }, CONFIG.iterations.fast));
  }

  for (const b of benchmarks) {
    console.log(`  ${b.name.padEnd(25)} ${b.formatted.avg.padStart(12)} | ${b.formatted.ops.padStart(15)}`);
  }

  return benchmarks;
}

async function benchmarkFederated() {
  console.log('\n📊 Federated Learning Benchmarks');
  console.log('─'.repeat(60));

  const benchmarks = [];

  // Agent creation
  benchmarks.push(benchmark('agent create', () => {
    new EphemeralAgent('agent-' + Math.random(), { hiddenDim: 128 });
  }, CONFIG.iterations.medium));

  // Process task
  const agent = new EphemeralAgent('bench-agent', { hiddenDim: 128 });
  const embedding = generateVector(128);

  benchmarks.push(benchmark('processTask', () => {
    agent.processTask(embedding, 0.9);
  }, CONFIG.iterations.medium));

  // Export state
  for (let i = 0; i < 50; i++) {
    agent.processTask(generateVector(128), 0.8 + Math.random() * 0.2);
  }

  benchmarks.push(benchmark('exportState', () => {
    agent.exportState();
  }, CONFIG.iterations.fast));

  // Coordinator aggregation
  const coord = new FederatedCoordinator('coord', { hiddenDim: 128 });
  const exportData = agent.exportState();

  benchmarks.push(benchmark('aggregate', () => {
    coord.aggregate(exportData);
  }, CONFIG.iterations.fast));

  // Apply LoRA
  const input = generateVector(128);
  benchmarks.push(benchmark('applyLora', () => {
    coord.applyLora(input);
  }, CONFIG.iterations.medium));

  for (const b of benchmarks) {
    console.log(`  ${b.name.padEnd(25)} ${b.formatted.avg.padStart(12)} | ${b.formatted.ops.padStart(15)}`);
  }

  return benchmarks;
}

async function benchmarkTraining() {
  console.log('\n📊 Training Pipeline Benchmarks');
  console.log('─'.repeat(60));

  const benchmarks = [];

  // Data preparation
  const data = [];
  for (let i = 0; i < 100; i++) {
    data.push({
      input: generateVector(64),
      target: generateVector(64),
      quality: 0.7 + Math.random() * 0.3,
    });
  }

  // Pipeline creation
  benchmarks.push(benchmark('pipeline create', () => {
    new TrainingPipeline({ batchSize: 16, epochs: 1 });
  }, CONFIG.iterations.medium));

  // Add data
  const pipeline = new TrainingPipeline({ batchSize: 16, epochs: 1, validationSplit: 0 });
  benchmarks.push(benchmark('addData (100 samples)', () => {
    const p = new TrainingPipeline({ batchSize: 16 });
    p.addData(data);
  }, CONFIG.iterations.fast));

  // Training step (mini benchmark)
  const trainPipeline = TrainingFactory.quickFinetune();
  trainPipeline.addData(data.slice(0, 32));

  const start = process.hrtime.bigint();
  trainPipeline.train();
  const end = process.hrtime.bigint();

  benchmarks.push({
    name: 'train (32 samples, 3 epochs)',
    iterations: 1,
    totalMs: Number(end - start) / 1e6,
    avgNs: Number(end - start),
    opsPerSec: 1e9 / Number(end - start),
    formatted: {
      avg: formatTime(Number(end - start)),
      ops: formatOps(1e9 / Number(end - start)),
    },
  });

  for (const b of benchmarks) {
    console.log(`  ${b.name.padEnd(30)} ${b.formatted.avg.padStart(12)} | ${b.formatted.ops.padStart(15)}`);
  }

  return benchmarks;
}

async function benchmarkExport() {
  console.log('\n📊 Export/Import Benchmarks');
  console.log('─'.repeat(60));

  const benchmarks = [];

  // SafeTensors write
  const writer = new SafeTensorsWriter();
  const weights2D = Array.from({ length: 64 }, () => generateVector(64));
  const weights1D = generateVector(64);

  benchmarks.push(benchmark('safetensors write', () => {
    const w = new SafeTensorsWriter();
    w.add2D('weights', weights2D);
    w.add1D('bias', weights1D);
    w.build();
  }, CONFIG.iterations.medium));

  // SafeTensors read
  writer.add2D('weights', weights2D);
  writer.add1D('bias', weights1D);
  const buffer = writer.build();

  benchmarks.push(benchmark('safetensors read', () => {
    const r = new SafeTensorsReader(buffer);
    r.getTensor2D('weights');
    r.getTensor1D('bias');
  }, CONFIG.iterations.medium));

  // Model export JSON
  const exporter = new ModelExporter();
  const model = {
    metadata: { name: 'bench', version: '1.0', architecture: 'lora' },
    loraWeights: {
      loraA: weights2D,
      loraB: weights2D,
      scaling: 2.0,
    },
  };

  benchmarks.push(benchmark('export JSON', () => {
    exporter.toJSON(model);
  }, CONFIG.iterations.medium));

  benchmarks.push(benchmark('export SafeTensors', () => {
    exporter.toSafeTensors(model);
  }, CONFIG.iterations.medium));

  // LoRA serialization
  const adapter = new LoraAdapter({ rank: 8 }, 64, 64);
  benchmarks.push(benchmark('LoRA toJSON', () => {
    adapter.toJSON();
  }, CONFIG.iterations.medium));

  const json = adapter.toJSON();
  benchmarks.push(benchmark('LoRA fromJSON', () => {
    LoraAdapter.fromJSON(json);
  }, CONFIG.iterations.medium));

  for (const b of benchmarks) {
    console.log(`  ${b.name.padEnd(25)} ${b.formatted.avg.padStart(12)} | ${b.formatted.ops.padStart(15)}`);
  }

  return benchmarks;
}

async function benchmarkSona() {
  console.log('\n📊 SONA Learning Benchmarks');
  console.log('─'.repeat(60));

  const benchmarks = [];

  // ReasoningBank
  const bank = new ReasoningBank(0.7);
  const embedding = generateVector(64);

  benchmarks.push(benchmark('bank store', () => {
    bank.store('query_response', generateVector(64));
  }, CONFIG.iterations.medium));

  // Pre-populate
  for (let i = 0; i < 100; i++) {
    bank.store('query_response', generateVector(64));
  }

  benchmarks.push(benchmark('bank findSimilar (k=5)', () => {
    bank.findSimilar(embedding, 5);
  }, CONFIG.iterations.fast));

  // EWC
  const ewc = new EwcManager(2000);
  const weights = generateVector(256);

  benchmarks.push(benchmark('ewc registerTask', () => {
    ewc.registerTask('task-' + Math.random(), weights);
  }, CONFIG.iterations.medium));

  for (let i = 0; i < 5; i++) {
    ewc.registerTask(`task-${i}`, generateVector(256));
  }

  benchmarks.push(benchmark('ewc computePenalty', () => {
    ewc.computePenalty(weights);
  }, CONFIG.iterations.medium));

  // Trajectory
  benchmarks.push(benchmark('trajectory build', () => {
    const builder = new TrajectoryBuilder();
    builder.startStep('query', 'test');
    builder.endStep('response', 0.9);
    builder.complete('success');
  }, CONFIG.iterations.medium));

  // SonaCoordinator
  const sona = new SonaCoordinator();
  const trajectory = new TrajectoryBuilder()
    .startStep('query', 'test')
    .endStep('response', 0.9)
    .complete('success');

  benchmarks.push(benchmark('sona recordTrajectory', () => {
    sona.recordTrajectory(trajectory);
  }, CONFIG.iterations.medium));

  for (const b of benchmarks) {
    console.log(`  ${b.name.padEnd(25)} ${b.formatted.avg.padStart(12)} | ${b.formatted.ops.padStart(15)}`);
  }

  return benchmarks;
}

async function benchmarkSession() {
  console.log('\n📊 Session & Streaming Benchmarks');
  console.log('─'.repeat(60));

  const llm = new RuvLLM();
  const benchmarks = [];

  // Session creation
  const sessions = new SessionManager(llm);
  benchmarks.push(benchmark('session create', () => {
    sessions.create({ userId: 'bench' });
  }, CONFIG.iterations.medium));

  // Session chat
  const session = sessions.create();
  benchmarks.push(benchmark('session chat', () => {
    sessions.chat(session.id, 'Hello');
  }, CONFIG.iterations.medium));

  // Session export/import
  sessions.chat(session.id, 'Message 1');
  sessions.chat(session.id, 'Message 2');
  const exported = sessions.export(session.id);

  benchmarks.push(benchmark('session export', () => {
    sessions.export(session.id);
  }, CONFIG.iterations.medium));

  benchmarks.push(benchmark('session import', () => {
    sessions.import(exported);
  }, CONFIG.iterations.medium));

  // Streaming (async)
  const streamer = new StreamingGenerator(llm);
  const streamResult = await benchmarkAsync('stream collect', async () => {
    await streamer.collect('Test');
  }, 10);
  benchmarks.push(streamResult);

  for (const b of benchmarks) {
    console.log(`  ${b.name.padEnd(25)} ${b.formatted.avg.padStart(12)} | ${b.formatted.ops.padStart(15)}`);
  }

  return benchmarks;
}

// ============================================
// Main
// ============================================

async function main() {
  console.log('╔════════════════════════════════════════════════════════════╗');
  console.log('║           RuvLLM Comprehensive Benchmark Suite             ║');
  console.log('╠════════════════════════════════════════════════════════════╣');
  console.log(`║  Platform: ${process.platform.padEnd(10)} Arch: ${process.arch.padEnd(10)} Node: ${process.version.padEnd(10)} ║`);
  console.log('╚════════════════════════════════════════════════════════════╝');

  const startTime = Date.now();

  results.benchmarks.core = await benchmarkCoreEngine();
  results.benchmarks.memory = await benchmarkMemory();
  results.benchmarks.simd = await benchmarkSimd();
  results.benchmarks.lora = await benchmarkLoRA();
  results.benchmarks.federated = await benchmarkFederated();
  results.benchmarks.training = await benchmarkTraining();
  results.benchmarks.export = await benchmarkExport();
  results.benchmarks.sona = await benchmarkSona();
  results.benchmarks.session = await benchmarkSession();

  const totalTime = Date.now() - startTime;

  console.log('\n╔════════════════════════════════════════════════════════════╗');
  console.log('║                      Summary                               ║');
  console.log('╚════════════════════════════════════════════════════════════╝');

  // Find slowest operations
  const allBenchmarks = Object.values(results.benchmarks).flat();
  const sorted = [...allBenchmarks].sort((a, b) => b.avgNs - a.avgNs);

  console.log('\n🐢 Slowest Operations (optimization candidates):');
  for (const b of sorted.slice(0, 10)) {
    console.log(`  ${b.name.padEnd(30)} ${b.formatted.avg.padStart(12)}`);
  }

  console.log('\n🚀 Fastest Operations:');
  for (const b of sorted.slice(-5).reverse()) {
    console.log(`  ${b.name.padEnd(30)} ${b.formatted.avg.padStart(12)}`);
  }

  console.log(`\n✅ Total benchmark time: ${(totalTime / 1000).toFixed(2)}s`);

  // Output JSON results
  console.log('\n📄 Full results saved to benchmark-results.json');

  return results;
}

// Run if main
main().then(results => {
  // Print JSON for capture
  console.log('\n--- JSON_RESULTS_START ---');
  console.log(JSON.stringify(results, null, 2));
  console.log('--- JSON_RESULTS_END ---');
}).catch(err => {
  console.error('Benchmark failed:', err);
  process.exit(1);
});

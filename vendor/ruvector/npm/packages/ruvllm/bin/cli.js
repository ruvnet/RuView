#!/usr/bin/env node
/**
 * RuvLLM CLI - Self-learning LLM orchestration
 *
 * Usage:
 *   ruvllm query "What is machine learning?"
 *   ruvllm generate "Write a haiku about AI"
 *   ruvllm memory add "Important context"
 *   ruvllm memory search "context"
 *   ruvllm models list
 *   ruvllm models download claude-code
 *   ruvllm stats
 *   ruvllm benchmark
 */

const { RuvLLM, SimdOps, version, hasSimdSupport, ModelDownloader, listModels, getModelInfo, RUVLTRA_MODELS, getDefaultModelsDir, runRoutingBenchmark, formatRoutingResults, baselineKeywordRouter, runEmbeddingBenchmark, formatEmbeddingResults, runFullBenchmark, formatFullResults, ROUTING_TEST_CASES, runFullComparison, formatComparisonResults, ContrastiveTrainer, AGENT_TRAINING_DATA, generateTrainingDataset, generateContrastivePairs, getDatasetStats, tripletLoss, cosineSimilarity } = require('../dist/cjs/index.js');

const args = process.argv.slice(2);
const command = args[0];

// Parse CLI arguments
function parseArgs(args) {
  const result = { flags: {}, positional: [] };
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg.startsWith('--')) {
      const key = arg.slice(2);
      const nextArg = args[i + 1];
      if (nextArg && !nextArg.startsWith('--')) {
        result.flags[key] = nextArg;
        i++;
      } else {
        result.flags[key] = true;
      }
    } else if (!result.command) {
      result.command = arg;
    } else {
      result.positional.push(arg);
    }
  }
  return result;
}

// Format output
function formatJson(obj) {
  return JSON.stringify(obj, null, 2);
}

function formatTable(data) {
  const maxKeyLen = Math.max(...Object.keys(data).map(k => k.length));
  return Object.entries(data)
    .map(([k, v]) => `  ${k.padEnd(maxKeyLen)} : ${v}`)
    .join('\n');
}

// Commands
async function runQuery(llm, text, flags) {
  const config = {};
  if (flags.temperature) config.temperature = parseFloat(flags.temperature);
  if (flags['max-tokens']) config.maxTokens = parseInt(flags['max-tokens']);
  if (flags['top-p']) config.topP = parseFloat(flags['top-p']);
  if (flags['top-k']) config.topK = parseInt(flags['top-k']);

  const response = llm.query(text, config);

  if (flags.json) {
    console.log(formatJson(response));
  } else {
    console.log('\n' + response.text);
    console.log(`\n--- Model: ${response.model} | Confidence: ${(response.confidence * 100).toFixed(1)}% | Latency: ${response.latencyMs.toFixed(2)}ms ---`);
  }
}

async function runGenerate(llm, prompt, flags) {
  const config = {};
  if (flags.temperature) config.temperature = parseFloat(flags.temperature);
  if (flags['max-tokens']) config.maxTokens = parseInt(flags['max-tokens']);
  if (flags['top-p']) config.topP = parseFloat(flags['top-p']);

  const text = llm.generate(prompt, config);
  console.log(text);
}

async function runMemoryAdd(llm, content, flags) {
  const metadata = flags.metadata ? JSON.parse(flags.metadata) : undefined;
  const id = llm.addMemory(content, metadata);
  console.log(`Added memory with ID: ${id}`);
}

async function runMemorySearch(llm, query, flags) {
  const k = flags.k ? parseInt(flags.k) : 10;
  const results = llm.searchMemory(query, k);

  if (flags.json) {
    console.log(formatJson(results));
  } else {
    if (results.length === 0) {
      console.log('No results found.');
      return;
    }
    results.forEach((r, i) => {
      console.log(`\n[${i + 1}] Score: ${r.score.toFixed(4)} | ID: ${r.id}`);
      console.log(`    ${r.content.slice(0, 100)}${r.content.length > 100 ? '...' : ''}`);
    });
  }
}

async function runStats(llm, flags) {
  const stats = llm.stats();

  if (flags.json) {
    console.log(formatJson(stats));
  } else {
    console.log('\nRuvLLM Statistics:');
    console.log(formatTable({
      'Total Queries': stats.totalQueries,
      'Memory Nodes': stats.memoryNodes,
      'Patterns Learned': stats.patternsLearned,
      'Avg Latency': `${stats.avgLatencyMs.toFixed(2)}ms`,
      'Cache Hit Rate': `${(stats.cacheHitRate * 100).toFixed(1)}%`,
      'Router Accuracy': `${(stats.routerAccuracy * 100).toFixed(1)}%`,
    }));
  }
}

async function runRoute(llm, text, flags) {
  const decision = llm.route(text);

  if (flags.json) {
    console.log(formatJson(decision));
  } else {
    console.log('\nRouting Decision:');
    console.log(formatTable({
      'Model': decision.model,
      'Context Size': decision.contextSize,
      'Temperature': decision.temperature.toFixed(2),
      'Top-P': decision.topP.toFixed(2),
      'Confidence': `${(decision.confidence * 100).toFixed(1)}%`,
    }));
  }
}

async function runEmbed(llm, text, flags) {
  const embedding = llm.embed(text);

  if (flags.json) {
    console.log(formatJson({ embedding, dimensions: embedding.length }));
  } else {
    console.log(`Embedding (${embedding.length} dimensions):`);
    console.log(`  First 10: [${embedding.slice(0, 10).map(x => x.toFixed(4)).join(', ')}...]`);
    console.log(`  Norm: ${Math.sqrt(embedding.reduce((s, x) => s + x * x, 0)).toFixed(4)}`);
  }
}

async function runSimilarity(llm, text1, text2, flags) {
  const score = llm.similarity(text1, text2);

  if (flags.json) {
    console.log(formatJson({ text1, text2, similarity: score }));
  } else {
    console.log(`Similarity: ${(score * 100).toFixed(2)}%`);
  }
}

async function runBenchmark(flags) {
  const simd = new SimdOps();
  const dims = flags.dims ? parseInt(flags.dims) : 768;
  const iterations = flags.iterations ? parseInt(flags.iterations) : 1000;

  // Generate test vectors
  const a = Array.from({ length: dims }, () => Math.random());
  const b = Array.from({ length: dims }, () => Math.random());

  console.log(`\nBenchmark: ${dims} dimensions, ${iterations} iterations`);
  console.log(`SIMD: ${simd.isNative() ? 'Native' : 'JavaScript fallback'}`);
  console.log(`Capabilities: ${simd.capabilities().join(', ')}`);
  console.log('');

  // Dot product benchmark
  let start = Date.now();
  for (let i = 0; i < iterations; i++) {
    simd.dotProduct(a, b);
  }
  let elapsed = Date.now() - start;
  console.log(`Dot Product:        ${elapsed}ms (${(iterations / elapsed * 1000).toFixed(0)} ops/sec)`);

  // Cosine similarity benchmark
  start = Date.now();
  for (let i = 0; i < iterations; i++) {
    simd.cosineSimilarity(a, b);
  }
  elapsed = Date.now() - start;
  console.log(`Cosine Similarity:  ${elapsed}ms (${(iterations / elapsed * 1000).toFixed(0)} ops/sec)`);

  // L2 distance benchmark
  start = Date.now();
  for (let i = 0; i < iterations; i++) {
    simd.l2Distance(a, b);
  }
  elapsed = Date.now() - start;
  console.log(`L2 Distance:        ${elapsed}ms (${(iterations / elapsed * 1000).toFixed(0)} ops/sec)`);

  // Softmax benchmark
  start = Date.now();
  for (let i = 0; i < iterations; i++) {
    simd.softmax(a);
  }
  elapsed = Date.now() - start;
  console.log(`Softmax:            ${elapsed}ms (${(iterations / elapsed * 1000).toFixed(0)} ops/sec)`);
}

async function runInfo(flags) {
  const llm = new RuvLLM();

  const info = {
    version: version(),
    native: llm.isNativeLoaded(),
    simd: hasSimdSupport(),
    capabilities: llm.simdCapabilities(),
    platform: process.platform,
    arch: process.arch,
    nodeVersion: process.version,
  };

  if (flags.json) {
    console.log(formatJson(info));
  } else {
    console.log('\nRuvLLM Info:');
    console.log(formatTable({
      'Version': info.version,
      'Native Module': info.native ? 'Loaded' : 'Fallback (JS)',
      'SIMD Support': info.simd ? 'Yes' : 'No',
      'Capabilities': info.capabilities.join(', '),
      'Platform': `${info.platform}-${info.arch}`,
      'Node.js': info.nodeVersion,
    }));
  }
}

// Model management commands
async function runModelsList(flags) {
  const downloader = new ModelDownloader();
  const status = downloader.getStatus();

  if (flags.json) {
    console.log(formatJson(status));
  } else {
    console.log('\n╔══════════════════════════════════════════════════════════════════════════╗');
    console.log('║                         RuvLTRA Models                                   ║');
    console.log('║         https://huggingface.co/ruv/ruvltra                               ║');
    console.log('╠══════════════════════════════════════════════════════════════════════════╣');
    console.log('║  Model        │ Size    │ Params │ Status     │ Use Case                 ║');
    console.log('╠══════════════════════════════════════════════════════════════════════════╣');

    for (const { model, downloaded } of status) {
      const statusIcon = downloaded ? '✓ Ready  ' : '○ Not DL ';
      const name = model.id.padEnd(12);
      const size = model.size.padEnd(7);
      const params = model.parameters.padEnd(6);
      const useCase = model.useCase.slice(0, 24).padEnd(24);
      console.log(`║  ${name} │ ${size} │ ${params} │ ${statusIcon} │ ${useCase} ║`);
    }

    console.log('╚══════════════════════════════════════════════════════════════════════════╝');
    console.log(`\nModels directory: ${getDefaultModelsDir()}`);
    console.log('\nDownload with: ruvllm models download <model-id>');
    console.log('  Examples:    ruvllm models download claude-code');
    console.log('               ruvllm models download --all');
  }
}

async function runModelsDownload(modelId, flags) {
  const downloader = new ModelDownloader();

  if (flags.all) {
    console.log('\nDownloading all RuvLTRA models...\n');
    const models = listModels();

    for (const model of models) {
      console.log(`\n[${model.id}] ${model.name} (${model.size})`);

      if (downloader.isDownloaded(model.id) && !flags.force) {
        console.log('  Already downloaded, skipping (use --force to re-download)');
        continue;
      }

      try {
        const lastPercent = { value: -1 };
        const path = await downloader.download(model.id, {
          force: flags.force,
          onProgress: (p) => {
            const percent = Math.floor(p.percent / 5) * 5; // Round to 5%
            if (percent !== lastPercent.value) {
              const bar = '█'.repeat(percent / 5) + '░'.repeat(20 - percent / 5);
              const speed = (p.speedBps / 1024 / 1024).toFixed(1);
              const eta = p.etaSeconds < 60
                ? `${Math.ceil(p.etaSeconds)}s`
                : `${Math.ceil(p.etaSeconds / 60)}m`;
              process.stdout.write(`\r  [${bar}] ${p.percent}% | ${speed} MB/s | ETA: ${eta}   `);
              lastPercent.value = percent;
            }
          },
        });
        console.log(`\n  ✓ Downloaded to: ${path}`);
      } catch (error) {
        console.error(`\n  ✗ Failed: ${error.message}`);
      }
    }

    console.log('\n\nDownload complete!');
    return;
  }

  if (!modelId) {
    console.error('Error: model ID required. Use --all to download all models.');
    console.error('\nAvailable models:');
    listModels().forEach(m => console.error(`  - ${m.id}: ${m.name} (${m.size})`));
    process.exit(1);
  }

  const model = getModelInfo(modelId);
  if (!model) {
    console.error(`Error: Unknown model "${modelId}"`);
    console.error('\nAvailable models:');
    listModels().forEach(m => console.error(`  - ${m.id}: ${m.name} (${m.size})`));
    process.exit(1);
  }

  console.log(`\nDownloading ${model.name} (${model.size})...`);
  console.log(`From: ${model.url}\n`);

  if (downloader.isDownloaded(modelId) && !flags.force) {
    const path = downloader.getModelPath(modelId);
    console.log(`Model already downloaded at: ${path}`);
    console.log('Use --force to re-download.');
    return;
  }

  const lastPercent = { value: -1 };
  try {
    const path = await downloader.download(modelId, {
      force: flags.force,
      onProgress: (p) => {
        const percent = Math.floor(p.percent / 2) * 2; // Round to 2%
        if (percent !== lastPercent.value) {
          const bar = '█'.repeat(Math.floor(percent / 5)) + '░'.repeat(20 - Math.floor(percent / 5));
          const downloaded = (p.downloaded / 1024 / 1024).toFixed(1);
          const total = (p.total / 1024 / 1024).toFixed(1);
          const speed = (p.speedBps / 1024 / 1024).toFixed(1);
          const eta = p.etaSeconds < 60
            ? `${Math.ceil(p.etaSeconds)}s`
            : `${Math.ceil(p.etaSeconds / 60)}m`;
          process.stdout.write(`\r[${bar}] ${p.percent}% | ${downloaded}/${total} MB | ${speed} MB/s | ETA: ${eta}   `);
          lastPercent.value = percent;
        }
      },
    });
    console.log(`\n\n✓ Downloaded to: ${path}`);
    console.log(`\nModel ready to use!`);
    console.log(`  Context length: ${model.contextLength} tokens`);
    console.log(`  Quantization:   ${model.quantization}`);
  } catch (error) {
    console.error(`\n\n✗ Download failed: ${error.message}`);
    process.exit(1);
  }
}

async function runModelsStatus(flags) {
  const downloader = new ModelDownloader();
  const status = downloader.getStatus();

  if (flags.json) {
    console.log(formatJson(status.map(s => ({
      id: s.model.id,
      name: s.model.name,
      downloaded: s.downloaded,
      path: s.path,
      size: s.model.size,
    }))));
  } else {
    console.log('\nModel Status:');
    console.log(`Directory: ${getDefaultModelsDir()}\n`);

    for (const { model, downloaded, path } of status) {
      const icon = downloaded ? '✓' : '○';
      const status = downloaded ? 'Ready' : 'Not downloaded';
      console.log(`  ${icon} ${model.name.padEnd(25)} ${status.padEnd(15)} ${model.size}`);
      if (downloaded) {
        console.log(`    Path: ${path}`);
      }
    }
  }
}

async function runModelsDelete(modelId, flags) {
  const downloader = new ModelDownloader();

  if (flags.all) {
    const count = downloader.deleteAll();
    console.log(`Deleted ${count} model(s).`);
    return;
  }

  if (!modelId) {
    console.error('Error: model ID required. Use --all to delete all models.');
    process.exit(1);
  }

  if (downloader.delete(modelId)) {
    console.log(`Deleted model: ${modelId}`);
  } else {
    console.log(`Model not found or not downloaded: ${modelId}`);
  }
}

// Benchmark commands for Claude Code use cases
async function runBenchmarkRouting(flags) {
  console.log('\nRunning Routing Benchmark...');
  console.log(`Testing ${ROUTING_TEST_CASES.length} task routing scenarios\n`);

  const llm = new RuvLLM({ embeddingDim: 768, learningEnabled: false });

  // Router function using the model
  const modelRouter = (task) => {
    try {
      const decision = llm.route(task);
      return { agent: decision.model, confidence: decision.confidence };
    } catch (e) {
      // Fallback to keyword router if model not available
      return baselineKeywordRouter(task);
    }
  };

  // Run with baseline (keyword) router
  console.log('Baseline (keyword matching):');
  const baselineResults = runRoutingBenchmark(baselineKeywordRouter);

  if (flags.json) {
    console.log(JSON.stringify(baselineResults, null, 2));
  } else {
    console.log(formatRoutingResults(baselineResults));
  }

  // Try model router if native is available
  if (llm.isNativeLoaded() && !flags['baseline-only']) {
    console.log('\nModel Router (RuvLTRA):');
    const modelResults = runRoutingBenchmark(modelRouter);
    if (flags.json) {
      console.log(JSON.stringify(modelResults, null, 2));
    } else {
      console.log(formatRoutingResults(modelResults));
    }

    // Comparison
    const improvement = modelResults.accuracy - baselineResults.accuracy;
    console.log(`\nComparison: Model ${improvement >= 0 ? '+' : ''}${(improvement * 100).toFixed(1)}% vs baseline`);
  }
}

async function runBenchmarkEmbedding(flags) {
  console.log('\nRunning Embedding Benchmark...');
  console.log('Testing similarity detection, clustering, and search quality\n');

  const llm = new RuvLLM({ embeddingDim: 768, learningEnabled: false });
  const simd = new SimdOps();

  // Embedder function
  const embedder = (text) => {
    try {
      return llm.embed(text);
    } catch (e) {
      // Fallback: simple hash-based embedding for testing
      const hash = text.split('').reduce((h, c) => ((h << 5) - h + c.charCodeAt(0)) | 0, 0);
      return Array.from({ length: 768 }, (_, i) => Math.sin(hash + i) * 0.5);
    }
  };

  // Similarity function
  const similarity = (a, b) => {
    try {
      return simd.cosineSimilarity(a, b);
    } catch (e) {
      // Fallback cosine similarity
      let dot = 0, normA = 0, normB = 0;
      for (let i = 0; i < a.length; i++) {
        dot += a[i] * b[i];
        normA += a[i] * a[i];
        normB += b[i] * b[i];
      }
      return dot / (Math.sqrt(normA) * Math.sqrt(normB));
    }
  };

  const results = runEmbeddingBenchmark(embedder, similarity);

  if (flags.json) {
    console.log(JSON.stringify(results, null, 2));
  } else {
    console.log(formatEmbeddingResults(results));
  }
}

async function runBenchmarkCompare(flags) {
  try {
    const results = await runFullComparison();

    if (flags.json) {
      console.log(JSON.stringify(results, null, 2));
    } else {
      console.log(formatComparisonResults(results));
    }
  } catch (error) {
    console.error('Comparison failed:', error.message);
    process.exit(1);
  }
}

// Training commands
async function runTrainContrastive(flags) {
  console.log('\n╔═══════════════════════════════════════════════════════════════════════════╗');
  console.log('║           RuvLTRA Contrastive Fine-tuning Pipeline                        ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════╝\n');

  const stats = getDatasetStats();
  const outputPath = flags.output || './training-output';

  console.log(`Training examples: ${stats.totalExamples}`);
  console.log(`Contrastive pairs: ${stats.contrastivePairs}`);
  console.log(`Agent types: ${stats.agentTypes}`);
  console.log(`Output: ${outputPath}\n`);

  // Create trainer
  const trainer = new ContrastiveTrainer({
    epochs: parseInt(flags.epochs) || 10,
    batchSize: parseInt(flags['batch-size']) || 16,
    learningRate: parseFloat(flags.lr) || 0.0001,
    margin: parseFloat(flags.margin) || 0.5,
    outputPath,
  });

  // Get embeddings for agents
  const llm = new RuvLLM({ embeddingDim: 768, learningEnabled: false });
  const agentEmbeddings = {};

  console.log('Computing agent embeddings...');
  for (const [agent, data] of Object.entries(AGENT_TRAINING_DATA)) {
    process.stdout.write(`  ${agent}... `);
    try {
      const emb = llm.embed(data.description);
      agentEmbeddings[agent] = emb;
      trainer.addAgentEmbedding(agent, emb);
      console.log('done');
    } catch (e) {
      // Fallback embedding
      const hash = data.description.split('').reduce((h, c) => ((h << 5) - h + c.charCodeAt(0)) | 0, 0);
      const emb = Array.from({ length: 768 }, (_, i) => Math.sin(hash + i) * 0.5);
      agentEmbeddings[agent] = emb;
      trainer.addAgentEmbedding(agent, emb);
      console.log('done (fallback)');
    }
  }

  // Generate triplets
  console.log('\nGenerating training triplets...');
  const examples = generateTrainingDataset();
  const agents = Object.keys(AGENT_TRAINING_DATA);

  let tripletCount = 0;
  for (const example of examples.slice(0, 200)) { // Limit for speed
    let taskEmb;
    try {
      taskEmb = llm.embed(example.task);
    } catch (e) {
      const hash = example.task.split('').reduce((h, c) => ((h << 5) - h + c.charCodeAt(0)) | 0, 0);
      taskEmb = Array.from({ length: 768 }, (_, i) => Math.sin(hash + i) * 0.5);
    }

    const positiveAgent = example.agent;
    const positiveEmb = agentEmbeddings[positiveAgent];

    // Hard negatives
    const hardNegatives = example.confusing_with
      ? [example.confusing_with]
      : agents.filter(a => a !== positiveAgent).slice(0, 2);

    for (const negAgent of hardNegatives) {
      const negEmb = agentEmbeddings[negAgent];
      if (negEmb) {
        trainer.addTriplet(
          example.task, taskEmb,
          positiveAgent, positiveEmb,
          negAgent, negEmb,
          !!example.confusing_with
        );
        tripletCount++;
      }
    }
  }

  console.log(`Created ${tripletCount} triplets\n`);

  // Train
  console.log('Training...');
  const result = trainer.train();

  // Export data
  console.log('\nExporting training data...');
  const exportPath = trainer.exportTrainingData();
  const loraConfig = trainer.generateLoRAConfig();
  const scriptPath = trainer.generateTrainingScript();

  // Summary
  console.log('\n═══════════════════════════════════════════════════════════════════════════');
  console.log('                              TRAINING SUMMARY');
  console.log('═══════════════════════════════════════════════════════════════════════════\n');

  console.log('Training data exported:');
  console.log(`  - ${exportPath}/triplets.jsonl (${tripletCount} triplets)`);
  console.log(`  - ${exportPath}/triplets.csv (spreadsheet format)`);
  console.log(`  - ${exportPath}/embeddings.json (precomputed embeddings)`);
  console.log(`  - ${exportPath}/lora_config.json (LoRA configuration)`);
  console.log(`  - ${exportPath}/train.sh (training script)\n`);

  console.log('Training loss (simulated):');
  console.log(`  Initial: ${result.initialLoss.toFixed(4)}`);
  console.log(`  Final:   ${result.finalLoss.toFixed(4)}`);
  console.log(`  Improvement: ${result.improvement.toFixed(1)}%\n`);

  console.log('To fine-tune on GPU:');
  console.log(`  cd ${exportPath}`);
  console.log('  chmod +x train.sh && ./train.sh\n');
}

async function runTrainDataset(flags) {
  console.log('\n╔═══════════════════════════════════════════════════════════════════════════╗');
  console.log('║               RuvLTRA Training Dataset Generator                          ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════╝\n');

  const stats = getDatasetStats();
  const outputPath = flags.output || './data/training';

  console.log(`Generating training data for ${stats.agentTypes} agent types...\n`);

  // Generate dataset
  const examples = generateTrainingDataset();
  const pairs = generateContrastivePairs();

  // Export
  const { existsSync, mkdirSync, writeFileSync } = require('fs');
  const { join } = require('path');

  if (!existsSync(outputPath)) {
    mkdirSync(outputPath, { recursive: true });
  }

  // JSONL format
  writeFileSync(
    join(outputPath, 'routing-examples.jsonl'),
    examples.map(e => JSON.stringify(e)).join('\n')
  );

  // Contrastive pairs
  writeFileSync(
    join(outputPath, 'contrastive-pairs.jsonl'),
    pairs.map(p => JSON.stringify(p)).join('\n')
  );

  // CSV format
  const csv = [
    'task,agent,complexity,confusing_with',
    ...examples.map(e => `"${e.task.replace(/"/g, '""')}",${e.agent},${e.complexity || ''},${e.confusing_with || ''}`)
  ].join('\n');
  writeFileSync(join(outputPath, 'routing-examples.csv'), csv);

  console.log('Export complete:');
  console.log(`  - ${join(outputPath, 'routing-examples.jsonl')} (${examples.length} examples)`);
  console.log(`  - ${join(outputPath, 'contrastive-pairs.jsonl')} (${pairs.length} pairs)`);
  console.log(`  - ${join(outputPath, 'routing-examples.csv')} (spreadsheet format)\n`);

  console.log('Dataset statistics:');
  console.log(`  Total examples: ${stats.totalExamples}`);
  console.log(`  Contrastive pairs: ${stats.contrastivePairs}`);
  console.log(`  Agent types: ${stats.agentTypes}`);
  console.log(`  Agents: ${stats.agents.join(', ')}\n`);
}

async function runTrainStats(flags) {
  const stats = getDatasetStats();

  if (flags.json) {
    console.log(JSON.stringify(stats, null, 2));
  } else {
    console.log('\nTraining Dataset Statistics:');
    console.log(`  Total examples: ${stats.totalExamples}`);
    console.log(`  Contrastive pairs: ${stats.contrastivePairs}`);
    console.log(`  Agent types: ${stats.agentTypes}`);
    console.log('\nAgents:');
    for (const agent of stats.agents) {
      const data = AGENT_TRAINING_DATA[agent];
      console.log(`  - ${agent}: ${data.examples.length} examples, ${data.keywords.length} keywords`);
    }
  }
}

async function runBenchmarkFull(flags) {
  console.log('\n╔═══════════════════════════════════════════════════════════════════════════╗');
  console.log('║                    RUVLTRA FULL BENCHMARK SUITE                           ║');
  console.log('║            Evaluating for Claude Code Use Cases                           ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════╝\n');

  const llm = new RuvLLM({ embeddingDim: 768, learningEnabled: false });
  const simd = new SimdOps();

  const modelName = llm.isNativeLoaded() ? 'RuvLTRA (native)' : 'RuvLTRA (JS fallback)';

  // Router
  const router = (task) => {
    try {
      const decision = llm.route(task);
      return { agent: decision.model, confidence: decision.confidence };
    } catch (e) {
      return baselineKeywordRouter(task);
    }
  };

  // Embedder
  const embedder = (text) => {
    try {
      return llm.embed(text);
    } catch (e) {
      const hash = text.split('').reduce((h, c) => ((h << 5) - h + c.charCodeAt(0)) | 0, 0);
      return Array.from({ length: 768 }, (_, i) => Math.sin(hash + i) * 0.5);
    }
  };

  // Similarity
  const similarity = (a, b) => {
    try {
      return simd.cosineSimilarity(a, b);
    } catch (e) {
      let dot = 0, normA = 0, normB = 0;
      for (let i = 0; i < a.length; i++) {
        dot += a[i] * b[i];
        normA += a[i] * a[i];
        normB += b[i] * b[i];
      }
      return dot / (Math.sqrt(normA) * Math.sqrt(normB));
    }
  };

  const results = runFullBenchmark(router, embedder, similarity, modelName);

  if (flags.json) {
    console.log(JSON.stringify(results, null, 2));
  } else {
    console.log(formatFullResults(results));
  }
}

function printHelp() {
  console.log(`
RuvLLM - Self-learning LLM Orchestration

Usage: ruvllm <command> [options]

Commands:
  query <text>              Query the LLM with automatic routing
  generate <prompt>         Generate text with SIMD inference
  route <text>              Get routing decision for query
  memory add <content>      Add content to memory
  memory search <query>     Search memory for similar content
  embed <text>              Get embedding for text
  similarity <t1> <t2>      Compute similarity between texts
  stats                     Show engine statistics
  benchmark                 Run SIMD performance benchmark
  info                      Show system information
  help                      Show this help message

Model Management:
  models list               List available RuvLTRA models
  models download <id>      Download a model from HuggingFace
  models download --all     Download all available models
  models status             Check which models are downloaded
  models delete <id>        Delete a downloaded model

Claude Code Benchmarks:
  benchmark routing         Test agent routing accuracy (100 tasks)
  benchmark embedding       Test embedding quality (similarity, search, clustering)
  benchmark full            Run complete benchmark suite
  benchmark compare         Compare Qwen base vs RuvLTRA Claude Code
  benchmark simd            Run SIMD performance benchmark

Training & Fine-tuning:
  train contrastive         Run contrastive fine-tuning pipeline
  train dataset             Generate training dataset (JSONL, CSV)
  train stats               Show training dataset statistics

Options:
  --json                    Output as JSON
  --temperature <float>     Sampling temperature (0.0-2.0)
  --max-tokens <int>        Maximum tokens to generate
  --top-p <float>           Nucleus sampling (0.0-1.0)
  --top-k <int>             Top-k sampling
  --k <int>                 Number of results for search
  --metadata <json>         Metadata for memory add
  --dims <int>              Dimensions for benchmark (default: 768)
  --iterations <int>        Iterations for benchmark (default: 1000)
  --force                   Force re-download even if model exists
  --all                     Apply to all models (download/delete)
  --output <path>           Output directory for training data
  --epochs <int>            Number of training epochs (default: 10)
  --batch-size <int>        Training batch size (default: 16)
  --lr <float>              Learning rate (default: 0.0001)
  --margin <float>          Triplet loss margin (default: 0.5)

Available Models (from https://huggingface.co/ruv/ruvltra):
  claude-code               RuvLTRA Claude Code (398MB) - Claude Code workflows
  small                     RuvLTRA Small (398MB) - Edge devices, IoT
  medium                    RuvLTRA Medium (669MB) - General purpose

Examples:
  ruvllm query "What is machine learning?"
  ruvllm generate "Write a poem about AI" --temperature 0.9
  ruvllm memory add "Important context" --metadata '{"type":"note"}'
  ruvllm memory search "context" --k 5
  ruvllm similarity "hello world" "hi there"
  ruvllm benchmark --dims 1024 --iterations 5000

  # Model management
  ruvllm models list
  ruvllm models download claude-code
  ruvllm models download --all
  ruvllm models status

  # Claude Code benchmarks
  ruvllm benchmark routing            # Test task routing accuracy
  ruvllm benchmark embedding          # Test embedding quality
  ruvllm benchmark full               # Run complete benchmark suite

  # Training & fine-tuning
  ruvllm train contrastive            # Run contrastive fine-tuning
  ruvllm train dataset                # Generate training dataset
  ruvllm train stats                  # Show dataset statistics
  ruvllm train contrastive --epochs 20 --output ./my-training

Learn more: https://github.com/ruvnet/ruvector
`);
}

// Main
async function main() {
  const parsed = parseArgs(args);
  const { command, positional, flags } = parsed;

  if (!command || command === 'help' || flags.help) {
    printHelp();
    return;
  }

  // Create engine for commands that need it
  const llm = new RuvLLM({
    embeddingDim: flags.dim ? parseInt(flags.dim) : 768,
    learningEnabled: flags['no-learning'] ? false : true,
  });

  try {
    switch (command) {
      case 'query':
        if (!positional[0]) {
          console.error('Error: query text required');
          process.exit(1);
        }
        await runQuery(llm, positional[0], flags);
        break;

      case 'generate':
        if (!positional[0]) {
          console.error('Error: prompt required');
          process.exit(1);
        }
        await runGenerate(llm, positional[0], flags);
        break;

      case 'route':
        if (!positional[0]) {
          console.error('Error: text required');
          process.exit(1);
        }
        await runRoute(llm, positional[0], flags);
        break;

      case 'memory':
        const subcommand = positional[0];
        if (subcommand === 'add') {
          if (!positional[1]) {
            console.error('Error: content required');
            process.exit(1);
          }
          await runMemoryAdd(llm, positional[1], flags);
        } else if (subcommand === 'search') {
          if (!positional[1]) {
            console.error('Error: query required');
            process.exit(1);
          }
          await runMemorySearch(llm, positional[1], flags);
        } else {
          console.error('Error: unknown memory subcommand. Use "add" or "search"');
          process.exit(1);
        }
        break;

      case 'embed':
        if (!positional[0]) {
          console.error('Error: text required');
          process.exit(1);
        }
        await runEmbed(llm, positional[0], flags);
        break;

      case 'similarity':
        if (!positional[0] || !positional[1]) {
          console.error('Error: two texts required');
          process.exit(1);
        }
        await runSimilarity(llm, positional[0], positional[1], flags);
        break;

      case 'stats':
        await runStats(llm, flags);
        break;

      case 'benchmark':
        const benchSubcmd = positional[0];
        if (benchSubcmd === 'routing') {
          await runBenchmarkRouting(flags);
        } else if (benchSubcmd === 'embedding' || benchSubcmd === 'embeddings') {
          await runBenchmarkEmbedding(flags);
        } else if (benchSubcmd === 'full' || benchSubcmd === 'all') {
          await runBenchmarkFull(flags);
        } else if (benchSubcmd === 'compare') {
          await runBenchmarkCompare(flags);
        } else if (benchSubcmd === 'simd' || !benchSubcmd) {
          // Default to SIMD benchmark for backwards compatibility
          await runBenchmark(flags);
        } else {
          console.error(`Unknown benchmark type: ${benchSubcmd}`);
          console.error('Available: routing, embedding, full, simd, compare');
          process.exit(1);
        }
        break;

      case 'info':
        await runInfo(flags);
        break;

      case 'models':
        const modelsSubcmd = positional[0];
        if (!modelsSubcmd || modelsSubcmd === 'list') {
          await runModelsList(flags);
        } else if (modelsSubcmd === 'download') {
          await runModelsDownload(positional[1], flags);
        } else if (modelsSubcmd === 'status') {
          await runModelsStatus(flags);
        } else if (modelsSubcmd === 'delete' || modelsSubcmd === 'remove') {
          await runModelsDelete(positional[1], flags);
        } else {
          // Treat subcommand as model ID for download
          await runModelsDownload(modelsSubcmd, flags);
        }
        break;

      case 'train':
        const trainSubcmd = positional[0];
        if (!trainSubcmd || trainSubcmd === 'contrastive') {
          await runTrainContrastive(flags);
        } else if (trainSubcmd === 'dataset') {
          await runTrainDataset(flags);
        } else if (trainSubcmd === 'stats') {
          await runTrainStats(flags);
        } else {
          console.error(`Unknown train subcommand: ${trainSubcmd}`);
          console.error('Available: contrastive, dataset, stats');
          process.exit(1);
        }
        break;

      default:
        console.error(`Unknown command: ${command}`);
        console.error('Run "ruvllm help" for usage information.');
        process.exit(1);
    }
  } catch (error) {
    console.error('Error:', error.message);
    if (flags.verbose) {
      console.error(error.stack);
    }
    process.exit(1);
  }
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});

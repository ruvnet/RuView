/**
 * RuvLTRA Benchmark Suite
 *
 * Comprehensive benchmarks for evaluating RuvLTRA models
 * on Claude Code-specific use cases.
 */

export * from './routing-benchmark';
export * from './embedding-benchmark';
export * from './model-comparison';

import {
  runRoutingBenchmark,
  formatRoutingResults,
  baselineKeywordRouter,
  ROUTING_TEST_CASES,
  type RoutingBenchmarkResults,
} from './routing-benchmark';

import {
  runEmbeddingBenchmark,
  formatEmbeddingResults,
  SIMILARITY_TEST_PAIRS,
  SEARCH_TEST_CASES,
  CLUSTER_TEST_CASES,
  type EmbeddingBenchmarkResults,
} from './embedding-benchmark';

export interface FullBenchmarkResults {
  routing: RoutingBenchmarkResults;
  embedding: EmbeddingBenchmarkResults;
  timestamp: string;
  model: string;
}

/**
 * Run all benchmarks with a given model
 */
export function runFullBenchmark(
  router: (task: string) => { agent: string; confidence: number },
  embedder: (text: string) => number[],
  similarityFn: (a: number[], b: number[]) => number,
  modelName: string = 'unknown'
): FullBenchmarkResults {
  const routing = runRoutingBenchmark(router);
  const embedding = runEmbeddingBenchmark(embedder, similarityFn);

  return {
    routing,
    embedding,
    timestamp: new Date().toISOString(),
    model: modelName,
  };
}

/**
 * Format full benchmark results
 */
export function formatFullResults(results: FullBenchmarkResults): string {
  const lines: string[] = [];

  lines.push('');
  lines.push('╔═══════════════════════════════════════════════════════════════════════════╗');
  lines.push('║                    RUVLTRA BENCHMARK SUITE                                ║');
  lines.push('║            Claude Code Use Case Evaluation                                ║');
  lines.push('╠═══════════════════════════════════════════════════════════════════════════╣');
  lines.push(`║  Model: ${results.model.padEnd(64)}║`);
  lines.push(`║  Date:  ${results.timestamp.padEnd(64)}║`);
  lines.push('╚═══════════════════════════════════════════════════════════════════════════╝');

  lines.push(formatRoutingResults(results.routing));
  lines.push(formatEmbeddingResults(results.embedding));

  // Overall assessment
  lines.push('');
  lines.push('═══════════════════════════════════════════════════════════════');
  lines.push('                      OVERALL ASSESSMENT');
  lines.push('═══════════════════════════════════════════════════════════════');

  const routingScore = results.routing.accuracy;
  const embeddingScore = (
    results.embedding.similarityAccuracy +
    results.embedding.searchMRR +
    results.embedding.clusterPurity
  ) / 3;

  const overallScore = (routingScore + embeddingScore) / 2;

  lines.push('');
  lines.push(`  Routing Score:   ${(routingScore * 100).toFixed(1)}%`);
  lines.push(`  Embedding Score: ${(embeddingScore * 100).toFixed(1)}%`);
  lines.push(`  ─────────────────────────`);
  lines.push(`  Overall Score:   ${(overallScore * 100).toFixed(1)}%`);
  lines.push('');

  if (overallScore >= 0.8) {
    lines.push('  ✓ EXCELLENT - Highly suitable for Claude Code workflows');
  } else if (overallScore >= 0.6) {
    lines.push('  ~ GOOD - Suitable for most Claude Code use cases');
  } else if (overallScore >= 0.4) {
    lines.push('  ~ ACCEPTABLE - May work but consider alternatives');
  } else {
    lines.push('  ✗ NEEDS IMPROVEMENT - Consider different model or fine-tuning');
  }

  lines.push('');
  lines.push('═══════════════════════════════════════════════════════════════');

  return lines.join('\n');
}

/**
 * Compare two models
 */
export function compareModels(
  results1: FullBenchmarkResults,
  results2: FullBenchmarkResults
): string {
  const lines: string[] = [];

  lines.push('');
  lines.push('╔═══════════════════════════════════════════════════════════════════════════╗');
  lines.push('║                       MODEL COMPARISON                                    ║');
  lines.push('╚═══════════════════════════════════════════════════════════════════════════╝');
  lines.push('');

  const metrics = [
    { name: 'Routing Accuracy', v1: results1.routing.accuracy, v2: results2.routing.accuracy },
    { name: 'Similarity Detection', v1: results1.embedding.similarityAccuracy, v2: results2.embedding.similarityAccuracy },
    { name: 'Search MRR', v1: results1.embedding.searchMRR, v2: results2.embedding.searchMRR },
    { name: 'Search NDCG', v1: results1.embedding.searchNDCG, v2: results2.embedding.searchNDCG },
    { name: 'Cluster Purity', v1: results1.embedding.clusterPurity, v2: results2.embedding.clusterPurity },
    { name: 'Routing Latency (ms)', v1: results1.routing.avgLatencyMs, v2: results2.routing.avgLatencyMs, lowerBetter: true },
  ];

  lines.push(`${'Metric'.padEnd(25)} ${results1.model.padEnd(15)} ${results2.model.padEnd(15)} Winner`);
  lines.push('─'.repeat(70));

  for (const m of metrics) {
    const val1 = m.lowerBetter ? m.v1 : m.v1;
    const val2 = m.lowerBetter ? m.v2 : m.v2;

    let winner: string;
    if (m.lowerBetter) {
      winner = val1 < val2 ? results1.model : val2 < val1 ? results2.model : 'tie';
    } else {
      winner = val1 > val2 ? results1.model : val2 > val1 ? results2.model : 'tie';
    }

    const v1Str = m.lowerBetter ? val1.toFixed(2) : (val1 * 100).toFixed(1) + '%';
    const v2Str = m.lowerBetter ? val2.toFixed(2) : (val2 * 100).toFixed(1) + '%';

    lines.push(`${m.name.padEnd(25)} ${v1Str.padEnd(15)} ${v2Str.padEnd(15)} ${winner}`);
  }

  return lines.join('\n');
}

// Export constants for external use
export {
  ROUTING_TEST_CASES,
  SIMILARITY_TEST_PAIRS,
  SEARCH_TEST_CASES,
  CLUSTER_TEST_CASES,
};

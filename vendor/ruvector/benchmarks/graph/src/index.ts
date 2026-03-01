/**
 * RuVector Graph Benchmark Suite Entry Point
 *
 * Usage:
 *   npm run graph:generate  - Generate synthetic datasets
 *   npm run graph:bench     - Run Rust benchmarks
 *   npm run graph:compare   - Compare with Neo4j
 *   npm run graph:report    - Generate reports
 *   npm run graph:all       - Run complete suite
 */

export { allScenarios, datasets } from './graph-scenarios.js';
export {
  generateSocialNetwork,
  generateKnowledgeGraph,
  generateTemporalGraph,
  generateAllDatasets,
  saveDataset
} from './graph-data-generator.js';
export { runComparison, runAllComparisons } from './comparison-runner.js';
export { generateReport } from './results-report.js';

/**
 * Quick benchmark runner
 */
export async function runQuickBenchmark() {
  console.log('🚀 RuVector Graph Benchmark Suite\n');

  const { generateReport } = await import('./results-report.js');

  // Generate report from existing results
  generateReport();
}

// Run if called directly
if (require.main === module) {
  runQuickBenchmark().catch(console.error);
}

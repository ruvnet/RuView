/**
 * Example: Running DSPy Benchmarks
 *
 * This script demonstrates how to use the benchmark suite
 * for comparing multiple models across various metrics.
 */

import { BenchmarkSuite, ModelConfig } from './dspy-benchmarks.js';

async function runFullBenchmarkSuite() {
  console.log('🎯 Running Full DSPy Benchmark Suite\n');

  const suite = new BenchmarkSuite('./training/results/benchmarks');

  // Option 1: Add common models
  suite.addCommonModels();

  // Option 2: Add custom models
  // const customModel: ModelConfig = {
  //   name: 'Custom Model',
  //   provider: 'openrouter',
  //   model: 'custom-model',
  //   costPer1kTokens: 0.002,
  //   maxTokens: 8192,
  // };
  // suite.addModel(customModel);

  // Run comprehensive comparison
  const comparison = await suite.runModelComparison(1000);

  // Run additional analyses
  await suite.runScalabilityTest();
  await suite.runCostAnalysis();
  await suite.runQualityConvergence(10);
  await suite.runDiversityAnalysis(5000);

  // Generate reports
  await suite.generateJSONReport(comparison);
  await suite.generateMarkdownReport(comparison);

  console.log('\n✅ All benchmarks completed!');
  console.log('\n📊 Key Findings:');
  console.log(`   Overall Winner: ${comparison.winner.overall}`);
  console.log(`   Best Quality: ${comparison.winner.quality}`);
  console.log(`   Best Performance: ${comparison.winner.performance}`);
  console.log(`   Most Cost-Effective: ${comparison.winner.cost}`);
  console.log(`   Pareto Frontier: ${comparison.paretoFrontier.join(', ')}`);

  console.log('\n💡 Recommendations by Use Case:');
  for (const [useCase, model] of Object.entries(comparison.recommendations)) {
    console.log(`   ${useCase}: ${model}`);
  }
}

async function runQuickComparison() {
  console.log('⚡ Running Quick Model Comparison\n');

  const suite = new BenchmarkSuite();

  // Add just a few models for quick testing
  suite.addModel({
    name: 'GPT-4',
    provider: 'openai',
    model: 'gpt-4',
    costPer1kTokens: 0.03,
    maxTokens: 8192,
  });

  suite.addModel({
    name: 'Claude 3.5 Sonnet',
    provider: 'anthropic',
    model: 'claude-3.5-sonnet',
    costPer1kTokens: 0.015,
    maxTokens: 200000,
  });

  suite.addModel({
    name: 'Gemini Pro',
    provider: 'gemini',
    model: 'gemini-pro',
    costPer1kTokens: 0.0005,
    maxTokens: 32768,
  });

  // Run comparison with smaller sample size
  const comparison = await suite.runModelComparison(500);

  // Generate reports
  await suite.generateJSONReport(comparison);
  await suite.generateMarkdownReport(comparison);

  console.log('\n✅ Quick comparison completed!');
}

async function runScalabilityOnly() {
  console.log('📈 Running Scalability Test Only\n');

  const suite = new BenchmarkSuite();
  suite.addCommonModels();

  const results = await suite.runScalabilityTest();

  console.log('\n📊 Scalability Summary:');
  for (const result of results) {
    console.log(`\n${result.modelName}:`);
    console.log(`  Scaling Efficiency: ${result.scalingEfficiency.toFixed(2)}x`);
    console.log(`  Best Throughput: ${Math.max(...result.throughputs).toFixed(0)} samples/s`);
    console.log(`  Cost at 100K: $${result.costs[result.costs.length - 1].toFixed(4)}`);
  }
}

async function runCostOptimization() {
  console.log('💰 Running Cost Optimization Analysis\n');

  const suite = new BenchmarkSuite();
  suite.addCommonModels();

  await suite.runModelComparison(1000);
  await suite.runCostAnalysis();

  console.log('\n✅ Cost analysis completed!');
}

// Main execution
async function main() {
  const mode = process.argv[2] || 'full';

  switch (mode) {
    case 'full':
      await runFullBenchmarkSuite();
      break;
    case 'quick':
      await runQuickComparison();
      break;
    case 'scalability':
      await runScalabilityOnly();
      break;
    case 'cost':
      await runCostOptimization();
      break;
    default:
      console.log('Usage: node run-benchmarks.js [full|quick|scalability|cost]');
      console.log('\nModes:');
      console.log('  full        - Run complete benchmark suite (default)');
      console.log('  quick       - Quick comparison with 3 models');
      console.log('  scalability - Scalability test only');
      console.log('  cost        - Cost optimization analysis only');
      process.exit(1);
  }
}

main().catch(console.error);

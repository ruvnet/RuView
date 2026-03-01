/**
 * Example Usage of DSPy Multi-Model Benchmark
 *
 * This example shows how to use the benchmark programmatically
 */

import { DSPyMultiModelBenchmark } from './dspy-multi-model-benchmark';

async function main() {
  // Create benchmark instance
  const benchmark = new DSPyMultiModelBenchmark('./training/results/custom-run');

  console.log('🔧 Configuring benchmark...\n');

  // Add OpenAI models
  if (process.env.OPENAI_API_KEY) {
    benchmark.addModel({
      name: 'GPT-4',
      provider: 'openai',
      modelId: 'gpt-4',
      apiKey: process.env.OPENAI_API_KEY,
      costPer1kTokens: { input: 0.03, output: 0.06 },
      maxTokens: 8192
    });

    benchmark.addModel({
      name: 'GPT-3.5-Turbo',
      provider: 'openai',
      modelId: 'gpt-3.5-turbo',
      apiKey: process.env.OPENAI_API_KEY,
      costPer1kTokens: { input: 0.0015, output: 0.002 },
      maxTokens: 16384
    });
  }

  // Add Anthropic models
  if (process.env.ANTHROPIC_API_KEY) {
    benchmark.addModel({
      name: 'Claude-3-Sonnet',
      provider: 'anthropic',
      modelId: 'claude-3-sonnet-20240229',
      apiKey: process.env.ANTHROPIC_API_KEY,
      costPer1kTokens: { input: 0.003, output: 0.015 },
      maxTokens: 200000
    });

    benchmark.addModel({
      name: 'Claude-3-Haiku',
      provider: 'anthropic',
      modelId: 'claude-3-haiku-20240307',
      apiKey: process.env.ANTHROPIC_API_KEY,
      costPer1kTokens: { input: 0.00025, output: 0.00125 },
      maxTokens: 200000
    });
  }

  // Run benchmark with 100 samples
  console.log('🚀 Running benchmark...\n');
  const results = await benchmark.runComparison(100);

  // Display results
  console.log('\n📊 Benchmark Results Summary:');
  console.log('='.repeat(70));
  console.log(`Models Compared: ${results.summary.modelsCompared}`);
  console.log(`Total Samples: ${results.summary.totalSamples}`);
  console.log(`Duration: ${(results.summary.totalDuration / 1000).toFixed(2)}s`);
  console.log('='.repeat(70));

  console.log('\n🏆 Winners:');
  console.log(`  Overall: ${results.summary.winner.overall}`);
  console.log(`  Quality: ${results.summary.winner.quality}`);
  console.log(`  Performance: ${results.summary.winner.performance}`);
  console.log(`  Cost: ${results.summary.winner.cost}`);
  console.log(`  Optimization: ${results.summary.winner.optimization}`);

  console.log('\n📈 Quality Rankings:');
  results.rankings.quality.forEach((item, i) => {
    console.log(`  ${i + 1}. ${item.model}: ${item.score.toFixed(3)}`);
  });

  console.log('\n💰 Cost Rankings:');
  results.rankings.cost.forEach((item, i) => {
    console.log(`  ${i + 1}. ${item.model}: ${item.score.toFixed(3)}`);
  });

  console.log('\n🎯 Recommendations:');
  console.log(`  Production: ${results.recommendations.production}`);
  console.log(`  Research: ${results.recommendations.research}`);
  console.log(`  Cost-Optimized: ${results.recommendations.costOptimized}`);
  console.log(`  Balanced: ${results.recommendations.balanced}`);

  // Generate detailed reports
  console.log('\n📝 Generating reports...');
  const reportPath = await benchmark.generateReport(results);
  console.log(`✅ Reports generated at: ${reportPath}`);
}

// Run if executed directly
if (require.main === module) {
  main().catch((error) => {
    console.error('❌ Error:', error.message);
    console.error(error.stack);
    process.exit(1);
  });
}

export { main };

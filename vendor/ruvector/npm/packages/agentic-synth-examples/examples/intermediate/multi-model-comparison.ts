/**
 * INTERMEDIATE TUTORIAL: Multi-Model Comparison
 *
 * Compare multiple AI models (Gemini, Claude, GPT-4) to find the best
 * performer for your specific task. Includes benchmarking, cost tracking,
 * and performance metrics.
 *
 * What you'll learn:
 * - Running parallel model comparisons
 * - Benchmarking quality and speed
 * - Tracking costs per model
 * - Selecting the best model for production
 *
 * Prerequisites:
 * - Set API keys: GEMINI_API_KEY, ANTHROPIC_API_KEY, OPENAI_API_KEY
 * - npm install dspy.ts @ruvector/agentic-synth
 *
 * Run: npx tsx examples/intermediate/multi-model-comparison.ts
 */

import { LM, ChainOfThought, Prediction } from 'dspy.ts';
import { AgenticSynth } from '@ruvector/agentic-synth';

// Model configuration with pricing
interface ModelConfig {
  name: string;
  provider: string;
  model: string;
  apiKey: string;
  costPer1kTokens: number; // Approximate pricing
  capabilities: string[];
}

// Available models to compare
const models: ModelConfig[] = [
  {
    name: 'Gemini Flash',
    provider: 'google-genai',
    model: 'gemini-2.0-flash-exp',
    apiKey: process.env.GEMINI_API_KEY || '',
    costPer1kTokens: 0.001, // Very cheap
    capabilities: ['fast', 'cost-effective', 'reasoning']
  },
  {
    name: 'Claude Sonnet 4',
    provider: 'anthropic',
    model: 'claude-sonnet-4-20250514',
    apiKey: process.env.ANTHROPIC_API_KEY || '',
    costPer1kTokens: 0.003, // Medium cost
    capabilities: ['high-quality', 'reasoning', 'code']
  },
  {
    name: 'GPT-4 Turbo',
    provider: 'openai',
    model: 'gpt-4-turbo-preview',
    apiKey: process.env.OPENAI_API_KEY || '',
    costPer1kTokens: 0.01, // More expensive
    capabilities: ['versatile', 'high-quality', 'creative']
  }
];

// Benchmark results interface
interface BenchmarkResult {
  modelName: string;
  qualityScore: number;
  avgResponseTime: number;
  estimatedCost: number;
  successRate: number;
  outputs: Prediction[];
  errors: string[];
}

// Test cases for comparison
const testCases = [
  {
    task: 'product_description',
    input: {
      product_name: 'Wireless Noise-Cancelling Headphones',
      category: 'Electronics',
      price: 299
    },
    expectedFeatures: ['noise cancellation', 'wireless', 'battery life']
  },
  {
    task: 'product_description',
    input: {
      product_name: 'Organic Herbal Tea Collection',
      category: 'Beverages',
      price: 24
    },
    expectedFeatures: ['organic', 'herbal', 'health benefits']
  },
  {
    task: 'product_description',
    input: {
      product_name: 'Professional Camera Tripod',
      category: 'Photography',
      price: 149
    },
    expectedFeatures: ['stability', 'adjustable', 'professional']
  },
  {
    task: 'product_description',
    input: {
      product_name: 'Smart Fitness Tracker',
      category: 'Wearables',
      price: 79
    },
    expectedFeatures: ['fitness tracking', 'smart features', 'health monitoring']
  }
];

// Quality evaluation function
function evaluateQuality(prediction: Prediction, testCase: typeof testCases[0]): number {
  let score = 0;
  const weights = {
    hasDescription: 0.3,
    descriptionLength: 0.2,
    hasFeatures: 0.2,
    featureCount: 0.15,
    relevance: 0.15
  };

  // Check if description exists and is well-formed
  if (prediction.description && typeof prediction.description === 'string') {
    score += weights.hasDescription;

    // Optimal length is 80-200 characters
    const length = prediction.description.length;
    if (length >= 80 && length <= 200) {
      score += weights.descriptionLength;
    } else if (length >= 50 && length <= 250) {
      score += weights.descriptionLength * 0.5;
    }
  }

  // Check features
  if (prediction.key_features && Array.isArray(prediction.key_features)) {
    score += weights.hasFeatures;

    // More features is better (up to 5)
    const featureCount = Math.min(prediction.key_features.length, 5);
    score += weights.featureCount * (featureCount / 5);
  }

  // Check relevance to expected features
  if (prediction.description) {
    const descLower = prediction.description.toLowerCase();
    const relevantFeatures = testCase.expectedFeatures.filter(feature =>
      descLower.includes(feature.toLowerCase())
    );
    score += weights.relevance * (relevantFeatures.length / testCase.expectedFeatures.length);
  }

  return score;
}

// Run benchmark for a single model
async function benchmarkModel(config: ModelConfig): Promise<BenchmarkResult> {
  console.log(`\n🔄 Testing ${config.name}...`);

  const result: BenchmarkResult = {
    modelName: config.name,
    qualityScore: 0,
    avgResponseTime: 0,
    estimatedCost: 0,
    successRate: 0,
    outputs: [],
    errors: []
  };

  if (!config.apiKey) {
    console.log(`   ⚠️  API key not found, skipping...`);
    result.errors.push('API key not configured');
    return result;
  }

  const lm = new LM({
    provider: config.provider as any,
    model: config.model,
    apiKey: config.apiKey,
    temperature: 0.7
  });

  const signature = {
    input: 'product_name: string, category: string, price: number',
    output: 'description: string, key_features: string[]'
  };

  const generator = new ChainOfThought(signature, { lm });

  const times: number[] = [];
  let totalScore = 0;
  let successCount = 0;

  // Run all test cases
  for (let i = 0; i < testCases.length; i++) {
    const testCase = testCases[i];

    try {
      const startTime = Date.now();
      const prediction = await generator.forward(testCase.input);
      const duration = Date.now() - startTime;

      times.push(duration);
      result.outputs.push(prediction);

      const score = evaluateQuality(prediction, testCase);
      totalScore += score;
      successCount++;

      console.log(`   ✓ Test ${i + 1}/${testCases.length} - Score: ${(score * 100).toFixed(0)}% - ${duration}ms`);

    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';
      result.errors.push(`Test ${i + 1}: ${errorMsg}`);
      console.log(`   ✗ Test ${i + 1}/${testCases.length} - Failed: ${errorMsg}`);
    }
  }

  // Calculate metrics
  result.avgResponseTime = times.length > 0
    ? times.reduce((a, b) => a + b, 0) / times.length
    : 0;
  result.qualityScore = successCount > 0 ? totalScore / testCases.length : 0;
  result.successRate = successCount / testCases.length;

  // Estimate cost (rough approximation based on avg tokens)
  const avgTokens = 500; // Rough estimate
  result.estimatedCost = (avgTokens / 1000) * config.costPer1kTokens * testCases.length;

  return result;
}

// Main comparison function
async function runComparison() {
  console.log('🏆 Multi-Model Comparison Benchmark\n');
  console.log('=' .repeat(70));
  console.log('\nComparing models:');
  models.forEach((m, i) => {
    console.log(`${i + 1}. ${m.name} - $${m.costPer1kTokens}/1K tokens`);
    console.log(`   Capabilities: ${m.capabilities.join(', ')}`);
  });
  console.log(`\nRunning ${testCases.length} test cases per model...\n`);
  console.log('=' .repeat(70));

  // Run all benchmarks in parallel
  const results = await Promise.all(
    models.map(config => benchmarkModel(config))
  );

  // Display results
  console.log('\n' + '=' .repeat(70));
  console.log('\n📊 BENCHMARK RESULTS\n');

  // Sort by quality score
  const sortedResults = [...results].sort((a, b) => b.qualityScore - a.qualityScore);

  console.log('┌─────────────────────┬──────────┬──────────┬──────────┬──────────┐');
  console.log('│ Model               │ Quality  │ Speed    │ Cost     │ Success  │');
  console.log('├─────────────────────┼──────────┼──────────┼──────────┼──────────┤');

  sortedResults.forEach((result, index) => {
    const quality = `${(result.qualityScore * 100).toFixed(1)}%`;
    const speed = `${result.avgResponseTime.toFixed(0)}ms`;
    const cost = `$${result.estimatedCost.toFixed(4)}`;
    const success = `${(result.successRate * 100).toFixed(0)}%`;

    const modelName = result.modelName.padEnd(19);
    const qualityPad = quality.padStart(8);
    const speedPad = speed.padStart(8);
    const costPad = cost.padStart(8);
    const successPad = success.padStart(8);

    const medal = index === 0 ? '🥇' : index === 1 ? '🥈' : index === 2 ? '🥉' : '  ';

    console.log(`│ ${medal} ${modelName}│${qualityPad}│${speedPad}│${costPad}│${successPad}│`);
  });

  console.log('└─────────────────────┴──────────┴──────────┴──────────┴──────────┘\n');

  // Winner analysis
  const winner = sortedResults[0];
  console.log('🎯 WINNER: ' + winner.modelName);
  console.log(`   Quality Score: ${(winner.qualityScore * 100).toFixed(1)}%`);
  console.log(`   Avg Response: ${winner.avgResponseTime.toFixed(0)}ms`);
  console.log(`   Total Cost: $${winner.estimatedCost.toFixed(4)}`);
  console.log(`   Success Rate: ${(winner.successRate * 100).toFixed(0)}%\n`);

  // Recommendations
  console.log('💡 RECOMMENDATIONS:\n');

  const fastest = [...results].sort((a, b) => a.avgResponseTime - b.avgResponseTime)[0];
  const cheapest = [...results].sort((a, b) => a.estimatedCost - b.estimatedCost)[0];
  const mostReliable = [...results].sort((a, b) => b.successRate - a.successRate)[0];

  console.log(`⚡ Fastest: ${fastest.modelName} (${fastest.avgResponseTime.toFixed(0)}ms avg)`);
  console.log(`💰 Cheapest: ${cheapest.modelName} ($${cheapest.estimatedCost.toFixed(4)} total)`);
  console.log(`🎯 Most Reliable: ${mostReliable.modelName} (${(mostReliable.successRate * 100).toFixed(0)}% success)\n`);

  console.log('Use case suggestions:');
  console.log('  • High-volume/cost-sensitive → ' + cheapest.modelName);
  console.log('  • Latency-critical/real-time → ' + fastest.modelName);
  console.log('  • Quality-critical/production → ' + winner.modelName + '\n');

  // Error report
  const errorsExist = results.some(r => r.errors.length > 0);
  if (errorsExist) {
    console.log('⚠️  ERRORS:\n');
    results.forEach(result => {
      if (result.errors.length > 0) {
        console.log(`${result.modelName}:`);
        result.errors.forEach(err => console.log(`  • ${err}`));
        console.log('');
      }
    });
  }

  console.log('=' .repeat(70));
  console.log('\n✅ Benchmark complete!\n');
  console.log('Next steps:');
  console.log('  1. Configure your production app with the winning model');
  console.log('  2. Set up fallback chains for reliability');
  console.log('  3. Monitor performance in production');
  console.log('  4. Re-run benchmarks periodically as models improve\n');

  return results;
}

// Run the comparison
if (import.meta.url === `file://${process.argv[1]}`) {
  runComparison().catch(error => {
    console.error('❌ Benchmark failed:', error);
    process.exit(1);
  });
}

export { runComparison, benchmarkModel, models };

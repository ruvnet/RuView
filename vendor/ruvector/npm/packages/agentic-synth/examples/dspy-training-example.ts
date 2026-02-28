/**
 * DSPy Training Session - Usage Example
 *
 * Demonstrates how to use the DSPy learning framework for multi-model training
 * with automatic prompt optimization and benchmarking.
 *
 * @example
 */

import {
  DSPyTrainingSession,
  ModelProvider,
  TrainingPhase,
  OptimizationEngine,
  BenchmarkCollector
} from '../training/dspy-learning-session.js';

/**
 * Example 1: Basic Training Session
 */
async function basicTrainingExample() {
  console.log('🚀 Starting Basic DSPy Training Session\n');

  // Configure training session with multiple models
  const session = new DSPyTrainingSession({
    models: [
      {
        provider: ModelProvider.CLAUDE,
        model: 'claude-sonnet-4',
        apiKey: process.env.ANTHROPIC_API_KEY || 'sk-ant-test',
        temperature: 0.7,
        maxTokens: 1000
      },
      {
        provider: ModelProvider.GPT4,
        model: 'gpt-4-turbo',
        apiKey: process.env.OPENAI_API_KEY || 'sk-test',
        temperature: 0.7,
        maxTokens: 1000
      },
      {
        provider: ModelProvider.GEMINI,
        model: 'gemini-2.0-flash-exp',
        apiKey: process.env.GEMINI_API_KEY || 'test-key',
        temperature: 0.7,
        maxTokens: 1000
      },
      {
        provider: ModelProvider.LLAMA,
        model: 'llama-3.1-70b',
        apiKey: process.env.TOGETHER_API_KEY || 'test-key',
        temperature: 0.7,
        maxTokens: 1000
      }
    ],
    optimizationRounds: 5,
    convergenceThreshold: 0.95,
    maxConcurrency: 4,
    enableCrossLearning: true,
    enableHooksIntegration: true,
    costBudget: 10.0, // $10 USD budget
    timeoutPerIteration: 30000,
    baselineIterations: 3,
    benchmarkSamples: 50
  });

  // Create DSPy signature for the task
  const optimizer = new OptimizationEngine();
  const signature = optimizer.createSignature(
    'product-description',
    'Generate compelling product descriptions',
    'high-quality, SEO-optimized product description',
    {
      examples: [
        {
          input: 'Wireless headphones with noise cancellation',
          output: 'Premium wireless headphones featuring advanced active noise cancellation technology...'
        },
        {
          input: 'Organic cotton t-shirt',
          output: 'Sustainably crafted organic cotton t-shirt that combines comfort with environmental responsibility...'
        }
      ],
      constraints: [
        'min_length:100',
        'max_length:500',
        'contains:product benefits',
        'contains:call to action'
      ],
      objectives: [
        'Maximize engagement',
        'Optimize for SEO',
        'Include emotional appeal',
        'Highlight unique value proposition'
      ]
    }
  );

  // Set up event listeners
  session.on('start', (data) => {
    console.log(`📊 Training started - Phase: ${data.phase}`);
  });

  session.on('phase', (phase) => {
    console.log(`\n🔄 Phase transition: ${phase}`);
  });

  session.on('iteration', (result) => {
    console.log(
      `  ✓ ${result.modelProvider} - Iteration ${result.iteration}: ` +
      `Quality: ${result.quality.score.toFixed(3)}, ` +
      `Latency: ${result.performance.latency.toFixed(0)}ms, ` +
      `Cost: $${result.performance.cost.toFixed(4)}`
    );
  });

  session.on('optimization_round', (round) => {
    console.log(`\n🔧 Optimization Round ${round}`);
  });

  session.on('converged', (provider) => {
    console.log(`  ⭐ ${provider} has converged!`);
  });

  session.on('benchmark_progress', (data) => {
    console.log(`  📈 Benchmark progress: ${data.completed}/${data.total}`);
  });

  session.on('budget_exceeded', (cost) => {
    console.log(`  ⚠️  Cost budget exceeded: $${cost.toFixed(2)}`);
  });

  session.on('report', (data) => {
    console.log('\n📊 Final Report:\n');
    console.log(data.report);
    console.log(`\n🏆 Best Model: ${data.bestModel}`);
    console.log(`💰 Total Cost: $${data.totalCost.toFixed(4)}`);
    console.log(`⏱️  Duration: ${(data.duration / 1000).toFixed(2)}s`);
  });

  session.on('complete', (data) => {
    console.log('\n✅ Training complete!');
  });

  session.on('error', (error) => {
    console.error('❌ Error:', error);
  });

  // Run the training session
  const basePrompt = `
Generate a compelling product description that:
- Highlights key features and benefits
- Uses persuasive language
- Includes SEO keywords naturally
- Has a clear call-to-action
- Maintains professional tone

Product: {product_name}
  `.trim();

  await session.run(basePrompt, signature);

  // Get final statistics
  const stats = session.getStatistics();
  console.log('\n📊 Final Statistics:', JSON.stringify(stats, null, 2));
}

/**
 * Example 2: Advanced Training with Real-time Monitoring
 */
async function advancedTrainingExample() {
  console.log('🚀 Starting Advanced DSPy Training with Real-time Monitoring\n');

  const session = new DSPyTrainingSession({
    models: [
      {
        provider: ModelProvider.CLAUDE,
        model: 'claude-sonnet-4',
        apiKey: process.env.ANTHROPIC_API_KEY || 'sk-ant-test'
      },
      {
        provider: ModelProvider.GEMINI,
        model: 'gemini-2.0-flash-exp',
        apiKey: process.env.GEMINI_API_KEY || 'test-key'
      }
    ],
    optimizationRounds: 10,
    enableCrossLearning: true,
    enableHooksIntegration: true,
    costBudget: 5.0
  });

  // Real-time metrics tracking
  const metricsHistory: any[] = [];

  session.on('metrics', (metrics) => {
    metricsHistory.push({
      timestamp: Date.now(),
      ...metrics
    });

    // Calculate moving averages
    if (metricsHistory.length >= 5) {
      const recent = metricsHistory.slice(-5);
      const avgQuality = recent.reduce((sum, m) => sum + m.quality.score, 0) / 5;
      const avgLatency = recent.reduce((sum, m) => sum + m.performance.latency, 0) / 5;

      console.log(
        `  📊 Moving avg (last 5): Quality: ${avgQuality.toFixed(3)}, ` +
        `Latency: ${avgLatency.toFixed(0)}ms`
      );
    }
  });

  // Hooks integration monitoring
  session.on('hooks_integration', (data) => {
    console.log(`  🔗 Hooks integration: ${data.action} - ${data.key}`);
  });

  const optimizer = new OptimizationEngine();
  const signature = optimizer.createSignature(
    'code-generation',
    'Generate TypeScript code',
    'production-ready TypeScript code with types',
    {
      constraints: [
        'contains:type definitions',
        'contains:error handling',
        'min_length:50'
      ],
      objectives: [
        'Follow TypeScript best practices',
        'Include JSDoc comments',
        'Use modern ES6+ syntax'
      ]
    }
  );

  const basePrompt = `
Generate production-ready TypeScript code that:
- Uses strong typing
- Includes proper error handling
- Follows best practices
- Has clear documentation

Task: {task_description}
  `.trim();

  await session.run(basePrompt, signature);
}

/**
 * Example 3: Cost-Optimized Training
 */
async function costOptimizedTrainingExample() {
  console.log('🚀 Starting Cost-Optimized DSPy Training\n');

  // Use only cost-effective models
  const session = new DSPyTrainingSession({
    models: [
      {
        provider: ModelProvider.GEMINI,
        model: 'gemini-2.0-flash-exp',
        apiKey: process.env.GEMINI_API_KEY || 'test-key'
      },
      {
        provider: ModelProvider.LLAMA,
        model: 'llama-3.1-70b',
        apiKey: process.env.TOGETHER_API_KEY || 'test-key'
      }
    ],
    optimizationRounds: 3,
    baselineIterations: 2,
    benchmarkSamples: 20,
    costBudget: 1.0, // Strict $1 budget
    enableCrossLearning: true
  });

  // Track cost efficiency
  let totalIterations = 0;
  let totalQuality = 0;

  session.on('iteration', (result) => {
    totalIterations++;
    totalQuality += result.quality.score;

    const avgQuality = totalQuality / totalIterations;
    const costPerIteration = session.getStatistics().totalCost / totalIterations;

    console.log(
      `  💰 Iteration ${totalIterations}: ` +
      `Avg Quality: ${avgQuality.toFixed(3)}, ` +
      `Cost/iter: $${costPerIteration.toFixed(4)}`
    );
  });

  const optimizer = new OptimizationEngine();
  const signature = optimizer.createSignature(
    'summary',
    'Summarize text',
    'concise summary',
    {
      constraints: ['max_length:200'],
      objectives: ['Maintain key information', 'Use clear language']
    }
  );

  const basePrompt = 'Summarize the following text: {text}';

  await session.run(basePrompt, signature);

  const stats = session.getStatistics();
  console.log(`\n💰 Final cost efficiency: $${stats.totalCost.toFixed(4)} for ${totalIterations} iterations`);
}

/**
 * Example 4: Quality-Focused Training
 */
async function qualityFocusedTrainingExample() {
  console.log('🚀 Starting Quality-Focused DSPy Training\n');

  // Use high-quality models with aggressive optimization
  const session = new DSPyTrainingSession({
    models: [
      {
        provider: ModelProvider.CLAUDE,
        model: 'claude-sonnet-4',
        apiKey: process.env.ANTHROPIC_API_KEY || 'sk-ant-test',
        temperature: 0.3 // Lower temperature for consistency
      },
      {
        provider: ModelProvider.GPT4,
        model: 'gpt-4-turbo',
        apiKey: process.env.OPENAI_API_KEY || 'sk-test',
        temperature: 0.3
      }
    ],
    optimizationRounds: 15, // More rounds for quality
    convergenceThreshold: 0.98, // Higher threshold
    baselineIterations: 5,
    benchmarkSamples: 100,
    enableCrossLearning: true
  });

  // Quality monitoring
  let highQualityCount = 0;
  let totalCount = 0;

  session.on('iteration', (result) => {
    totalCount++;
    if (result.quality.score >= 0.9) {
      highQualityCount++;
    }

    const highQualityRate = highQualityCount / totalCount;
    console.log(
      `  ⭐ Quality rate: ${(highQualityRate * 100).toFixed(1)}% ` +
      `(${highQualityCount}/${totalCount} >= 0.9)`
    );
  });

  const optimizer = new OptimizationEngine();
  const signature = optimizer.createSignature(
    'technical-writing',
    'Write technical documentation',
    'clear, accurate technical documentation',
    {
      examples: [
        {
          input: 'Explain async/await in JavaScript',
          output: 'Async/await is a modern JavaScript feature that simplifies asynchronous code...'
        }
      ],
      constraints: [
        'min_length:200',
        'contains:code examples',
        'contains:best practices'
      ],
      objectives: [
        'Maximize clarity',
        'Include practical examples',
        'Follow technical writing standards',
        'Ensure accuracy'
      ]
    }
  );

  const basePrompt = `
Write clear, accurate technical documentation for:
{topic}

Requirements:
- Include code examples
- Explain concepts clearly
- Follow best practices
- Provide practical examples
  `.trim();

  await session.run(basePrompt, signature);
}

/**
 * Example 5: Benchmark Comparison
 */
async function benchmarkComparisonExample() {
  console.log('🚀 Starting Benchmark Comparison\n');

  const collector = new BenchmarkCollector();

  // Run training session
  const session = new DSPyTrainingSession({
    models: [
      {
        provider: ModelProvider.CLAUDE,
        model: 'claude-sonnet-4',
        apiKey: process.env.ANTHROPIC_API_KEY || 'sk-ant-test'
      },
      {
        provider: ModelProvider.GPT4,
        model: 'gpt-4-turbo',
        apiKey: process.env.OPENAI_API_KEY || 'sk-test'
      },
      {
        provider: ModelProvider.GEMINI,
        model: 'gemini-2.0-flash-exp',
        apiKey: process.env.GEMINI_API_KEY || 'test-key'
      },
      {
        provider: ModelProvider.LLAMA,
        model: 'llama-3.1-70b',
        apiKey: process.env.TOGETHER_API_KEY || 'test-key'
      }
    ],
    optimizationRounds: 5,
    benchmarkSamples: 50
  });

  session.on('iteration', (result) => {
    collector.addResult(result);
  });

  session.on('complete', () => {
    console.log('\n📊 Benchmark Comparison:\n');

    // Get comparison
    const comparison = collector.getComparison();

    // Display comparison table
    console.log('Model       | Iterations | Avg Quality | Avg Latency | Total Cost | Improvement');
    console.log('------------|------------|-------------|-------------|------------|------------');

    for (const [provider, stats] of Object.entries(comparison)) {
      if (!stats) continue;

      console.log(
        `${provider.padEnd(11)} | ` +
        `${String(stats.totalIterations).padEnd(10)} | ` +
        `${stats.avgQualityScore.toFixed(3).padEnd(11)} | ` +
        `${stats.avgLatency.toFixed(0).padEnd(11)}ms | ` +
        `$${stats.totalCost.toFixed(4).padEnd(9)} | ` +
        `${(stats.improvementRate * 100).toFixed(1)}%`
      );
    }

    // Highlight best model
    const bestModel = collector.getBestModel();
    console.log(`\n🏆 Winner: ${bestModel}`);

    // Generate full report
    console.log('\n' + collector.generateReport());
  });

  const optimizer = new OptimizationEngine();
  const signature = optimizer.createSignature(
    'general-task',
    'Complete task',
    'high-quality output'
  );

  const basePrompt = 'Complete the following task: {task}';

  await session.run(basePrompt, signature);
}

// ============================================================================
// Main Execution
// ============================================================================

async function main() {
  console.log('=' .repeat(80));
  console.log('DSPy.ts Training Session Examples');
  console.log('=' .repeat(80));
  console.log();

  const examples = [
    { name: 'Basic Training', fn: basicTrainingExample },
    { name: 'Advanced Monitoring', fn: advancedTrainingExample },
    { name: 'Cost-Optimized', fn: costOptimizedTrainingExample },
    { name: 'Quality-Focused', fn: qualityFocusedTrainingExample },
    { name: 'Benchmark Comparison', fn: benchmarkComparisonExample }
  ];

  // Run the example specified by command line arg, or default to first
  const exampleIndex = parseInt(process.argv[2] || '0');

  if (exampleIndex < 0 || exampleIndex >= examples.length) {
    console.log('Available examples:');
    examples.forEach((ex, i) => {
      console.log(`  ${i}: ${ex.name}`);
    });
    console.log(`\nUsage: node dspy-training-example.js [0-${examples.length - 1}]`);
    return;
  }

  const example = examples[exampleIndex];
  console.log(`Running: ${example.name}\n`);

  try {
    await example.fn();
  } catch (error) {
    console.error('\n❌ Example failed:', error);
    process.exit(1);
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}

export {
  basicTrainingExample,
  advancedTrainingExample,
  costOptimizedTrainingExample,
  qualityFocusedTrainingExample,
  benchmarkComparisonExample
};

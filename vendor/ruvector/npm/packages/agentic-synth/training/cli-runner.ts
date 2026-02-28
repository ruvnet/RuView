#!/usr/bin/env node
/**
 * DSPy Training Session CLI Runner
 *
 * Usage:
 *   npm run train:dspy -- --models claude,gemini --rounds 5 --budget 10
 *   node training/cli-runner.ts --models gpt4,llama --quality-focused
 */

import { Command } from 'commander';
import {
  DSPyTrainingSession,
  ModelProvider,
  OptimizationEngine,
  type ModelConfig
} from './dspy-learning-session.js';
import * as fs from 'fs';
import * as path from 'path';

const program = new Command();

program
  .name('dspy-trainer')
  .description('DSPy.ts multi-model training CLI')
  .version('1.0.0');

program
  .command('train')
  .description('Run DSPy training session')
  .option('-m, --models <models>', 'Comma-separated model providers (claude,gpt4,gemini,llama)', 'gemini,claude')
  .option('-r, --rounds <number>', 'Optimization rounds', '5')
  .option('-b, --budget <number>', 'Cost budget in USD', '10')
  .option('-s, --samples <number>', 'Benchmark samples', '50')
  .option('-c, --convergence <number>', 'Convergence threshold', '0.95')
  .option('-p, --prompt <prompt>', 'Base prompt template')
  .option('-o, --output <file>', 'Output report file', 'dspy-training-report.md')
  .option('--quality-focused', 'Use quality-focused configuration')
  .option('--cost-optimized', 'Use cost-optimized configuration')
  .option('--disable-cross-learning', 'Disable cross-model learning')
  .option('--disable-hooks', 'Disable hooks integration')
  .option('--verbose', 'Verbose logging')
  .action(async (options) => {
    try {
      console.log('🚀 Starting DSPy Training Session\n');

      // Parse model providers
      const modelProviders = options.models.split(',').map((m: string) =>
        m.trim().toLowerCase() as ModelProvider
      );

      // Build model configurations
      const models: ModelConfig[] = [];

      for (const provider of modelProviders) {
        const config = buildModelConfig(provider, options);
        if (config) {
          models.push(config);
          console.log(`✓ Configured ${provider}: ${config.model}`);
        }
      }

      if (models.length === 0) {
        console.error('❌ No valid models configured');
        process.exit(1);
      }

      console.log('');

      // Build training configuration
      const trainingConfig = {
        models,
        optimizationRounds: parseInt(options.rounds),
        convergenceThreshold: parseFloat(options.convergence),
        maxConcurrency: models.length,
        enableCrossLearning: !options.disableCrossLearning,
        enableHooksIntegration: !options.disableHooks,
        costBudget: parseFloat(options.budget),
        timeoutPerIteration: 30000,
        baselineIterations: options.qualityFocused ? 5 : 3,
        benchmarkSamples: parseInt(options.samples)
      };

      // Apply presets
      if (options.qualityFocused) {
        console.log('📊 Using quality-focused configuration');
        trainingConfig.optimizationRounds = 15;
        trainingConfig.convergenceThreshold = 0.98;
        trainingConfig.benchmarkSamples = 100;
      }

      if (options.costOptimized) {
        console.log('💰 Using cost-optimized configuration');
        trainingConfig.optimizationRounds = 3;
        trainingConfig.baselineIterations = 2;
        trainingConfig.benchmarkSamples = 20;
      }

      // Create session
      const session = new DSPyTrainingSession(trainingConfig);

      // Set up event handlers
      setupEventHandlers(session, options);

      // Create optimizer and signature
      const optimizer = new OptimizationEngine();

      // Use custom prompt or default
      const basePrompt = options.prompt || `
Generate high-quality output that is:
- Clear and well-structured
- Accurate and relevant
- Engaging and professional
- Appropriate for the context

Task: {task_description}
      `.trim();

      const signature = optimizer.createSignature(
        'general-task',
        'Complete the given task',
        'High-quality completion',
        {
          constraints: ['min_length:50'],
          objectives: [
            'Maximize clarity',
            'Ensure accuracy',
            'Maintain professional tone'
          ]
        }
      );

      // Run training
      console.log('🎯 Starting training pipeline...\n');

      const reportData: any = {
        config: trainingConfig,
        iterations: [],
        phases: [],
        finalStats: null
      };

      session.on('iteration', (result) => {
        reportData.iterations.push(result);
      });

      session.on('phase', (phase) => {
        reportData.phases.push(phase);
      });

      session.on('complete', (data) => {
        reportData.finalStats = data;

        console.log('\n✅ Training Complete!\n');
        console.log(data.report);

        // Save report to file
        const reportPath = path.resolve(options.output);
        const report = generateMarkdownReport(reportData);

        fs.writeFileSync(reportPath, report, 'utf-8');
        console.log(`\n📄 Report saved to: ${reportPath}`);

        process.exit(0);
      });

      session.on('error', (error) => {
        console.error('\n❌ Training failed:', error);
        process.exit(1);
      });

      await session.run(basePrompt, signature);

    } catch (error) {
      console.error('❌ Error:', error);
      process.exit(1);
    }
  });

program
  .command('presets')
  .description('List available training presets')
  .action(() => {
    console.log('Available Presets:\n');

    console.log('📊 --quality-focused');
    console.log('   - 15 optimization rounds');
    console.log('   - 0.98 convergence threshold');
    console.log('   - 100 benchmark samples');
    console.log('   - Best for production use\n');

    console.log('💰 --cost-optimized');
    console.log('   - 3 optimization rounds');
    console.log('   - 2 baseline iterations');
    console.log('   - 20 benchmark samples');
    console.log('   - Best for experimentation\n');

    console.log('⚡ Default');
    console.log('   - 5 optimization rounds');
    console.log('   - 0.95 convergence threshold');
    console.log('   - 50 benchmark samples');
    console.log('   - Balanced configuration\n');
  });

program
  .command('models')
  .description('List available model providers')
  .action(() => {
    console.log('Available Models:\n');

    console.log('🤖 claude - Claude Sonnet 4');
    console.log('   API Key: ANTHROPIC_API_KEY');
    console.log('   Cost: $0.003 per 1K tokens');
    console.log('   Best for: Quality, reasoning\n');

    console.log('🤖 gpt4 - GPT-4 Turbo');
    console.log('   API Key: OPENAI_API_KEY');
    console.log('   Cost: $0.03 per 1K tokens');
    console.log('   Best for: Complex tasks, accuracy\n');

    console.log('🤖 gemini - Gemini 2.0 Flash');
    console.log('   API Key: GEMINI_API_KEY');
    console.log('   Cost: $0.00025 per 1K tokens');
    console.log('   Best for: Cost efficiency, speed\n');

    console.log('🤖 llama - Llama 3.1 70B');
    console.log('   API Key: TOGETHER_API_KEY');
    console.log('   Cost: $0.0002 per 1K tokens');
    console.log('   Best for: Open source, low cost\n');
  });

program.parse();

// Helper functions

function buildModelConfig(provider: ModelProvider, options: any): ModelConfig | null {
  const baseConfig = {
    provider,
    apiKey: '',
    temperature: options.qualityFocused ? 0.3 : 0.7
  };

  switch (provider) {
    case ModelProvider.CLAUDE:
      return {
        ...baseConfig,
        model: 'claude-sonnet-4',
        apiKey: process.env.ANTHROPIC_API_KEY || ''
      };

    case ModelProvider.GPT4:
      return {
        ...baseConfig,
        model: 'gpt-4-turbo',
        apiKey: process.env.OPENAI_API_KEY || ''
      };

    case ModelProvider.GEMINI:
      return {
        ...baseConfig,
        model: 'gemini-2.0-flash-exp',
        apiKey: process.env.GEMINI_API_KEY || ''
      };

    case ModelProvider.LLAMA:
      return {
        ...baseConfig,
        model: 'llama-3.1-70b',
        apiKey: process.env.TOGETHER_API_KEY || ''
      };

    default:
      console.warn(`⚠️  Unknown model provider: ${provider}`);
      return null;
  }
}

function setupEventHandlers(session: DSPyTrainingSession, options: any): void {
  const verbose = options.verbose;

  session.on('start', (data) => {
    console.log(`📊 Training started - Phase: ${data.phase}`);
  });

  session.on('phase', (phase) => {
    console.log(`\n🔄 Phase: ${phase.toUpperCase()}`);
  });

  session.on('iteration', (result) => {
    if (verbose) {
      console.log(
        `  ${result.modelProvider.padEnd(8)} | ` +
        `Iter ${String(result.iteration).padStart(3)} | ` +
        `Q: ${result.quality.score.toFixed(3)} | ` +
        `L: ${result.performance.latency.toFixed(0).padStart(4)}ms | ` +
        `$${result.performance.cost.toFixed(4)}`
      );
    } else {
      // Progress dots
      process.stdout.write('.');
    }
  });

  session.on('optimization_round', (round) => {
    if (!verbose) console.log('');
    console.log(`\n🔧 Optimization Round ${round}`);
  });

  session.on('converged', (provider) => {
    console.log(`  ⭐ ${provider} converged!`);
  });

  session.on('benchmark_progress', (data) => {
    if (data.completed % 10 === 0) {
      console.log(`  📈 Benchmark: ${data.completed}/${data.total}`);
    }
  });

  session.on('budget_exceeded', (cost) => {
    console.log(`  ⚠️  Budget exceeded: $${cost.toFixed(2)}`);
  });

  session.on('metrics', (metrics) => {
    if (verbose) {
      console.log(`  📊 ${metrics.provider}: Quality=${metrics.quality.score.toFixed(3)}`);
    }
  });
}

function generateMarkdownReport(data: any): string {
  let report = '# DSPy Training Session Report\n\n';
  report += `Generated: ${new Date().toISOString()}\n\n`;

  report += '## Configuration\n\n';
  report += '```json\n';
  report += JSON.stringify(data.config, null, 2);
  report += '\n```\n\n';

  report += '## Training Summary\n\n';
  report += `- Total Iterations: ${data.iterations.length}\n`;
  report += `- Phases Completed: ${data.phases.length}\n`;

  if (data.finalStats) {
    report += `- Best Model: ${data.finalStats.bestModel}\n`;
    report += `- Total Cost: $${data.finalStats.totalCost.toFixed(4)}\n`;
    report += `- Duration: ${(data.finalStats.duration / 1000).toFixed(2)}s\n\n`;
  }

  report += '## Detailed Report\n\n';
  if (data.finalStats && data.finalStats.report) {
    report += data.finalStats.report;
  }

  report += '\n## Iteration Details\n\n';
  report += '| Iteration | Model | Phase | Quality | Latency | Cost |\n';
  report += '|-----------|-------|-------|---------|---------|------|\n';

  data.iterations.slice(-20).forEach((iter: any) => {
    report += `| ${iter.iteration} | ${iter.modelProvider} | ${iter.phase} | `;
    report += `${iter.quality.score.toFixed(3)} | ${iter.performance.latency.toFixed(0)}ms | `;
    report += `$${iter.performance.cost.toFixed(4)} |\n`;
  });

  return report;
}

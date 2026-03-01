/**
 * Simple test to verify dspy.ts integration works at runtime
 */

import { DSPyAgenticSynthTrainer } from './dspy-real-integration.js';

async function test() {
  console.log('🧪 Testing DSPy.ts Real Integration\n');

  // Simple schema
  const schema = {
    type: 'object',
    properties: {
      id: { type: 'string' },
      name: { type: 'string' },
      value: { type: 'number' }
    }
  };

  // Simple examples
  const examples = [
    {
      input: JSON.stringify(schema),
      output: JSON.stringify({ id: '1', name: 'Test', value: 42 }),
      quality: 0.9
    }
  ];

  try {
    // Create trainer
    console.log('✓ Creating trainer...');
    const trainer = new DSPyAgenticSynthTrainer({
      models: ['gpt-3.5-turbo'],
      optimizationRounds: 2,
      minQualityScore: 0.7,
      batchSize: 3
    });

    console.log('✓ Trainer created');

    // Check if API key is set
    if (!process.env.OPENAI_API_KEY) {
      console.log('\n⚠️  OPENAI_API_KEY not set. Skipping initialization test.');
      console.log('   Set OPENAI_API_KEY to test full functionality.\n');
      console.log('✅ Integration code structure is valid!');
      return;
    }

    // Initialize
    console.log('✓ Initializing DSPy.ts...');
    await trainer.initialize();
    console.log('✓ Initialization complete\n');

    // Get stats
    const stats = trainer.getStatistics();
    console.log('📊 Statistics:');
    console.log(`   Total Iterations: ${stats.totalIterations}`);
    console.log(`   Best Score: ${stats.bestScore}`);
    console.log(`   Training Examples: ${stats.trainingExamples}`);

    console.log('\n✅ All tests passed!');

  } catch (error: any) {
    console.error('\n❌ Test failed:', error.message);
    if (error.details) {
      console.error('Details:', error.details);
    }
    process.exit(1);
  }
}

test().catch(console.error);

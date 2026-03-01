#!/usr/bin/env node
/**
 * Quick test to verify dspy-multi-model-benchmark imports work correctly
 */

console.log('🔍 Testing DSPy Multi-Model Benchmark imports...\n');

try {
  // Test dspy.ts import
  console.log('1. Testing dspy.ts import...');
  const dspy = require('dspy.ts/dist/src/index');
  console.log('   ✓ dspy.ts imported successfully');

  // Check required exports
  const required = [
    'configureLM',
    'getLM',
    'PredictModule',
    'ChainOfThought',
    'BootstrapFewShot',
    'MIPROv2',
    'exactMatch',
    'f1Score',
    'bleuScore',
    'rougeL'
  ];

  console.log('\n2. Checking required exports...');
  let missing = [];
  for (const name of required) {
    if (name in dspy) {
      console.log(`   ✓ ${name}`);
    } else {
      console.log(`   ✗ ${name} - MISSING`);
      missing.push(name);
    }
  }

  if (missing.length > 0) {
    console.log(`\n❌ Missing exports: ${missing.join(', ')}`);
    process.exit(1);
  }

  console.log('\n3. Testing module instantiation...');

  // Test PredictModule
  const predict = new dspy.PredictModule({
    name: 'TestModule',
    signature: {
      inputs: [{ name: 'text', type: 'string' }],
      outputs: [{ name: 'result', type: 'string' }]
    },
    promptTemplate: ({ text }) => `Process: ${text}`
  });
  console.log('   ✓ PredictModule instantiated');

  // Test ChainOfThought
  const cot = new dspy.ChainOfThought({
    name: 'TestCoT',
    signature: {
      inputs: [{ name: 'question', type: 'string' }],
      outputs: [{ name: 'answer', type: 'string' }]
    }
  });
  console.log('   ✓ ChainOfThought instantiated');

  console.log('\n✅ All imports and instantiations successful!');
  console.log('\n📝 Next steps:');
  console.log('   1. Set API keys: OPENAI_API_KEY and/or ANTHROPIC_API_KEY');
  console.log('   2. Run benchmark: npx tsx training/dspy-multi-model-benchmark.ts');
  console.log('   3. Or use helper script: ./training/run-multi-model-benchmark.sh\n');

} catch (error) {
  console.error('\n❌ Test failed:', error.message);
  console.error('\nStack trace:');
  console.error(error.stack);
  process.exit(1);
}

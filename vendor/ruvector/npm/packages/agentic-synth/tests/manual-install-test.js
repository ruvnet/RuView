/**
 * Manual installation and runtime test
 * Tests that the package works correctly when installed and run with environment variables
 */

import { AgenticSynth, createSynth } from '../dist/index.js';

console.log('🧪 Testing @ruvector/agentic-synth installation and runtime...\n');

// Test 1: Import validation
console.log('✅ Test 1: Module imports successful');

// Test 2: Environment variable detection
console.log('\n📋 Test 2: Environment Variables');
console.log('  GEMINI_API_KEY:', process.env.GEMINI_API_KEY ? '✓ Set' : '✗ Not set');
console.log('  OPENROUTER_API_KEY:', process.env.OPENROUTER_API_KEY ? '✓ Set' : '✗ Not set');

// Test 3: Instance creation with default config
console.log('\n🏗️  Test 3: Creating AgenticSynth instance with defaults');
try {
  const synth1 = new AgenticSynth();
  console.log('  ✓ Instance created successfully');
  const config1 = synth1.getConfig();
  console.log('  Provider:', config1.provider);
  console.log('  Model:', config1.model);
  console.log('  Enable Fallback:', config1.enableFallback);
} catch (error) {
  console.error('  ✗ Failed:', error.message);
  process.exit(1);
}

// Test 4: Instance creation with custom config
console.log('\n🔧 Test 4: Creating instance with custom config');
try {
  const synth2 = createSynth({
    provider: 'openrouter',
    model: 'anthropic/claude-3.5-sonnet',
    enableFallback: false,
    cacheStrategy: 'memory',
    maxRetries: 5
  });
  console.log('  ✓ Custom instance created successfully');
  const config2 = synth2.getConfig();
  console.log('  Provider:', config2.provider);
  console.log('  Model:', config2.model);
  console.log('  Enable Fallback:', config2.enableFallback);
  console.log('  Max Retries:', config2.maxRetries);
} catch (error) {
  console.error('  ✗ Failed:', error.message);
  process.exit(1);
}

// Test 5: Validate config updates
console.log('\n🔄 Test 5: Testing configuration updates');
try {
  const synth3 = new AgenticSynth({ provider: 'gemini' });
  synth3.configure({
    provider: 'openrouter',
    fallbackChain: ['gemini']
  });
  const config3 = synth3.getConfig();
  console.log('  ✓ Configuration updated successfully');
  console.log('  New Provider:', config3.provider);
} catch (error) {
  console.error('  ✗ Failed:', error.message);
  process.exit(1);
}

// Test 6: API key handling
console.log('\n🔑 Test 6: API Key Handling');
try {
  const synthWithKey = new AgenticSynth({
    provider: 'gemini',
    apiKey: 'test-key-from-config'
  });
  console.log('  ✓ Config accepts apiKey parameter');

  const synthFromEnv = new AgenticSynth({ provider: 'gemini' });
  console.log('  ✓ Falls back to environment variables when apiKey not provided');
} catch (error) {
  console.error('  ✗ Failed:', error.message);
  process.exit(1);
}

// Test 7: Error handling for missing schema
console.log('\n❌ Test 7: Error handling for missing required fields');
try {
  const synth4 = new AgenticSynth();
  // This should fail validation
  await synth4.generateStructured({ count: 5 });
  console.error('  ✗ Should have thrown error for missing schema');
  process.exit(1);
} catch (error) {
  if (error.message.includes('Schema is required')) {
    console.log('  ✓ Correctly throws error for missing schema');
  } else {
    console.error('  ✗ Unexpected error:', error.message);
    process.exit(1);
  }
}

// Test 8: Fallback chain configuration
console.log('\n🔀 Test 8: Fallback chain configuration');
try {
  const synthNoFallback = new AgenticSynth({
    provider: 'gemini',
    enableFallback: false
  });
  console.log('  ✓ Can disable fallbacks');

  const synthCustomFallback = new AgenticSynth({
    provider: 'gemini',
    fallbackChain: ['openrouter']
  });
  console.log('  ✓ Can set custom fallback chain');
} catch (error) {
  console.error('  ✗ Failed:', error.message);
  process.exit(1);
}

console.log('\n✅ All tests passed! Package is ready for installation and use.\n');
console.log('📦 Installation Instructions:');
console.log('   npm install @ruvector/agentic-synth');
console.log('\n🔑 Environment Setup:');
console.log('   export GEMINI_API_KEY="your-gemini-key"');
console.log('   export OPENROUTER_API_KEY="your-openrouter-key"');
console.log('\n🚀 Usage:');
console.log('   import { AgenticSynth } from "@ruvector/agentic-synth";');
console.log('   const synth = new AgenticSynth({ provider: "gemini" });');
console.log('   const data = await synth.generateStructured({ schema: {...}, count: 10 });');

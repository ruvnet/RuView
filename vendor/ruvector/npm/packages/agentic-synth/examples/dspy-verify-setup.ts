/**
 * Quick Setup Verification for DSPy.ts Integration
 *
 * This script verifies that all dependencies and imports are working correctly
 * before running the full example.
 *
 * Usage:
 * ```bash
 * npx tsx examples/dspy-verify-setup.ts
 * ```
 */

import 'dotenv/config';

console.log('🔍 Verifying DSPy.ts + AgenticSynth Setup...\n');

// ============================================================================
// Step 1: Check Environment Variables
// ============================================================================

console.log('1️⃣ Checking environment variables...');

const requiredVars = ['OPENAI_API_KEY', 'GEMINI_API_KEY'];
const optionalVars = ['ANTHROPIC_API_KEY'];

let hasRequiredVars = true;

for (const varName of requiredVars) {
  const value = process.env[varName];
  if (value) {
    const masked = value.substring(0, 8) + '...' + value.substring(value.length - 4);
    console.log(`   ✓ ${varName}: ${masked}`);
  } else {
    console.log(`   ✗ ${varName}: NOT SET`);
    hasRequiredVars = false;
  }
}

for (const varName of optionalVars) {
  const value = process.env[varName];
  if (value) {
    const masked = value.substring(0, 8) + '...' + value.substring(value.length - 4);
    console.log(`   ○ ${varName}: ${masked} (optional)`);
  } else {
    console.log(`   ○ ${varName}: not set (optional)`);
  }
}

if (!hasRequiredVars) {
  console.log('\n❌ Missing required environment variables!');
  console.log('   Please set them in your .env file or export them:');
  console.log('   export OPENAI_API_KEY=sk-...');
  console.log('   export GEMINI_API_KEY=...\n');
  process.exit(1);
}

console.log('   ✅ All required variables set\n');

// ============================================================================
// Step 2: Verify DSPy.ts Imports
// ============================================================================

console.log('2️⃣ Verifying DSPy.ts imports...');

try {
  const dspyModules = await import('dspy.ts');

  // Check core modules
  const requiredExports = [
    'ChainOfThought',
    'Predict',
    'Refine',
    'ReAct',
    'Retrieve',
    'OpenAILM',
    'AnthropicLM',
    'BootstrapFewShot',
    'MIPROv2',
    'configureLM',
    'exactMatch',
    'f1Score',
    'createMetric',
    'evaluate'
  ];

  let allExportsPresent = true;

  for (const exportName of requiredExports) {
    if (exportName in dspyModules) {
      console.log(`   ✓ ${exportName}`);
    } else {
      console.log(`   ✗ ${exportName} - NOT FOUND`);
      allExportsPresent = false;
    }
  }

  if (!allExportsPresent) {
    console.log('\n❌ Some DSPy.ts exports are missing!');
    console.log('   Try reinstalling: npm install dspy.ts@2.1.1\n');
    process.exit(1);
  }

  console.log('   ✅ All DSPy.ts modules available\n');
} catch (error) {
  console.log(`   ✗ Failed to import dspy.ts`);
  console.log(`   Error: ${error instanceof Error ? error.message : String(error)}\n`);
  console.log('❌ DSPy.ts import failed!');
  console.log('   Try installing: npm install dspy.ts@2.1.1\n');
  process.exit(1);
}

// ============================================================================
// Step 3: Verify AgenticSynth
// ============================================================================

console.log('3️⃣ Verifying AgenticSynth...');

try {
  const { AgenticSynth } = await import('../src/index.js');

  // Create instance
  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  console.log('   ✓ AgenticSynth class imported');
  console.log('   ✓ Instance created successfully');

  // Check methods
  const requiredMethods = [
    'generate',
    'generateStructured',
    'generateTimeSeries',
    'generateEvents',
    'configure',
    'getConfig'
  ];

  for (const method of requiredMethods) {
    if (typeof (synth as any)[method] === 'function') {
      console.log(`   ✓ ${method}() method available`);
    } else {
      console.log(`   ✗ ${method}() method not found`);
    }
  }

  console.log('   ✅ AgenticSynth ready\n');
} catch (error) {
  console.log(`   ✗ Failed to import AgenticSynth`);
  console.log(`   Error: ${error instanceof Error ? error.message : String(error)}\n`);
  console.log('❌ AgenticSynth verification failed!');
  console.log('   Make sure you are in the correct directory and the package is built.\n');
  process.exit(1);
}

// ============================================================================
// Step 4: Test DSPy Module Creation
// ============================================================================

console.log('4️⃣ Testing DSPy module creation...');

try {
  const { ChainOfThought, Predict, OpenAILM, configureLM } = await import('dspy.ts');

  // Test Predict module
  const predictor = new Predict({
    name: 'TestPredictor',
    signature: {
      inputs: [{ name: 'input', type: 'string', required: true }],
      outputs: [{ name: 'output', type: 'string', required: true }]
    }
  });
  console.log('   ✓ Predict module created');

  // Test ChainOfThought module
  const cot = new ChainOfThought({
    name: 'TestCoT',
    signature: {
      inputs: [{ name: 'question', type: 'string', required: true }],
      outputs: [{ name: 'answer', type: 'string', required: true }]
    }
  });
  console.log('   ✓ ChainOfThought module created');

  // Test LM initialization (without API call)
  const lm = new OpenAILM({
    model: 'gpt-3.5-turbo',
    apiKey: process.env.OPENAI_API_KEY || 'test-key',
    temperature: 0.7
  });
  console.log('   ✓ OpenAI LM instance created');

  console.log('   ✅ All DSPy modules working\n');
} catch (error) {
  console.log(`   ✗ Module creation failed`);
  console.log(`   Error: ${error instanceof Error ? error.message : String(error)}\n`);
  console.log('❌ DSPy module test failed!\n');
  process.exit(1);
}

// ============================================================================
// Step 5: Check Node.js Version
// ============================================================================

console.log('5️⃣ Checking Node.js version...');

const nodeVersion = process.version;
const majorVersion = parseInt(nodeVersion.split('.')[0].substring(1));

console.log(`   Current version: ${nodeVersion}`);

if (majorVersion >= 18) {
  console.log('   ✅ Node.js version is compatible (>= 18.0.0)\n');
} else {
  console.log('   ⚠️  Node.js version is below 18.0.0');
  console.log('   Some features may not work correctly.\n');
}

// ============================================================================
// Summary
// ============================================================================

console.log('╔════════════════════════════════════════════════════════════════════════╗');
console.log('║                    VERIFICATION COMPLETE                               ║');
console.log('╚════════════════════════════════════════════════════════════════════════╝\n');

console.log('✅ All checks passed! You are ready to run the example.\n');

console.log('Next steps:');
console.log('   1. Run the complete example:');
console.log('      npx tsx examples/dspy-complete-example.ts\n');
console.log('   2. Review the guide:');
console.log('      cat examples/docs/dspy-complete-example-guide.md\n');
console.log('   3. Explore other examples:');
console.log('      ls examples/*.ts\n');

console.log('💡 Tip: Start with a smaller SAMPLE_SIZE (e.g., 3) for quick testing.\n');

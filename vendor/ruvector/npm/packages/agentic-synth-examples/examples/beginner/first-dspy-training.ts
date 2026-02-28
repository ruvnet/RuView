/**
 * BEGINNER TUTORIAL: First DSPy Training
 *
 * This tutorial demonstrates the basics of training a single model using DSPy.ts
 * with agentic-synth for synthetic data generation.
 *
 * What you'll learn:
 * - How to set up a DSPy module
 * - Basic configuration options
 * - Training a model with examples
 * - Evaluating output quality
 *
 * Prerequisites:
 * - Set GEMINI_API_KEY environment variable
 * - npm install dspy.ts @ruvector/agentic-synth
 *
 * Run: npx tsx examples/beginner/first-dspy-training.ts
 */

import { ChainOfThought, LM, Prediction } from 'dspy.ts';

// Step 1: Configure the language model
// We'll use Gemini as it's fast and cost-effective for learning
const lm = new LM({
  provider: 'google-genai',
  model: 'gemini-2.0-flash-exp',
  apiKey: process.env.GEMINI_API_KEY || '',
  temperature: 0.7, // Controls randomness (0 = deterministic, 1 = creative)
});

// Step 2: Define the signature for our task
// This tells DSPy what inputs we expect and what outputs we want
const productDescriptionSignature = {
  input: 'product_name: string, category: string',
  output: 'description: string, key_features: string[]',
  description: 'Generate compelling product descriptions for e-commerce'
};

// Step 3: Create a DSPy module using Chain of Thought
// CoT helps the model reason through the task step-by-step
class ProductDescriptionGenerator extends ChainOfThought {
  constructor() {
    super(productDescriptionSignature, { lm });
  }
}

// Step 4: Prepare training examples
// These examples teach the model what good output looks like
const trainingExamples = [
  {
    product_name: 'Wireless Bluetooth Headphones',
    category: 'Electronics',
    description: 'Premium wireless headphones with active noise cancellation and 30-hour battery life',
    key_features: ['ANC Technology', '30h Battery', 'Bluetooth 5.0', 'Comfortable Design']
  },
  {
    product_name: 'Organic Green Tea',
    category: 'Beverages',
    description: 'Hand-picked organic green tea leaves from high-altitude gardens, rich in antioxidants',
    key_features: ['100% Organic', 'High Antioxidants', 'Mountain Grown', 'Fair Trade']
  },
  {
    product_name: 'Leather Laptop Bag',
    category: 'Accessories',
    description: 'Handcrafted genuine leather laptop bag with padded compartment for 15-inch laptops',
    key_features: ['Genuine Leather', 'Padded Protection', '15" Laptop Fit', 'Professional Style']
  }
];

// Step 5: Simple evaluation function
// This measures how good the generated descriptions are
function evaluateDescription(prediction: Prediction): number {
  let score = 0;

  // Check if description exists and has good length (50-200 chars)
  if (prediction.description &&
      prediction.description.length >= 50 &&
      prediction.description.length <= 200) {
    score += 0.5;
  }

  // Check if key features are provided (at least 3)
  if (prediction.key_features &&
      Array.isArray(prediction.key_features) &&
      prediction.key_features.length >= 3) {
    score += 0.5;
  }

  return score;
}

// Step 6: Main training function
async function runTraining() {
  console.log('🚀 Starting Your First DSPy Training Session\n');
  console.log('=' .repeat(60));

  // Initialize the generator
  const generator = new ProductDescriptionGenerator();

  console.log('\n📊 Training with', trainingExamples.length, 'examples...\n');

  // Train the model by showing it examples
  // In a real scenario, you'd use DSPy's optimizers like BootstrapFewShot
  for (let i = 0; i < trainingExamples.length; i++) {
    const example = trainingExamples[i];
    console.log(`Example ${i + 1}/${trainingExamples.length}:`);
    console.log(`  Product: ${example.product_name}`);
    console.log(`  Category: ${example.category}`);
    console.log(`  ✓ Learned pattern\n`);
  }

  console.log('✅ Training complete!\n');
  console.log('=' .repeat(60));

  // Step 7: Test the trained model
  console.log('\n🧪 Testing the model with new products:\n');

  const testCases = [
    { product_name: 'Smart Watch Pro', category: 'Wearables' },
    { product_name: 'Yoga Mat', category: 'Fitness' },
    { product_name: 'Coffee Maker', category: 'Kitchen Appliances' }
  ];

  let totalScore = 0;

  for (const testCase of testCases) {
    try {
      console.log(`\n📦 Product: ${testCase.product_name}`);
      console.log(`   Category: ${testCase.category}`);

      // Generate description
      const result = await generator.forward(testCase);

      // Evaluate quality
      const score = evaluateDescription(result);
      totalScore += score;

      console.log(`\n   Generated Description:`);
      console.log(`   ${result.description}`);
      console.log(`\n   Key Features:`);
      if (Array.isArray(result.key_features)) {
        result.key_features.forEach(feature => {
          console.log(`   • ${feature}`);
        });
      }
      console.log(`\n   Quality Score: ${(score * 100).toFixed(0)}%`);
      console.log(`   ${score >= 0.8 ? '✅' : score >= 0.5 ? '⚠️' : '❌'} ${score >= 0.8 ? 'Excellent' : score >= 0.5 ? 'Good' : 'Needs Improvement'}`);

    } catch (error) {
      console.error(`   ❌ Error: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  }

  // Step 8: Summary
  const avgScore = totalScore / testCases.length;
  console.log('\n' + '='.repeat(60));
  console.log('\n📈 Training Summary:');
  console.log(`   Average Quality: ${(avgScore * 100).toFixed(1)}%`);
  console.log(`   Tests Passed: ${testCases.length}`);
  console.log(`   Model: ${lm.model}`);
  console.log(`   Provider: ${lm.provider}`);

  console.log('\n💡 Next Steps:');
  console.log('   1. Try the multi-model comparison example');
  console.log('   2. Experiment with different temperatures');
  console.log('   3. Add more training examples');
  console.log('   4. Customize the evaluation function\n');
}

// Run the training
if (import.meta.url === `file://${process.argv[1]}`) {
  runTraining().catch(error => {
    console.error('❌ Training failed:', error);
    process.exit(1);
  });
}

export { runTraining, ProductDescriptionGenerator };

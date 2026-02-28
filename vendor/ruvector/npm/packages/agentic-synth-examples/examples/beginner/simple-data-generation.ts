/**
 * BEGINNER TUTORIAL: Simple Data Generation
 *
 * Learn how to generate structured synthetic data with agentic-synth.
 * Perfect for creating test data, mock APIs, or prototyping.
 *
 * What you'll learn:
 * - Defining data schemas
 * - Generating structured data
 * - Saving output to files
 * - Working with different formats
 *
 * Prerequisites:
 * - Set GEMINI_API_KEY environment variable
 * - npm install @ruvector/agentic-synth
 *
 * Run: npx tsx examples/beginner/simple-data-generation.ts
 */

import { AgenticSynth } from '@ruvector/agentic-synth';
import { writeFileSync } from 'fs';
import { join } from 'path';

// Step 1: Define your data schema
// This is like a blueprint for the data you want to generate
const userSchema = {
  // Basic fields with types
  id: { type: 'string', required: true },
  name: { type: 'string', required: true },
  email: { type: 'string', required: true },
  age: { type: 'number', required: true, minimum: 18, maximum: 80 },

  // Enum fields (restricted choices)
  role: {
    type: 'string',
    required: true,
    enum: ['user', 'admin', 'moderator']
  },

  // Nested object
  address: {
    type: 'object',
    required: false,
    properties: {
      street: { type: 'string' },
      city: { type: 'string' },
      country: { type: 'string' },
      postalCode: { type: 'string' }
    }
  },

  // Array field
  interests: {
    type: 'array',
    required: false,
    items: { type: 'string' }
  }
};

// Step 2: Initialize AgenticSynth
// We're using Gemini because it's fast and cost-effective
const synth = new AgenticSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY,
  model: 'gemini-2.0-flash-exp',
  cacheStrategy: 'memory', // Cache results to save API calls
  cacheTTL: 3600 // Cache for 1 hour
});

// Step 3: Main generation function
async function generateUserData() {
  console.log('🎯 Simple Data Generation Tutorial\n');
  console.log('=' .repeat(60));

  // Step 3a: Generate a small batch first (5 users)
  console.log('\n📊 Generating 5 sample users...\n');

  try {
    const result = await synth.generateStructured({
      count: 5,
      schema: userSchema,
      format: 'json', // Can also be 'csv' or 'array'
      constraints: {
        // Additional constraints for more realistic data
        emailDomain: '@example.com',
        nameFormat: 'FirstName LastName',
        countryList: ['USA', 'UK', 'Canada', 'Australia']
      }
    });

    // Step 4: Display the results
    console.log('✅ Generation Complete!\n');
    console.log(`Generated ${result.metadata.count} users in ${result.metadata.duration}ms`);
    console.log(`Provider: ${result.metadata.provider}`);
    console.log(`Model: ${result.metadata.model}`);
    console.log(`Cached: ${result.metadata.cached ? 'Yes ⚡' : 'No'}\n`);

    // Show the generated data
    console.log('👥 Generated Users:\n');
    result.data.forEach((user: any, index: number) => {
      console.log(`${index + 1}. ${user.name} (${user.role})`);
      console.log(`   📧 ${user.email}`);
      console.log(`   🎂 Age: ${user.age}`);
      if (user.address) {
        console.log(`   🏠 ${user.address.city}, ${user.address.country}`);
      }
      if (user.interests && user.interests.length > 0) {
        console.log(`   ❤️  Interests: ${user.interests.join(', ')}`);
      }
      console.log('');
    });

    // Step 5: Save to file
    const outputDir = join(process.cwd(), 'examples', 'output');
    const outputFile = join(outputDir, 'sample-users.json');

    try {
      // Create output directory if it doesn't exist
      const { mkdirSync } = await import('fs');
      mkdirSync(outputDir, { recursive: true });

      // Save the data
      writeFileSync(outputFile, JSON.stringify(result.data, null, 2));
      console.log(`💾 Data saved to: ${outputFile}\n`);
    } catch (error) {
      console.warn('⚠️  Could not save file:', error instanceof Error ? error.message : 'Unknown error');
    }

    // Step 6: Generate a larger batch
    console.log('=' .repeat(60));
    console.log('\n📈 Now generating 20 users (to demonstrate scaling)...\n');

    const largeResult = await synth.generateStructured({
      count: 20,
      schema: userSchema,
      format: 'json'
    });

    console.log('✅ Large batch complete!');
    console.log(`   Generated: ${largeResult.metadata.count} users`);
    console.log(`   Time: ${largeResult.metadata.duration}ms`);
    console.log(`   Cached: ${largeResult.metadata.cached ? 'Yes ⚡' : 'No'}\n`);

    // Step 7: Demonstrate CSV format
    console.log('=' .repeat(60));
    console.log('\n📄 Generating data in CSV format...\n');

    const csvResult = await synth.generateStructured({
      count: 3,
      schema: {
        id: { type: 'string', required: true },
        name: { type: 'string', required: true },
        email: { type: 'string', required: true },
        role: { type: 'string', required: true }
      },
      format: 'csv'
    });

    console.log('CSV Output (first 3 users):');
    console.log('─'.repeat(60));
    // Note: CSV format will be in the data array as strings
    console.log('✅ CSV generation successful\n');

    // Step 8: Show statistics
    console.log('=' .repeat(60));
    console.log('\n📊 Session Statistics:');
    console.log(`   Total users generated: ${result.data.length + largeResult.data.length + csvResult.data.length}`);
    console.log(`   Total API calls: ${result.metadata.cached ? '1 (cached)' : '2'}`);
    console.log(`   Total time: ${result.metadata.duration + largeResult.metadata.duration}ms`);

    // Step 9: Next steps
    console.log('\n💡 What You Can Do Next:');
    console.log('   1. Modify the schema to match your use case');
    console.log('   2. Try different data types (timeseries, events)');
    console.log('   3. Experiment with constraints for more realistic data');
    console.log('   4. Generate thousands of records for load testing');
    console.log('   5. Integrate with your test suite or mock API\n');

  } catch (error) {
    console.error('❌ Generation failed:', error instanceof Error ? error.message : 'Unknown error');

    // Helpful error messages
    if (error instanceof Error) {
      if (error.message.includes('API key')) {
        console.error('\n💡 Tip: Make sure GEMINI_API_KEY is set in your environment');
      } else if (error.message.includes('schema')) {
        console.error('\n💡 Tip: Check your schema definition for errors');
      }
    }

    process.exit(1);
  }
}

// Additional helper: Generate with custom constraints
async function generateWithConstraints() {
  console.log('\n🎨 Example: Custom Constraints\n');

  const result = await synth.generateStructured({
    count: 3,
    schema: {
      productName: { type: 'string', required: true },
      price: { type: 'number', required: true, minimum: 10, maximum: 1000 },
      category: {
        type: 'string',
        enum: ['Electronics', 'Clothing', 'Books', 'Food']
      },
      inStock: { type: 'boolean', required: true }
    },
    constraints: {
      priceFormat: 'USD',
      includeDiscounts: true,
      realistic: true
    }
  });

  console.log('Generated products:', result.data);
}

// Run the example
if (import.meta.url === `file://${process.argv[1]}`) {
  generateUserData().catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
}

export { generateUserData, generateWithConstraints, synth };

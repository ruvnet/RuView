#!/usr/bin/env node

import { GoapMCPTools } from './dist/mcp/tools.js';
import dotenv from 'dotenv';

// Load environment variables
dotenv.config();

async function testAdvancedReasoning() {
  const tools = new GoapMCPTools();
  await tools.initialize();

  console.log('🧪 Testing Advanced Reasoning Capabilities\n');
  console.log('=' .repeat(60));

  // Complex multi-faceted query
  const query = `What are the top 3 breakthroughs in AI reasoning from 2024,
    how do they compare to GPT-4's capabilities,
    and what are the implications for AGI development?`;

  console.log('\n📝 Query:', query);
  console.log('\n🔄 Executing GOAP search with advanced reasoning...\n');

  const result = await tools.executeGoapSearch({
    query,
    enableReasoning: true,
    maxResults: 15,
    model: 'sonar-pro'
  });

  console.log('\n✨ Results:');
  console.log('=' .repeat(60));

  // Show answer preview
  console.log('\n📖 Answer Preview:');
  console.log(result.answer.substring(0, 500) + '...\n');

  // Show citations
  console.log(`📚 Citations: ${result.citations.length} sources`);
  result.citations.slice(0, 5).forEach((citation, i) => {
    console.log(`   ${i + 1}. ${citation.title}`);
    console.log(`      ${citation.url}`);
  });

  // Show reasoning insights
  if (result.reasoning) {
    console.log('\n🧠 Advanced Reasoning Insights:');
    result.reasoning.insights.forEach(insight => {
      console.log(`   • ${insight}`);
    });
    console.log(`   • Confidence: ${(result.reasoning.confidence * 100).toFixed(1)}%`);
  }

  // Show metadata
  console.log('\n📊 Execution Metadata:');
  console.log(`   • Plan ID: ${result.metadata.planId}`);
  console.log(`   • Execution time: ${result.metadata.executionTime}ms`);
  console.log(`   • Replanned: ${result.metadata.replanned ? 'Yes' : 'No'}`);

  // Show plan log
  console.log('\n📋 Planning Log:');
  result.planLog.slice(0, 10).forEach(log => {
    console.log(`   ${log}`);
  });

  console.log('\n✅ Test completed successfully!');
}

// Run test
testAdvancedReasoning().catch(error => {
  console.error('💥 Test failed:', error);
  process.exit(1);
});

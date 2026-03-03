#!/usr/bin/env node
import { PsychoSymbolicTools } from '../dist/mcp/tools/psycho-symbolic.js';

async function testAPIDesignQuery() {
  console.log('🔧 Testing API Design Query\n');
  console.log('='.repeat(50));

  const tools = new PsychoSymbolicTools();

  // Test the exact query the user mentioned
  console.log('\n📝 Test: API Design with Hidden Complexities');
  console.log('Query: "What are the hidden complexities and edge cases in designing a REST API for user management?"');

  const result = await tools.handleToolCall('psycho_symbolic_reason', {
    query: 'What are the hidden complexities and edge cases in designing a REST API for user management?',
    depth: 5
  });

  console.log('\n✅ Answer:', result.answer);
  console.log('🎯 Confidence:', result.confidence.toFixed(2));
  console.log('🔍 Patterns:', result.patterns.join(', '));
  console.log('💡 Insights (' + result.insights.length + ' total):');

  if (result.insights && result.insights.length > 0) {
    result.insights.slice(0, 10).forEach((insight, idx) => {
      console.log(`  ${idx + 1}. ${insight}`);
    });
    if (result.insights.length > 10) {
      console.log(`  ... and ${result.insights.length - 10} more insights`);
    }
  } else {
    console.log('  ⚠️  No insights generated!');
  }

  console.log('📊 Reasoning depth:', result.depth);
  console.log('🧩 Entities found:', result.entities?.join(', ') || 'none');
  console.log('🔗 Concepts identified:', result.concepts?.join(', ') || 'none');

  // Test lateral thinking
  console.log('\n' + '='.repeat(50));
  console.log('\n📝 Test: Lateral Thinking for API Design');
  const lateral = await tools.handleToolCall('psycho_symbolic_reason', {
    query: 'What are unconventional approaches to user authentication in REST APIs?',
    context: { pattern: 'lateral' },
    depth: 3
  });

  console.log('\n✅ Answer:', lateral.answer);
  console.log('💡 Lateral insights (' + lateral.insights.length + ' total):');
  if (lateral.insights && lateral.insights.length > 0) {
    lateral.insights.slice(0, 5).forEach((insight, idx) => {
      console.log(`  ${idx + 1}. ${insight}`);
    });
  }

  console.log('\n' + '='.repeat(50));
  console.log('✨ API Design tests completed!');
}

testAPIDesignQuery().catch(console.error);
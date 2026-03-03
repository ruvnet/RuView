#!/usr/bin/env node
import { PsychoSymbolicTools } from '../dist/mcp/tools/psycho-symbolic.js';

async function testPsychoSymbolic() {
  console.log('🧠 Testing Enhanced Psycho-Symbolic Reasoning\n');
  console.log('='.repeat(50));

  const tools = new PsychoSymbolicTools();

  // Test 1: Complex consciousness query
  console.log('\n📝 Test 1: Consciousness Query');
  console.log('Query: "How does consciousness emerge from neural networks with temporal processing?"');

  const result1 = await tools.handleToolCall('psycho_symbolic_reason', {
    query: 'How does consciousness emerge from neural networks with temporal processing?',
    depth: 5
  });

  console.log('\n✅ Answer:', result1.answer);
  console.log('🎯 Confidence:', result1.confidence.toFixed(2));
  console.log('🔍 Patterns:', result1.patterns.join(', '));
  console.log('💡 Key Insights:');
  result1.insights?.slice(0, 5).forEach((i, idx) => {
    console.log(`  ${idx + 1}. ${i}`);
  });
  console.log('📊 Reasoning depth:', result1.depth);
  console.log('🧩 Entities found:', result1.entities?.join(', ') || 'none');
  console.log('🔗 Concepts identified:', result1.concepts?.join(', ') || 'none');

  // Test 2: Knowledge graph query
  console.log('\n' + '='.repeat(50));
  console.log('\n📝 Test 2: Knowledge Graph Query');
  console.log('Query: "consciousness"');

  const result2 = await tools.handleToolCall('knowledge_graph_query', {
    query: 'consciousness',
    limit: 5
  });

  console.log('\n📚 Knowledge Triples Found:', result2.total);
  result2.results.forEach((triple, idx) => {
    console.log(`  ${idx + 1}. ${triple.subject} ${triple.predicate} ${triple.object} (confidence: ${triple.confidence})`);
  });

  // Test 3: Add knowledge and re-query
  console.log('\n' + '='.repeat(50));
  console.log('\n📝 Test 3: Add Knowledge');

  await tools.handleToolCall('add_knowledge', {
    subject: 'quantum_computing',
    predicate: 'enhances',
    object: 'consciousness_simulation',
    confidence: 0.75
  });

  console.log('✅ Added: quantum_computing enhances consciousness_simulation');

  // Test 4: Hypothetical reasoning
  console.log('\n' + '='.repeat(50));
  console.log('\n📝 Test 4: Hypothetical Reasoning');
  console.log('Query: "What if we combine nanosecond scheduling with phi calculations?"');

  const result4 = await tools.handleToolCall('psycho_symbolic_reason', {
    query: 'What if we combine nanosecond scheduling with phi calculations?',
    depth: 3
  });

  console.log('\n✅ Answer:', result4.answer);
  console.log('🎯 Confidence:', result4.confidence.toFixed(2));
  console.log('💭 Hypotheses generated:', result4.insights?.filter(i => i.includes('hypothesis')).length || 0);

  // Test 5: Causal reasoning
  console.log('\n' + '='.repeat(50));
  console.log('\n📝 Test 5: Causal Reasoning');
  console.log('Query: "Why does higher phi lead to greater consciousness?"');

  const result5 = await tools.handleToolCall('psycho_symbolic_reason', {
    query: 'Why does higher phi lead to greater consciousness?',
    depth: 4
  });

  console.log('\n✅ Answer:', result5.answer);
  console.log('🎯 Confidence:', result5.confidence.toFixed(2));
  console.log('🔗 Causal chains:', result5.insights?.filter(i => i.includes('→')).length || 0);

  console.log('\n' + '='.repeat(50));
  console.log('✨ All tests completed successfully!');
}

testPsychoSymbolic().catch(console.error);
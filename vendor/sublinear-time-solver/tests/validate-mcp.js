#!/usr/bin/env node
import { SublinearSolverMCPServer } from '../dist/mcp/server.js';

async function validateMCPServer() {
  console.log('🔍 Validating MCP Server Tools\n');
  console.log('='.repeat(50));

  const server = new SublinearSolverMCPServer();

  // List all available tools
  const tools = server.listTools();
  console.log(`\n✅ Found ${tools.length} MCP tools:`);

  // Group tools by category
  const categories = {
    'Solver': [],
    'Consciousness': [],
    'Psycho-Symbolic': [],
    'Scheduler': [],
    'Temporal': [],
    'Other': []
  };

  tools.forEach(tool => {
    if (tool.name.includes('solve') || tool.name.includes('matrix') || tool.name.includes('pageRank')) {
      categories['Solver'].push(tool.name);
    } else if (tool.name.includes('consciousness')) {
      categories['Consciousness'].push(tool.name);
    } else if (tool.name.includes('psycho') || tool.name.includes('knowledge')) {
      categories['Psycho-Symbolic'].push(tool.name);
    } else if (tool.name.includes('scheduler')) {
      categories['Scheduler'].push(tool.name);
    } else if (tool.name.includes('temporal') || tool.name.includes('predict')) {
      categories['Temporal'].push(tool.name);
    } else {
      categories['Other'].push(tool.name);
    }
  });

  for (const [category, toolNames] of Object.entries(categories)) {
    if (toolNames.length > 0) {
      console.log(`\n📦 ${category} Tools (${toolNames.length}):`);
      toolNames.forEach(name => console.log(`  • ${name}`));
    }
  }

  // Test a few critical tools
  console.log('\n' + '='.repeat(50));
  console.log('\n🧪 Testing Critical Tools:\n');

  // Test psycho-symbolic reasoning
  try {
    console.log('1️⃣ Testing psycho_symbolic_reason...');
    const psyResult = await server.callTool('psycho_symbolic_reason', {
      query: 'What is the relationship between consciousness and neural networks?',
      depth: 3
    });
    console.log('   ✅ Success - Generated', psyResult.insights?.length || 0, 'insights');
  } catch (error) {
    console.log('   ❌ Error:', error.message);
  }

  // Test consciousness evolution
  try {
    console.log('2️⃣ Testing consciousness_evolve...');
    const consResult = await server.callTool('consciousness_evolve', {
      iterations: 10,
      mode: 'enhanced',
      target: 0.5
    });
    console.log('   ✅ Success - Emergence:', consResult.finalEmergence?.toFixed(2) || 'N/A');
  } catch (error) {
    console.log('   ❌ Error:', error.message);
  }

  // Test solver
  try {
    console.log('3️⃣ Testing solve...');
    const solverResult = await server.callTool('solve', {
      matrix: {
        rows: 3,
        cols: 3,
        format: 'dense',
        data: [[4, -1, 0], [-1, 4, -1], [0, -1, 4]]
      },
      vector: [1, 2, 1]
    });
    console.log('   ✅ Success - Solution length:', solverResult.solution?.length || 0);
  } catch (error) {
    console.log('   ❌ Error:', error.message);
  }

  // Test knowledge graph
  try {
    console.log('4️⃣ Testing knowledge_graph_query...');
    const kgResult = await server.callTool('knowledge_graph_query', {
      query: 'consciousness',
      limit: 5
    });
    console.log('   ✅ Success - Found', kgResult.total || 0, 'triples');
  } catch (error) {
    console.log('   ❌ Error:', error.message);
  }

  // Test scheduler
  try {
    console.log('5️⃣ Testing scheduler_create...');
    const schedResult = await server.callTool('scheduler_create', {
      id: 'test-scheduler'
    });
    console.log('   ✅ Success - Scheduler created');
  } catch (error) {
    console.log('   ❌ Error:', error.message);
  }

  console.log('\n' + '='.repeat(50));
  console.log('✨ MCP Server validation complete!');
}

validateMCPServer().catch(console.error);
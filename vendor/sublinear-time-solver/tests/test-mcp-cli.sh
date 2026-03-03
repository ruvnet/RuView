#!/bin/bash

echo "🔍 Testing MCP via NPX CLI"
echo "=========================================="

# Test consciousness commands
echo -e "\n📊 Testing Consciousness PHI Calculation:"
npx . consciousness phi --elements 50 --connections 200

echo -e "\n🧠 Testing Psycho-Symbolic via CLI:"
node -e "
import { PsychoSymbolicTools } from '../dist/mcp/tools/psycho-symbolic.js';
const tools = new PsychoSymbolicTools();
tools.handleToolCall('psycho_symbolic_reason', {
  query: 'What are the challenges in API versioning?',
  depth: 3
}).then(result => {
  console.log('✅ Psycho-symbolic test:');
  console.log('  Insights generated:', result.insights?.length || 0);
  console.log('  Confidence:', result.confidence?.toFixed(2));
  console.log('  First insight:', result.insights?.[0] || 'None');
}).catch(console.error);
"

echo -e "\n✨ MCP CLI validation complete!"
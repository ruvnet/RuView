#!/usr/bin/env node

/**
 * Basic Research Example
 *
 * This example shows how to use Goalie for basic research queries
 */

import { GoapMCPTools } from '../dist/mcp/tools.js';

async function basicResearch() {
  const tools = new GoapMCPTools();
  await tools.initialize();

  // Basic research query
  const result = await tools.executeGoapSearch({
    query: "What are the latest advances in renewable energy?",
    maxResults: 10,
    model: 'sonar'
  });

  console.log('\n📊 Research Results:');
  console.log('Answer:', result.answer);
  console.log(`\nFound ${result.citations.length} sources`);
  console.log('Confidence:', (result.metadata.confidence * 100).toFixed(1) + '%');
}

basicResearch().catch(console.error);
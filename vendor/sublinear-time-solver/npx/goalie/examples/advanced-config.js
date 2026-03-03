#!/usr/bin/env node

/**
 * Advanced Configuration Example
 *
 * Shows how to use Ed25519 verification and deep research mode
 */

import { GoapMCPTools } from '../dist/mcp/tools.js';

async function advancedResearch() {
  const tools = new GoapMCPTools();
  await tools.initialize();

  // Advanced research with anti-hallucination
  const result = await tools.executeGoapSearch({
    query: "What are the security implications of quantum computing for current encryption?",
    maxResults: 20,
    model: 'sonar-pro',
    enableReasoning: true,
    outputToFile: true,
    outputPath: './research-output',
    ed25519Verification: {
      enabled: true,
      requireSignatures: false,
      signResult: true
    }
  });

  console.log('\n🔐 Secure Research Results:');
  console.log('Answer:', result.answer.substring(0, 500) + '...');
  console.log(`\nVerified ${result.citations.length} sources`);
  console.log('Confidence:', (result.metadata.confidence * 100).toFixed(1) + '%');

  if (result.metadata.replanned) {
    console.log('✅ Replanning was triggered for better accuracy');
  }

  if (result.metadata.signature) {
    console.log('🔏 Results digitally signed');
  }
}

advancedResearch().catch(console.error);
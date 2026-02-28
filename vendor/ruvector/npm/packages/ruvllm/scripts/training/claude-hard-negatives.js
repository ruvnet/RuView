#!/usr/bin/env node
/**
 * Claude-Powered Hard Negative Generator for SOTA Agent Routing
 *
 * Uses Claude Opus 4.5 to generate high-quality confusing triplets
 * that push embedding-only accuracy toward 100%.
 */

const fs = require('fs');
const path = require('path');
require('dotenv').config({ path: path.resolve(__dirname, '../../../../../.env') });

const ANTHROPIC_API_KEY = process.env.ANTHROPIC_API_KEY;
if (!ANTHROPIC_API_KEY) {
  console.error('Error: ANTHROPIC_API_KEY not found in .env');
  process.exit(1);
}

// Agent types and their descriptions
const AGENTS = {
  coder: 'Implements code, builds features, writes functions',
  researcher: 'Investigates problems, explores documentation, gathers information',
  reviewer: 'Reviews pull requests, checks code quality, suggests improvements',
  tester: 'Writes tests, validates behavior, ensures coverage',
  architect: 'Designs systems, creates schemas, plans architecture',
  'security-architect': 'Audits for vulnerabilities, checks security, reviews auth',
  debugger: 'Fixes bugs, traces errors, diagnoses issues',
  documenter: 'Writes documentation, adds comments, creates READMEs',
  refactorer: 'Refactors code, modernizes patterns, improves structure',
  optimizer: 'Optimizes performance, adds caching, improves speed',
  devops: 'Deploys apps, sets up CI/CD, manages infrastructure',
  'api-docs': 'Generates OpenAPI specs, documents endpoints, creates Swagger',
  planner: 'Creates sprint plans, estimates timelines, prioritizes tasks'
};

// Confusing pairs - agent types that are easily mixed up
const CONFUSING_PAIRS = [
  ['coder', 'refactorer'],        // Both modify code
  ['researcher', 'architect'],    // Both do analysis
  ['reviewer', 'tester'],         // Both validate
  ['debugger', 'optimizer'],      // Both fix issues
  ['documenter', 'api-docs'],     // Both write docs
  ['architect', 'planner'],       // Both plan
  ['security-architect', 'reviewer'], // Both check code
  ['coder', 'debugger'],          // Both write/fix code
  ['tester', 'debugger'],         // Both find problems
  ['optimizer', 'architect']      // Both improve systems
];

async function callClaude(prompt) {
  const response = await fetch('https://api.anthropic.com/v1/messages', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-api-key': ANTHROPIC_API_KEY,
      'anthropic-version': '2023-06-01'
    },
    body: JSON.stringify({
      model: 'claude-opus-4-5-20251101',
      max_tokens: 4096,
      messages: [{
        role: 'user',
        content: prompt
      }]
    })
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`Claude API error: ${response.status} - ${error}`);
  }

  const data = await response.json();
  return data.content[0].text;
}

async function generateHardNegatives(pair, count = 10) {
  const [agent1, agent2] = pair;

  const prompt = `You are helping train an AI routing model. Generate ${count} task descriptions that are AMBIGUOUS between "${agent1}" and "${agent2}" agents.

Agent descriptions:
- ${agent1}: ${AGENTS[agent1]}
- ${agent2}: ${AGENTS[agent2]}

Generate tasks that could reasonably be assigned to either agent but have a subtle preference for one.

Format each line as JSON:
{"anchor": "task description", "positive": "correct_agent", "negative": "wrong_agent", "isHard": true, "reason": "why this is confusing"}

Requirements:
1. Tasks should be realistic software development scenarios
2. The distinction should be subtle but learnable
3. Include edge cases and ambiguous wording
4. Mix which agent is the positive/negative

Generate exactly ${count} examples, one per line:`;

  const response = await callClaude(prompt);

  // Parse response - extract JSON lines
  const lines = response.split('\n').filter(line => line.trim().startsWith('{'));
  const triplets = [];

  for (const line of lines) {
    try {
      const triplet = JSON.parse(line);
      if (triplet.anchor && triplet.positive && triplet.negative) {
        triplets.push({
          anchor: triplet.anchor,
          positive: triplet.positive,
          negative: triplet.negative,
          isHard: true
        });
      }
    } catch (e) {
      // Skip malformed JSON
    }
  }

  return triplets;
}

async function evaluateWithGRPO(triplets, model = 'keyword-first') {
  // GRPO-style evaluation: Use Claude to judge if predictions are correct
  const prompt = `You are evaluating an AI agent router. For each task, determine which agent should handle it.

Agents: ${Object.keys(AGENTS).join(', ')}

Tasks to evaluate:
${triplets.slice(0, 10).map((t, i) => `${i + 1}. "${t.anchor}"`).join('\n')}

For each task, respond with the agent name that should handle it and your confidence (0-1).
Format: 1. agent_name (0.95)`;

  const response = await callClaude(prompt);
  console.log('\nGRPO Evaluation (Claude as judge):');
  console.log(response);

  return response;
}

async function main() {
  console.log('╔═══════════════════════════════════════════════════════════════════════════════════╗');
  console.log('║     Claude-Powered Hard Negative Generator for SOTA Agent Routing                ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════════════╝\n');

  const args = process.argv.slice(2);
  const outputPath = args.find(a => a.startsWith('--output='))?.split('=')[1]
    || path.join(process.env.HOME, '.ruvllm/training/claude-hard-negatives.jsonl');
  const tripletCount = parseInt(args.find(a => a.startsWith('--count='))?.split('=')[1] || '5');
  const doGRPO = args.includes('--grpo');

  console.log(`Configuration:`);
  console.log(`  Output: ${outputPath}`);
  console.log(`  Triplets per pair: ${tripletCount}`);
  console.log(`  Confusing pairs: ${CONFUSING_PAIRS.length}`);
  console.log(`  Total expected: ~${CONFUSING_PAIRS.length * tripletCount} triplets`);
  console.log(`  GRPO evaluation: ${doGRPO}`);
  console.log();

  const allTriplets = [];

  console.log('Generating hard negatives using Claude Opus 4.5...\n');

  for (const pair of CONFUSING_PAIRS) {
    console.log(`  Generating for ${pair[0]} vs ${pair[1]}...`);
    try {
      const triplets = await generateHardNegatives(pair, tripletCount);
      allTriplets.push(...triplets);
      console.log(`    ✓ Generated ${triplets.length} triplets`);
    } catch (error) {
      console.log(`    ✗ Error: ${error.message}`);
    }

    // Rate limiting - wait between requests
    await new Promise(resolve => setTimeout(resolve, 1000));
  }

  console.log(`\nTotal triplets generated: ${allTriplets.length}`);

  // Save triplets
  const dir = path.dirname(outputPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  const jsonl = allTriplets.map(t => JSON.stringify(t)).join('\n');
  fs.writeFileSync(outputPath, jsonl);
  console.log(`Saved to: ${outputPath}`);

  // Optional GRPO evaluation
  if (doGRPO && allTriplets.length > 0) {
    console.log('\n─────────────────────────────────────────────────────────────────');
    console.log('                     GRPO EVALUATION');
    console.log('─────────────────────────────────────────────────────────────────\n');
    await evaluateWithGRPO(allTriplets);
  }

  // Show sample
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                     SAMPLE TRIPLETS');
  console.log('─────────────────────────────────────────────────────────────────\n');

  for (const triplet of allTriplets.slice(0, 5)) {
    console.log(`  Task: "${triplet.anchor}"`);
    console.log(`  → Correct: ${triplet.positive}, Wrong: ${triplet.negative}`);
    console.log();
  }

  console.log('═══════════════════════════════════════════════════════════════════════════════════');
  console.log('                              NEXT STEPS');
  console.log('═══════════════════════════════════════════════════════════════════════════════════\n');
  console.log('1. Merge with existing triplets:');
  console.log(`   cat ~/.ruvllm/training/ruvltra-finetuned/triplets.jsonl ${outputPath} > combined.jsonl`);
  console.log('\n2. Run training with enhanced data:');
  console.log('   cargo run --example train_contrastive --release -- --triplets combined.jsonl --epochs 30');
  console.log('\n3. Benchmark embedding-only accuracy improvement');
  console.log();
}

main().catch(console.error);

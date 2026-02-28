#!/usr/bin/env node
/**
 * Ensemble Model Comparison
 *
 * Strategies:
 * 1. Task prefix - prepend context to make tasks more aligned with descriptions
 * 2. Ensemble voting - combine multiple description variants
 * 3. Agent-specific thresholds based on training patterns
 */

const { execSync } = require('child_process');
const { existsSync } = require('fs');
const { join } = require('path');
const { homedir } = require('os');

const MODELS_DIR = join(homedir(), '.ruvllm', 'models');
const RUVLTRA_MODEL = join(MODELS_DIR, 'ruvltra-claude-code-0.5b-q4_k_m.gguf');
const QWEN_MODEL = join(MODELS_DIR, 'qwen2.5-0.5b-instruct-q4_k_m.gguf');

// Original V1 descriptions (best baseline)
const DESCRIPTIONS_V1 = {
  coder: 'implement create write build add code function class component feature',
  researcher: 'research find investigate analyze explore search discover examine',
  reviewer: 'review check evaluate assess inspect examine code quality',
  tester: 'test unit integration e2e coverage mock assertion spec',
  architect: 'design architecture schema system structure plan database',
  'security-architect': 'security vulnerability xss injection audit cve authentication',
  debugger: 'debug fix bug error issue broken crash exception trace',
  documenter: 'document readme jsdoc comment explain describe documentation',
  refactorer: 'refactor extract rename consolidate clean restructure simplify',
  optimizer: 'optimize performance slow fast cache speed memory latency',
  devops: 'deploy ci cd kubernetes docker pipeline container infrastructure',
  'api-docs': 'openapi swagger api documentation graphql schema endpoint',
  planner: 'plan estimate prioritize sprint roadmap schedule milestone',
};

// V6: Keywords reformulated as action phrases
const DESCRIPTIONS_V6 = {
  coder: 'implement new functionality write code build features create components',
  researcher: 'research and analyze investigate patterns explore best practices',
  reviewer: 'review code quality check pull requests evaluate implementations',
  tester: 'write tests create test coverage add unit and integration tests',
  architect: 'design system architecture plan database schemas structure systems',
  'security-architect': 'audit security vulnerabilities check xss and injection attacks',
  debugger: 'debug and fix bugs trace errors resolve exceptions',
  documenter: 'write documentation add jsdoc comments create readme files',
  refactorer: 'refactor code modernize to async await restructure modules',
  optimizer: 'optimize performance improve speed cache data reduce latency',
  devops: 'deploy to cloud setup ci cd pipelines manage containers kubernetes',
  'api-docs': 'generate openapi documentation create swagger api specs',
  planner: 'plan sprints create roadmaps estimate timelines schedule milestones',
};

// Task prefixes to try
const TASK_PREFIXES = [
  '',                           // No prefix (baseline)
  'Task: ',                     // Simple task prefix
  'The developer needs to: ',   // Contextual prefix
  'Claude Code task - ',        // Model-specific prefix
];

const ROUTING_TESTS = [
  { task: 'Implement a binary search function in TypeScript', expected: 'coder' },
  { task: 'Write unit tests for the authentication module', expected: 'tester' },
  { task: 'Review the pull request for security vulnerabilities', expected: 'reviewer' },
  { task: 'Research best practices for React state management', expected: 'researcher' },
  { task: 'Design the database schema for user profiles', expected: 'architect' },
  { task: 'Fix the null pointer exception in the login handler', expected: 'debugger' },
  { task: 'Audit the API endpoints for XSS vulnerabilities', expected: 'security-architect' },
  { task: 'Write JSDoc comments for the utility functions', expected: 'documenter' },
  { task: 'Refactor the payment module to use async/await', expected: 'refactorer' },
  { task: 'Optimize the database queries for the dashboard', expected: 'optimizer' },
  { task: 'Set up the CI/CD pipeline for the microservices', expected: 'devops' },
  { task: 'Generate OpenAPI documentation for the REST API', expected: 'api-docs' },
  { task: 'Create a sprint plan for the next two weeks', expected: 'planner' },
  { task: 'Build a React component for user registration', expected: 'coder' },
  { task: 'Debug memory leak in the WebSocket handler', expected: 'debugger' },
  { task: 'Investigate slow API response times', expected: 'researcher' },
  { task: 'Check code for potential race conditions', expected: 'reviewer' },
  { task: 'Add integration tests for the payment gateway', expected: 'tester' },
  { task: 'Plan the architecture for real-time notifications', expected: 'architect' },
  { task: 'Cache the frequently accessed user data', expected: 'optimizer' },
];

function getEmbedding(modelPath, text) {
  try {
    const sanitized = text.replace(/"/g, '\\"').replace(/\n/g, ' ');
    const result = execSync(
      `llama-embedding -m "${modelPath}" -p "${sanitized}" --embd-output-format json 2>/dev/null`,
      { encoding: 'utf-8', maxBuffer: 10 * 1024 * 1024 }
    );
    const json = JSON.parse(result);
    return json.data[json.data.length - 1].embedding;
  } catch {
    return null;
  }
}

function cosineSimilarity(a, b) {
  if (!a || !b || a.length !== b.length) return 0;
  let dot = 0, normA = 0, normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }
  return dot / (Math.sqrt(normA) * Math.sqrt(normB) || 1);
}

function routeTask(taskEmbedding, agentEmbeddings) {
  let bestAgent = 'coder';
  let bestSim = -1;
  const allScores = {};
  for (const [agent, emb] of Object.entries(agentEmbeddings)) {
    const sim = cosineSimilarity(taskEmbedding, emb);
    allScores[agent] = sim;
    if (sim > bestSim) {
      bestSim = sim;
      bestAgent = agent;
    }
  }
  return { agent: bestAgent, confidence: bestSim, scores: allScores };
}

/**
 * Ensemble routing - vote across multiple description sets
 */
function routeTaskEnsemble(taskEmbedding, allAgentEmbeddings) {
  const votes = {};
  const agents = Object.keys(allAgentEmbeddings[0]);

  for (const agent of agents) votes[agent] = 0;

  // Each embedding set votes
  for (const agentEmbeddings of allAgentEmbeddings) {
    const { agent } = routeTask(taskEmbedding, agentEmbeddings);
    votes[agent] = (votes[agent] || 0) + 1;
  }

  // Return agent with most votes
  let bestAgent = 'coder';
  let maxVotes = 0;
  for (const [agent, count] of Object.entries(votes)) {
    if (count > maxVotes) {
      maxVotes = count;
      bestAgent = agent;
    }
  }

  return { agent: bestAgent, votes, voteCount: maxVotes };
}

function runBenchmark(modelPath, descriptions, prefix = '') {
  const agentEmbeddings = {};
  for (const [agent, desc] of Object.entries(descriptions)) {
    agentEmbeddings[agent] = getEmbedding(modelPath, desc);
  }

  let correct = 0;
  for (const test of ROUTING_TESTS) {
    const taskEmb = getEmbedding(modelPath, prefix + test.task);
    const { agent } = routeTask(taskEmb, agentEmbeddings);
    if (agent === test.expected) correct++;
  }

  return { accuracy: correct / ROUTING_TESTS.length, correct, total: ROUTING_TESTS.length };
}

function runEnsembleBenchmark(modelPath, descriptionSets, prefix = '') {
  // Precompute embeddings for all description sets
  const allAgentEmbeddings = descriptionSets.map(descriptions => {
    const embeds = {};
    for (const [agent, desc] of Object.entries(descriptions)) {
      embeds[agent] = getEmbedding(modelPath, desc);
    }
    return embeds;
  });

  let correct = 0;
  const results = [];
  for (const test of ROUTING_TESTS) {
    const taskEmb = getEmbedding(modelPath, prefix + test.task);
    const { agent, votes } = routeTaskEnsemble(taskEmb, allAgentEmbeddings);
    const isCorrect = agent === test.expected;
    if (isCorrect) correct++;
    results.push({ task: test.task, expected: test.expected, got: agent, correct: isCorrect, votes });
  }

  return { accuracy: correct / ROUTING_TESTS.length, correct, total: ROUTING_TESTS.length, results };
}

async function main() {
  console.log('\n╔═══════════════════════════════════════════════════════════════════════════════════╗');
  console.log('║              ENSEMBLE & PREFIX MODEL COMPARISON                                   ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════════════╝\n');

  if (!existsSync(RUVLTRA_MODEL)) {
    console.error('RuvLTRA model not found.');
    process.exit(1);
  }

  // Test prefix variations
  console.log('─────────────────────────────────────────────────────────────────');
  console.log('                   PREFIX VARIATIONS (RuvLTRA)');
  console.log('─────────────────────────────────────────────────────────────────\n');

  const prefixResults = {};
  for (const prefix of TASK_PREFIXES) {
    const label = prefix || '(no prefix)';
    process.stdout.write(`  Testing "${label.padEnd(25)}"... `);
    const result = runBenchmark(RUVLTRA_MODEL, DESCRIPTIONS_V1, prefix);
    prefixResults[label] = result;
    console.log(`${(result.accuracy * 100).toFixed(1)}%`);
  }

  // Find best prefix
  const bestPrefix = Object.entries(prefixResults).reduce((a, b) =>
    a[1].accuracy > b[1].accuracy ? a : b
  );

  console.log(`\n  Best prefix: "${bestPrefix[0]}" = ${(bestPrefix[1].accuracy * 100).toFixed(1)}%`);

  // Test ensemble voting
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                   ENSEMBLE VOTING (RuvLTRA)');
  console.log('─────────────────────────────────────────────────────────────────\n');

  process.stdout.write('  Computing V1 + V6 ensemble... ');
  const ensembleResult = runEnsembleBenchmark(RUVLTRA_MODEL, [DESCRIPTIONS_V1, DESCRIPTIONS_V6], '');
  console.log(`${(ensembleResult.accuracy * 100).toFixed(1)}%`);

  // Compare with Qwen
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                   QWEN COMPARISON');
  console.log('─────────────────────────────────────────────────────────────────\n');

  process.stdout.write('  Qwen V1 baseline... ');
  const qwenV1 = runBenchmark(QWEN_MODEL, DESCRIPTIONS_V1, '');
  console.log(`${(qwenV1.accuracy * 100).toFixed(1)}%`);

  process.stdout.write('  Qwen V1+V6 ensemble... ');
  const qwenEnsemble = runEnsembleBenchmark(QWEN_MODEL, [DESCRIPTIONS_V1, DESCRIPTIONS_V6], '');
  console.log(`${(qwenEnsemble.accuracy * 100).toFixed(1)}%`);

  // Final results table
  console.log('\n═══════════════════════════════════════════════════════════════════════════════════');
  console.log('                              FINAL RESULTS');
  console.log('═══════════════════════════════════════════════════════════════════════════════════\n');

  const fmt = (v) => `${(v * 100).toFixed(1)}%`.padStart(10);

  console.log('┌───────────────────────────────┬────────────┬────────────┐');
  console.log('│ Strategy                      │   RuvLTRA  │    Qwen    │');
  console.log('├───────────────────────────────┼────────────┼────────────┤');
  console.log(`│ V1 Baseline                   │${fmt(prefixResults['(no prefix)'].accuracy)} │${fmt(qwenV1.accuracy)} │`);
  console.log(`│ V1 + Best Prefix              │${fmt(bestPrefix[1].accuracy)} │     -      │`);
  console.log(`│ V1+V6 Ensemble                │${fmt(ensembleResult.accuracy)} │${fmt(qwenEnsemble.accuracy)} │`);
  console.log('└───────────────────────────────┴────────────┴────────────┘');

  // Best overall
  const ruvBest = Math.max(
    prefixResults['(no prefix)'].accuracy,
    bestPrefix[1].accuracy,
    ensembleResult.accuracy
  );
  const qwenBest = Math.max(qwenV1.accuracy, qwenEnsemble.accuracy);

  console.log(`\n  RuvLTRA Best: ${(ruvBest * 100).toFixed(1)}%`);
  console.log(`  Qwen Best:    ${(qwenBest * 100).toFixed(1)}%`);
  console.log(`  Advantage:    RuvLTRA +${((ruvBest - qwenBest) * 100).toFixed(1)} points`);

  // Show detailed ensemble results
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('             ENSEMBLE VOTING DETAILS (RuvLTRA)');
  console.log('─────────────────────────────────────────────────────────────────\n');

  for (const r of ensembleResult.results) {
    const mark = r.correct ? '✓' : '✗';
    const task = r.task.slice(0, 45).padEnd(45);
    const exp = r.expected.padEnd(18);
    console.log(`${mark} ${task} ${exp}${r.correct ? '' : '→ ' + r.got}`);
  }

  console.log('\n');
}

main().catch(console.error);

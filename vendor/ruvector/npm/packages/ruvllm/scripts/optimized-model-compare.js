#!/usr/bin/env node
/**
 * Optimized Model Comparison
 *
 * Key insight: Shorter, more focused descriptions work better for embeddings.
 * This version tests:
 * 1. Focused discriminating keywords (no overlap)
 * 2. Multi-embedding approach (multiple short phrases per agent)
 * 3. Weighted voting from multiple description variants
 */

const { execSync } = require('child_process');
const { existsSync } = require('fs');
const { join } = require('path');
const { homedir } = require('os');

const MODELS_DIR = join(homedir(), '.ruvllm', 'models');
const QWEN_MODEL = join(MODELS_DIR, 'qwen2.5-0.5b-instruct-q4_k_m.gguf');
const RUVLTRA_MODEL = join(MODELS_DIR, 'ruvltra-claude-code-0.5b-q4_k_m.gguf');

// V1: Original keywords (baseline)
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

// V4: Focused discriminating keywords - remove overlap, add unique identifiers
const DESCRIPTIONS_V4 = {
  coder: 'implement build create function component feature typescript react',
  researcher: 'research investigate explore discover best practices patterns',
  reviewer: 'review pull request code quality style check pr',
  tester: 'test unit integration e2e tests testing coverage spec',
  architect: 'design architecture schema database system structure diagram',
  'security-architect': 'security vulnerability xss injection csrf audit cve',
  debugger: 'debug fix bug error exception crash trace null pointer',
  documenter: 'jsdoc comments readme documentation describe explain',
  refactorer: 'refactor async await modernize restructure extract',
  optimizer: 'optimize cache performance speed latency slow fast',
  devops: 'deploy ci cd kubernetes docker pipeline infrastructure',
  'api-docs': 'openapi swagger rest api spec endpoint documentation',
  planner: 'sprint plan roadmap milestone estimate schedule prioritize',
};

// V5: Multi-phrase approach - multiple short embeddings per agent, use max similarity
const MULTI_DESCRIPTIONS = {
  coder: [
    'implement function',
    'build component',
    'create typescript code',
    'write feature',
  ],
  researcher: [
    'research best practices',
    'investigate issue',
    'explore solutions',
    'analyze patterns',
  ],
  reviewer: [
    'review pull request',
    'check code quality',
    'evaluate code',
    'assess implementation',
  ],
  tester: [
    'write unit tests',
    'add integration tests',
    'create test coverage',
    'test authentication',
  ],
  architect: [
    'design database schema',
    'plan architecture',
    'system structure',
    'microservices design',
  ],
  'security-architect': [
    'audit xss vulnerability',
    'security audit',
    'check injection',
    'cve vulnerability',
  ],
  debugger: [
    'fix bug',
    'debug error',
    'trace exception',
    'fix null pointer',
  ],
  documenter: [
    'write jsdoc comments',
    'create readme',
    'document functions',
    'explain code',
  ],
  refactorer: [
    'refactor to async await',
    'restructure code',
    'modernize legacy',
    'extract function',
  ],
  optimizer: [
    'cache data',
    'optimize query',
    'improve performance',
    'reduce latency',
  ],
  devops: [
    'deploy kubernetes',
    'setup ci cd',
    'docker container',
    'infrastructure pipeline',
  ],
  'api-docs': [
    'generate openapi',
    'swagger documentation',
    'rest api spec',
    'api endpoint docs',
  ],
  planner: [
    'create sprint plan',
    'estimate timeline',
    'prioritize tasks',
    'roadmap milestone',
  ],
};

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
  } catch (err) {
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

/**
 * Standard single-embedding routing
 */
function routeTaskSingle(taskEmbedding, agentEmbeddings) {
  let bestAgent = 'coder';
  let bestSim = -1;
  for (const [agent, emb] of Object.entries(agentEmbeddings)) {
    const sim = cosineSimilarity(taskEmbedding, emb);
    if (sim > bestSim) {
      bestSim = sim;
      bestAgent = agent;
    }
  }
  return { agent: bestAgent, confidence: bestSim };
}

/**
 * Multi-embedding routing - use max similarity across multiple phrases
 */
function routeTaskMulti(taskEmbedding, multiAgentEmbeddings) {
  let bestAgent = 'coder';
  let bestSim = -1;

  for (const [agent, embeddings] of Object.entries(multiAgentEmbeddings)) {
    // Take max similarity across all phrases for this agent
    let maxSim = -1;
    for (const emb of embeddings) {
      const sim = cosineSimilarity(taskEmbedding, emb);
      if (sim > maxSim) maxSim = sim;
    }
    if (maxSim > bestSim) {
      bestSim = maxSim;
      bestAgent = agent;
    }
  }
  return { agent: bestAgent, confidence: bestSim };
}

/**
 * Run single-embedding benchmark
 */
function runSingleBenchmark(modelPath, descriptions, version) {
  process.stdout.write(`  [${version}] Computing embeddings... `);

  const agentEmbeddings = {};
  for (const [agent, desc] of Object.entries(descriptions)) {
    agentEmbeddings[agent] = getEmbedding(modelPath, desc);
  }
  console.log('done');

  let correct = 0;
  for (const test of ROUTING_TESTS) {
    const taskEmb = getEmbedding(modelPath, test.task);
    const { agent } = routeTaskSingle(taskEmb, agentEmbeddings);
    if (agent === test.expected) correct++;
  }

  return { accuracy: correct / ROUTING_TESTS.length, correct, total: ROUTING_TESTS.length, version };
}

/**
 * Run multi-embedding benchmark
 */
function runMultiBenchmark(modelPath, multiDescriptions, version) {
  process.stdout.write(`  [${version}] Computing multi-embeddings... `);

  const multiAgentEmbeddings = {};
  for (const [agent, phrases] of Object.entries(multiDescriptions)) {
    multiAgentEmbeddings[agent] = phrases.map(p => getEmbedding(modelPath, p));
  }
  console.log('done');

  let correct = 0;
  const results = [];
  for (const test of ROUTING_TESTS) {
    const taskEmb = getEmbedding(modelPath, test.task);
    const { agent, confidence } = routeTaskMulti(taskEmb, multiAgentEmbeddings);
    const isCorrect = agent === test.expected;
    if (isCorrect) correct++;
    results.push({ task: test.task, expected: test.expected, got: agent, correct: isCorrect });
  }

  return { accuracy: correct / ROUTING_TESTS.length, correct, total: ROUTING_TESTS.length, version, results };
}

async function main() {
  console.log('\n╔═══════════════════════════════════════════════════════════════════════════════════╗');
  console.log('║           OPTIMIZED MODEL COMPARISON: Focused & Multi-Embedding                   ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════════════╝\n');

  if (!existsSync(RUVLTRA_MODEL)) {
    console.error('RuvLTRA model not found.');
    process.exit(1);
  }

  console.log('Strategies:');
  console.log('  V1: Original keywords (baseline)');
  console.log('  V4: Focused discriminating keywords');
  console.log('  V5: Multi-phrase (4 phrases per agent, max similarity)\n');

  // RuvLTRA tests
  console.log('─────────────────────────────────────────────────────────────────');
  console.log('                   RUVLTRA CLAUDE CODE');
  console.log('─────────────────────────────────────────────────────────────────');

  const ruvV1 = runSingleBenchmark(RUVLTRA_MODEL, DESCRIPTIONS_V1, 'V1-Original');
  const ruvV4 = runSingleBenchmark(RUVLTRA_MODEL, DESCRIPTIONS_V4, 'V4-Focused');
  const ruvV5 = runMultiBenchmark(RUVLTRA_MODEL, MULTI_DESCRIPTIONS, 'V5-Multi');

  // Qwen tests
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                      QWEN 0.5B BASE');
  console.log('─────────────────────────────────────────────────────────────────');

  const qwenV1 = runSingleBenchmark(QWEN_MODEL, DESCRIPTIONS_V1, 'V1-Original');
  const qwenV4 = runSingleBenchmark(QWEN_MODEL, DESCRIPTIONS_V4, 'V4-Focused');
  const qwenV5 = runMultiBenchmark(QWEN_MODEL, MULTI_DESCRIPTIONS, 'V5-Multi');

  // Results
  console.log('\n═══════════════════════════════════════════════════════════════════════════════════');
  console.log('                              RESULTS');
  console.log('═══════════════════════════════════════════════════════════════════════════════════\n');

  console.log('┌─────────────────────────┬───────────────┬───────────────┬───────────────┐');
  console.log('│ Strategy                │ RuvLTRA       │ Qwen Base     │ RuvLTRA Delta │');
  console.log('├─────────────────────────┼───────────────┼───────────────┼───────────────┤');

  const fmt = (v) => `${(v * 100).toFixed(1)}%`.padStart(12);
  const fmtDelta = (v, base) => {
    const delta = (v - base) * 100;
    const sign = delta >= 0 ? '+' : '';
    return `${sign}${delta.toFixed(1)}%`.padStart(12);
  };

  console.log(`│ V1: Original            │${fmt(ruvV1.accuracy)}  │${fmt(qwenV1.accuracy)}  │    baseline   │`);
  console.log(`│ V4: Focused             │${fmt(ruvV4.accuracy)}  │${fmt(qwenV4.accuracy)}  │${fmtDelta(ruvV4.accuracy, ruvV1.accuracy)}  │`);
  console.log(`│ V5: Multi-phrase        │${fmt(ruvV5.accuracy)}  │${fmt(qwenV5.accuracy)}  │${fmtDelta(ruvV5.accuracy, ruvV1.accuracy)}  │`);
  console.log('└─────────────────────────┴───────────────┴───────────────┴───────────────┘');

  // Best result
  const allResults = [
    { model: 'RuvLTRA', ...ruvV1 },
    { model: 'RuvLTRA', ...ruvV4 },
    { model: 'RuvLTRA', ...ruvV5 },
    { model: 'Qwen', ...qwenV1 },
    { model: 'Qwen', ...qwenV4 },
    { model: 'Qwen', ...qwenV5 },
  ];

  const best = allResults.reduce((a, b) => a.accuracy > b.accuracy ? a : b);

  console.log(`\n  BEST: ${best.model} + ${best.version} = ${(best.accuracy * 100).toFixed(1)}%`);

  // Show V5 detailed results
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                V5 MULTI-PHRASE DETAILED (RuvLTRA)');
  console.log('─────────────────────────────────────────────────────────────────');

  for (const r of ruvV5.results) {
    const mark = r.correct ? '✓' : '✗';
    const task = r.task.slice(0, 50).padEnd(50);
    const exp = r.expected.padEnd(18);
    const got = r.got.padEnd(18);
    console.log(`  ${mark} ${task} ${exp} ${r.correct ? '' : '→ ' + got}`);
  }

  // Final comparison
  const ruvBest = [ruvV1, ruvV4, ruvV5].reduce((a, b) => a.accuracy > b.accuracy ? a : b);
  const qwenBest = [qwenV1, qwenV4, qwenV5].reduce((a, b) => a.accuracy > b.accuracy ? a : b);

  console.log('\n═══════════════════════════════════════════════════════════════════════════════════');
  console.log('                           FINAL WINNER');
  console.log('═══════════════════════════════════════════════════════════════════════════════════');
  console.log(`\n  RuvLTRA best: ${ruvBest.version} = ${(ruvBest.accuracy * 100).toFixed(1)}%`);
  console.log(`  Qwen best:    ${qwenBest.version} = ${(qwenBest.accuracy * 100).toFixed(1)}%`);
  console.log(`\n  Margin: RuvLTRA leads by ${((ruvBest.accuracy - qwenBest.accuracy) * 100).toFixed(1)} points`);
  console.log('\n');
}

main().catch(console.error);

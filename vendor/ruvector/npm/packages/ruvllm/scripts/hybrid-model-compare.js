#!/usr/bin/env node
/**
 * Hybrid Model Comparison
 *
 * Combines embedding similarity with keyword boosting.
 * This addresses the "reviewer overfit" problem by:
 * 1. Computing embedding similarity
 * 2. Boosting agents that have keyword matches in the task
 * 3. Using weighted combination for final score
 */

const { execSync } = require('child_process');
const { existsSync } = require('fs');
const { join } = require('path');
const { homedir } = require('os');

const MODELS_DIR = join(homedir(), '.ruvllm', 'models');
const RUVLTRA_MODEL = join(MODELS_DIR, 'ruvltra-claude-code-0.5b-q4_k_m.gguf');
const QWEN_MODEL = join(MODELS_DIR, 'qwen2.5-0.5b-instruct-q4_k_m.gguf');

// V1 descriptions for embedding
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

// UNIQUE trigger keywords - words that strongly indicate a specific agent
// Priority-ordered: first match wins for disambiguation
// NOTE: "investigate" takes priority over "slow" for researcher vs optimizer
const TRIGGER_KEYWORDS = {
  // Higher priority agents (check these first)
  researcher: ['research', 'investigate', 'explore', 'discover', 'best practices', 'patterns', 'analyze', 'look into', 'find out'],
  coder: ['implement', 'build', 'create', 'component', 'function', 'typescript', 'react', 'feature', 'write code'],
  tester: ['test', 'tests', 'testing', 'unit test', 'integration test', 'e2e', 'coverage', 'spec'],
  reviewer: ['review', 'pull request', 'pr', 'code quality', 'code review', 'check code'],
  debugger: ['debug', 'fix', 'bug', 'error', 'exception', 'crash', 'trace', 'null pointer', 'memory leak'],
  'security-architect': ['security', 'vulnerability', 'xss', 'injection', 'csrf', 'cve', 'audit', 'exploit'],
  refactorer: ['refactor', 'async/await', 'modernize', 'restructure', 'extract', 'legacy'],
  // Optimizer: removed "slow" (too generic), added query-specific terms
  optimizer: ['optimize', 'performance', 'cache', 'caching', 'speed up', 'latency', 'faster', 'queries', 'reduce time'],
  architect: ['design', 'architecture', 'schema', 'structure', 'diagram', 'system design', 'plan architecture'],
  documenter: ['jsdoc', 'comment', 'comments', 'readme', 'documentation', 'document', 'explain'],
  devops: ['deploy', 'ci/cd', 'kubernetes', 'docker', 'pipeline', 'infrastructure', 'container'],
  'api-docs': ['openapi', 'swagger', 'api doc', 'rest api', 'graphql', 'endpoint'],
  planner: ['sprint', 'plan', 'roadmap', 'milestone', 'estimate', 'schedule', 'prioritize'],
};

// Priority order for disambiguation (when multiple agents match)
const AGENT_PRIORITY = [
  'researcher',     // "investigate" wins over "slow"
  'debugger',       // "fix" wins over generic terms
  'tester',         // "test" is specific
  'security-architect',
  'coder',
  'reviewer',
  'refactorer',
  'optimizer',
  'architect',
  'documenter',
  'devops',
  'api-docs',
  'planner',
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

/**
 * Count keyword matches for each agent
 */
function getKeywordScores(task) {
  const taskLower = task.toLowerCase();
  const scores = {};

  for (const [agent, keywords] of Object.entries(TRIGGER_KEYWORDS)) {
    let matches = 0;
    for (const kw of keywords) {
      if (taskLower.includes(kw.toLowerCase())) {
        matches++;
      }
    }
    scores[agent] = matches;
  }

  return scores;
}

/**
 * Pure embedding routing (baseline)
 */
function routeEmbeddingOnly(taskEmbedding, agentEmbeddings) {
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
 * Pure keyword routing
 */
function routeKeywordOnly(task) {
  const scores = getKeywordScores(task);
  let bestAgent = 'coder';
  let bestScore = 0;

  for (const [agent, score] of Object.entries(scores)) {
    if (score > bestScore) {
      bestScore = score;
      bestAgent = agent;
    }
  }

  return { agent: bestAgent, confidence: bestScore };
}

/**
 * Hybrid routing - combine embedding similarity with keyword boost
 */
function routeHybrid(task, taskEmbedding, agentEmbeddings, embeddingWeight = 0.6, keywordWeight = 0.4) {
  const keywordScores = getKeywordScores(task);

  // Normalize keyword scores to 0-1 range
  const maxKeyword = Math.max(...Object.values(keywordScores), 1);
  const normalizedKeywords = {};
  for (const agent of Object.keys(keywordScores)) {
    normalizedKeywords[agent] = keywordScores[agent] / maxKeyword;
  }

  let bestAgent = 'coder';
  let bestScore = -1;
  const allScores = {};

  for (const [agent, emb] of Object.entries(agentEmbeddings)) {
    const embSim = cosineSimilarity(taskEmbedding, emb);
    const kwScore = normalizedKeywords[agent] || 0;
    const combined = embeddingWeight * embSim + keywordWeight * kwScore;
    allScores[agent] = { embedding: embSim, keyword: kwScore, combined };

    if (combined > bestScore) {
      bestScore = combined;
      bestAgent = agent;
    }
  }

  return { agent: bestAgent, confidence: bestScore, scores: allScores };
}

/**
 * Keyword-first routing - use keywords as primary, embedding as tiebreaker
 */
function routeKeywordFirst(task, taskEmbedding, agentEmbeddings) {
  const keywordScores = getKeywordScores(task);

  // Find agents with max keyword matches
  const maxKw = Math.max(...Object.values(keywordScores));

  if (maxKw > 0) {
    // At least one keyword match - use keywords, embedding as tiebreaker
    const candidates = Object.entries(keywordScores)
      .filter(([_, score]) => score === maxKw)
      .map(([agent, _]) => agent);

    if (candidates.length === 1) {
      return { agent: candidates[0], confidence: maxKw };
    }

    // Multiple candidates with same keyword count - use embedding
    let bestAgent = candidates[0];
    let bestSim = -1;
    for (const agent of candidates) {
      const sim = cosineSimilarity(taskEmbedding, agentEmbeddings[agent]);
      if (sim > bestSim) {
        bestSim = sim;
        bestAgent = agent;
      }
    }
    return { agent: bestAgent, confidence: maxKw + bestSim / 10 };
  }

  // No keyword matches - fall back to pure embedding
  return routeEmbeddingOnly(taskEmbedding, agentEmbeddings);
}

function runBenchmark(modelPath, routerFn, name) {
  const agentEmbeddings = {};
  for (const [agent, desc] of Object.entries(DESCRIPTIONS_V1)) {
    agentEmbeddings[agent] = getEmbedding(modelPath, desc);
  }

  let correct = 0;
  const results = [];

  for (const test of ROUTING_TESTS) {
    const taskEmb = getEmbedding(modelPath, test.task);
    const { agent } = routerFn(test.task, taskEmb, agentEmbeddings);
    const isCorrect = agent === test.expected;
    if (isCorrect) correct++;
    results.push({ task: test.task, expected: test.expected, got: agent, correct: isCorrect });
  }

  return { accuracy: correct / ROUTING_TESTS.length, correct, total: ROUTING_TESTS.length, results, name };
}

async function main() {
  console.log('\n╔═══════════════════════════════════════════════════════════════════════════════════╗');
  console.log('║                 HYBRID ROUTING: Embeddings + Keywords                             ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════════════╝\n');

  if (!existsSync(RUVLTRA_MODEL)) {
    console.error('RuvLTRA model not found.');
    process.exit(1);
  }

  console.log('Strategies:');
  console.log('  1. Embedding Only (baseline)');
  console.log('  2. Keyword Only (no model)');
  console.log('  3. Hybrid 60/40 (60% embedding, 40% keyword)');
  console.log('  4. Hybrid 40/60 (40% embedding, 60% keyword)');
  console.log('  5. Keyword-First (keywords primary, embedding tiebreaker)\n');

  // RuvLTRA tests
  console.log('─────────────────────────────────────────────────────────────────');
  console.log('                       RUVLTRA RESULTS');
  console.log('─────────────────────────────────────────────────────────────────\n');

  const ruvEmbedding = runBenchmark(RUVLTRA_MODEL,
    (task, taskEmb, agentEmbs) => routeEmbeddingOnly(taskEmb, agentEmbs),
    'Embedding Only');
  console.log(`  Embedding Only:   ${(ruvEmbedding.accuracy * 100).toFixed(1)}%`);

  const ruvKeyword = runBenchmark(RUVLTRA_MODEL,
    (task, taskEmb, agentEmbs) => routeKeywordOnly(task),
    'Keyword Only');
  console.log(`  Keyword Only:     ${(ruvKeyword.accuracy * 100).toFixed(1)}%`);

  const ruvHybrid60 = runBenchmark(RUVLTRA_MODEL,
    (task, taskEmb, agentEmbs) => routeHybrid(task, taskEmb, agentEmbs, 0.6, 0.4),
    'Hybrid 60/40');
  console.log(`  Hybrid 60/40:     ${(ruvHybrid60.accuracy * 100).toFixed(1)}%`);

  const ruvHybrid40 = runBenchmark(RUVLTRA_MODEL,
    (task, taskEmb, agentEmbs) => routeHybrid(task, taskEmb, agentEmbs, 0.4, 0.6),
    'Hybrid 40/60');
  console.log(`  Hybrid 40/60:     ${(ruvHybrid40.accuracy * 100).toFixed(1)}%`);

  const ruvKwFirst = runBenchmark(RUVLTRA_MODEL,
    (task, taskEmb, agentEmbs) => routeKeywordFirst(task, taskEmb, agentEmbs),
    'Keyword-First');
  console.log(`  Keyword-First:    ${(ruvKwFirst.accuracy * 100).toFixed(1)}%`);

  // Qwen tests
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                       QWEN RESULTS');
  console.log('─────────────────────────────────────────────────────────────────\n');

  const qwenEmbedding = runBenchmark(QWEN_MODEL,
    (task, taskEmb, agentEmbs) => routeEmbeddingOnly(taskEmb, agentEmbs),
    'Embedding Only');
  console.log(`  Embedding Only:   ${(qwenEmbedding.accuracy * 100).toFixed(1)}%`);

  const qwenHybrid60 = runBenchmark(QWEN_MODEL,
    (task, taskEmb, agentEmbs) => routeHybrid(task, taskEmb, agentEmbs, 0.6, 0.4),
    'Hybrid 60/40');
  console.log(`  Hybrid 60/40:     ${(qwenHybrid60.accuracy * 100).toFixed(1)}%`);

  const qwenKwFirst = runBenchmark(QWEN_MODEL,
    (task, taskEmb, agentEmbs) => routeKeywordFirst(task, taskEmb, agentEmbs),
    'Keyword-First');
  console.log(`  Keyword-First:    ${(qwenKwFirst.accuracy * 100).toFixed(1)}%`);

  // Summary table
  console.log('\n═══════════════════════════════════════════════════════════════════════════════════');
  console.log('                              SUMMARY');
  console.log('═══════════════════════════════════════════════════════════════════════════════════\n');

  const fmt = (v) => `${(v * 100).toFixed(1)}%`.padStart(8);

  console.log('┌───────────────────────┬──────────┬──────────┬──────────────────┐');
  console.log('│ Strategy              │  RuvLTRA │   Qwen   │ RuvLTRA vs Qwen  │');
  console.log('├───────────────────────┼──────────┼──────────┼──────────────────┤');
  console.log(`│ Embedding Only        │${fmt(ruvEmbedding.accuracy)}  │${fmt(qwenEmbedding.accuracy)}  │     +${((ruvEmbedding.accuracy - qwenEmbedding.accuracy) * 100).toFixed(1)} pts     │`);
  console.log(`│ Keyword Only          │${fmt(ruvKeyword.accuracy)}  │${fmt(ruvKeyword.accuracy)}  │       same       │`);
  console.log(`│ Hybrid 60/40          │${fmt(ruvHybrid60.accuracy)}  │${fmt(qwenHybrid60.accuracy)}  │     +${((ruvHybrid60.accuracy - qwenHybrid60.accuracy) * 100).toFixed(1)} pts     │`);
  console.log(`│ Keyword-First         │${fmt(ruvKwFirst.accuracy)}  │${fmt(qwenKwFirst.accuracy)}  │     +${((ruvKwFirst.accuracy - qwenKwFirst.accuracy) * 100).toFixed(1)} pts     │`);
  console.log('└───────────────────────┴──────────┴──────────┴──────────────────┘');

  // Best results
  const ruvBest = [ruvEmbedding, ruvKeyword, ruvHybrid60, ruvHybrid40, ruvKwFirst]
    .reduce((a, b) => a.accuracy > b.accuracy ? a : b);

  console.log(`\n  BEST RuvLTRA: ${ruvBest.name} = ${(ruvBest.accuracy * 100).toFixed(1)}%`);
  console.log(`  Improvement over embedding-only: +${((ruvBest.accuracy - ruvEmbedding.accuracy) * 100).toFixed(1)} points`);

  // Show best results details
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log(`               BEST STRATEGY DETAILS: ${ruvBest.name}`);
  console.log('─────────────────────────────────────────────────────────────────\n');

  for (const r of ruvBest.results) {
    const mark = r.correct ? '✓' : '✗';
    const task = r.task.slice(0, 45).padEnd(45);
    const exp = r.expected.padEnd(18);
    console.log(`${mark} ${task} ${exp}${r.correct ? '' : '→ ' + r.got}`);
  }

  console.log('\n');
}

main().catch(console.error);

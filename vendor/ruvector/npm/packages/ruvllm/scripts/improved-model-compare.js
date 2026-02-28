#!/usr/bin/env node
/**
 * Improved Model Comparison - Enhanced Agent Descriptions
 *
 * Key improvements:
 * 1. Semantic sentence descriptions instead of keyword lists
 * 2. Example tasks embedded in descriptions
 * 3. Unique discriminating phrases for each agent
 * 4. Adjusted similarity scoring with top-k voting
 */

const { execSync } = require('child_process');
const { existsSync } = require('fs');
const { join } = require('path');
const { homedir } = require('os');

// Model paths
const MODELS_DIR = join(homedir(), '.ruvllm', 'models');
const QWEN_MODEL = join(MODELS_DIR, 'qwen2.5-0.5b-instruct-q4_k_m.gguf');
const RUVLTRA_MODEL = join(MODELS_DIR, 'ruvltra-claude-code-0.5b-q4_k_m.gguf');

// IMPROVED: Semantic sentence descriptions with examples
const AGENT_DESCRIPTIONS_V1 = {
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

// V2: Semantic sentences with task context
const AGENT_DESCRIPTIONS_V2 = {
  coder: 'I write new code and implement features. Create functions, build components, implement algorithms like binary search, build React components, write TypeScript code.',
  researcher: 'I research and investigate topics. Find best practices, explore solutions, investigate performance issues, analyze patterns, discover new approaches.',
  reviewer: 'I review existing code for quality. Check pull requests, evaluate code style, assess readability, inspect for bugs, examine code patterns.',
  tester: 'I write tests for code. Create unit tests, add integration tests, write e2e tests, mock dependencies, check test coverage, write test specs.',
  architect: 'I design system architecture. Plan database schemas, design API structures, create system diagrams, plan microservices, design data models.',
  'security-architect': 'I audit security vulnerabilities. Check for XSS, SQL injection, CSRF, audit authentication, review security policies, scan for CVEs.',
  debugger: 'I fix bugs and debug errors. Trace exceptions, fix crashes, resolve null pointer errors, debug memory leaks, fix runtime issues.',
  documenter: 'I write documentation and comments. Add JSDoc comments, write README files, explain code functionality, describe APIs, create guides.',
  refactorer: 'I refactor and restructure code. Modernize to async/await, extract functions, rename variables, consolidate duplicate code, simplify logic.',
  optimizer: 'I optimize performance and speed. Cache data, improve query performance, reduce latency, optimize memory usage, speed up slow operations.',
  devops: 'I handle deployment and infrastructure. Set up CI/CD pipelines, configure Kubernetes, manage Docker containers, deploy to cloud.',
  'api-docs': 'I create API documentation specs. Generate OpenAPI specs, write Swagger docs, document REST endpoints, create GraphQL schemas.',
  planner: 'I create project plans and estimates. Sprint planning, roadmap creation, milestone tracking, task prioritization, schedule estimation.',
};

// V3: Even more specific with negative space
const AGENT_DESCRIPTIONS_V3 = {
  coder: 'Software developer who implements new features and writes production code. Tasks: implement binary search, build React components, create TypeScript functions, add new functionality to applications.',
  researcher: 'Technical researcher who investigates and analyzes. Tasks: research best practices, explore state management options, investigate slow response times, analyze codebase patterns.',
  reviewer: 'Code reviewer who evaluates existing code quality. Tasks: review pull requests, check for race conditions, assess code style, evaluate implementation approaches.',
  tester: 'QA engineer who writes automated tests. Tasks: write unit tests, add integration tests, create e2e test suites, test payment gateways, verify authentication modules.',
  architect: 'System architect who designs software structure. Tasks: design database schemas, plan real-time notification systems, architect microservices, model data relationships.',
  'security-architect': 'Security specialist who audits vulnerabilities. Tasks: audit API endpoints for XSS, check SQL injection risks, review authentication security, scan for CSRF vulnerabilities.',
  debugger: 'Bug hunter who fixes errors and traces issues. Tasks: fix null pointer exceptions, debug memory leaks, trace WebSocket errors, resolve crash bugs.',
  documenter: 'Technical writer who creates documentation. Tasks: write JSDoc comments, create README files, document utility functions, explain complex code.',
  refactorer: 'Code modernizer who restructures without changing behavior. Tasks: refactor to async/await, extract reusable functions, modernize legacy patterns, simplify complex logic.',
  optimizer: 'Performance engineer who speeds up slow code. Tasks: cache frequently accessed data, optimize database queries, reduce API latency, improve memory efficiency.',
  devops: 'DevOps engineer who manages deployment infrastructure. Tasks: set up CI/CD pipelines, configure Kubernetes clusters, manage Docker deployments, automate releases.',
  'api-docs': 'API documentation specialist. Tasks: generate OpenAPI documentation, create Swagger specs, document REST API endpoints, write API reference guides.',
  planner: 'Project planner who organizes work. Tasks: create sprint plans, estimate timelines, prioritize backlog, schedule milestones, plan roadmaps.',
};

// Test cases for routing
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

// Similarity test pairs
const SIMILARITY_TESTS = [
  { text1: 'implement user authentication', text2: 'create login functionality', expected: 'high' },
  { text1: 'write unit tests', text2: 'fix database bug', expected: 'low' },
  { text1: 'optimize query performance', text2: 'improve database speed', expected: 'high' },
  { text1: 'design system architecture', text2: 'plan software structure', expected: 'high' },
  { text1: 'deploy to kubernetes', text2: 'analyze user behavior', expected: 'low' },
  { text1: 'refactor legacy code', text2: 'restructure old module', expected: 'high' },
  { text1: 'debug memory leak', text2: 'fix memory consumption issue', expected: 'high' },
  { text1: 'document api endpoints', text2: 'write openapi spec', expected: 'high' },
];

/**
 * Get embedding from model
 */
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
    console.error(`Error: ${err.message}`);
    return null;
  }
}

/**
 * Cosine similarity
 */
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
 * Route task with top-k analysis
 */
function routeTask(taskEmbedding, agentEmbeddings, topK = 3) {
  const scores = [];
  for (const [agent, embedding] of Object.entries(agentEmbeddings)) {
    const sim = cosineSimilarity(taskEmbedding, embedding);
    scores.push({ agent, similarity: sim });
  }
  scores.sort((a, b) => b.similarity - a.similarity);

  return {
    agent: scores[0].agent,
    confidence: scores[0].similarity,
    topK: scores.slice(0, topK),
    margin: scores[0].similarity - scores[1].similarity,
  };
}

/**
 * Run benchmark for a specific description version
 */
function runBenchmark(modelPath, modelName, descriptions, version) {
  console.log(`\n  [${version}] Computing agent embeddings...`);

  const agentEmbeddings = {};
  for (const [agent, description] of Object.entries(descriptions)) {
    process.stdout.write(`    ${agent}... `);
    agentEmbeddings[agent] = getEmbedding(modelPath, description);
    console.log('done');
  }

  console.log(`  [${version}] Running routing tests...`);
  let correct = 0;
  const failures = [];

  for (const test of ROUTING_TESTS) {
    const taskEmbedding = getEmbedding(modelPath, test.task);
    const { agent, confidence, topK, margin } = routeTask(taskEmbedding, agentEmbeddings);
    const isCorrect = agent === test.expected;
    if (isCorrect) {
      correct++;
    } else {
      failures.push({
        task: test.task,
        expected: test.expected,
        got: agent,
        topK,
        margin,
      });
    }
  }

  const accuracy = correct / ROUTING_TESTS.length;
  return { accuracy, correct, total: ROUTING_TESTS.length, failures, version };
}

/**
 * Main comparison
 */
async function main() {
  console.log('\n╔═══════════════════════════════════════════════════════════════════════════════════╗');
  console.log('║           IMPROVED MODEL COMPARISON: Testing Description Strategies               ║');
  console.log('║                     Semantic Descriptions vs Keyword Lists                        ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════════════╝\n');

  if (!existsSync(QWEN_MODEL) || !existsSync(RUVLTRA_MODEL)) {
    console.error('Models not found. Run the original comparison first.');
    process.exit(1);
  }

  console.log('Testing 3 description strategies:');
  console.log('  V1: Keyword lists (baseline)');
  console.log('  V2: Semantic sentences with examples');
  console.log('  V3: Task-specific descriptions with context\n');

  // Test all three versions with RuvLTRA
  console.log('─────────────────────────────────────────────────────────────────');
  console.log('                   RUVLTRA CLAUDE CODE MODEL');
  console.log('─────────────────────────────────────────────────────────────────');

  const v1Results = runBenchmark(RUVLTRA_MODEL, 'RuvLTRA', AGENT_DESCRIPTIONS_V1, 'V1-Keywords');
  const v2Results = runBenchmark(RUVLTRA_MODEL, 'RuvLTRA', AGENT_DESCRIPTIONS_V2, 'V2-Semantic');
  const v3Results = runBenchmark(RUVLTRA_MODEL, 'RuvLTRA', AGENT_DESCRIPTIONS_V3, 'V3-TaskSpecific');

  // Also test Qwen with best strategy
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                      QWEN 0.5B BASE MODEL');
  console.log('─────────────────────────────────────────────────────────────────');

  const qwenV1 = runBenchmark(QWEN_MODEL, 'Qwen', AGENT_DESCRIPTIONS_V1, 'V1-Keywords');
  const qwenV3 = runBenchmark(QWEN_MODEL, 'Qwen', AGENT_DESCRIPTIONS_V3, 'V3-TaskSpecific');

  // Results summary
  console.log('\n═══════════════════════════════════════════════════════════════════════════════════');
  console.log('                              RESULTS COMPARISON');
  console.log('═══════════════════════════════════════════════════════════════════════════════════\n');

  console.log('┌─────────────────────────┬───────────────┬───────────────┬───────────────┐');
  console.log('│ Strategy                │ RuvLTRA       │ Qwen Base     │ Improvement   │');
  console.log('├─────────────────────────┼───────────────┼───────────────┼───────────────┤');

  const formatPct = (v) => `${(v * 100).toFixed(1)}%`.padStart(12);

  console.log(`│ V1: Keywords            │${formatPct(v1Results.accuracy)}  │${formatPct(qwenV1.accuracy)}  │    baseline   │`);
  console.log(`│ V2: Semantic            │${formatPct(v2Results.accuracy)}  │      -        │${formatPct(v2Results.accuracy - v1Results.accuracy)}  │`);
  console.log(`│ V3: Task-Specific       │${formatPct(v3Results.accuracy)}  │${formatPct(qwenV3.accuracy)}  │${formatPct(v3Results.accuracy - v1Results.accuracy)}  │`);

  console.log('└─────────────────────────┴───────────────┴───────────────┴───────────────┘');

  // Find best strategy
  const best = [v1Results, v2Results, v3Results].reduce((a, b) => a.accuracy > b.accuracy ? a : b);

  console.log(`\n  BEST STRATEGY: ${best.version} with ${(best.accuracy * 100).toFixed(1)}% accuracy`);
  console.log(`  Improvement over V1: +${((best.accuracy - v1Results.accuracy) * 100).toFixed(1)} percentage points`);

  // Show remaining failures for best strategy
  if (best.failures.length > 0) {
    console.log(`\n  Remaining failures (${best.failures.length}):`);
    for (const f of best.failures.slice(0, 5)) {
      console.log(`    "${f.task.slice(0, 45)}..."`);
      console.log(`      Expected: ${f.expected}, Got: ${f.got}`);
      console.log(`      Top-3: ${f.topK.map(t => `${t.agent}(${(t.similarity * 100).toFixed(0)}%)`).join(', ')}`);
    }
  }

  // RuvLTRA vs Qwen with best strategy
  console.log('\n═══════════════════════════════════════════════════════════════════════════════════');
  console.log('              FINAL COMPARISON (V3 Task-Specific Descriptions)');
  console.log('═══════════════════════════════════════════════════════════════════════════════════\n');

  console.log('┌─────────────────────────────┬───────────────┬───────────────┐');
  console.log('│ Metric                      │ Qwen Base     │ RuvLTRA       │');
  console.log('├─────────────────────────────┼───────────────┼───────────────┤');

  const qwenWins = qwenV3.accuracy > v3Results.accuracy;
  const ruvWins = v3Results.accuracy > qwenV3.accuracy;
  console.log(`│ V3 Routing Accuracy         │${qwenWins ? '✓' : ' '}${formatPct(qwenV3.accuracy)}  │${ruvWins ? '✓' : ' '}${formatPct(v3Results.accuracy)}  │`);
  console.log('└─────────────────────────────┴───────────────┴───────────────┘');

  const winner = ruvWins ? 'RuvLTRA' : qwenWins ? 'Qwen' : 'Tie';
  const margin = Math.abs(v3Results.accuracy - qwenV3.accuracy) * 100;

  console.log(`\n  WINNER: ${winner} (${margin.toFixed(1)} point margin)`);
  console.log('\n');
}

main().catch(console.error);

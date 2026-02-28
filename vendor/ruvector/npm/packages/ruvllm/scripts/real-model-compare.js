#!/usr/bin/env node
/**
 * Real Model Comparison - Qwen 0.5B vs RuvLTRA Claude Code
 *
 * Uses llama-embedding for actual model inference.
 */

const { execSync } = require('child_process');
const { existsSync } = require('fs');
const { join } = require('path');
const { homedir } = require('os');

// Model paths
const MODELS_DIR = join(homedir(), '.ruvllm', 'models');
const QWEN_MODEL = join(MODELS_DIR, 'qwen2.5-0.5b-instruct-q4_k_m.gguf');
const RUVLTRA_MODEL = join(MODELS_DIR, 'ruvltra-claude-code-0.5b-q4_k_m.gguf');

// Agent descriptions for routing
const AGENT_DESCRIPTIONS = {
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
 * Get embedding from model using llama-embedding
 */
function getEmbedding(modelPath, text) {
  try {
    const sanitized = text.replace(/"/g, '\\"').replace(/\n/g, ' ');
    const result = execSync(
      `llama-embedding -m "${modelPath}" -p "${sanitized}" --embd-output-format json 2>/dev/null`,
      { encoding: 'utf-8', maxBuffer: 10 * 1024 * 1024 }
    );

    const json = JSON.parse(result);
    // Return the last embedding (the full prompt embedding)
    return json.data[json.data.length - 1].embedding;
  } catch (err) {
    console.error(`Error getting embedding: ${err.message}`);
    return null;
  }
}

/**
 * Compute cosine similarity
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
 * Route task to agent using embedding similarity
 */
function routeTask(taskEmbedding, agentEmbeddings) {
  let bestAgent = 'coder';
  let bestSimilarity = -1;

  for (const [agent, embedding] of Object.entries(agentEmbeddings)) {
    const sim = cosineSimilarity(taskEmbedding, embedding);
    if (sim > bestSimilarity) {
      bestSimilarity = sim;
      bestAgent = agent;
    }
  }

  return { agent: bestAgent, confidence: bestSimilarity };
}

/**
 * Run routing benchmark for a model
 */
function runRoutingBenchmark(modelPath, modelName) {
  console.log(`\n  Computing agent embeddings for ${modelName}...`);

  // Pre-compute agent embeddings
  const agentEmbeddings = {};
  for (const [agent, description] of Object.entries(AGENT_DESCRIPTIONS)) {
    process.stdout.write(`    ${agent}... `);
    agentEmbeddings[agent] = getEmbedding(modelPath, description);
    console.log('done');
  }

  console.log(`  Running routing tests...`);
  let correct = 0;
  const results = [];

  for (const test of ROUTING_TESTS) {
    process.stdout.write(`    "${test.task.slice(0, 40)}..." `);
    const taskEmbedding = getEmbedding(modelPath, test.task);
    const { agent, confidence } = routeTask(taskEmbedding, agentEmbeddings);
    const isCorrect = agent === test.expected;
    if (isCorrect) correct++;
    console.log(`${agent} (expected: ${test.expected}) ${isCorrect ? '✓' : '✗'}`);
    results.push({ task: test.task, expected: test.expected, actual: agent, correct: isCorrect, confidence });
  }

  const accuracy = correct / ROUTING_TESTS.length;
  return { accuracy, correct, total: ROUTING_TESTS.length, results };
}

/**
 * Run similarity benchmark for a model
 */
function runSimilarityBenchmark(modelPath, modelName) {
  console.log(`\n  Running similarity tests for ${modelName}...`);

  let correct = 0;
  const results = [];

  for (const test of SIMILARITY_TESTS) {
    process.stdout.write(`    "${test.text1}" vs "${test.text2}"... `);

    const emb1 = getEmbedding(modelPath, test.text1);
    const emb2 = getEmbedding(modelPath, test.text2);
    const similarity = cosineSimilarity(emb1, emb2);

    // Threshold: > 0.7 is high, < 0.5 is low
    const predicted = similarity > 0.6 ? 'high' : 'low';
    const isCorrect = predicted === test.expected;
    if (isCorrect) correct++;

    console.log(`${(similarity * 100).toFixed(1)}% (${predicted}, expected: ${test.expected}) ${isCorrect ? '✓' : '✗'}`);
    results.push({ text1: test.text1, text2: test.text2, similarity, predicted, expected: test.expected, correct: isCorrect });
  }

  const accuracy = correct / SIMILARITY_TESTS.length;
  return { accuracy, correct, total: SIMILARITY_TESTS.length, results };
}

/**
 * Main comparison
 */
async function main() {
  console.log('\n╔═══════════════════════════════════════════════════════════════════════════════════╗');
  console.log('║              REAL MODEL COMPARISON: Qwen 0.5B vs RuvLTRA Claude Code              ║');
  console.log('║                          Using llama-embedding inference                          ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════════════╝\n');

  // Check models exist
  if (!existsSync(QWEN_MODEL)) {
    console.error(`Qwen model not found at: ${QWEN_MODEL}`);
    console.error('Download with: curl -L -o ~/.ruvllm/models/qwen2.5-0.5b-instruct-q4_k_m.gguf "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf"');
    process.exit(1);
  }

  if (!existsSync(RUVLTRA_MODEL)) {
    console.error(`RuvLTRA model not found at: ${RUVLTRA_MODEL}`);
    console.error('Download with: ruvllm models download claude-code');
    process.exit(1);
  }

  console.log('Models found:');
  console.log(`  Qwen:    ${QWEN_MODEL}`);
  console.log(`  RuvLTRA: ${RUVLTRA_MODEL}`);

  // Run benchmarks for both models
  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                    QWEN 0.5B BASE MODEL');
  console.log('─────────────────────────────────────────────────────────────────');

  const qwenRouting = runRoutingBenchmark(QWEN_MODEL, 'Qwen 0.5B');
  const qwenSimilarity = runSimilarityBenchmark(QWEN_MODEL, 'Qwen 0.5B');

  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                   RUVLTRA CLAUDE CODE MODEL');
  console.log('─────────────────────────────────────────────────────────────────');

  const ruvltraRouting = runRoutingBenchmark(RUVLTRA_MODEL, 'RuvLTRA Claude Code');
  const ruvltraSimilarity = runSimilarityBenchmark(RUVLTRA_MODEL, 'RuvLTRA Claude Code');

  // Results summary
  console.log('\n═══════════════════════════════════════════════════════════════════════════════════');
  console.log('                              COMPARISON RESULTS');
  console.log('═══════════════════════════════════════════════════════════════════════════════════\n');

  console.log('┌─────────────────────────────┬───────────────┬───────────────┐');
  console.log('│ Metric                      │ Qwen Base     │ RuvLTRA       │');
  console.log('├─────────────────────────────┼───────────────┼───────────────┤');

  const qwenRoutingPct = `${(qwenRouting.accuracy * 100).toFixed(1)}%`;
  const ruvltraRoutingPct = `${(ruvltraRouting.accuracy * 100).toFixed(1)}%`;
  const routingWinner = ruvltraRouting.accuracy > qwenRouting.accuracy ? '✓' : ' ';
  const routingLoser = qwenRouting.accuracy > ruvltraRouting.accuracy ? '✓' : ' ';
  console.log(`│ Routing Accuracy            │${routingLoser}${qwenRoutingPct.padStart(12)}  │${routingWinner}${ruvltraRoutingPct.padStart(12)}  │`);

  const qwenSimPct = `${(qwenSimilarity.accuracy * 100).toFixed(1)}%`;
  const ruvltraSimPct = `${(ruvltraSimilarity.accuracy * 100).toFixed(1)}%`;
  const simWinner = ruvltraSimilarity.accuracy > qwenSimilarity.accuracy ? '✓' : ' ';
  const simLoser = qwenSimilarity.accuracy > ruvltraSimilarity.accuracy ? '✓' : ' ';
  console.log(`│ Similarity Detection        │${simLoser}${qwenSimPct.padStart(12)}  │${simWinner}${ruvltraSimPct.padStart(12)}  │`);

  // Overall score
  const qwenOverall = (qwenRouting.accuracy * 0.6 + qwenSimilarity.accuracy * 0.4);
  const ruvltraOverall = (ruvltraRouting.accuracy * 0.6 + ruvltraSimilarity.accuracy * 0.4);
  const qwenOverallPct = `${(qwenOverall * 100).toFixed(1)}%`;
  const ruvltraOverallPct = `${(ruvltraOverall * 100).toFixed(1)}%`;
  const overallWinner = ruvltraOverall > qwenOverall ? '✓' : ' ';
  const overallLoser = qwenOverall > ruvltraOverall ? '✓' : ' ';
  console.log('├─────────────────────────────┼───────────────┼───────────────┤');
  console.log(`│ Overall Score (60/40)       │${overallLoser}${qwenOverallPct.padStart(12)}  │${overallWinner}${ruvltraOverallPct.padStart(12)}  │`);

  console.log('└─────────────────────────────┴───────────────┴───────────────┘');

  // Winner announcement
  const winner = ruvltraOverall > qwenOverall ? 'RuvLTRA Claude Code' : 'Qwen 0.5B Base';
  const improvement = Math.abs(ruvltraOverall - qwenOverall) * 100;

  console.log('\n═══════════════════════════════════════════════════════════════════════════════════');
  console.log(`  WINNER: ${winner}`);
  console.log('═══════════════════════════════════════════════════════════════════════════════════');

  if (ruvltraOverall > qwenOverall) {
    console.log(`\n  RuvLTRA outperforms Qwen base by ${improvement.toFixed(1)} percentage points.`);
    console.log('  Fine-tuning for Claude Code workflows provides measurable improvements.');
  } else if (qwenOverall > ruvltraOverall) {
    console.log(`\n  Qwen base outperforms RuvLTRA by ${improvement.toFixed(1)} percentage points.`);
    console.log('  Consider additional fine-tuning or different training approach.');
  } else {
    console.log('\n  Both models perform equally. Fine-tuning may need adjustment.');
  }

  console.log('\n');
}

main().catch(console.error);

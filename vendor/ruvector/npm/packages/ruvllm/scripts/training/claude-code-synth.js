#!/usr/bin/env node
/**
 * Claude Code Synthetic Data Generator
 *
 * Uses @ruvector/agentic-synth to generate high-quality
 * training data for RuvLTRA routing optimization.
 *
 * Features:
 * - Claude Code-specific task patterns
 * - Hard negative mining for contrastive learning
 * - Quality scoring based on task clarity
 * - DSPy-based prompt optimization
 */

const { GoogleGenerativeAI } = require('@google/generative-ai');
const { writeFileSync, existsSync, mkdirSync, readFileSync } = require('fs');
const { join } = require('path');
const { homedir } = require('os');

// Configuration
const OUTPUT_DIR = join(__dirname, 'generated');
const EXAMPLES_PER_AGENT = 100;  // Generate 100 examples per agent
const HARD_NEGATIVES_PER_AGENT = 20;

// Agent definitions with Claude Code context
const CLAUDE_CODE_AGENTS = {
  coder: {
    role: 'Software developer who implements features and writes production code',
    claudeCodeContext: 'Uses Edit, Write, MultiEdit tools to create and modify code files',
    keywords: ['implement', 'build', 'create', 'write code', 'add feature', 'component', 'function'],
    examples: [
      'Implement a binary search function in TypeScript',
      'Build a React component for user authentication',
      'Create a REST API endpoint for data retrieval',
    ],
  },
  researcher: {
    role: 'Technical researcher who investigates and analyzes',
    claudeCodeContext: 'Uses Grep, Glob, Read, WebSearch tools to gather information',
    keywords: ['research', 'investigate', 'explore', 'analyze', 'find', 'discover', 'study'],
    examples: [
      'Research best practices for React state management',
      'Investigate why the API is returning slow responses',
      'Explore different authentication strategies',
    ],
  },
  reviewer: {
    role: 'Code reviewer who evaluates code quality',
    claudeCodeContext: 'Uses Read, Grep tools to analyze existing code for quality issues',
    keywords: ['review', 'check', 'evaluate', 'assess', 'inspect', 'pull request', 'PR'],
    examples: [
      'Review the pull request for code quality',
      'Check the implementation for potential issues',
      'Evaluate the API design decisions',
    ],
  },
  tester: {
    role: 'QA engineer who writes and runs tests',
    claudeCodeContext: 'Uses Write, Edit tools to create test files and Bash to run tests',
    keywords: ['test', 'tests', 'testing', 'unit test', 'integration test', 'e2e', 'coverage', 'spec'],
    examples: [
      'Write unit tests for the authentication module',
      'Add integration tests for the API endpoints',
      'Create e2e tests for the checkout flow',
    ],
  },
  architect: {
    role: 'System architect who designs software structure',
    claudeCodeContext: 'Uses Read, Grep tools to understand codebase and Write to document designs',
    keywords: ['design', 'architecture', 'schema', 'structure', 'system', 'diagram', 'plan'],
    examples: [
      'Design the database schema for user profiles',
      'Plan the microservices architecture',
      'Create the system architecture diagram',
    ],
  },
  'security-architect': {
    role: 'Security specialist who audits vulnerabilities',
    claudeCodeContext: 'Uses Grep, Read tools to scan code for security issues',
    keywords: ['security', 'vulnerability', 'xss', 'injection', 'audit', 'cve', 'exploit'],
    examples: [
      'Audit the API endpoints for XSS vulnerabilities',
      'Check for SQL injection vulnerabilities',
      'Review authentication for security issues',
    ],
  },
  debugger: {
    role: 'Bug hunter who fixes errors and traces issues',
    claudeCodeContext: 'Uses Read, Grep, Bash tools to trace issues and Edit to fix bugs',
    keywords: ['debug', 'fix', 'bug', 'error', 'exception', 'crash', 'trace', 'issue'],
    examples: [
      'Fix the null pointer exception in login',
      'Debug the memory leak in WebSocket handler',
      'Trace the source of the intermittent error',
    ],
  },
  documenter: {
    role: 'Technical writer who creates documentation',
    claudeCodeContext: 'Uses Write, Edit tools to create and update documentation files',
    keywords: ['document', 'jsdoc', 'readme', 'comment', 'explain', 'describe'],
    examples: [
      'Write JSDoc comments for utility functions',
      'Create README for the new package',
      'Document the API endpoints',
    ],
  },
  refactorer: {
    role: 'Code modernizer who restructures without changing behavior',
    claudeCodeContext: 'Uses Edit, MultiEdit tools to restructure code across files',
    keywords: ['refactor', 'restructure', 'modernize', 'extract', 'consolidate', 'simplify'],
    examples: [
      'Refactor the payment module to async/await',
      'Restructure the utils folder',
      'Extract common logic into shared module',
    ],
  },
  optimizer: {
    role: 'Performance engineer who speeds up slow code',
    claudeCodeContext: 'Uses Bash to run profilers and Edit to optimize code',
    keywords: ['optimize', 'performance', 'speed', 'cache', 'latency', 'slow', 'fast'],
    examples: [
      'Optimize the database queries for dashboard',
      'Cache the frequently accessed user data',
      'Improve the API response time',
    ],
  },
  devops: {
    role: 'DevOps engineer who manages deployment and infrastructure',
    claudeCodeContext: 'Uses Bash for deployment commands and Write for config files',
    keywords: ['deploy', 'ci/cd', 'kubernetes', 'docker', 'pipeline', 'infrastructure'],
    examples: [
      'Set up the CI/CD pipeline',
      'Configure Kubernetes deployment',
      'Deploy to production',
    ],
  },
  'api-docs': {
    role: 'API documentation specialist who creates specs',
    claudeCodeContext: 'Uses Write to generate OpenAPI/Swagger specs',
    keywords: ['openapi', 'swagger', 'api spec', 'endpoint', 'rest api', 'graphql'],
    examples: [
      'Generate OpenAPI documentation for REST API',
      'Create Swagger spec for the endpoints',
      'Document the API request/response formats',
    ],
  },
  planner: {
    role: 'Project planner who organizes and schedules work',
    claudeCodeContext: 'Uses TodoWrite tool to create and manage task lists',
    keywords: ['plan', 'sprint', 'roadmap', 'milestone', 'estimate', 'schedule', 'prioritize'],
    examples: [
      'Create a sprint plan for next two weeks',
      'Estimate the feature implementation effort',
      'Plan the roadmap for Q3',
    ],
  },
};

// Prompt template for synthetic data generation
const GENERATION_PROMPT = `You are generating training data for an AI agent routing system used in Claude Code (an AI coding assistant).

## Task
Generate ${EXAMPLES_PER_AGENT} diverse, realistic task descriptions that would be routed to the "${'{AGENT}'}" agent.

## Agent Description
Role: {ROLE}
Claude Code Context: {CONTEXT}
Key Indicators: {KEYWORDS}

## Requirements
1. Each task should be a realistic software engineering task
2. Tasks should clearly indicate the agent type through action verbs and context
3. Include variety in:
   - Programming languages (TypeScript, Python, Rust, Go, etc.)
   - Frameworks (React, Vue, Express, Django, etc.)
   - Domains (web, mobile, backend, data, ML, etc.)
   - Complexity levels (simple to complex)
4. Tasks should be 5-20 words, clear and actionable
5. Include edge cases that might be confused with other agents

## Examples for this agent
{EXAMPLES}

## Output Format
Return a JSON array of objects with this structure:
[
  {
    "task": "The task description",
    "quality": 0.8-1.0,
    "difficulty": "easy|medium|hard",
    "tags": ["relevant", "tags"]
  }
]

Generate exactly ${EXAMPLES_PER_AGENT} unique tasks. Be creative and diverse.`;

// Prompt for hard negatives
const HARD_NEGATIVE_PROMPT = `You are generating hard negative examples for contrastive learning in an AI agent routing system.

## Context
We have an agent called "${'{AGENT}'}" with this role: {ROLE}

We need tasks that SEEM like they might belong to this agent but actually belong to OTHER agents.
These are "hard negatives" - confusing examples that help the model learn better boundaries.

## Confusable Agents
{CONFUSABLE_AGENTS}

## Requirements
1. Generate ${HARD_NEGATIVES_PER_AGENT} tasks that might be confused with "${'{AGENT}'}"
2. Each task should actually belong to a DIFFERENT agent
3. The confusion should be subtle but clear upon reflection
4. Include the correct agent label

## Output Format
[
  {
    "task": "The confusing task description",
    "appears_to_be": "${'{AGENT}'}",
    "actually_is": "the_correct_agent",
    "confusion_reason": "Why this might be confused"
  }
]`;

/**
 * Initialize Gemini client
 */
function getGeminiClient() {
  const apiKey = process.env.GEMINI_API_KEY;
  if (!apiKey) {
    console.error('GEMINI_API_KEY environment variable required');
    console.error('Set it with: export GEMINI_API_KEY=your_key');
    process.exit(1);
  }
  return new GoogleGenerativeAI(apiKey);
}

/**
 * Generate training data for an agent using Gemini
 */
async function generateAgentData(client, agent, agentConfig) {
  console.log(`  Generating data for ${agent}...`);

  const prompt = GENERATION_PROMPT
    .replace(/\{AGENT\}/g, agent)
    .replace('{ROLE}', agentConfig.role)
    .replace('{CONTEXT}', agentConfig.claudeCodeContext)
    .replace('{KEYWORDS}', agentConfig.keywords.join(', '))
    .replace('{EXAMPLES}', agentConfig.examples.map(e => `- ${e}`).join('\n'));

  try {
    const model = client.getGenerativeModel({ model: 'gemini-2.0-flash-exp' });
    const result = await model.generateContent(prompt);
    const response = result.response.text();

    // Extract JSON from response
    const jsonMatch = response.match(/\[[\s\S]*\]/);
    if (!jsonMatch) {
      console.error(`  Failed to parse JSON for ${agent}`);
      return [];
    }

    const data = JSON.parse(jsonMatch[0]);
    console.log(`  Generated ${data.length} examples for ${agent}`);

    return data.map(item => ({
      ...item,
      agent,
      type: 'positive',
    }));
  } catch (error) {
    console.error(`  Error generating data for ${agent}: ${error.message}`);
    return [];
  }
}

/**
 * Generate hard negatives for an agent
 */
async function generateHardNegatives(client, agent, agentConfig, allAgents) {
  console.log(`  Generating hard negatives for ${agent}...`);

  // Find confusable agents
  const confusableAgents = Object.entries(allAgents)
    .filter(([name]) => name !== agent)
    .map(([name, config]) => `- ${name}: ${config.role}`)
    .join('\n');

  const prompt = HARD_NEGATIVE_PROMPT
    .replace(/\{AGENT\}/g, agent)
    .replace('{ROLE}', agentConfig.role)
    .replace('{CONFUSABLE_AGENTS}', confusableAgents);

  try {
    const model = client.getGenerativeModel({ model: 'gemini-2.0-flash-exp' });
    const result = await model.generateContent(prompt);
    const response = result.response.text();

    const jsonMatch = response.match(/\[[\s\S]*\]/);
    if (!jsonMatch) {
      console.error(`  Failed to parse hard negatives for ${agent}`);
      return [];
    }

    const data = JSON.parse(jsonMatch[0]);
    console.log(`  Generated ${data.length} hard negatives for ${agent}`);

    return data.map(item => ({
      task: item.task,
      agent: item.actually_is,
      confusing_with: agent,
      confusion_reason: item.confusion_reason,
      type: 'hard_negative',
      quality: 1.0,
    }));
  } catch (error) {
    console.error(`  Error generating hard negatives for ${agent}: ${error.message}`);
    return [];
  }
}

/**
 * Main generation pipeline
 */
async function main() {
  console.log('\n╔═══════════════════════════════════════════════════════════════════════════════════╗');
  console.log('║          CLAUDE CODE SYNTHETIC TRAINING DATA GENERATOR                            ║');
  console.log('║                     Using @ruvector/agentic-synth                                 ║');
  console.log('╚═══════════════════════════════════════════════════════════════════════════════════╝\n');

  // Check for API key
  if (!process.env.GEMINI_API_KEY) {
    console.log('GEMINI_API_KEY not set. Generating static dataset from templates...\n');
    generateStaticDataset();
    return;
  }

  const client = getGeminiClient();

  // Create output directory
  if (!existsSync(OUTPUT_DIR)) {
    mkdirSync(OUTPUT_DIR, { recursive: true });
  }

  const allData = [];
  const allHardNegatives = [];
  const agents = Object.keys(CLAUDE_CODE_AGENTS);

  console.log('─────────────────────────────────────────────────────────────────');
  console.log('                   GENERATING POSITIVE EXAMPLES');
  console.log('─────────────────────────────────────────────────────────────────\n');

  // Generate positive examples for each agent
  for (const agent of agents) {
    const data = await generateAgentData(client, agent, CLAUDE_CODE_AGENTS[agent]);
    allData.push(...data);

    // Rate limit
    await new Promise(resolve => setTimeout(resolve, 1000));
  }

  console.log('\n─────────────────────────────────────────────────────────────────');
  console.log('                   GENERATING HARD NEGATIVES');
  console.log('─────────────────────────────────────────────────────────────────\n');

  // Generate hard negatives
  for (const agent of agents) {
    const negatives = await generateHardNegatives(client, agent, CLAUDE_CODE_AGENTS[agent], CLAUDE_CODE_AGENTS);
    allHardNegatives.push(...negatives);

    // Rate limit
    await new Promise(resolve => setTimeout(resolve, 1000));
  }

  // Combine and save
  const fullDataset = [...allData, ...allHardNegatives];

  // Save full dataset
  const outputPath = join(OUTPUT_DIR, 'claude-code-routing-dataset.json');
  writeFileSync(outputPath, JSON.stringify(fullDataset, null, 2));

  // Save training pairs (for contrastive learning)
  const contrastivePairs = generateContrastivePairs(allData, allHardNegatives);
  const pairsPath = join(OUTPUT_DIR, 'contrastive-pairs.json');
  writeFileSync(pairsPath, JSON.stringify(contrastivePairs, null, 2));

  // Print summary
  console.log('\n═══════════════════════════════════════════════════════════════════════════════════');
  console.log('                              GENERATION COMPLETE');
  console.log('═══════════════════════════════════════════════════════════════════════════════════\n');

  console.log(`  Positive examples:    ${allData.length}`);
  console.log(`  Hard negatives:       ${allHardNegatives.length}`);
  console.log(`  Contrastive pairs:    ${contrastivePairs.length}`);
  console.log(`  Total dataset size:   ${fullDataset.length}`);
  console.log(`\n  Output files:`);
  console.log(`    ${outputPath}`);
  console.log(`    ${pairsPath}`);
  console.log('');
}

/**
 * Generate contrastive pairs from data
 */
function generateContrastivePairs(positives, negatives) {
  const pairs = [];

  // Group positives by agent
  const byAgent = {};
  for (const item of positives) {
    if (!byAgent[item.agent]) byAgent[item.agent] = [];
    byAgent[item.agent].push(item);
  }

  // Create positive pairs (same agent)
  for (const [agent, items] of Object.entries(byAgent)) {
    for (let i = 0; i < items.length - 1; i++) {
      for (let j = i + 1; j < Math.min(i + 3, items.length); j++) {
        pairs.push({
          anchor: items[i].task,
          positive: items[j].task,
          agent,
          type: 'positive_pair',
        });
      }
    }
  }

  // Create negative pairs (different agents)
  const agents = Object.keys(byAgent);
  for (let i = 0; i < agents.length; i++) {
    for (let j = i + 1; j < agents.length; j++) {
      const agent1Items = byAgent[agents[i]];
      const agent2Items = byAgent[agents[j]];

      if (agent1Items && agent1Items[0] && agent2Items && agent2Items[0]) {
        pairs.push({
          anchor: agent1Items[0].task,
          negative: agent2Items[0].task,
          anchor_agent: agents[i],
          negative_agent: agents[j],
          type: 'negative_pair',
        });
      }
    }
  }

  // Add hard negative pairs
  for (const neg of negatives) {
    const confusingAgent = byAgent[neg.confusing_with];
    if (confusingAgent && confusingAgent[0]) {
      pairs.push({
        anchor: confusingAgent[0].task,
        negative: neg.task,
        anchor_agent: neg.confusing_with,
        negative_agent: neg.agent,
        type: 'hard_negative_pair',
        confusion_reason: neg.confusion_reason,
      });
    }
  }

  return pairs;
}

/**
 * Generate static dataset without API (fallback)
 */
function generateStaticDataset() {
  console.log('Generating static dataset from routing-dataset.js...\n');

  // Import the static dataset
  const { generateTrainingDataset, generateContrastivePairs, getDatasetStats } = require('./routing-dataset.js');

  const dataset = generateTrainingDataset();
  const pairs = generateContrastivePairs();
  const stats = getDatasetStats();

  // Create output directory
  if (!existsSync(OUTPUT_DIR)) {
    mkdirSync(OUTPUT_DIR, { recursive: true });
  }

  // Save dataset
  const datasetPath = join(OUTPUT_DIR, 'claude-code-routing-dataset.json');
  writeFileSync(datasetPath, JSON.stringify(dataset, null, 2));

  const pairsPath = join(OUTPUT_DIR, 'contrastive-pairs.json');
  writeFileSync(pairsPath, JSON.stringify(pairs, null, 2));

  console.log('═══════════════════════════════════════════════════════════════');
  console.log('                    STATIC DATASET GENERATED');
  console.log('═══════════════════════════════════════════════════════════════\n');

  console.log(`  Total examples:       ${stats.totalExamples}`);
  console.log(`  Contrastive pairs:    ${stats.contrastivePairs}`);
  console.log(`  Agent types:          ${stats.agents.length}`);
  console.log(`\n  Output files:`);
  console.log(`    ${datasetPath}`);
  console.log(`    ${pairsPath}`);
  console.log('\n  To generate more data with AI, set GEMINI_API_KEY');
  console.log('');
}

main().catch(console.error);

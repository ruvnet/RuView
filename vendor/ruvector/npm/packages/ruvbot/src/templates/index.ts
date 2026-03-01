/**
 * RuvBot Template Library
 *
 * Pre-built templates for common long-running agent patterns.
 * Deploy with: npx ruvbot deploy <template-name>
 */

export interface Template {
  id: string;
  name: string;
  description: string;
  category: 'practical' | 'intermediate' | 'advanced' | 'exotic';
  agents: AgentSpec[];
  config: TemplateConfig;
  example: string;
}

export interface AgentSpec {
  type: string;
  name: string;
  role: string;
  model?: string;
  systemPrompt?: string;
}

export interface TemplateConfig {
  topology: 'hierarchical' | 'mesh' | 'star' | 'ring' | 'hive-mind';
  maxAgents: number;
  consensus?: 'raft' | 'byzantine' | 'gossip' | 'crdt';
  memory?: 'local' | 'distributed' | 'hybrid';
  workers?: string[];
}

// =============================================================================
// PRACTICAL TEMPLATES
// =============================================================================

export const CODE_REVIEWER: Template = {
  id: 'code-reviewer',
  name: 'Code Review Bot',
  description: 'Automated code review with security scanning and best practices',
  category: 'practical',
  agents: [
    { type: 'reviewer', name: 'code-reviewer', role: 'Review code for quality and patterns' },
    { type: 'security-auditor', name: 'security-scanner', role: 'Scan for vulnerabilities' },
  ],
  config: {
    topology: 'star',
    maxAgents: 3,
    workers: ['audit', 'testgaps'],
  },
  example: `npx ruvbot deploy code-reviewer --repo ./my-project`,
};

export const DOC_GENERATOR: Template = {
  id: 'doc-generator',
  name: 'Documentation Generator',
  description: 'Auto-generate and maintain project documentation',
  category: 'practical',
  agents: [
    { type: 'researcher', name: 'code-analyzer', role: 'Analyze codebase structure' },
    { type: 'api-docs', name: 'doc-writer', role: 'Generate API documentation' },
  ],
  config: {
    topology: 'star',
    maxAgents: 2,
    workers: ['document', 'map'],
  },
  example: `npx ruvbot deploy doc-generator --output ./docs`,
};

export const TEST_GENERATOR: Template = {
  id: 'test-generator',
  name: 'Test Suite Generator',
  description: 'Generate comprehensive test suites with TDD approach',
  category: 'practical',
  agents: [
    { type: 'tester', name: 'test-writer', role: 'Write unit and integration tests' },
    { type: 'coder', name: 'mock-generator', role: 'Generate mocks and fixtures' },
  ],
  config: {
    topology: 'star',
    maxAgents: 3,
    workers: ['testgaps', 'benchmark'],
  },
  example: `npx ruvbot deploy test-generator --coverage 80`,
};

// =============================================================================
// INTERMEDIATE TEMPLATES
// =============================================================================

export const FEATURE_SWARM: Template = {
  id: 'feature-swarm',
  name: 'Feature Development Swarm',
  description: 'Parallel feature development with coordinated agents',
  category: 'intermediate',
  agents: [
    { type: 'planner', name: 'architect', role: 'Design feature architecture' },
    { type: 'coder', name: 'implementer', role: 'Implement feature code' },
    { type: 'tester', name: 'qa', role: 'Write and run tests' },
    { type: 'reviewer', name: 'reviewer', role: 'Code review and refinement' },
  ],
  config: {
    topology: 'hierarchical',
    maxAgents: 6,
    consensus: 'raft',
    memory: 'hybrid',
    workers: ['optimize', 'testgaps'],
  },
  example: `npx ruvbot deploy feature-swarm --feature "Add user auth"`,
};

export const REFACTOR_SQUAD: Template = {
  id: 'refactor-squad',
  name: 'Refactoring Squad',
  description: 'Coordinated codebase refactoring across multiple files',
  category: 'intermediate',
  agents: [
    { type: 'system-architect', name: 'architect', role: 'Plan refactoring strategy' },
    { type: 'coder', name: 'refactorer-1', role: 'Execute refactoring' },
    { type: 'coder', name: 'refactorer-2', role: 'Execute refactoring' },
    { type: 'tester', name: 'regression', role: 'Ensure no regressions' },
  ],
  config: {
    topology: 'mesh',
    maxAgents: 5,
    consensus: 'raft',
    memory: 'distributed',
    workers: ['map', 'optimize'],
  },
  example: `npx ruvbot deploy refactor-squad --pattern "extract-service"`,
};

export const CI_CD_PIPELINE: Template = {
  id: 'ci-cd-pipeline',
  name: 'CI/CD Pipeline Agent',
  description: 'Automated build, test, and deployment pipeline',
  category: 'intermediate',
  agents: [
    { type: 'cicd-engineer', name: 'pipeline-manager', role: 'Orchestrate CI/CD' },
    { type: 'tester', name: 'test-runner', role: 'Execute test suites' },
    { type: 'security-auditor', name: 'security-gate', role: 'Security validation' },
  ],
  config: {
    topology: 'star',
    maxAgents: 4,
    workers: ['audit', 'benchmark'],
  },
  example: `npx ruvbot deploy ci-cd-pipeline --trigger push`,
};

// =============================================================================
// ADVANCED TEMPLATES
// =============================================================================

export const SELF_LEARNING_BOT: Template = {
  id: 'self-learning-bot',
  name: 'Self-Learning Assistant',
  description: 'AI that improves from interactions using neural patterns',
  category: 'advanced',
  agents: [
    { type: 'safla-neural', name: 'learner', role: 'Learn from interactions' },
    { type: 'memory-coordinator', name: 'memory', role: 'Manage persistent memory' },
    { type: 'coder', name: 'executor', role: 'Execute learned patterns' },
  ],
  config: {
    topology: 'hierarchical',
    maxAgents: 4,
    consensus: 'raft',
    memory: 'hybrid',
    workers: ['ultralearn', 'consolidate', 'predict'],
  },
  example: `npx ruvbot deploy self-learning-bot --domain "code-assistance"`,
};

export const RESEARCH_SWARM: Template = {
  id: 'research-swarm',
  name: 'Research Swarm',
  description: 'Distributed research across multiple sources and domains',
  category: 'advanced',
  agents: [
    { type: 'researcher', name: 'lead-researcher', role: 'Coordinate research' },
    { type: 'researcher', name: 'web-researcher', role: 'Search web sources' },
    { type: 'researcher', name: 'code-researcher', role: 'Analyze codebases' },
    { type: 'analyst', name: 'synthesizer', role: 'Synthesize findings' },
  ],
  config: {
    topology: 'mesh',
    maxAgents: 6,
    consensus: 'gossip',
    memory: 'distributed',
    workers: ['deepdive', 'map'],
  },
  example: `npx ruvbot deploy research-swarm --topic "vector databases"`,
};

export const PERFORMANCE_OPTIMIZER: Template = {
  id: 'performance-optimizer',
  name: 'Performance Optimizer',
  description: 'Continuous performance monitoring and optimization',
  category: 'advanced',
  agents: [
    { type: 'perf-analyzer', name: 'profiler', role: 'Profile performance' },
    { type: 'performance-optimizer', name: 'optimizer', role: 'Implement optimizations' },
    { type: 'tester', name: 'benchmark-runner', role: 'Run benchmarks' },
  ],
  config: {
    topology: 'star',
    maxAgents: 4,
    memory: 'hybrid',
    workers: ['optimize', 'benchmark'],
  },
  example: `npx ruvbot deploy performance-optimizer --target ./src`,
};

// =============================================================================
// EXOTIC TEMPLATES
// =============================================================================

export const BYZANTINE_VALIDATOR: Template = {
  id: 'byzantine-validator',
  name: 'Byzantine Fault-Tolerant Validator',
  description: 'High-stakes validation with Byzantine consensus (tolerates 33% malicious)',
  category: 'exotic',
  agents: [
    { type: 'byzantine-coordinator', name: 'primary', role: 'Lead consensus' },
    { type: 'consensus-coordinator', name: 'validator-1', role: 'Validate decisions' },
    { type: 'consensus-coordinator', name: 'validator-2', role: 'Validate decisions' },
    { type: 'consensus-coordinator', name: 'validator-3', role: 'Validate decisions' },
    { type: 'security-manager', name: 'crypto-verifier', role: 'Cryptographic verification' },
  ],
  config: {
    topology: 'mesh',
    maxAgents: 7,
    consensus: 'byzantine',
    memory: 'distributed',
    workers: ['audit'],
  },
  example: `npx ruvbot deploy byzantine-validator --quorum 4`,
};

export const HIVE_MIND: Template = {
  id: 'hive-mind',
  name: 'Hive-Mind Collective',
  description: 'Emergent collective intelligence with queen-led coordination',
  category: 'exotic',
  agents: [
    { type: 'queen-coordinator', name: 'queen', role: 'Strategic orchestration' },
    { type: 'worker-specialist', name: 'worker-1', role: 'Task execution' },
    { type: 'worker-specialist', name: 'worker-2', role: 'Task execution' },
    { type: 'worker-specialist', name: 'worker-3', role: 'Task execution' },
    { type: 'scout-explorer', name: 'scout-1', role: 'Information reconnaissance' },
    { type: 'scout-explorer', name: 'scout-2', role: 'Information reconnaissance' },
    { type: 'swarm-memory-manager', name: 'memory-sync', role: 'Distributed memory' },
    { type: 'collective-intelligence-coordinator', name: 'hive-brain', role: 'Collective decisions' },
  ],
  config: {
    topology: 'hive-mind',
    maxAgents: 15,
    consensus: 'crdt',
    memory: 'distributed',
    workers: ['ultralearn', 'consolidate', 'predict', 'map'],
  },
  example: `npx ruvbot deploy hive-mind --objective "Build complete app"`,
};

export const MULTI_REPO_COORDINATOR: Template = {
  id: 'multi-repo-coordinator',
  name: 'Multi-Repository Coordinator',
  description: 'Coordinate changes across multiple repositories',
  category: 'exotic',
  agents: [
    { type: 'repo-architect', name: 'coordinator', role: 'Cross-repo orchestration' },
    { type: 'sync-coordinator', name: 'sync-manager', role: 'Version synchronization' },
    { type: 'pr-manager', name: 'pr-coordinator', role: 'PR management' },
    { type: 'release-manager', name: 'release', role: 'Release coordination' },
  ],
  config: {
    topology: 'hierarchical',
    maxAgents: 8,
    consensus: 'raft',
    memory: 'distributed',
    workers: ['audit', 'document'],
  },
  example: `npx ruvbot deploy multi-repo-coordinator --repos "repo1,repo2,repo3"`,
};

export const ADVERSARIAL_TESTER: Template = {
  id: 'adversarial-tester',
  name: 'Adversarial Security Tester',
  description: 'Red team vs blue team security testing with adversarial agents',
  category: 'exotic',
  agents: [
    { type: 'security-architect', name: 'red-team-lead', role: 'Attack planning' },
    { type: 'security-auditor', name: 'attacker-1', role: 'Execute attacks' },
    { type: 'security-auditor', name: 'attacker-2', role: 'Execute attacks' },
    { type: 'security-manager', name: 'blue-team-lead', role: 'Defense coordination' },
    { type: 'security-auditor', name: 'defender', role: 'Implement defenses' },
  ],
  config: {
    topology: 'mesh',
    maxAgents: 6,
    consensus: 'byzantine',
    memory: 'distributed',
    workers: ['audit', 'deepdive'],
  },
  example: `npx ruvbot deploy adversarial-tester --target ./api`,
};

// =============================================================================
// TEMPLATE REGISTRY
// =============================================================================

export const TEMPLATES: Record<string, Template> = {
  // Practical
  'code-reviewer': CODE_REVIEWER,
  'doc-generator': DOC_GENERATOR,
  'test-generator': TEST_GENERATOR,

  // Intermediate
  'feature-swarm': FEATURE_SWARM,
  'refactor-squad': REFACTOR_SQUAD,
  'ci-cd-pipeline': CI_CD_PIPELINE,

  // Advanced
  'self-learning-bot': SELF_LEARNING_BOT,
  'research-swarm': RESEARCH_SWARM,
  'performance-optimizer': PERFORMANCE_OPTIMIZER,

  // Exotic
  'byzantine-validator': BYZANTINE_VALIDATOR,
  'hive-mind': HIVE_MIND,
  'multi-repo-coordinator': MULTI_REPO_COORDINATOR,
  'adversarial-tester': ADVERSARIAL_TESTER,
};

export const TEMPLATE_LIST = Object.values(TEMPLATES);

export function getTemplate(id: string): Template | undefined {
  return TEMPLATES[id];
}

export function listTemplates(category?: Template['category']): Template[] {
  if (category) {
    return TEMPLATE_LIST.filter(t => t.category === category);
  }
  return TEMPLATE_LIST;
}

export function getTemplatesByCategory(): Record<string, Template[]> {
  return {
    practical: listTemplates('practical'),
    intermediate: listTemplates('intermediate'),
    advanced: listTemplates('advanced'),
    exotic: listTemplates('exotic'),
  };
}

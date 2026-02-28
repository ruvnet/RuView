/**
 * Routing Benchmark for RuvLTRA Models
 *
 * Tests whether the model correctly routes tasks to appropriate agents.
 * This measures the actual value proposition for Claude Code workflows.
 */

export interface RoutingTestCase {
  id: string;
  task: string;
  expectedAgent: string;
  category: string;
  difficulty: 'easy' | 'medium' | 'hard';
}

export interface RoutingResult {
  testId: string;
  task: string;
  expectedAgent: string;
  predictedAgent: string;
  confidence: number;
  correct: boolean;
  latencyMs: number;
}

export interface RoutingBenchmarkResults {
  accuracy: number;
  accuracyByCategory: Record<string, number>;
  accuracyByDifficulty: Record<string, number>;
  avgLatencyMs: number;
  p50LatencyMs: number;
  p95LatencyMs: number;
  totalTests: number;
  correct: number;
  results: RoutingResult[];
}

/**
 * Agent types in Claude Code / claude-flow ecosystem
 */
export const AGENT_TYPES = [
  'coder',
  'researcher',
  'reviewer',
  'tester',
  'architect',
  'security-architect',
  'debugger',
  'documenter',
  'refactorer',
  'optimizer',
  'devops',
  'api-docs',
  'planner',
] as const;

export type AgentType = (typeof AGENT_TYPES)[number];

/**
 * Ground truth test dataset for routing
 * 100 tasks with expected agent assignments
 */
export const ROUTING_TEST_CASES: RoutingTestCase[] = [
  // === CODER tasks (write new code) ===
  { id: 'C001', task: 'Implement a binary search function in TypeScript', expectedAgent: 'coder', category: 'implementation', difficulty: 'easy' },
  { id: 'C002', task: 'Write a React component for user authentication', expectedAgent: 'coder', category: 'implementation', difficulty: 'medium' },
  { id: 'C003', task: 'Create a REST API endpoint for user registration', expectedAgent: 'coder', category: 'implementation', difficulty: 'medium' },
  { id: 'C004', task: 'Implement a WebSocket server for real-time chat', expectedAgent: 'coder', category: 'implementation', difficulty: 'hard' },
  { id: 'C005', task: 'Write a function to parse CSV files', expectedAgent: 'coder', category: 'implementation', difficulty: 'easy' },
  { id: 'C006', task: 'Create a middleware for request logging', expectedAgent: 'coder', category: 'implementation', difficulty: 'easy' },
  { id: 'C007', task: 'Implement pagination for the API responses', expectedAgent: 'coder', category: 'implementation', difficulty: 'medium' },
  { id: 'C008', task: 'Write a custom React hook for form validation', expectedAgent: 'coder', category: 'implementation', difficulty: 'medium' },
  { id: 'C009', task: 'Create a database migration script', expectedAgent: 'coder', category: 'implementation', difficulty: 'medium' },
  { id: 'C010', task: 'Implement a rate limiter for the API', expectedAgent: 'coder', category: 'implementation', difficulty: 'medium' },

  // === RESEARCHER tasks (investigate, explore) ===
  { id: 'R001', task: 'Research best practices for GraphQL schema design', expectedAgent: 'researcher', category: 'research', difficulty: 'medium' },
  { id: 'R002', task: 'Find out how the authentication flow works in this codebase', expectedAgent: 'researcher', category: 'research', difficulty: 'easy' },
  { id: 'R003', task: 'Investigate why the build is failing on CI', expectedAgent: 'researcher', category: 'research', difficulty: 'medium' },
  { id: 'R004', task: 'Research alternatives to Redux for state management', expectedAgent: 'researcher', category: 'research', difficulty: 'medium' },
  { id: 'R005', task: 'Find all usages of the deprecated API in the codebase', expectedAgent: 'researcher', category: 'research', difficulty: 'easy' },
  { id: 'R006', task: 'Analyze the performance characteristics of our database queries', expectedAgent: 'researcher', category: 'research', difficulty: 'hard' },
  { id: 'R007', task: 'Research GDPR compliance requirements for user data', expectedAgent: 'researcher', category: 'research', difficulty: 'medium' },
  { id: 'R008', task: 'Find examples of similar implementations in open source', expectedAgent: 'researcher', category: 'research', difficulty: 'easy' },

  // === REVIEWER tasks (code review, quality) ===
  { id: 'V001', task: 'Review this pull request for code quality', expectedAgent: 'reviewer', category: 'review', difficulty: 'medium' },
  { id: 'V002', task: 'Check if this code follows our style guidelines', expectedAgent: 'reviewer', category: 'review', difficulty: 'easy' },
  { id: 'V003', task: 'Review the API design for consistency', expectedAgent: 'reviewer', category: 'review', difficulty: 'medium' },
  { id: 'V004', task: 'Evaluate the error handling in this module', expectedAgent: 'reviewer', category: 'review', difficulty: 'medium' },
  { id: 'V005', task: 'Review the database schema changes', expectedAgent: 'reviewer', category: 'review', difficulty: 'hard' },
  { id: 'V006', task: 'Check for potential memory leaks in this code', expectedAgent: 'reviewer', category: 'review', difficulty: 'hard' },
  { id: 'V007', task: 'Review the accessibility of the UI components', expectedAgent: 'reviewer', category: 'review', difficulty: 'medium' },

  // === TESTER tasks (write tests, QA) ===
  { id: 'T001', task: 'Write unit tests for the user service', expectedAgent: 'tester', category: 'testing', difficulty: 'medium' },
  { id: 'T002', task: 'Create integration tests for the checkout flow', expectedAgent: 'tester', category: 'testing', difficulty: 'hard' },
  { id: 'T003', task: 'Add test coverage for edge cases in the parser', expectedAgent: 'tester', category: 'testing', difficulty: 'medium' },
  { id: 'T004', task: 'Write E2E tests for the login page', expectedAgent: 'tester', category: 'testing', difficulty: 'medium' },
  { id: 'T005', task: 'Create performance tests for the API', expectedAgent: 'tester', category: 'testing', difficulty: 'hard' },
  { id: 'T006', task: 'Add snapshot tests for React components', expectedAgent: 'tester', category: 'testing', difficulty: 'easy' },
  { id: 'T007', task: 'Write tests for the authentication middleware', expectedAgent: 'tester', category: 'testing', difficulty: 'medium' },
  { id: 'T008', task: 'Create mock data for testing', expectedAgent: 'tester', category: 'testing', difficulty: 'easy' },

  // === ARCHITECT tasks (design, system) ===
  { id: 'A001', task: 'Design the microservices architecture for the platform', expectedAgent: 'architect', category: 'architecture', difficulty: 'hard' },
  { id: 'A002', task: 'Create a system design for the notification service', expectedAgent: 'architect', category: 'architecture', difficulty: 'hard' },
  { id: 'A003', task: 'Plan the database schema for the new feature', expectedAgent: 'architect', category: 'architecture', difficulty: 'medium' },
  { id: 'A004', task: 'Design the API contract for the mobile app', expectedAgent: 'architect', category: 'architecture', difficulty: 'medium' },
  { id: 'A005', task: 'Create an ADR for the caching strategy', expectedAgent: 'architect', category: 'architecture', difficulty: 'medium' },
  { id: 'A006', task: 'Design the event-driven architecture for order processing', expectedAgent: 'architect', category: 'architecture', difficulty: 'hard' },
  { id: 'A007', task: 'Plan the migration strategy from monolith to microservices', expectedAgent: 'architect', category: 'architecture', difficulty: 'hard' },

  // === SECURITY tasks ===
  { id: 'S001', task: 'Audit the authentication implementation for vulnerabilities', expectedAgent: 'security-architect', category: 'security', difficulty: 'hard' },
  { id: 'S002', task: 'Review the code for SQL injection vulnerabilities', expectedAgent: 'security-architect', category: 'security', difficulty: 'medium' },
  { id: 'S003', task: 'Check for XSS vulnerabilities in the frontend', expectedAgent: 'security-architect', category: 'security', difficulty: 'medium' },
  { id: 'S004', task: 'Implement secure password hashing', expectedAgent: 'security-architect', category: 'security', difficulty: 'medium' },
  { id: 'S005', task: 'Review the API for authorization bypass issues', expectedAgent: 'security-architect', category: 'security', difficulty: 'hard' },
  { id: 'S006', task: 'Audit third-party dependencies for known CVEs', expectedAgent: 'security-architect', category: 'security', difficulty: 'medium' },
  { id: 'S007', task: 'Design the secrets management strategy', expectedAgent: 'security-architect', category: 'security', difficulty: 'hard' },

  // === DEBUGGER tasks ===
  { id: 'D001', task: 'Fix the null pointer exception in the user controller', expectedAgent: 'debugger', category: 'debugging', difficulty: 'easy' },
  { id: 'D002', task: 'Debug why the API returns 500 intermittently', expectedAgent: 'debugger', category: 'debugging', difficulty: 'hard' },
  { id: 'D003', task: 'Find the cause of the memory leak', expectedAgent: 'debugger', category: 'debugging', difficulty: 'hard' },
  { id: 'D004', task: 'Fix the race condition in the checkout process', expectedAgent: 'debugger', category: 'debugging', difficulty: 'hard' },
  { id: 'D005', task: 'Debug the failing test in CI', expectedAgent: 'debugger', category: 'debugging', difficulty: 'medium' },
  { id: 'D006', task: 'Fix the timezone issue in date handling', expectedAgent: 'debugger', category: 'debugging', difficulty: 'medium' },
  { id: 'D007', task: 'Resolve the circular dependency error', expectedAgent: 'debugger', category: 'debugging', difficulty: 'medium' },
  { id: 'D008', task: 'Fix the broken build after the merge', expectedAgent: 'debugger', category: 'debugging', difficulty: 'easy' },

  // === DOCUMENTER tasks ===
  { id: 'O001', task: 'Write documentation for the API endpoints', expectedAgent: 'documenter', category: 'documentation', difficulty: 'medium' },
  { id: 'O002', task: 'Create a README for the new package', expectedAgent: 'documenter', category: 'documentation', difficulty: 'easy' },
  { id: 'O003', task: 'Document the deployment process', expectedAgent: 'documenter', category: 'documentation', difficulty: 'medium' },
  { id: 'O004', task: 'Write JSDoc comments for the utility functions', expectedAgent: 'documenter', category: 'documentation', difficulty: 'easy' },
  { id: 'O005', task: 'Create a migration guide for v2 to v3', expectedAgent: 'documenter', category: 'documentation', difficulty: 'medium' },
  { id: 'O006', task: 'Document the architecture decisions', expectedAgent: 'documenter', category: 'documentation', difficulty: 'medium' },

  // === REFACTORER tasks ===
  { id: 'F001', task: 'Refactor the user service to use dependency injection', expectedAgent: 'refactorer', category: 'refactoring', difficulty: 'medium' },
  { id: 'F002', task: 'Extract common logic into a shared utility', expectedAgent: 'refactorer', category: 'refactoring', difficulty: 'easy' },
  { id: 'F003', task: 'Split the large component into smaller ones', expectedAgent: 'refactorer', category: 'refactoring', difficulty: 'medium' },
  { id: 'F004', task: 'Rename the ambiguous variable names in this module', expectedAgent: 'refactorer', category: 'refactoring', difficulty: 'easy' },
  { id: 'F005', task: 'Convert the callbacks to async/await', expectedAgent: 'refactorer', category: 'refactoring', difficulty: 'medium' },
  { id: 'F006', task: 'Remove dead code from the legacy module', expectedAgent: 'refactorer', category: 'refactoring', difficulty: 'easy' },
  { id: 'F007', task: 'Consolidate duplicate API handlers', expectedAgent: 'refactorer', category: 'refactoring', difficulty: 'medium' },

  // === OPTIMIZER tasks ===
  { id: 'P001', task: 'Optimize the slow database query', expectedAgent: 'optimizer', category: 'performance', difficulty: 'hard' },
  { id: 'P002', task: 'Reduce the bundle size of the frontend', expectedAgent: 'optimizer', category: 'performance', difficulty: 'medium' },
  { id: 'P003', task: 'Improve the API response time', expectedAgent: 'optimizer', category: 'performance', difficulty: 'hard' },
  { id: 'P004', task: 'Add caching to reduce database load', expectedAgent: 'optimizer', category: 'performance', difficulty: 'medium' },
  { id: 'P005', task: 'Optimize the image loading performance', expectedAgent: 'optimizer', category: 'performance', difficulty: 'medium' },
  { id: 'P006', task: 'Profile and optimize memory usage', expectedAgent: 'optimizer', category: 'performance', difficulty: 'hard' },
  { id: 'P007', task: 'Implement lazy loading for the dashboard', expectedAgent: 'optimizer', category: 'performance', difficulty: 'medium' },

  // === DEVOPS tasks ===
  { id: 'E001', task: 'Set up the CI/CD pipeline for the new service', expectedAgent: 'devops', category: 'devops', difficulty: 'medium' },
  { id: 'E002', task: 'Configure Kubernetes deployment for production', expectedAgent: 'devops', category: 'devops', difficulty: 'hard' },
  { id: 'E003', task: 'Set up monitoring and alerting', expectedAgent: 'devops', category: 'devops', difficulty: 'medium' },
  { id: 'E004', task: 'Create Docker containers for the microservices', expectedAgent: 'devops', category: 'devops', difficulty: 'medium' },
  { id: 'E005', task: 'Configure auto-scaling for the API servers', expectedAgent: 'devops', category: 'devops', difficulty: 'hard' },
  { id: 'E006', task: 'Set up the staging environment', expectedAgent: 'devops', category: 'devops', difficulty: 'medium' },
  { id: 'E007', task: 'Implement blue-green deployment strategy', expectedAgent: 'devops', category: 'devops', difficulty: 'hard' },

  // === API-DOCS tasks ===
  { id: 'I001', task: 'Generate OpenAPI spec for the REST API', expectedAgent: 'api-docs', category: 'api-documentation', difficulty: 'medium' },
  { id: 'I002', task: 'Create Swagger documentation for the endpoints', expectedAgent: 'api-docs', category: 'api-documentation', difficulty: 'medium' },
  { id: 'I003', task: 'Document the GraphQL schema', expectedAgent: 'api-docs', category: 'api-documentation', difficulty: 'medium' },
  { id: 'I004', task: 'Add example requests and responses to API docs', expectedAgent: 'api-docs', category: 'api-documentation', difficulty: 'easy' },

  // === PLANNER tasks ===
  { id: 'L001', task: 'Break down the feature into implementation tasks', expectedAgent: 'planner', category: 'planning', difficulty: 'medium' },
  { id: 'L002', task: 'Create a sprint plan for the next milestone', expectedAgent: 'planner', category: 'planning', difficulty: 'medium' },
  { id: 'L003', task: 'Estimate effort for the refactoring project', expectedAgent: 'planner', category: 'planning', difficulty: 'medium' },
  { id: 'L004', task: 'Prioritize the bug fixes for the release', expectedAgent: 'planner', category: 'planning', difficulty: 'easy' },
  { id: 'L005', task: 'Plan the technical debt reduction roadmap', expectedAgent: 'planner', category: 'planning', difficulty: 'hard' },

  // === AMBIGUOUS / EDGE CASES ===
  { id: 'X001', task: 'The login is broken, users cannot sign in', expectedAgent: 'debugger', category: 'ambiguous', difficulty: 'medium' },
  { id: 'X002', task: 'We need better error messages', expectedAgent: 'coder', category: 'ambiguous', difficulty: 'easy' },
  { id: 'X003', task: 'Make the app faster', expectedAgent: 'optimizer', category: 'ambiguous', difficulty: 'hard' },
  { id: 'X004', task: 'The code is a mess, clean it up', expectedAgent: 'refactorer', category: 'ambiguous', difficulty: 'medium' },
  { id: 'X005', task: 'Is this implementation secure?', expectedAgent: 'security-architect', category: 'ambiguous', difficulty: 'medium' },
];

/**
 * Simple keyword-based routing for baseline comparison
 */
export function baselineKeywordRouter(task: string): { agent: AgentType; confidence: number } {
  const taskLower = task.toLowerCase();

  const patterns: { keywords: string[]; agent: AgentType; weight: number }[] = [
    { keywords: ['implement', 'create', 'write', 'add', 'build'], agent: 'coder', weight: 1 },
    { keywords: ['research', 'find', 'investigate', 'analyze', 'explore'], agent: 'researcher', weight: 1 },
    { keywords: ['review', 'check', 'evaluate', 'assess'], agent: 'reviewer', weight: 1 },
    { keywords: ['test', 'unit test', 'integration test', 'e2e', 'coverage'], agent: 'tester', weight: 1.2 },
    { keywords: ['design', 'architect', 'schema', 'adr', 'system design'], agent: 'architect', weight: 1.2 },
    { keywords: ['security', 'vulnerability', 'xss', 'sql injection', 'audit', 'cve'], agent: 'security-architect', weight: 1.5 },
    { keywords: ['debug', 'fix', 'bug', 'error', 'broken', 'issue'], agent: 'debugger', weight: 1.2 },
    { keywords: ['document', 'readme', 'jsdoc', 'comment'], agent: 'documenter', weight: 1 },
    { keywords: ['refactor', 'extract', 'rename', 'consolidate', 'split'], agent: 'refactorer', weight: 1.2 },
    { keywords: ['optimize', 'performance', 'slow', 'cache', 'faster'], agent: 'optimizer', weight: 1.2 },
    { keywords: ['deploy', 'ci/cd', 'kubernetes', 'docker', 'pipeline'], agent: 'devops', weight: 1.2 },
    { keywords: ['openapi', 'swagger', 'api doc', 'graphql schema'], agent: 'api-docs', weight: 1.3 },
    { keywords: ['plan', 'estimate', 'prioritize', 'sprint', 'roadmap'], agent: 'planner', weight: 1 },
  ];

  let bestMatch: { agent: AgentType; score: number } = { agent: 'coder', score: 0 };

  for (const pattern of patterns) {
    let score = 0;
    for (const keyword of pattern.keywords) {
      if (taskLower.includes(keyword)) {
        score += pattern.weight;
      }
    }
    if (score > bestMatch.score) {
      bestMatch = { agent: pattern.agent, score };
    }
  }

  return {
    agent: bestMatch.agent,
    confidence: Math.min(bestMatch.score / 3, 1), // Normalize to 0-1
  };
}

/**
 * Run the routing benchmark
 */
export function runRoutingBenchmark(
  router: (task: string) => { agent: string; confidence: number }
): RoutingBenchmarkResults {
  const results: RoutingResult[] = [];
  const latencies: number[] = [];

  for (const testCase of ROUTING_TEST_CASES) {
    const start = performance.now();
    const prediction = router(testCase.task);
    const latencyMs = performance.now() - start;

    latencies.push(latencyMs);

    results.push({
      testId: testCase.id,
      task: testCase.task,
      expectedAgent: testCase.expectedAgent,
      predictedAgent: prediction.agent,
      confidence: prediction.confidence,
      correct: prediction.agent === testCase.expectedAgent,
      latencyMs,
    });
  }

  // Calculate metrics
  const correct = results.filter(r => r.correct).length;
  const accuracy = correct / results.length;

  // Accuracy by category
  const categories = [...new Set(ROUTING_TEST_CASES.map(t => t.category))];
  const accuracyByCategory: Record<string, number> = {};
  for (const cat of categories) {
    const catResults = results.filter((r, i) => ROUTING_TEST_CASES[i].category === cat);
    accuracyByCategory[cat] = catResults.filter(r => r.correct).length / catResults.length;
  }

  // Accuracy by difficulty
  const difficulties = ['easy', 'medium', 'hard'];
  const accuracyByDifficulty: Record<string, number> = {};
  for (const diff of difficulties) {
    const diffResults = results.filter((r, i) => ROUTING_TEST_CASES[i].difficulty === diff);
    accuracyByDifficulty[diff] = diffResults.filter(r => r.correct).length / diffResults.length;
  }

  // Latency percentiles
  const sortedLatencies = [...latencies].sort((a, b) => a - b);
  const p50 = sortedLatencies[Math.floor(sortedLatencies.length * 0.5)];
  const p95 = sortedLatencies[Math.floor(sortedLatencies.length * 0.95)];
  const avgLatency = latencies.reduce((a, b) => a + b, 0) / latencies.length;

  return {
    accuracy,
    accuracyByCategory,
    accuracyByDifficulty,
    avgLatencyMs: avgLatency,
    p50LatencyMs: p50,
    p95LatencyMs: p95,
    totalTests: results.length,
    correct,
    results,
  };
}

/**
 * Format benchmark results for display
 */
export function formatRoutingResults(results: RoutingBenchmarkResults): string {
  const lines: string[] = [];

  lines.push('');
  lines.push('╔══════════════════════════════════════════════════════════════╗');
  lines.push('║              ROUTING BENCHMARK RESULTS                       ║');
  lines.push('╠══════════════════════════════════════════════════════════════╣');
  lines.push(`║  Overall Accuracy: ${(results.accuracy * 100).toFixed(1)}% (${results.correct}/${results.totalTests})`.padEnd(63) + '║');
  lines.push('╠══════════════════════════════════════════════════════════════╣');
  lines.push('║  By Category:                                                ║');

  for (const [cat, acc] of Object.entries(results.accuracyByCategory).sort((a, b) => b[1] - a[1])) {
    const bar = '█'.repeat(Math.floor(acc * 20)) + '░'.repeat(20 - Math.floor(acc * 20));
    lines.push(`║    ${cat.padEnd(18)} [${bar}] ${(acc * 100).toFixed(0).padStart(3)}%  ║`);
  }

  lines.push('╠══════════════════════════════════════════════════════════════╣');
  lines.push('║  By Difficulty:                                              ║');

  for (const [diff, acc] of Object.entries(results.accuracyByDifficulty)) {
    const bar = '█'.repeat(Math.floor(acc * 20)) + '░'.repeat(20 - Math.floor(acc * 20));
    lines.push(`║    ${diff.padEnd(18)} [${bar}] ${(acc * 100).toFixed(0).padStart(3)}%  ║`);
  }

  lines.push('╠══════════════════════════════════════════════════════════════╣');
  lines.push('║  Latency:                                                    ║');
  lines.push(`║    Average: ${results.avgLatencyMs.toFixed(2)}ms`.padEnd(63) + '║');
  lines.push(`║    P50:     ${results.p50LatencyMs.toFixed(2)}ms`.padEnd(63) + '║');
  lines.push(`║    P95:     ${results.p95LatencyMs.toFixed(2)}ms`.padEnd(63) + '║');
  lines.push('╚══════════════════════════════════════════════════════════════╝');

  // Show failures
  const failures = results.results.filter(r => !r.correct);
  if (failures.length > 0 && failures.length <= 20) {
    lines.push('');
    lines.push('Misrouted tasks:');
    for (const f of failures.slice(0, 10)) {
      lines.push(`  [${f.testId}] "${f.task.slice(0, 50)}..."`);
      lines.push(`         Expected: ${f.expectedAgent}, Got: ${f.predictedAgent}`);
    }
    if (failures.length > 10) {
      lines.push(`  ... and ${failures.length - 10} more`);
    }
  }

  return lines.join('\n');
}

export default {
  ROUTING_TEST_CASES,
  AGENT_TYPES,
  baselineKeywordRouter,
  runRoutingBenchmark,
  formatRoutingResults,
};

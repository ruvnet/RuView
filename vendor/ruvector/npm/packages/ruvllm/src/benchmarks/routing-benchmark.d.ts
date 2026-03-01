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
export declare const AGENT_TYPES: readonly ["coder", "researcher", "reviewer", "tester", "architect", "security-architect", "debugger", "documenter", "refactorer", "optimizer", "devops", "api-docs", "planner"];
export type AgentType = (typeof AGENT_TYPES)[number];
/**
 * Ground truth test dataset for routing
 * 100 tasks with expected agent assignments
 */
export declare const ROUTING_TEST_CASES: RoutingTestCase[];
/**
 * Simple keyword-based routing for baseline comparison
 */
export declare function baselineKeywordRouter(task: string): {
    agent: AgentType;
    confidence: number;
};
/**
 * Run the routing benchmark
 */
export declare function runRoutingBenchmark(router: (task: string) => {
    agent: string;
    confidence: number;
}): RoutingBenchmarkResults;
/**
 * Format benchmark results for display
 */
export declare function formatRoutingResults(results: RoutingBenchmarkResults): string;
declare const _default: {
    ROUTING_TEST_CASES: RoutingTestCase[];
    AGENT_TYPES: readonly ["coder", "researcher", "reviewer", "tester", "architect", "security-architect", "debugger", "documenter", "refactorer", "optimizer", "devops", "api-docs", "planner"];
    baselineKeywordRouter: typeof baselineKeywordRouter;
    runRoutingBenchmark: typeof runRoutingBenchmark;
    formatRoutingResults: typeof formatRoutingResults;
};
export default _default;
//# sourceMappingURL=routing-benchmark.d.ts.map
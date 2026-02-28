/**
 * Agent Lifecycle Management Examples
 *
 * Demonstrates agent lifecycle patterns including spawning/termination,
 * state synchronization, health checks, recovery patterns, and
 * version migration for dynamic agent systems.
 *
 * Integrates with:
 * - claude-flow: Agent lifecycle hooks and state management
 * - ruv-swarm: Dynamic agent spawning and coordination
 * - Kubernetes: Container orchestration patterns
 */
/**
 * Generate agent spawning and termination lifecycle events
 */
export declare function agentSpawningTermination(): Promise<{
    lifecycleEvents: import("../../dist/index.js").GenerationResult<unknown>;
    spawnStrategies: import("../../dist/index.js").GenerationResult<unknown>;
    resourcePool: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate state synchronization data for distributed agents
 */
export declare function stateSynchronization(): Promise<{
    stateSnapshots: import("../../dist/index.js").GenerationResult<unknown>;
    syncEvents: import("../../dist/index.js").GenerationResult<unknown>;
    consistencyChecks: import("../../dist/index.js").GenerationResult<unknown>;
    syncTopology: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate health check and monitoring data
 */
export declare function healthCheckScenarios(): Promise<{
    healthChecks: import("../../dist/index.js").GenerationResult<unknown>;
    healthConfigs: import("../../dist/index.js").GenerationResult<unknown>;
    healthTimeSeries: import("../../dist/index.js").GenerationResult<unknown>;
    healingActions: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate failure recovery pattern data
 */
export declare function recoveryPatterns(): Promise<{
    failures: import("../../dist/index.js").GenerationResult<unknown>;
    recoveryStrategies: import("../../dist/index.js").GenerationResult<unknown>;
    circuitBreakers: import("../../dist/index.js").GenerationResult<unknown>;
    backupOperations: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate agent version migration data
 */
export declare function versionMigration(): Promise<{
    versions: import("../../dist/index.js").GenerationResult<unknown>;
    migrations: import("../../dist/index.js").GenerationResult<unknown>;
    canaryDeployments: import("../../dist/index.js").GenerationResult<unknown>;
    rollbacks: import("../../dist/index.js").GenerationResult<unknown>;
}>;
export declare function runAllLifecycleExamples(): Promise<void>;
//# sourceMappingURL=agent-lifecycle.d.ts.map
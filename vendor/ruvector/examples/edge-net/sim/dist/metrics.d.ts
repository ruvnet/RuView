/**
 * Metrics Collection and Aggregation
 * Tracks network performance across all phases
 */
import { Network, NetworkPhase } from './network.js';
export interface PhaseMetrics {
    phase: NetworkPhase;
    startTick: number;
    endTick: number;
    duration: number;
    nodeCount: {
        start: number;
        end: number;
        peak: number;
    };
    energy: {
        totalEarned: number;
        totalSpent: number;
        netEnergy: number;
        avgPerNode: number;
        sustainability: number;
    };
    genesis: {
        avgMultiplier: number;
        activeCount: number;
        readOnlyCount: number;
        retiredCount: number;
    };
    network: {
        avgConnections: number;
        avgSuccessRate: number;
        taskThroughput: number;
        tasksCompleted: number;
    };
    validation: {
        passed: boolean;
        reasons: string[];
    };
}
export declare class MetricsCollector {
    private network;
    private phaseMetrics;
    private currentPhaseStart;
    private currentPhaseNodeCount;
    private peakNodeCount;
    constructor(network: Network);
    /**
     * Initialize metrics collection
     */
    initialize(): void;
    /**
     * Collect metrics for the current tick
     */
    collect(): void;
    /**
     * Handle phase transition
     */
    onPhaseTransition(oldPhase: NetworkPhase, newPhase: NetworkPhase): void;
    /**
     * Finalize metrics for a completed phase
     */
    private finalizePhase;
    /**
     * Validate phase completion criteria
     */
    private validatePhase;
    /**
     * Finalize current phase (for end of simulation)
     */
    finalizeCurrent(): void;
    /**
     * Get all collected metrics
     */
    getAllMetrics(): PhaseMetrics[];
    /**
     * Get metrics for a specific phase
     */
    getPhaseMetrics(phase: NetworkPhase): PhaseMetrics | undefined;
    /**
     * Get overall success rate
     */
    getOverallSuccess(): {
        passed: boolean;
        totalPassed: number;
        totalPhases: number;
    };
}
//# sourceMappingURL=metrics.d.ts.map
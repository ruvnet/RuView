/**
 * Report Generation
 * Generates comprehensive JSON reports of simulation results
 */
import { Network } from './network.js';
import { MetricsCollector, PhaseMetrics } from './metrics.js';
export interface SimulationReport {
    metadata: {
        timestamp: string;
        simulationVersion: string;
        duration: number;
        totalTicks: number;
    };
    configuration: {
        genesisNodeCount: number;
        targetNodeCount: number;
        nodesPerTick: number;
        taskGenerationRate: number;
        baseTaskReward: number;
    };
    summary: {
        phasesCompleted: number;
        totalPassed: boolean;
        phasesPassed: number;
        phasesTotal: number;
        finalNodeCount: number;
        finalPhase: string;
    };
    phases: {
        [key: string]: PhaseMetrics;
    };
    finalState: {
        nodeCount: number;
        genesisNodes: any;
        economy: any;
        network: any;
        topPerformers: any[];
    };
    validation: {
        overallPassed: boolean;
        criticalIssues: string[];
        warnings: string[];
        successes: string[];
    };
}
export declare class ReportGenerator {
    private network;
    private metrics;
    private startTime;
    constructor(network: Network, metrics: MetricsCollector);
    /**
     * Generate comprehensive simulation report
     */
    generateReport(): SimulationReport;
    /**
     * Get top performing nodes
     */
    private getTopPerformers;
    /**
     * Collect all validation issues
     */
    private collectValidation;
    /**
     * Save report to file
     */
    saveReport(filepath: string): void;
    /**
     * Print summary to console
     */
    printSummary(): void;
}
//# sourceMappingURL=report.d.ts.map
/**
 * ReasonGraph - Production-Ready Knowledge Discovery Platform
 * Main entry point for the complete system integration
 */
import { AdvancedReasoningEngine } from './advanced-reasoning-engine.js';
import { ReasonGraphResearchInterface } from './research-interface.js';
import { ReasonGraphPerformanceOptimizer } from './performance-optimizer.js';
export interface ReasonGraphConfig {
    port: number;
    enableOptimization: boolean;
    enableRealTimeMonitoring: boolean;
    cacheSize: number;
    performanceTargets: {
        queryResponseMs: number;
        throughputQps: number;
        breakthroughRate: number;
    };
}
export declare class ReasonGraphPlatform {
    private reasoningEngine;
    private researchInterface;
    private performanceOptimizer;
    private mcpServer;
    private config;
    constructor(config?: Partial<ReasonGraphConfig>);
    private initializeComponents;
    /**
     * Start the complete ReasonGraph platform
     */
    start(): Promise<void>;
    /**
     * Stop the platform gracefully
     */
    stop(): Promise<void>;
    /**
     * Get comprehensive platform status
     */
    getStatus(): Promise<{
        status: string;
        uptime: number;
        performance: any;
        cache: any;
        capabilities: string[];
    }>;
    /**
     * Perform a comprehensive research query
     */
    research(question: string, domain?: string, options?: {
        enableCreativity?: boolean;
        enableTemporalAdvantage?: boolean;
        enableConsciousnessVerification?: boolean;
        depth?: number;
    }): Promise<any>;
    /**
     * Display platform capabilities
     */
    private displayCapabilities;
    /**
     * Run comprehensive system tests
     */
    runSystemTests(): Promise<{
        passed: number;
        failed: number;
        results: any[];
    }>;
}
export { AdvancedReasoningEngine, ReasonGraphResearchInterface, ReasonGraphPerformanceOptimizer };
export default ReasonGraphPlatform;

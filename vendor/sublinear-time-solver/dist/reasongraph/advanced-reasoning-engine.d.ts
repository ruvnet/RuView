/**
 * Advanced Reasoning Engine for ReasonGraph
 * Combines psycho-symbolic reasoning with consciousness-guided discovery
 * Maintains O(n log n) sublinear performance for scalable research
 */
export interface ReasoningQuery {
    question: string;
    domain: string;
    depth: number;
    creativityLevel: number;
    temporalAdvantage: boolean;
    consciousnessVerification: boolean;
}
export interface ReasoningResult {
    answer: string;
    confidence: number;
    reasoning_path: any[];
    breakthrough_potential: number;
    temporal_advantage_ms: number;
    consciousness_verified: boolean;
    novel_insights: string[];
    contradictions_detected: any[];
    performance_metrics: {
        query_time_ms: number;
        complexity_order: string;
        memory_usage_mb: number;
    };
}
export declare class AdvancedReasoningEngine {
    private psychoSymbolic;
    private consciousness;
    private temporal;
    private solver;
    private knowledgeGraph;
    constructor();
    /**
     * Enhanced multi-step reasoning with consciousness verification
     */
    performAdvancedReasoning(query: ReasoningQuery): Promise<ReasoningResult>;
    /**
     * Generate creative insights using consciousness-inspired patterns
     */
    private generateCreativeInsights;
    /**
     * Find analogies across different domains using knowledge graph
     */
    private findCrossDomainAnalogies;
    /**
     * Calculate breakthrough potential based on consciousness and creativity
     */
    private calculateBreakthroughPotential;
    /**
     * Synthesize comprehensive answer from multiple reasoning sources
     */
    private synthesizeAnswer;
    /**
     * Calculate algorithmic complexity for performance monitoring
     */
    private calculateComplexity;
    /**
     * Estimate memory usage for performance tracking
     */
    private estimateMemoryUsage;
    /**
     * Research-focused query interface
     */
    researchQuery(question: string, domain?: string, options?: {
        enableCreativity?: boolean;
        enableTemporalAdvantage?: boolean;
        enableConsciousnessVerification?: boolean;
        depth?: number;
    }): Promise<ReasoningResult>;
    /**
     * Batch research processing for multiple questions
     */
    batchResearch(queries: string[], domain?: string): Promise<ReasoningResult[]>;
    /**
     * Real-time monitoring of reasoning performance
     */
    getPerformanceMetrics(): {
        totalQueries: number;
        averageResponseTime: number;
        breakthroughRate: number;
        consciousnessVerificationRate: number;
    };
}
export default AdvancedReasoningEngine;

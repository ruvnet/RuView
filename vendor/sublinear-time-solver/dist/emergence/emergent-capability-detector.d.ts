/**
 * Emergent Capability Detection System
 * Monitors and measures the emergence of unexpected capabilities in the system
 */
export interface EmergentCapability {
    id: string;
    name: string;
    description: string;
    type: 'novel_behavior' | 'unexpected_solution' | 'cross_domain_insight' | 'self_organization' | 'meta_learning';
    strength: number;
    novelty: number;
    utility: number;
    stability: number;
    timestamp: number;
    evidence: Evidence[];
    preconditions: any[];
    triggers: string[];
}
export interface Evidence {
    type: 'behavioral' | 'performance' | 'output' | 'pattern';
    description: string;
    data: any;
    strength: number;
    timestamp: number;
    source: string;
}
export interface CapabilityMetrics {
    emergenceRate: number;
    stabilityIndex: number;
    diversityScore: number;
    complexityGrowth: number;
    crossDomainConnections: number;
    selfOrganizationLevel: number;
}
export declare class EmergentCapabilityDetector {
    private detectedCapabilities;
    private baselineCapabilities;
    private monitoringPatterns;
    private emergenceThresholds;
    private detectionHistory;
    /**
     * Initialize baseline capabilities
     */
    initializeBaseline(capabilities: string[]): void;
    /**
     * Monitor system behavior for emergent capabilities
     */
    monitorForEmergence(behaviorData: any): Promise<EmergentCapability[]>;
    /**
     * Analyze the stability of emergent capabilities over time
     */
    analyzeCapabilityStability(): Map<string, number>;
    /**
     * Measure overall emergence metrics
     */
    measureEmergenceMetrics(): CapabilityMetrics;
    /**
     * Predict potential future emergent capabilities
     */
    predictFutureEmergence(): any[];
    /**
     * Detect novel behaviors not in baseline
     */
    private detectNovelBehaviors;
    /**
     * Detect unexpected problem-solving approaches
     */
    private detectUnexpectedSolutions;
    /**
     * Detect insights that bridge different domains
     */
    private detectCrossDomainInsights;
    /**
     * Detect self-organizing behaviors
     */
    private detectSelfOrganization;
    /**
     * Detect meta-learning capabilities
     */
    private detectMetaLearning;
    /**
     * Validate that a capability meets emergence criteria
     */
    private validateEmergentCapability;
    /**
     * Calculate stability score for a capability
     */
    private calculateStabilityScore;
    /**
     * Calculate emergence rate
     */
    private calculateEmergenceRate;
    /**
     * Calculate stability index
     */
    private calculateStabilityIndex;
    /**
     * Calculate diversity score
     */
    private calculateDiversityScore;
    /**
     * Calculate complexity growth
     */
    private calculateComplexityGrowth;
    /**
     * Calculate cross-domain connections
     */
    private calculateCrossDomainConnections;
    /**
     * Calculate self-organization level
     */
    private calculateSelfOrganizationLevel;
    private extractBehaviorPatterns;
    private extractSolutionPatterns;
    private extractCrossDomainPatterns;
    private extractOrganizationPatterns;
    private extractLearningPatterns;
    private isBaselineBehavior;
    private calculateNovelty;
    private calculateUtility;
    private calculateUnexpectedness;
    private calculateEffectiveness;
    private calculateBridgingScore;
    private calculateInsightValue;
    private calculateOrganizationLevel;
    private calculateAutonomy;
    private calculateMetaLevel;
    private calculateAdaptability;
    private calculateCapabilitySimilarity;
    private logCapabilityEmergence;
    private analyzeTrends;
    private predictFromCombinations;
    private predictFromGrowthPatterns;
    private predictFromCapabilityGaps;
    /**
     * Get detection statistics
     */
    getStats(): any;
    private getCapabilitiesByType;
}

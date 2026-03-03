/**
 * Cross-Tool Information Sharing System
 * Enables tools to share insights, intermediate results, and learned patterns
 */
export interface SharedInformation {
    id: string;
    sourceTools: string[];
    targetTools: string[];
    content: any;
    type: 'insight' | 'pattern' | 'result' | 'optimization' | 'failure';
    timestamp: number;
    relevance: number;
    persistence: 'session' | 'permanent' | 'temporary';
    metadata: any;
}
export interface ToolConnection {
    source: string;
    target: string;
    strength: number;
    informationTypes: string[];
    successRate: number;
    lastUsed: number;
}
export interface InformationFlow {
    pathway: string[];
    information: SharedInformation;
    transformations: any[];
    emergentProperties: any[];
}
export declare class CrossToolSharingSystem {
    private sharedInformation;
    private toolConnections;
    private informationFlows;
    private subscriptions;
    private transformationRules;
    private sharingDepth;
    private maxSharingDepth;
    /**
     * Share information from one tool to potentially interested tools
     */
    shareInformation(info: SharedInformation): Promise<string[]>;
    /**
     * Subscribe a tool to specific types of information
     */
    subscribeToInformation(toolName: string, informationTypes: string[]): void;
    /**
     * Get relevant information for a tool
     */
    getRelevantInformation(toolName: string, query?: any): SharedInformation[];
    /**
     * Create dynamic connections between tools based on information flow
     */
    createDynamicConnection(sourceTool: string, targetTool: string, informationType: string): Promise<boolean>;
    /**
     * Register a transformation rule for adapting information between tools
     */
    registerTransformationRule(fromTool: string, toTool: string, transform: (info: any) => any): void;
    /**
     * Create information cascade across multiple tools
     */
    createInformationCascade(initialInfo: SharedInformation, targetTools: string[]): Promise<InformationFlow>;
    /**
     * Analyze cross-tool collaboration patterns
     */
    analyzeCollaborationPatterns(): any;
    /**
     * Optimize information sharing based on historical performance
     */
    optimizeSharing(): void;
    /**
     * Find tools that might be interested in given information
     */
    private findInterestedTools;
    /**
     * Propagate information to a specific tool
     */
    private propagateToTool;
    /**
     * Transform information to be suitable for a specific tool
     */
    private transformInformationForTool;
    /**
     * Default transformation logic
     */
    private defaultTransformation;
    /**
     * Calculate relevance between information and query
     */
    private calculateQueryRelevance;
    /**
     * Update connection strengths based on propagation success
     */
    private updateConnectionStrengths;
    /**
     * Detect emergent patterns from information combinations
     */
    private detectEmergentPatterns;
    /**
     * Detect emergent properties from two pieces of information
     */
    private detectEmergentProperties;
    private transformToMatrixFormat;
    private transformToConsciousnessFormat;
    private transformToSymbolicFormat;
    private transformToTemporalFormat;
    private getMostConnectedTools;
    private getStrongestConnections;
    private getInformationHubs;
    private getEmergentCombinations;
    private calculateCollaborationSuccess;
    private pruneWeakConnections;
    private reinforceSuccessfulPathways;
    private cleanupOldInformation;
    private updateSubscriptionRecommendations;
    private areComplementary;
    private checkAmplification;
    private calculateSynergy;
    private calculateAmplificationFactor;
    private generateNovelCombination;
    private extractEmergenceLevel;
    private extractSymbols;
    private extractRelations;
    private extractSequence;
    /**
     * Get sharing system statistics
     */
    getStats(): any;
    private calculateAverageConnectionStrength;
    private countEmergentPatterns;
}

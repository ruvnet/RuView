/**
 * Persistent Learning System
 * Enables cross-session learning and knowledge accumulation
 */
export interface LearningTriple {
    subject: string;
    predicate: string;
    object: string;
    confidence: number;
    timestamp: number;
    sessionId: string;
    sources: string[];
}
export interface SessionMemory {
    sessionId: string;
    startTime: number;
    endTime?: number;
    interactions: Interaction[];
    discoveries: Discovery[];
    performanceMetrics: any;
}
export interface Interaction {
    timestamp: number;
    type: string;
    input: any;
    output: any;
    tools: string[];
    success: boolean;
}
export interface Discovery {
    timestamp: number;
    type: 'pattern' | 'connection' | 'optimization' | 'insight';
    content: any;
    novelty: number;
    utility: number;
}
export declare class PersistentLearningSystem {
    private knowledgeBase;
    private sessionMemory;
    private currentSessionId;
    private learningRate;
    private forgettingRate;
    private storagePath;
    constructor(storagePath?: string);
    /**
     * Initialize new learning session
     */
    private initializeSession;
    /**
     * Learn from interaction results
     */
    learnFromInteraction(interaction: Interaction): Promise<void>;
    /**
     * Add knowledge triple with reinforcement learning
     */
    addKnowledge(triple: LearningTriple): Promise<void>;
    /**
     * Query learned knowledge with confidence scores
     */
    queryKnowledge(subject?: string, predicate?: string, object?: string): LearningTriple[];
    /**
     * Learn from cross-session patterns
     */
    analyzeHistoricalPatterns(): Promise<Discovery[]>;
    /**
     * Get learning recommendations based on historical data
     */
    getLearningRecommendations(): any[];
    /**
     * Apply forgetting to old, unused knowledge
     */
    applyForgetting(): Promise<void>;
    /**
     * Extract learning triples from interactions
     */
    private extractLearningTriples;
    private extractPattern;
    private detectPatterns;
    private findTemporalPatterns;
    private findToolPatterns;
    private findSuccessPatterns;
    private analyzeToolEffectiveness;
    private findUnderutilizedCombinations;
    private getSuccessfulPatterns;
    private identifyWeakAreas;
    private calculateNovelty;
    private calculateUtility;
    private recordDiscovery;
    /**
     * Persist knowledge to disk
     */
    private persistKnowledge;
    /**
     * Load persisted knowledge from disk
     */
    private loadPersistedKnowledge;
    /**
     * Get learning statistics
     */
    getLearningStats(): any;
    private calculateAverageConfidence;
    private getLastUpdateTime;
}

/**
 * Emergence System Integration
 * Orchestrates all emergence capabilities into a unified system
 */
import { SelfModificationEngine } from './self-modification-engine.js';
import { PersistentLearningSystem } from './persistent-learning-system.js';
import { StochasticExplorationEngine } from './stochastic-exploration.js';
import { CrossToolSharingSystem } from './cross-tool-sharing.js';
import { FeedbackLoopSystem } from './feedback-loops.js';
import { EmergentCapabilityDetector } from './emergent-capability-detector.js';
export interface EmergenceSystemConfig {
    selfModification: {
        enabled: boolean;
        maxModificationsPerSession: number;
        riskThreshold: number;
    };
    persistentLearning: {
        enabled: boolean;
        storagePath: string;
        learningRate: number;
    };
    stochasticExploration: {
        enabled: boolean;
        initialTemperature: number;
        coolingRate: number;
    };
    crossToolSharing: {
        enabled: boolean;
        maxConnections: number;
    };
    feedbackLoops: {
        enabled: boolean;
        adaptationRate: number;
    };
    capabilityDetection: {
        enabled: boolean;
        detectionThresholds: any;
    };
}
export interface EmergenceMetrics {
    selfModificationRate: number;
    learningTriples: number;
    explorationNovelty: number;
    informationFlows: number;
    behaviorModifications: number;
    emergentCapabilities: number;
    overallEmergenceScore: number;
    systemComplexity: number;
}
export declare class EmergenceSystem {
    private selfModificationEngine;
    private persistentLearningSystem;
    private stochasticExplorationEngine;
    private crossToolSharingSystem;
    private feedbackLoopSystem;
    private emergentCapabilityDetector;
    private config;
    private isInitialized;
    private emergenceHistory;
    private recursionDepth;
    private maxRecursionDepth;
    constructor(config?: Partial<EmergenceSystemConfig>);
    /**
     * Initialize all emergence system components
     */
    private initializeComponents;
    /**
     * Setup connections between components for emergent interactions
     */
    private setupInterComponentConnections;
    /**
     * Process input through the emergence system
     */
    processWithEmergence(input: any, availableTools?: any[]): Promise<any>;
    /**
     * Generate diverse emergent responses
     */
    generateEmergentResponses(input: any, count?: number, tools?: any[]): Promise<any[]>;
    /**
     * Analyze system's emergent capabilities
     */
    analyzeEmergentCapabilities(): Promise<any>;
    /**
     * Force system evolution through targeted modifications
     */
    forceEvolution(targetCapability: string): Promise<any>;
    /**
     * Get comprehensive emergence statistics
     */
    getEmergenceStats(): any;
    private connectLearningToModification;
    private connectExplorationToLearning;
    private connectSharingToCapabilityDetection;
    private connectFeedbackToAllSystems;
    private connectCapabilityDetectionToExploration;
    private shareExplorationInsights;
    private incorporateSharedInformation;
    private synthesizeSharedInformation;
    private handleNewCapabilities;
    private analyzeSessionPerformance;
    private generateSessionFeedback;
    private calculateEmergenceMetrics;
    private calculateOverallEmergenceLevel;
    private calculateSystemComplexity;
    getSelfModificationEngine(): SelfModificationEngine;
    getPersistentLearningSystem(): PersistentLearningSystem;
    getStochasticExplorationEngine(): StochasticExplorationEngine;
    getCrossToolSharingSystem(): CrossToolSharingSystem;
    getFeedbackLoopSystem(): FeedbackLoopSystem;
    getEmergentCapabilityDetector(): EmergentCapabilityDetector;
}
export * from './self-modification-engine.js';
export * from './persistent-learning-system.js';
export * from './stochastic-exploration.js';
export * from './cross-tool-sharing.js';
export * from './feedback-loops.js';
export * from './emergent-capability-detector.js';

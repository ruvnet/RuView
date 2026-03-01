/**
 * ReasoningBank Learning Integration Example
 *
 * Demonstrates using agentic-jujutsu's ReasoningBank intelligence features
 * to learn from data generation patterns, track quality over time,
 * implement adaptive schema evolution, and create self-improving generators.
 */
interface GenerationTrajectory {
    id: string;
    timestamp: Date;
    schema: any;
    parameters: any;
    quality: number;
    performance: {
        duration: number;
        recordCount: number;
        errorRate: number;
    };
    verdict: 'success' | 'failure' | 'partial';
    lessons: string[];
}
interface LearningPattern {
    patternId: string;
    type: 'schema' | 'parameters' | 'strategy';
    description: string;
    successRate: number;
    timesApplied: number;
    averageQuality: number;
    recommendations: string[];
}
interface AdaptiveSchema {
    version: string;
    schema: any;
    performance: number;
    generation: number;
    parentVersion?: string;
    mutations: string[];
}
declare class ReasoningBankDataGenerator {
    private synth;
    private repoPath;
    private trajectories;
    private patterns;
    private schemas;
    constructor(repoPath: string);
    /**
     * Initialize ReasoningBank-enabled repository
     */
    initialize(): Promise<void>;
    /**
     * Generate data with trajectory tracking
     */
    generateWithLearning(schema: any, parameters: any, description: string): Promise<{
        data: any[];
        trajectory: GenerationTrajectory;
    }>;
    /**
     * Learn from generation trajectory and update patterns
     */
    private learnFromTrajectory;
    /**
     * Adaptive schema evolution based on learning
     */
    evolveSchema(baseSchema: any, targetQuality?: number, maxGenerations?: number): Promise<AdaptiveSchema>;
    /**
     * Pattern recognition across trajectories
     */
    recognizePatterns(): Promise<LearningPattern[]>;
    /**
     * Self-improvement through continuous learning
     */
    continuousImprovement(iterations?: number): Promise<any>;
    private calculateQuality;
    private judgeVerdict;
    private extractLessons;
    private generatePatternId;
    private describePattern;
    private generateRecommendations;
    private applyLearningToSchema;
    private mutateSchema;
    private groupBySchemaStructure;
    private synthesizeRecommendations;
    private getBestPattern;
    private schemaFromPattern;
    private getBaseSchema;
    private saveTrajectory;
    private savePattern;
    private saveSchema;
    private commitWithReasoning;
    private distillMemory;
    private loadLearningState;
}
export { ReasoningBankDataGenerator, GenerationTrajectory, LearningPattern, AdaptiveSchema };
//# sourceMappingURL=reasoning-bank-learning.d.ts.map
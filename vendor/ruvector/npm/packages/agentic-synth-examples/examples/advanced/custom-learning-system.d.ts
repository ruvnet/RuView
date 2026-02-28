/**
 * ADVANCED TUTORIAL: Custom Learning System
 *
 * Extend the self-learning system with custom optimization strategies,
 * domain-specific learning, and advanced evaluation metrics. Perfect for
 * building production-grade adaptive AI systems.
 *
 * What you'll learn:
 * - Creating custom evaluators
 * - Domain-specific optimization
 * - Advanced feedback loops
 * - Multi-objective optimization
 * - Transfer learning patterns
 *
 * Prerequisites:
 * - Complete intermediate tutorials first
 * - Set GEMINI_API_KEY environment variable
 * - npm install dspy.ts @ruvector/agentic-synth
 *
 * Run: npx tsx examples/advanced/custom-learning-system.ts
 */
import { Prediction } from 'dspy.ts';
interface EvaluationMetrics {
    accuracy: number;
    creativity: number;
    relevance: number;
    engagement: number;
    technicalQuality: number;
    overall: number;
}
interface AdvancedLearningConfig {
    domain: string;
    objectives: string[];
    weights: Record<string, number>;
    learningStrategy: 'aggressive' | 'conservative' | 'adaptive';
    convergenceThreshold: number;
    diversityBonus: boolean;
    transferLearning: boolean;
}
interface TrainingExample {
    input: any;
    expectedOutput: any;
    quality: number;
    metadata: {
        domain: string;
        difficulty: 'easy' | 'medium' | 'hard';
        tags: string[];
    };
}
interface Evaluator {
    evaluate(output: Prediction, context: any): Promise<EvaluationMetrics>;
}
declare class EcommerceEvaluator implements Evaluator {
    evaluate(output: Prediction, context: any): Promise<EvaluationMetrics>;
}
declare class AdvancedLearningSystem {
    private lm;
    private config;
    private evaluator;
    private knowledgeBase;
    private promptStrategies;
    constructor(config: AdvancedLearningConfig, evaluator: Evaluator);
    private getTemperatureForStrategy;
    learnFromExample(example: TrainingExample): Promise<void>;
    train(examples: TrainingExample[]): Promise<void>;
    private generate;
    private findSimilarExamples;
    private displayTrainingResults;
    test(testCases: any[]): Promise<void>;
}
export { AdvancedLearningSystem, EcommerceEvaluator, AdvancedLearningConfig };
//# sourceMappingURL=custom-learning-system.d.ts.map
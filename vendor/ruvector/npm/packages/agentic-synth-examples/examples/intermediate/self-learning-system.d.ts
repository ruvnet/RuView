/**
 * INTERMEDIATE TUTORIAL: Self-Learning System
 *
 * Build an adaptive AI system that improves its output quality over time
 * through feedback loops and pattern recognition. This demonstrates how
 * to create systems that learn from their mistakes and successes.
 *
 * What you'll learn:
 * - Building feedback loops
 * - Tracking quality improvements
 * - Adaptive prompt engineering
 * - Learning from examples
 *
 * Prerequisites:
 * - Set GEMINI_API_KEY environment variable
 * - npm install dspy.ts @ruvector/agentic-synth
 *
 * Run: npx tsx examples/intermediate/self-learning-system.ts
 */
import { Prediction } from 'dspy.ts';
interface LearningConfig {
    targetQualityThreshold: number;
    maxIterations: number;
    improvementRate: number;
    minImprovement: number;
}
interface Feedback {
    quality: number;
    strengths: string[];
    weaknesses: string[];
    suggestions: string[];
}
interface LearningEntry {
    iteration: number;
    quality: number;
    output: Prediction;
    feedback: Feedback;
    promptModifications: string[];
    timestamp: Date;
}
declare class SelfLearningGenerator {
    private lm;
    private history;
    private config;
    private basePrompt;
    private currentPromptAdditions;
    constructor(config?: Partial<LearningConfig>);
    private evaluateOutput;
    private adaptPrompt;
    private generate;
    learn(input: any, criteria?: any): Promise<void>;
    private displaySummary;
    getLearnedImprovements(): string[];
    getHistory(): LearningEntry[];
}
export { SelfLearningGenerator, LearningConfig, LearningEntry };
//# sourceMappingURL=self-learning-system.d.ts.map
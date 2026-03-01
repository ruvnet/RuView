/**
 * Self-Learning Generator
 * Adaptive system that improves output quality through feedback loops
 */
import { EventEmitter } from 'events';
import type { LearningMetrics } from '../types/index.js';
export interface SelfLearningConfig {
    task: string;
    learningRate: number;
    iterations: number;
    qualityThreshold?: number;
    maxAttempts?: number;
}
export interface GenerateOptions {
    prompt: string;
    tests?: ((output: any) => boolean)[];
    initialQuality?: number;
}
export declare class SelfLearningGenerator extends EventEmitter {
    private config;
    private history;
    private currentQuality;
    constructor(config: SelfLearningConfig);
    /**
     * Generate with self-learning and improvement
     */
    generate(options: GenerateOptions): Promise<{
        output: any;
        finalQuality: number;
        improvement: number;
        iterations: number;
        metrics: LearningMetrics[];
    }>;
    /**
     * Generate output for current iteration
     */
    private generateOutput;
    /**
     * Evaluate output quality
     */
    private evaluate;
    /**
     * Calculate test pass rate
     */
    private calculateTestPassRate;
    /**
     * Generate feedback for current iteration
     */
    private generateFeedback;
    /**
     * Get learning history
     */
    getHistory(): LearningMetrics[];
    /**
     * Reset learning state
     */
    reset(): void;
}
//# sourceMappingURL=self-learning.d.ts.map
/**
 * Continual Learning Dataset Generation
 *
 * This example demonstrates:
 * - Incremental training data generation
 * - Domain adaptation scenarios
 * - Catastrophic forgetting prevention data
 * - Transfer learning datasets
 */
import type { GenerationResult } from '../../src/types.js';
/**
 * Generate incremental training batches for continual learning
 */
export declare function generateIncrementalData(): Promise<GenerationResult<unknown>[]>;
/**
 * Generate source and target domain data for domain adaptation
 */
export declare function generateDomainAdaptationData(): Promise<{
    source: GenerationResult<unknown>;
    target: GenerationResult<unknown>;
    labeledTarget: GenerationResult<unknown>;
}>;
/**
 * Generate replay buffer and interleaved training data
 */
export declare function generateAntiCatastrophicData(): Promise<{
    task1: GenerationResult<unknown>;
    task2: GenerationResult<unknown>;
    replay: GenerationResult<unknown>;
    interleaved: GenerationResult<unknown>;
}>;
/**
 * Generate pre-training and fine-tuning datasets
 */
export declare function generateTransferLearningData(): Promise<{
    pretraining: GenerationResult<unknown>;
    finetuning: GenerationResult<unknown>;
    fewShot: GenerationResult<unknown>;
}>;
/**
 * Generate data organized by difficulty for curriculum learning
 */
export declare function generateCurriculumData(): Promise<{
    stage: number;
    difficulty: string;
    data: GenerationResult<unknown>;
}[]>;
/**
 * Generate streaming data for online learning
 */
export declare function generateOnlineLearningStream(): Promise<GenerationResult<unknown>>;
/**
 * Demonstrate complete continual learning pipeline
 */
export declare function completeContinualLearningPipeline(): Promise<void>;
export declare function runAllContinualLearningExamples(): Promise<void>;
//# sourceMappingURL=continual-learning.d.ts.map
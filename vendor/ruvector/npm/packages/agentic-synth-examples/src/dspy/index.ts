/**
 * DSPy Training Examples
 *
 * Comprehensive examples for DSPy.ts multi-model training and benchmarking:
 * - DSPyTrainingSession: Advanced multi-model training framework
 * - MultiModelBenchmark: Comprehensive benchmarking suite
 *
 * @packageDocumentation
 */

// Export training session components
export {
  DSPyTrainingSession,
  ModelTrainingAgent,
  ClaudeSonnetAgent,
  GPT4Agent,
  LlamaAgent,
  GeminiAgent,
  BenchmarkCollector,
  OptimizationEngine,
  ModelProvider,
  TrainingPhase,
  TrainingConfigSchema
} from './training-session';

export type {
  QualityMetrics,
  PerformanceMetrics,
  IterationResult,
  ModelConfig,
  DSPySignature,
  TrainingConfig
} from './training-session';

// Export benchmark components
export {
  MultiModelBenchmark
} from './benchmark';

export type {
  ModelConfig as BenchmarkModelConfig,
  BenchmarkMetrics,
  BenchmarkResult,
  ComparisonReport
} from './benchmark';

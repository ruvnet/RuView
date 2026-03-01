/**
 * @ruvector/agentic-synth-examples
 *
 * Production-ready examples for agentic-synth including:
 * - DSPy multi-model training and benchmarking
 * - Self-learning adaptive systems
 * - Stock market simulation
 * - Security testing scenarios
 * - CI/CD pipeline data generation
 * - Multi-agent swarm coordination
 */

// DSPy training and benchmarking
export {
  DSPyTrainingSession,
  MultiModelBenchmark,
  ModelTrainingAgent,
  ClaudeSonnetAgent,
  GPT4Agent,
  LlamaAgent,
  GeminiAgent,
  BenchmarkCollector,
  OptimizationEngine,
  ModelProvider,
  TrainingPhase
} from './dspy/index.js';
export type {
  QualityMetrics,
  PerformanceMetrics,
  IterationResult,
  ModelConfig,
  DSPySignature,
  TrainingConfig,
  BenchmarkMetrics,
  BenchmarkResult,
  ComparisonReport
} from './dspy/index.js';

// Example generators
export { SelfLearningGenerator } from './self-learning/index.js';
export type {
  SelfLearningConfig,
  FeedbackData,
  LearningMetrics
} from './self-learning/index.js';

export { StockMarketSimulator } from './stock-market/index.js';
export type {
  StockMarketConfig,
  OHLCVData,
  MarketNewsEvent,
  MarketCondition,
  MarketStatistics
} from './stock-market/index.js';

export { SecurityTestingGenerator } from './security/index.js';
export type {
  VulnerabilityTestCase,
  SecurityLogEntry,
  AnomalyPattern,
  PenetrationTestScenario,
  VulnerabilitySeverity,
  VulnerabilityType
} from './security/index.js';

export { CICDDataGenerator } from './cicd/index.js';
export type {
  PipelineExecution,
  TestResults,
  DeploymentRecord,
  PerformanceMetrics as CICDPerformanceMetrics,
  MonitoringAlert,
  PipelineStatus
} from './cicd/index.js';

export { SwarmCoordinator } from './swarm/index.js';
export type {
  Agent,
  AgentMemory,
  CoordinationTask,
  DistributedLearningPattern,
  SwarmStatistics,
  AgentRole,
  CoordinationStrategy
} from './swarm/index.js';

/**
 * Factory functions for quick initialization
 */
export const Examples = {
  /**
   * Create a self-learning generator
   */
  createSelfLearning: (config?: any) => new SelfLearningGenerator(config),

  /**
   * Create a stock market simulator
   */
  createStockMarket: (config?: any) => new StockMarketSimulator(config),

  /**
   * Create a security testing generator
   */
  createSecurity: (config?: any) => new SecurityTestingGenerator(config),

  /**
   * Create a CI/CD data generator
   */
  createCICD: (config?: any) => new CICDDataGenerator(config),

  /**
   * Create a swarm coordinator
   */
  createSwarm: (config?: any) => new SwarmCoordinator(config)
};

// Import all generators
import { SelfLearningGenerator } from './self-learning/index.js';
import { StockMarketSimulator } from './stock-market/index.js';
import { SecurityTestingGenerator } from './security/index.js';
import { CICDDataGenerator } from './cicd/index.js';
import { SwarmCoordinator } from './swarm/index.js';

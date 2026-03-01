/**
 * Core module exports
 *
 * These wrappers provide safe, type-flexible interfaces to the underlying
 * native packages, handling array type conversions automatically.
 */

export * from './gnn-wrapper';
export * from './attention-fallbacks';
export * from './agentdb-fast';
export * from './sona-wrapper';
export * from './intelligence-engine';
export * from './onnx-embedder';
export * from './onnx-optimized';
export * from './parallel-intelligence';
export * from './parallel-workers';
export * from './router-wrapper';
export * from './graph-wrapper';
export * from './cluster-wrapper';
export * from './ast-parser';
export * from './diff-embeddings';
export * from './coverage-router';
export * from './graph-algorithms';
export * from './tensor-compress';
export * from './learning-engine';
export * from './adaptive-embedder';
export * from './neural-embeddings';
export * from './neural-perf';
export * from './rvf-wrapper';

// Analysis module (consolidated security, complexity, patterns)
export * from '../analysis';

// Re-export default objects for convenience
export { default as gnnWrapper } from './gnn-wrapper';
export { default as attentionFallbacks } from './attention-fallbacks';
export { default as agentdbFast } from './agentdb-fast';
export { default as Sona } from './sona-wrapper';
export { default as IntelligenceEngine } from './intelligence-engine';
export { default as OnnxEmbedder } from './onnx-embedder';
export { default as OptimizedOnnxEmbedder } from './onnx-optimized';
export { default as ParallelIntelligence } from './parallel-intelligence';
export { default as ExtendedWorkerPool } from './parallel-workers';
export { default as SemanticRouter } from './router-wrapper';
export { default as CodeGraph } from './graph-wrapper';
export { default as RuvectorCluster } from './cluster-wrapper';
export { default as CodeParser } from './ast-parser';

// Alias for backward compatibility
export { CodeParser as ASTParser } from './ast-parser';

// New v2.1 modules
export { default as TensorCompress } from './tensor-compress';
export { default as LearningEngine } from './learning-engine';
export { default as AdaptiveEmbedder } from './adaptive-embedder';
export { default as NeuralSubstrate } from './neural-embeddings';

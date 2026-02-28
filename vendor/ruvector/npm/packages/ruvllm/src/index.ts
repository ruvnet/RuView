/**
 * @ruvector/ruvllm - Self-learning LLM orchestration
 *
 * RuvLLM combines SONA adaptive learning with HNSW memory,
 * FastGRNN routing, and SIMD-optimized inference.
 *
 * @example
 * ```typescript
 * import { RuvLLM, SessionManager, SonaCoordinator } from '@ruvector/ruvllm';
 *
 * const llm = new RuvLLM({ learningEnabled: true });
 * const sessions = new SessionManager(llm);
 * const sona = new SonaCoordinator();
 *
 * // Query with session context
 * const session = sessions.create();
 * const response = sessions.chat(session.id, 'What is AI?');
 *
 * // Track learning trajectory
 * const trajectory = new TrajectoryBuilder()
 *   .startStep('query', 'What is AI?')
 *   .endStep(response.text, response.confidence)
 *   .complete('success');
 *
 * sona.recordTrajectory(trajectory);
 * ```
 *
 * @example Federated Learning
 * ```typescript
 * import { EphemeralAgent, FederatedCoordinator } from '@ruvector/ruvllm';
 *
 * // Central coordinator
 * const coordinator = new FederatedCoordinator('coord-1');
 *
 * // Ephemeral agents process tasks and export
 * const agent = new EphemeralAgent('agent-1');
 * agent.processTask(embedding, 0.9);
 * const exportData = agent.exportState();
 *
 * // Aggregate learning
 * coordinator.aggregate(exportData);
 * ```
 *
 * @example LoRA Adapters
 * ```typescript
 * import { LoraAdapter, LoraManager } from '@ruvector/ruvllm';
 *
 * const adapter = new LoraAdapter({ rank: 8, alpha: 16 });
 * const output = adapter.forward(input);
 * ```
 */

// Core types
export * from './types';

// Main engine
export * from './engine';

// SIMD operations
export * from './simd';

// Session management
export * from './session';

// Streaming support
export * from './streaming';

// SONA learning system
export * from './sona';

// Federated learning
export * from './federated';

// LoRA adapters
export * from './lora';

// Export/serialization
export * from './export';

// Training pipeline
export * from './training';

// Contrastive fine-tuning
export * from './contrastive';

// Model downloader and registry
export * from './models';

// Benchmarks for Claude Code use cases
export * from './benchmarks';

// External Intelligence Providers (ADR-043)
export * from './intelligence';

// Native bindings utilities
export { version, hasSimdSupport } from './native';

// Default export
export { RuvLLM as default } from './engine';

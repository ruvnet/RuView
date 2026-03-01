/**
 * Core Context - Agent, Session, Skill, ChatEnhancer
 *
 * The heart of RuvBot, handling conversation management and agent behavior.
 * ChatEnhancer provides the ultimate chatbot experience with skills and memory.
 */

export * from './agent';
export * from './session';
export * from './skill';

// ChatEnhancer - The ultimate chatbot integration layer
export {
  ChatEnhancer,
  createChatEnhancer,
  type ChatEnhancerConfig,
  type EnhancedChatContext,
  type EnhancedChatResponse,
} from './ChatEnhancer.js';

// Re-export memory types from learning module
export type {
  Embedder,
  VectorIndex,
  MemoryEntry,
  MemoryType,
  MemoryMetadata,
  VectorSearchResult,
} from '../learning/memory/MemoryManager.js';

/**
 * RuvBot - Clawdbot-style Personal AI Assistant
 *
 * A multi-tenant, self-learning AI assistant built on RuVector.
 *
 * Architecture: Domain-Driven Design with four bounded contexts:
 * - Core: Agent, Session, Memory, Skill
 * - Infrastructure: Persistence, Messaging, Workers
 * - Integration: Slack, Webhooks, Providers
 * - Learning: Embeddings, Training, Patterns
 *
 * @packageDocumentation
 */

// Core Context
export * from './core/index.js';

// Infrastructure Context - explicit exports to avoid duplicates
export * from './infrastructure/persistence/index.js';
export {
  type EventBus,
  type DomainEvent,
  type EventHandler,
  type Subscription,
  type QueueManager,
  type Job,
  type JobOptions,
} from './infrastructure/messaging/index.js';
export {
  type WorkerPool,
  type JobHandler,
  type WorkerJob,
  type WorkerJobOptions,
  type WorkerContext,
  type JobResult,
  type WorkerPoolStatus,
  WORKER_TYPES,
  type WorkerType,
} from './infrastructure/workers/index.js';

// Integration Context
export * from './integration/index.js';

// Learning Context
export * from './learning/index.js';

// Channels Context
export * from './channels/index.js';

// Swarm Context
export * from './swarm/index.js';

// Security Context
export * from './security/index.js';

// Plugins Context
export * from './plugins/index.js';

// Types - exclude duplicates (DomainEvent, EventHandler already exported from infrastructure)
export type {
  TenantId,
  WorkspaceId,
  UserId,
  AgentId,
  SessionId,
  TurnId,
  MemoryId,
  SkillId,
  PatternId,
  TrajectoryId,
  TenantContext,
  Role,
  GeoLocation,
  TimeRange,
  SemanticVersion,
  Result,
  RuvBotConfig,
  DatabaseConfig,
  RedisConfig,
  VectorStoreConfig,
  LLMConfig as RuvBotLLMConfig,
  SlackConfig as RuvBotSlackConfig,
  WebhooksConfig,
  LearningConfig,
} from './types.js';
export { ok, err } from './types.js';

/**
 * @ruvector/replication - Data Replication and Synchronization
 *
 * A TypeScript implementation of data replication capabilities including:
 * - Multi-node replica management
 * - Synchronous, asynchronous, and semi-synchronous replication modes
 * - Conflict resolution with vector clocks
 * - Change data capture and streaming
 * - Automatic failover and split-brain prevention
 *
 * @example
 * ```typescript
 * import {
 *   ReplicaSet,
 *   ReplicaRole,
 *   SyncManager,
 *   ReplicationLog,
 *   SyncMode,
 *   ChangeOperation,
 * } from '@ruvector/replication';
 *
 * // Create a replica set
 * const replicaSet = new ReplicaSet('my-cluster');
 *
 * // Add replicas
 * replicaSet.addReplica('replica-1', '192.168.1.10:9001', ReplicaRole.Primary);
 * replicaSet.addReplica('replica-2', '192.168.1.11:9001', ReplicaRole.Secondary);
 * replicaSet.addReplica('replica-3', '192.168.1.12:9001', ReplicaRole.Secondary);
 *
 * // Create replication log and sync manager
 * const log = new ReplicationLog('replica-1');
 * const syncManager = new SyncManager(replicaSet, log);
 *
 * // Configure semi-sync replication
 * syncManager.setSyncMode(SyncMode.SemiSync, 1);
 *
 * // Listen for events
 * syncManager.on('changeReceived', (change) => {
 *   console.log(`Change: ${change.operation} on ${change.key}`);
 * });
 *
 * // Record a change
 * await syncManager.recordChange('user:123', ChangeOperation.Update, { name: 'Alice' });
 * ```
 *
 * @packageDocumentation
 */

// Types
export {
  ReplicaId,
  LogicalClock,
  ReplicaRole,
  ReplicaStatus,
  SyncMode,
  HealthStatus,
  Replica,
  ChangeOperation,
  ChangeEvent,
  VectorClockValue,
  LogEntry,
  FailoverPolicy,
  ReplicationError,
  ReplicationErrorCode,
  ReplicationEvent,
  ReplicaSetConfig,
  SyncConfig,
} from './types.js';

// Vector Clock
export {
  VectorClock,
  VectorClockComparison,
  ConflictResolver,
  LastWriteWins,
  MergeFunction,
} from './vector-clock.js';

// Replica Set
export { ReplicaSet } from './replica-set.js';

// Sync Manager
export { SyncManager, ReplicationLog } from './sync-manager.js';

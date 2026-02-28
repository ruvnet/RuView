/**
 * Replication Types
 * Data replication and synchronization types
 */

import type EventEmitter from 'eventemitter3';

/** Unique identifier for a replica */
export type ReplicaId = string;

/** Logical timestamp for ordering events */
export type LogicalClock = number;

/** Role of a replica in the set */
export enum ReplicaRole {
  Primary = 'primary',
  Secondary = 'secondary',
  Arbiter = 'arbiter',
}

/** Status of a replica */
export enum ReplicaStatus {
  Active = 'active',
  Syncing = 'syncing',
  Offline = 'offline',
  Failed = 'failed',
}

/** Synchronization mode */
export enum SyncMode {
  /** All replicas must confirm before commit */
  Synchronous = 'synchronous',
  /** Commit immediately, replicate in background */
  Asynchronous = 'asynchronous',
  /** Wait for minimum number of replicas */
  SemiSync = 'semi-sync',
}

/** Health status of a replica */
export enum HealthStatus {
  Healthy = 'healthy',
  Degraded = 'degraded',
  Unhealthy = 'unhealthy',
  Unknown = 'unknown',
}

/** Replica information */
export interface Replica {
  id: ReplicaId;
  address: string;
  role: ReplicaRole;
  status: ReplicaStatus;
  lastSeen: number;
  lag: number; // Replication lag in milliseconds
}

/** Change operation type */
export enum ChangeOperation {
  Insert = 'insert',
  Update = 'update',
  Delete = 'delete',
}

/** Change event for CDC */
export interface ChangeEvent<T = unknown> {
  /** Unique event ID */
  id: string;
  /** Operation type */
  operation: ChangeOperation;
  /** Affected key/path */
  key: string;
  /** New value (for insert/update) */
  value?: T;
  /** Previous value (for update/delete) */
  previousValue?: T;
  /** Timestamp of the change */
  timestamp: number;
  /** Source replica */
  sourceReplica: ReplicaId;
  /** Vector clock for ordering */
  vectorClock: VectorClockValue;
}

/** Vector clock entry for a single node */
export type VectorClockValue = Map<ReplicaId, LogicalClock>;

/** Log entry for replication */
export interface LogEntry<T = unknown> {
  /** Unique entry ID */
  id: string;
  /** Sequence number */
  sequence: number;
  /** Operation data */
  data: T;
  /** Timestamp */
  timestamp: number;
  /** Vector clock */
  vectorClock: VectorClockValue;
}

/** Failover policy */
export enum FailoverPolicy {
  /** Automatic failover with quorum */
  Automatic = 'automatic',
  /** Manual intervention required */
  Manual = 'manual',
  /** Priority-based failover */
  Priority = 'priority',
}

/** Replication error types */
export class ReplicationError extends Error {
  constructor(
    message: string,
    public readonly code: ReplicationErrorCode,
  ) {
    super(message);
    this.name = 'ReplicationError';
  }

  static replicaNotFound(id: string): ReplicationError {
    return new ReplicationError(`Replica not found: ${id}`, ReplicationErrorCode.ReplicaNotFound);
  }

  static noPrimary(): ReplicationError {
    return new ReplicationError('No primary replica available', ReplicationErrorCode.NoPrimary);
  }

  static timeout(operation: string): ReplicationError {
    return new ReplicationError(`Replication timeout: ${operation}`, ReplicationErrorCode.Timeout);
  }

  static quorumNotMet(needed: number, available: number): ReplicationError {
    return new ReplicationError(
      `Quorum not met: needed ${needed}, got ${available}`,
      ReplicationErrorCode.QuorumNotMet,
    );
  }

  static splitBrain(): ReplicationError {
    return new ReplicationError('Split-brain detected', ReplicationErrorCode.SplitBrain);
  }

  static conflictResolution(reason: string): ReplicationError {
    return new ReplicationError(
      `Conflict resolution failed: ${reason}`,
      ReplicationErrorCode.ConflictResolution,
    );
  }
}

export enum ReplicationErrorCode {
  ReplicaNotFound = 'REPLICA_NOT_FOUND',
  NoPrimary = 'NO_PRIMARY',
  Timeout = 'TIMEOUT',
  SyncFailed = 'SYNC_FAILED',
  ConflictResolution = 'CONFLICT_RESOLUTION',
  FailoverFailed = 'FAILOVER_FAILED',
  Network = 'NETWORK',
  QuorumNotMet = 'QUORUM_NOT_MET',
  SplitBrain = 'SPLIT_BRAIN',
  InvalidState = 'INVALID_STATE',
}

/** Events emitted by replication components */
export enum ReplicationEvent {
  ReplicaAdded = 'replicaAdded',
  ReplicaRemoved = 'replicaRemoved',
  ReplicaStatusChanged = 'replicaStatusChanged',
  PrimaryChanged = 'primaryChanged',
  ChangeReceived = 'changeReceived',
  SyncStarted = 'syncStarted',
  SyncCompleted = 'syncCompleted',
  ConflictDetected = 'conflictDetected',
  ConflictResolved = 'conflictResolved',
  FailoverStarted = 'failoverStarted',
  FailoverCompleted = 'failoverCompleted',
  Error = 'error',
}

/** Configuration for replica set */
export interface ReplicaSetConfig {
  /** Cluster name */
  name: string;
  /** Minimum replicas for quorum */
  minQuorum: number;
  /** Heartbeat interval in milliseconds */
  heartbeatInterval: number;
  /** Timeout for health checks in milliseconds */
  healthCheckTimeout: number;
  /** Failover policy */
  failoverPolicy: FailoverPolicy;
}

/** Configuration for sync manager */
export interface SyncConfig {
  /** Sync mode */
  mode: SyncMode;
  /** Minimum replicas for semi-sync */
  minReplicas?: number;
  /** Batch size for streaming changes */
  batchSize: number;
  /** Maximum lag before triggering catchup */
  maxLag: number;
}

/**
 * Sync Manager Implementation
 * Manages data synchronization across replicas
 */

import EventEmitter from 'eventemitter3';
import {
  type ReplicaId,
  type SyncConfig,
  type LogEntry,
  type ChangeEvent,
  SyncMode,
  ReplicationError,
  ReplicationEvent,
  ChangeOperation,
} from './types.js';
import { VectorClock, type ConflictResolver, LastWriteWins } from './vector-clock.js';
import type { ReplicaSet } from './replica-set.js';

/** Default sync configuration */
const DEFAULT_SYNC_CONFIG: SyncConfig = {
  mode: SyncMode.Asynchronous,
  batchSize: 100,
  maxLag: 5000,
};

/** Replication log for tracking changes */
export class ReplicationLog<T = unknown> {
  private entries: LogEntry<T>[] = [];
  private sequence = 0;
  private readonly replicaId: ReplicaId;
  private vectorClock: VectorClock;

  constructor(replicaId: ReplicaId) {
    this.replicaId = replicaId;
    this.vectorClock = new VectorClock();
  }

  /** Get the current sequence number */
  get currentSequence(): number {
    return this.sequence;
  }

  /** Get the current vector clock */
  get clock(): VectorClock {
    return this.vectorClock.clone();
  }

  /** Append an entry to the log */
  append(data: T): LogEntry<T> {
    this.sequence++;
    this.vectorClock.increment(this.replicaId);

    const entry: LogEntry<T> = {
      id: `${this.replicaId}-${this.sequence}`,
      sequence: this.sequence,
      data,
      timestamp: Date.now(),
      vectorClock: this.vectorClock.getValue(),
    };

    this.entries.push(entry);
    return entry;
  }

  /** Get entries since a sequence number */
  getEntriesSince(sequence: number, limit?: number): LogEntry<T>[] {
    const filtered = this.entries.filter((e) => e.sequence > sequence);
    return limit ? filtered.slice(0, limit) : filtered;
  }

  /** Get entry by ID */
  getEntry(id: string): LogEntry<T> | undefined {
    return this.entries.find((e) => e.id === id);
  }

  /** Get all entries */
  getAllEntries(): LogEntry<T>[] {
    return [...this.entries];
  }

  /** Apply entries from another replica */
  applyEntries(entries: LogEntry<T>[]): void {
    for (const entry of entries) {
      const entryClock = new VectorClock(entry.vectorClock);
      this.vectorClock.merge(entryClock);
    }
    // Note: In a real implementation, entries would be merged properly
  }

  /** Clear the log */
  clear(): void {
    this.entries = [];
    this.sequence = 0;
    this.vectorClock = new VectorClock();
  }
}

/** Manages synchronization across replicas */
export class SyncManager<T = unknown> extends EventEmitter {
  private readonly replicaSet: ReplicaSet;
  private readonly log: ReplicationLog<T>;
  private config: SyncConfig;
  private conflictResolver: ConflictResolver<T>;
  private pendingChanges: ChangeEvent<T>[] = [];
  private syncTimer: ReturnType<typeof setInterval> | null = null;

  constructor(
    replicaSet: ReplicaSet,
    log: ReplicationLog<T>,
    config?: Partial<SyncConfig>,
  ) {
    super();
    this.replicaSet = replicaSet;
    this.log = log;
    this.config = { ...DEFAULT_SYNC_CONFIG, ...config };
    // Default to timestamp-based resolution
    this.conflictResolver = new LastWriteWins() as unknown as ConflictResolver<T>;
  }

  /** Set sync mode */
  setSyncMode(mode: SyncMode, minReplicas?: number): void {
    this.config.mode = mode;
    if (minReplicas !== undefined) {
      this.config.minReplicas = minReplicas;
    }
  }

  /** Set custom conflict resolver */
  setConflictResolver(resolver: ConflictResolver<T>): void {
    this.conflictResolver = resolver;
  }

  /** Record a change for replication */
  async recordChange(
    key: string,
    operation: ChangeOperation,
    value?: T,
    previousValue?: T,
  ): Promise<void> {
    const primary = this.replicaSet.primary;
    if (!primary) {
      throw ReplicationError.noPrimary();
    }

    const entry = this.log.append({ key, operation, value, previousValue } as unknown as T);

    const change: ChangeEvent<T> = {
      id: entry.id,
      operation,
      key,
      value,
      previousValue,
      timestamp: entry.timestamp,
      sourceReplica: primary.id,
      vectorClock: entry.vectorClock,
    };

    this.emit(ReplicationEvent.ChangeReceived, change);

    // Handle based on sync mode
    switch (this.config.mode) {
      case SyncMode.Synchronous:
        await this.syncAll(change);
        break;
      case SyncMode.SemiSync:
        await this.syncMinimum(change);
        break;
      case SyncMode.Asynchronous:
        this.pendingChanges.push(change);
        break;
    }
  }

  /** Sync a change to all replicas */
  private async syncAll(change: ChangeEvent<T>): Promise<void> {
    const secondaries = this.replicaSet.secondaries;
    if (secondaries.length === 0) return;

    this.emit(ReplicationEvent.SyncStarted, { replicas: secondaries.map((r) => r.id) });

    // In a real implementation, this would send to all replicas
    // For now, we just emit the completion event
    this.emit(ReplicationEvent.SyncCompleted, { change, replicas: secondaries.map((r) => r.id) });
  }

  /** Sync to minimum number of replicas (semi-sync) */
  private async syncMinimum(change: ChangeEvent<T>): Promise<void> {
    const minReplicas = this.config.minReplicas ?? 1;
    const secondaries = this.replicaSet.secondaries;

    if (secondaries.length < minReplicas) {
      throw ReplicationError.quorumNotMet(minReplicas, secondaries.length);
    }

    // Sync to minimum number of replicas
    const targetReplicas = secondaries.slice(0, minReplicas);
    this.emit(ReplicationEvent.SyncStarted, { replicas: targetReplicas.map((r) => r.id) });

    // In a real implementation, this would wait for acknowledgments
    this.emit(ReplicationEvent.SyncCompleted, { change, replicas: targetReplicas.map((r) => r.id) });
  }

  /** Start background sync for async mode */
  startBackgroundSync(interval: number = 1000): void {
    if (this.syncTimer) return;

    this.syncTimer = setInterval(async () => {
      if (this.pendingChanges.length > 0) {
        const batch = this.pendingChanges.splice(0, this.config.batchSize);
        for (const change of batch) {
          await this.syncAll(change);
        }
      }
    }, interval);
  }

  /** Stop background sync */
  stopBackgroundSync(): void {
    if (this.syncTimer) {
      clearInterval(this.syncTimer);
      this.syncTimer = null;
    }
  }

  /** Resolve a conflict between local and remote values */
  resolveConflict(
    local: T,
    remote: T,
    localClock: VectorClock,
    remoteClock: VectorClock,
  ): T {
    // Check for causal relationship
    if (localClock.happensBefore(remoteClock)) {
      return remote; // Remote is newer
    } else if (localClock.happensAfter(remoteClock)) {
      return local; // Local is newer
    }

    // Concurrent - need conflict resolution
    this.emit(ReplicationEvent.ConflictDetected, { local, remote });
    const resolved = this.conflictResolver.resolve(local, remote, localClock, remoteClock);
    this.emit(ReplicationEvent.ConflictResolved, { local, remote, resolved });

    return resolved;
  }

  /** Get sync statistics */
  getStats(): {
    pendingChanges: number;
    lastSequence: number;
    syncMode: SyncMode;
  } {
    return {
      pendingChanges: this.pendingChanges.length,
      lastSequence: this.log.currentSequence,
      syncMode: this.config.mode,
    };
  }
}

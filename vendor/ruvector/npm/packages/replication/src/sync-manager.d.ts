/**
 * Sync Manager Implementation
 * Manages data synchronization across replicas
 */
import EventEmitter from 'eventemitter3';
import { type ReplicaId, type SyncConfig, type LogEntry, SyncMode, ChangeOperation } from './types.js';
import { VectorClock, type ConflictResolver } from './vector-clock.js';
import type { ReplicaSet } from './replica-set.js';
/** Replication log for tracking changes */
export declare class ReplicationLog<T = unknown> {
    private entries;
    private sequence;
    private readonly replicaId;
    private vectorClock;
    constructor(replicaId: ReplicaId);
    /** Get the current sequence number */
    get currentSequence(): number;
    /** Get the current vector clock */
    get clock(): VectorClock;
    /** Append an entry to the log */
    append(data: T): LogEntry<T>;
    /** Get entries since a sequence number */
    getEntriesSince(sequence: number, limit?: number): LogEntry<T>[];
    /** Get entry by ID */
    getEntry(id: string): LogEntry<T> | undefined;
    /** Get all entries */
    getAllEntries(): LogEntry<T>[];
    /** Apply entries from another replica */
    applyEntries(entries: LogEntry<T>[]): void;
    /** Clear the log */
    clear(): void;
}
/** Manages synchronization across replicas */
export declare class SyncManager<T = unknown> extends EventEmitter {
    private readonly replicaSet;
    private readonly log;
    private config;
    private conflictResolver;
    private pendingChanges;
    private syncTimer;
    constructor(replicaSet: ReplicaSet, log: ReplicationLog<T>, config?: Partial<SyncConfig>);
    /** Set sync mode */
    setSyncMode(mode: SyncMode, minReplicas?: number): void;
    /** Set custom conflict resolver */
    setConflictResolver(resolver: ConflictResolver<T>): void;
    /** Record a change for replication */
    recordChange(key: string, operation: ChangeOperation, value?: T, previousValue?: T): Promise<void>;
    /** Sync a change to all replicas */
    private syncAll;
    /** Sync to minimum number of replicas (semi-sync) */
    private syncMinimum;
    /** Start background sync for async mode */
    startBackgroundSync(interval?: number): void;
    /** Stop background sync */
    stopBackgroundSync(): void;
    /** Resolve a conflict between local and remote values */
    resolveConflict(local: T, remote: T, localClock: VectorClock, remoteClock: VectorClock): T;
    /** Get sync statistics */
    getStats(): {
        pendingChanges: number;
        lastSequence: number;
        syncMode: SyncMode;
    };
}
//# sourceMappingURL=sync-manager.d.ts.map
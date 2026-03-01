"use strict";
/**
 * Sync Manager Implementation
 * Manages data synchronization across replicas
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.SyncManager = exports.ReplicationLog = void 0;
const eventemitter3_1 = __importDefault(require("eventemitter3"));
const types_js_1 = require("./types.js");
const vector_clock_js_1 = require("./vector-clock.js");
/** Default sync configuration */
const DEFAULT_SYNC_CONFIG = {
    mode: types_js_1.SyncMode.Asynchronous,
    batchSize: 100,
    maxLag: 5000,
};
/** Replication log for tracking changes */
class ReplicationLog {
    constructor(replicaId) {
        this.entries = [];
        this.sequence = 0;
        this.replicaId = replicaId;
        this.vectorClock = new vector_clock_js_1.VectorClock();
    }
    /** Get the current sequence number */
    get currentSequence() {
        return this.sequence;
    }
    /** Get the current vector clock */
    get clock() {
        return this.vectorClock.clone();
    }
    /** Append an entry to the log */
    append(data) {
        this.sequence++;
        this.vectorClock.increment(this.replicaId);
        const entry = {
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
    getEntriesSince(sequence, limit) {
        const filtered = this.entries.filter((e) => e.sequence > sequence);
        return limit ? filtered.slice(0, limit) : filtered;
    }
    /** Get entry by ID */
    getEntry(id) {
        return this.entries.find((e) => e.id === id);
    }
    /** Get all entries */
    getAllEntries() {
        return [...this.entries];
    }
    /** Apply entries from another replica */
    applyEntries(entries) {
        for (const entry of entries) {
            const entryClock = new vector_clock_js_1.VectorClock(entry.vectorClock);
            this.vectorClock.merge(entryClock);
        }
        // Note: In a real implementation, entries would be merged properly
    }
    /** Clear the log */
    clear() {
        this.entries = [];
        this.sequence = 0;
        this.vectorClock = new vector_clock_js_1.VectorClock();
    }
}
exports.ReplicationLog = ReplicationLog;
/** Manages synchronization across replicas */
class SyncManager extends eventemitter3_1.default {
    constructor(replicaSet, log, config) {
        super();
        this.pendingChanges = [];
        this.syncTimer = null;
        this.replicaSet = replicaSet;
        this.log = log;
        this.config = { ...DEFAULT_SYNC_CONFIG, ...config };
        // Default to timestamp-based resolution
        this.conflictResolver = new vector_clock_js_1.LastWriteWins();
    }
    /** Set sync mode */
    setSyncMode(mode, minReplicas) {
        this.config.mode = mode;
        if (minReplicas !== undefined) {
            this.config.minReplicas = minReplicas;
        }
    }
    /** Set custom conflict resolver */
    setConflictResolver(resolver) {
        this.conflictResolver = resolver;
    }
    /** Record a change for replication */
    async recordChange(key, operation, value, previousValue) {
        const primary = this.replicaSet.primary;
        if (!primary) {
            throw types_js_1.ReplicationError.noPrimary();
        }
        const entry = this.log.append({ key, operation, value, previousValue });
        const change = {
            id: entry.id,
            operation,
            key,
            value,
            previousValue,
            timestamp: entry.timestamp,
            sourceReplica: primary.id,
            vectorClock: entry.vectorClock,
        };
        this.emit(types_js_1.ReplicationEvent.ChangeReceived, change);
        // Handle based on sync mode
        switch (this.config.mode) {
            case types_js_1.SyncMode.Synchronous:
                await this.syncAll(change);
                break;
            case types_js_1.SyncMode.SemiSync:
                await this.syncMinimum(change);
                break;
            case types_js_1.SyncMode.Asynchronous:
                this.pendingChanges.push(change);
                break;
        }
    }
    /** Sync a change to all replicas */
    async syncAll(change) {
        const secondaries = this.replicaSet.secondaries;
        if (secondaries.length === 0)
            return;
        this.emit(types_js_1.ReplicationEvent.SyncStarted, { replicas: secondaries.map((r) => r.id) });
        // In a real implementation, this would send to all replicas
        // For now, we just emit the completion event
        this.emit(types_js_1.ReplicationEvent.SyncCompleted, { change, replicas: secondaries.map((r) => r.id) });
    }
    /** Sync to minimum number of replicas (semi-sync) */
    async syncMinimum(change) {
        const minReplicas = this.config.minReplicas ?? 1;
        const secondaries = this.replicaSet.secondaries;
        if (secondaries.length < minReplicas) {
            throw types_js_1.ReplicationError.quorumNotMet(minReplicas, secondaries.length);
        }
        // Sync to minimum number of replicas
        const targetReplicas = secondaries.slice(0, minReplicas);
        this.emit(types_js_1.ReplicationEvent.SyncStarted, { replicas: targetReplicas.map((r) => r.id) });
        // In a real implementation, this would wait for acknowledgments
        this.emit(types_js_1.ReplicationEvent.SyncCompleted, { change, replicas: targetReplicas.map((r) => r.id) });
    }
    /** Start background sync for async mode */
    startBackgroundSync(interval = 1000) {
        if (this.syncTimer)
            return;
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
    stopBackgroundSync() {
        if (this.syncTimer) {
            clearInterval(this.syncTimer);
            this.syncTimer = null;
        }
    }
    /** Resolve a conflict between local and remote values */
    resolveConflict(local, remote, localClock, remoteClock) {
        // Check for causal relationship
        if (localClock.happensBefore(remoteClock)) {
            return remote; // Remote is newer
        }
        else if (localClock.happensAfter(remoteClock)) {
            return local; // Local is newer
        }
        // Concurrent - need conflict resolution
        this.emit(types_js_1.ReplicationEvent.ConflictDetected, { local, remote });
        const resolved = this.conflictResolver.resolve(local, remote, localClock, remoteClock);
        this.emit(types_js_1.ReplicationEvent.ConflictResolved, { local, remote, resolved });
        return resolved;
    }
    /** Get sync statistics */
    getStats() {
        return {
            pendingChanges: this.pendingChanges.length,
            lastSequence: this.log.currentSequence,
            syncMode: this.config.mode,
        };
    }
}
exports.SyncManager = SyncManager;
//# sourceMappingURL=sync-manager.js.map
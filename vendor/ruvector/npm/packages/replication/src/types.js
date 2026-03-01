"use strict";
/**
 * Replication Types
 * Data replication and synchronization types
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.ReplicationEvent = exports.ReplicationErrorCode = exports.ReplicationError = exports.FailoverPolicy = exports.ChangeOperation = exports.HealthStatus = exports.SyncMode = exports.ReplicaStatus = exports.ReplicaRole = void 0;
/** Role of a replica in the set */
var ReplicaRole;
(function (ReplicaRole) {
    ReplicaRole["Primary"] = "primary";
    ReplicaRole["Secondary"] = "secondary";
    ReplicaRole["Arbiter"] = "arbiter";
})(ReplicaRole || (exports.ReplicaRole = ReplicaRole = {}));
/** Status of a replica */
var ReplicaStatus;
(function (ReplicaStatus) {
    ReplicaStatus["Active"] = "active";
    ReplicaStatus["Syncing"] = "syncing";
    ReplicaStatus["Offline"] = "offline";
    ReplicaStatus["Failed"] = "failed";
})(ReplicaStatus || (exports.ReplicaStatus = ReplicaStatus = {}));
/** Synchronization mode */
var SyncMode;
(function (SyncMode) {
    /** All replicas must confirm before commit */
    SyncMode["Synchronous"] = "synchronous";
    /** Commit immediately, replicate in background */
    SyncMode["Asynchronous"] = "asynchronous";
    /** Wait for minimum number of replicas */
    SyncMode["SemiSync"] = "semi-sync";
})(SyncMode || (exports.SyncMode = SyncMode = {}));
/** Health status of a replica */
var HealthStatus;
(function (HealthStatus) {
    HealthStatus["Healthy"] = "healthy";
    HealthStatus["Degraded"] = "degraded";
    HealthStatus["Unhealthy"] = "unhealthy";
    HealthStatus["Unknown"] = "unknown";
})(HealthStatus || (exports.HealthStatus = HealthStatus = {}));
/** Change operation type */
var ChangeOperation;
(function (ChangeOperation) {
    ChangeOperation["Insert"] = "insert";
    ChangeOperation["Update"] = "update";
    ChangeOperation["Delete"] = "delete";
})(ChangeOperation || (exports.ChangeOperation = ChangeOperation = {}));
/** Failover policy */
var FailoverPolicy;
(function (FailoverPolicy) {
    /** Automatic failover with quorum */
    FailoverPolicy["Automatic"] = "automatic";
    /** Manual intervention required */
    FailoverPolicy["Manual"] = "manual";
    /** Priority-based failover */
    FailoverPolicy["Priority"] = "priority";
})(FailoverPolicy || (exports.FailoverPolicy = FailoverPolicy = {}));
/** Replication error types */
class ReplicationError extends Error {
    constructor(message, code) {
        super(message);
        this.code = code;
        this.name = 'ReplicationError';
    }
    static replicaNotFound(id) {
        return new ReplicationError(`Replica not found: ${id}`, ReplicationErrorCode.ReplicaNotFound);
    }
    static noPrimary() {
        return new ReplicationError('No primary replica available', ReplicationErrorCode.NoPrimary);
    }
    static timeout(operation) {
        return new ReplicationError(`Replication timeout: ${operation}`, ReplicationErrorCode.Timeout);
    }
    static quorumNotMet(needed, available) {
        return new ReplicationError(`Quorum not met: needed ${needed}, got ${available}`, ReplicationErrorCode.QuorumNotMet);
    }
    static splitBrain() {
        return new ReplicationError('Split-brain detected', ReplicationErrorCode.SplitBrain);
    }
    static conflictResolution(reason) {
        return new ReplicationError(`Conflict resolution failed: ${reason}`, ReplicationErrorCode.ConflictResolution);
    }
}
exports.ReplicationError = ReplicationError;
var ReplicationErrorCode;
(function (ReplicationErrorCode) {
    ReplicationErrorCode["ReplicaNotFound"] = "REPLICA_NOT_FOUND";
    ReplicationErrorCode["NoPrimary"] = "NO_PRIMARY";
    ReplicationErrorCode["Timeout"] = "TIMEOUT";
    ReplicationErrorCode["SyncFailed"] = "SYNC_FAILED";
    ReplicationErrorCode["ConflictResolution"] = "CONFLICT_RESOLUTION";
    ReplicationErrorCode["FailoverFailed"] = "FAILOVER_FAILED";
    ReplicationErrorCode["Network"] = "NETWORK";
    ReplicationErrorCode["QuorumNotMet"] = "QUORUM_NOT_MET";
    ReplicationErrorCode["SplitBrain"] = "SPLIT_BRAIN";
    ReplicationErrorCode["InvalidState"] = "INVALID_STATE";
})(ReplicationErrorCode || (exports.ReplicationErrorCode = ReplicationErrorCode = {}));
/** Events emitted by replication components */
var ReplicationEvent;
(function (ReplicationEvent) {
    ReplicationEvent["ReplicaAdded"] = "replicaAdded";
    ReplicationEvent["ReplicaRemoved"] = "replicaRemoved";
    ReplicationEvent["ReplicaStatusChanged"] = "replicaStatusChanged";
    ReplicationEvent["PrimaryChanged"] = "primaryChanged";
    ReplicationEvent["ChangeReceived"] = "changeReceived";
    ReplicationEvent["SyncStarted"] = "syncStarted";
    ReplicationEvent["SyncCompleted"] = "syncCompleted";
    ReplicationEvent["ConflictDetected"] = "conflictDetected";
    ReplicationEvent["ConflictResolved"] = "conflictResolved";
    ReplicationEvent["FailoverStarted"] = "failoverStarted";
    ReplicationEvent["FailoverCompleted"] = "failoverCompleted";
    ReplicationEvent["Error"] = "error";
})(ReplicationEvent || (exports.ReplicationEvent = ReplicationEvent = {}));
//# sourceMappingURL=types.js.map
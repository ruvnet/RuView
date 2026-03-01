"use strict";
/**
 * Replica Set Management
 * Manages a set of replicas for distributed data storage
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.ReplicaSet = void 0;
const eventemitter3_1 = __importDefault(require("eventemitter3"));
const types_js_1 = require("./types.js");
/** Default configuration */
const DEFAULT_CONFIG = {
    name: 'default',
    minQuorum: 2,
    heartbeatInterval: 1000,
    healthCheckTimeout: 5000,
    failoverPolicy: types_js_1.FailoverPolicy.Automatic,
};
/** Manages a set of replicas */
class ReplicaSet extends eventemitter3_1.default {
    constructor(name, config) {
        super();
        this.replicas = new Map();
        this.heartbeatTimer = null;
        this.config = { ...DEFAULT_CONFIG, name, ...config };
    }
    /** Get replica set name */
    get name() {
        return this.config.name;
    }
    /** Get the primary replica */
    get primary() {
        for (const replica of this.replicas.values()) {
            if (replica.role === types_js_1.ReplicaRole.Primary && replica.status === types_js_1.ReplicaStatus.Active) {
                return replica;
            }
        }
        return undefined;
    }
    /** Get all secondary replicas */
    get secondaries() {
        return Array.from(this.replicas.values()).filter((r) => r.role === types_js_1.ReplicaRole.Secondary && r.status === types_js_1.ReplicaStatus.Active);
    }
    /** Get all active replicas */
    get activeReplicas() {
        return Array.from(this.replicas.values()).filter((r) => r.status === types_js_1.ReplicaStatus.Active);
    }
    /** Get replica count */
    get size() {
        return this.replicas.size;
    }
    /** Check if quorum is met */
    get hasQuorum() {
        const activeCount = this.activeReplicas.length;
        return activeCount >= this.config.minQuorum;
    }
    /** Add a replica to the set */
    addReplica(id, address, role) {
        if (this.replicas.has(id)) {
            throw new Error(`Replica ${id} already exists`);
        }
        // Check if adding a primary when one exists
        if (role === types_js_1.ReplicaRole.Primary && this.primary) {
            throw new Error('Primary already exists in replica set');
        }
        const replica = {
            id,
            address,
            role,
            status: types_js_1.ReplicaStatus.Active,
            lastSeen: Date.now(),
            lag: 0,
        };
        this.replicas.set(id, replica);
        this.emit(types_js_1.ReplicationEvent.ReplicaAdded, replica);
        return replica;
    }
    /** Remove a replica from the set */
    removeReplica(id) {
        const replica = this.replicas.get(id);
        if (!replica)
            return false;
        this.replicas.delete(id);
        this.emit(types_js_1.ReplicationEvent.ReplicaRemoved, replica);
        // If primary was removed, trigger failover
        if (replica.role === types_js_1.ReplicaRole.Primary && this.config.failoverPolicy === types_js_1.FailoverPolicy.Automatic) {
            this.triggerFailover();
        }
        return true;
    }
    /** Get a replica by ID */
    getReplica(id) {
        return this.replicas.get(id);
    }
    /** Update replica status */
    updateStatus(id, status) {
        const replica = this.replicas.get(id);
        if (!replica) {
            throw types_js_1.ReplicationError.replicaNotFound(id);
        }
        const previousStatus = replica.status;
        replica.status = status;
        replica.lastSeen = Date.now();
        if (previousStatus !== status) {
            this.emit(types_js_1.ReplicationEvent.ReplicaStatusChanged, {
                replica,
                previousStatus,
                newStatus: status,
            });
            // Check for failover conditions
            if (replica.role === types_js_1.ReplicaRole.Primary &&
                status === types_js_1.ReplicaStatus.Failed &&
                this.config.failoverPolicy === types_js_1.FailoverPolicy.Automatic) {
                this.triggerFailover();
            }
        }
    }
    /** Update replica lag */
    updateLag(id, lag) {
        const replica = this.replicas.get(id);
        if (replica) {
            replica.lag = lag;
            replica.lastSeen = Date.now();
        }
    }
    /** Promote a secondary to primary */
    promote(id) {
        const replica = this.replicas.get(id);
        if (!replica) {
            throw types_js_1.ReplicationError.replicaNotFound(id);
        }
        if (replica.role === types_js_1.ReplicaRole.Primary) {
            return; // Already primary
        }
        // Demote current primary
        const currentPrimary = this.primary;
        if (currentPrimary) {
            currentPrimary.role = types_js_1.ReplicaRole.Secondary;
        }
        // Promote new primary
        replica.role = types_js_1.ReplicaRole.Primary;
        this.emit(types_js_1.ReplicationEvent.PrimaryChanged, {
            previousPrimary: currentPrimary?.id,
            newPrimary: id,
        });
    }
    /** Trigger automatic failover */
    triggerFailover() {
        this.emit(types_js_1.ReplicationEvent.FailoverStarted, {});
        // Find the best candidate (lowest lag, active secondary)
        const candidates = this.secondaries
            .filter((r) => r.status === types_js_1.ReplicaStatus.Active)
            .sort((a, b) => a.lag - b.lag);
        if (candidates.length === 0) {
            this.emit(types_js_1.ReplicationEvent.Error, types_js_1.ReplicationError.noPrimary());
            return;
        }
        const newPrimary = candidates[0];
        this.promote(newPrimary.id);
        this.emit(types_js_1.ReplicationEvent.FailoverCompleted, { newPrimary: newPrimary.id });
    }
    /** Start heartbeat monitoring */
    startHeartbeat() {
        if (this.heartbeatTimer)
            return;
        this.heartbeatTimer = setInterval(() => {
            const now = Date.now();
            for (const replica of this.replicas.values()) {
                if (now - replica.lastSeen > this.config.healthCheckTimeout) {
                    if (replica.status === types_js_1.ReplicaStatus.Active) {
                        this.updateStatus(replica.id, types_js_1.ReplicaStatus.Offline);
                    }
                }
            }
        }, this.config.heartbeatInterval);
    }
    /** Stop heartbeat monitoring */
    stopHeartbeat() {
        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
            this.heartbeatTimer = null;
        }
    }
    /** Get all replicas */
    getAllReplicas() {
        return Array.from(this.replicas.values());
    }
    /** Get replica set stats */
    getStats() {
        const replicas = Array.from(this.replicas.values());
        return {
            total: replicas.length,
            active: replicas.filter((r) => r.status === types_js_1.ReplicaStatus.Active).length,
            syncing: replicas.filter((r) => r.status === types_js_1.ReplicaStatus.Syncing).length,
            offline: replicas.filter((r) => r.status === types_js_1.ReplicaStatus.Offline).length,
            failed: replicas.filter((r) => r.status === types_js_1.ReplicaStatus.Failed).length,
            hasQuorum: this.hasQuorum,
        };
    }
}
exports.ReplicaSet = ReplicaSet;
//# sourceMappingURL=replica-set.js.map
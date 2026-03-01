/**
 * Replica Set Management
 * Manages a set of replicas for distributed data storage
 */
import EventEmitter from 'eventemitter3';
import { type Replica, type ReplicaId, type ReplicaSetConfig, ReplicaRole, ReplicaStatus } from './types.js';
/** Manages a set of replicas */
export declare class ReplicaSet extends EventEmitter {
    private replicas;
    private config;
    private heartbeatTimer;
    constructor(name: string, config?: Partial<ReplicaSetConfig>);
    /** Get replica set name */
    get name(): string;
    /** Get the primary replica */
    get primary(): Replica | undefined;
    /** Get all secondary replicas */
    get secondaries(): Replica[];
    /** Get all active replicas */
    get activeReplicas(): Replica[];
    /** Get replica count */
    get size(): number;
    /** Check if quorum is met */
    get hasQuorum(): boolean;
    /** Add a replica to the set */
    addReplica(id: ReplicaId, address: string, role: ReplicaRole): Replica;
    /** Remove a replica from the set */
    removeReplica(id: ReplicaId): boolean;
    /** Get a replica by ID */
    getReplica(id: ReplicaId): Replica | undefined;
    /** Update replica status */
    updateStatus(id: ReplicaId, status: ReplicaStatus): void;
    /** Update replica lag */
    updateLag(id: ReplicaId, lag: number): void;
    /** Promote a secondary to primary */
    promote(id: ReplicaId): void;
    /** Trigger automatic failover */
    private triggerFailover;
    /** Start heartbeat monitoring */
    startHeartbeat(): void;
    /** Stop heartbeat monitoring */
    stopHeartbeat(): void;
    /** Get all replicas */
    getAllReplicas(): Replica[];
    /** Get replica set stats */
    getStats(): {
        total: number;
        active: number;
        syncing: number;
        offline: number;
        failed: number;
        hasQuorum: boolean;
    };
}
//# sourceMappingURL=replica-set.d.ts.map
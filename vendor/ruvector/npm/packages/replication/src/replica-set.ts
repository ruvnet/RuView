/**
 * Replica Set Management
 * Manages a set of replicas for distributed data storage
 */

import EventEmitter from 'eventemitter3';
import {
  type Replica,
  type ReplicaId,
  type ReplicaSetConfig,
  ReplicaRole,
  ReplicaStatus,
  ReplicationError,
  ReplicationEvent,
  FailoverPolicy,
} from './types.js';

/** Default configuration */
const DEFAULT_CONFIG: ReplicaSetConfig = {
  name: 'default',
  minQuorum: 2,
  heartbeatInterval: 1000,
  healthCheckTimeout: 5000,
  failoverPolicy: FailoverPolicy.Automatic,
};

/** Manages a set of replicas */
export class ReplicaSet extends EventEmitter {
  private replicas: Map<ReplicaId, Replica> = new Map();
  private config: ReplicaSetConfig;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;

  constructor(name: string, config?: Partial<ReplicaSetConfig>) {
    super();
    this.config = { ...DEFAULT_CONFIG, name, ...config };
  }

  /** Get replica set name */
  get name(): string {
    return this.config.name;
  }

  /** Get the primary replica */
  get primary(): Replica | undefined {
    for (const replica of this.replicas.values()) {
      if (replica.role === ReplicaRole.Primary && replica.status === ReplicaStatus.Active) {
        return replica;
      }
    }
    return undefined;
  }

  /** Get all secondary replicas */
  get secondaries(): Replica[] {
    return Array.from(this.replicas.values()).filter(
      (r) => r.role === ReplicaRole.Secondary && r.status === ReplicaStatus.Active,
    );
  }

  /** Get all active replicas */
  get activeReplicas(): Replica[] {
    return Array.from(this.replicas.values()).filter((r) => r.status === ReplicaStatus.Active);
  }

  /** Get replica count */
  get size(): number {
    return this.replicas.size;
  }

  /** Check if quorum is met */
  get hasQuorum(): boolean {
    const activeCount = this.activeReplicas.length;
    return activeCount >= this.config.minQuorum;
  }

  /** Add a replica to the set */
  addReplica(id: ReplicaId, address: string, role: ReplicaRole): Replica {
    if (this.replicas.has(id)) {
      throw new Error(`Replica ${id} already exists`);
    }

    // Check if adding a primary when one exists
    if (role === ReplicaRole.Primary && this.primary) {
      throw new Error('Primary already exists in replica set');
    }

    const replica: Replica = {
      id,
      address,
      role,
      status: ReplicaStatus.Active,
      lastSeen: Date.now(),
      lag: 0,
    };

    this.replicas.set(id, replica);
    this.emit(ReplicationEvent.ReplicaAdded, replica);

    return replica;
  }

  /** Remove a replica from the set */
  removeReplica(id: ReplicaId): boolean {
    const replica = this.replicas.get(id);
    if (!replica) return false;

    this.replicas.delete(id);
    this.emit(ReplicationEvent.ReplicaRemoved, replica);

    // If primary was removed, trigger failover
    if (replica.role === ReplicaRole.Primary && this.config.failoverPolicy === FailoverPolicy.Automatic) {
      this.triggerFailover();
    }

    return true;
  }

  /** Get a replica by ID */
  getReplica(id: ReplicaId): Replica | undefined {
    return this.replicas.get(id);
  }

  /** Update replica status */
  updateStatus(id: ReplicaId, status: ReplicaStatus): void {
    const replica = this.replicas.get(id);
    if (!replica) {
      throw ReplicationError.replicaNotFound(id);
    }

    const previousStatus = replica.status;
    replica.status = status;
    replica.lastSeen = Date.now();

    if (previousStatus !== status) {
      this.emit(ReplicationEvent.ReplicaStatusChanged, {
        replica,
        previousStatus,
        newStatus: status,
      });

      // Check for failover conditions
      if (
        replica.role === ReplicaRole.Primary &&
        status === ReplicaStatus.Failed &&
        this.config.failoverPolicy === FailoverPolicy.Automatic
      ) {
        this.triggerFailover();
      }
    }
  }

  /** Update replica lag */
  updateLag(id: ReplicaId, lag: number): void {
    const replica = this.replicas.get(id);
    if (replica) {
      replica.lag = lag;
      replica.lastSeen = Date.now();
    }
  }

  /** Promote a secondary to primary */
  promote(id: ReplicaId): void {
    const replica = this.replicas.get(id);
    if (!replica) {
      throw ReplicationError.replicaNotFound(id);
    }

    if (replica.role === ReplicaRole.Primary) {
      return; // Already primary
    }

    // Demote current primary
    const currentPrimary = this.primary;
    if (currentPrimary) {
      currentPrimary.role = ReplicaRole.Secondary;
    }

    // Promote new primary
    replica.role = ReplicaRole.Primary;
    this.emit(ReplicationEvent.PrimaryChanged, {
      previousPrimary: currentPrimary?.id,
      newPrimary: id,
    });
  }

  /** Trigger automatic failover */
  private triggerFailover(): void {
    this.emit(ReplicationEvent.FailoverStarted, {});

    // Find the best candidate (lowest lag, active secondary)
    const candidates = this.secondaries
      .filter((r) => r.status === ReplicaStatus.Active)
      .sort((a, b) => a.lag - b.lag);

    if (candidates.length === 0) {
      this.emit(ReplicationEvent.Error, ReplicationError.noPrimary());
      return;
    }

    const newPrimary = candidates[0];
    this.promote(newPrimary.id);

    this.emit(ReplicationEvent.FailoverCompleted, { newPrimary: newPrimary.id });
  }

  /** Start heartbeat monitoring */
  startHeartbeat(): void {
    if (this.heartbeatTimer) return;

    this.heartbeatTimer = setInterval(() => {
      const now = Date.now();
      for (const replica of this.replicas.values()) {
        if (now - replica.lastSeen > this.config.healthCheckTimeout) {
          if (replica.status === ReplicaStatus.Active) {
            this.updateStatus(replica.id, ReplicaStatus.Offline);
          }
        }
      }
    }, this.config.heartbeatInterval);
  }

  /** Stop heartbeat monitoring */
  stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  /** Get all replicas */
  getAllReplicas(): Replica[] {
    return Array.from(this.replicas.values());
  }

  /** Get replica set stats */
  getStats(): {
    total: number;
    active: number;
    syncing: number;
    offline: number;
    failed: number;
    hasQuorum: boolean;
  } {
    const replicas = Array.from(this.replicas.values());
    return {
      total: replicas.length,
      active: replicas.filter((r) => r.status === ReplicaStatus.Active).length,
      syncing: replicas.filter((r) => r.status === ReplicaStatus.Syncing).length,
      offline: replicas.filter((r) => r.status === ReplicaStatus.Offline).length,
      failed: replicas.filter((r) => r.status === ReplicaStatus.Failed).length,
      hasQuorum: this.hasQuorum,
    };
  }
}

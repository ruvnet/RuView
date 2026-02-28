/**
 * Vector Clock Implementation
 * For conflict detection and resolution in distributed systems
 */

import type { ReplicaId, LogicalClock, VectorClockValue } from './types.js';

/** Comparison result between vector clocks */
export enum VectorClockComparison {
  /** First happens before second */
  Before = 'before',
  /** First happens after second */
  After = 'after',
  /** Clocks are concurrent (no causal relationship) */
  Concurrent = 'concurrent',
  /** Clocks are equal */
  Equal = 'equal',
}

/** Vector clock for tracking causality in distributed systems */
export class VectorClock {
  private clock: Map<ReplicaId, LogicalClock>;

  constructor(initial?: VectorClockValue | Map<ReplicaId, LogicalClock>) {
    this.clock = new Map(initial);
  }

  /** Get the clock value for a replica */
  get(replicaId: ReplicaId): LogicalClock {
    return this.clock.get(replicaId) ?? 0;
  }

  /** Increment the clock for a replica */
  increment(replicaId: ReplicaId): void {
    const current = this.get(replicaId);
    this.clock.set(replicaId, current + 1);
  }

  /** Update with a received clock (merge) */
  merge(other: VectorClock): void {
    for (const [replicaId, otherTime] of other.clock) {
      const myTime = this.get(replicaId);
      this.clock.set(replicaId, Math.max(myTime, otherTime));
    }
  }

  /** Create a copy of this clock */
  clone(): VectorClock {
    return new VectorClock(new Map(this.clock));
  }

  /** Get the clock value as a Map */
  getValue(): VectorClockValue {
    return new Map(this.clock);
  }

  /** Compare two vector clocks */
  compare(other: VectorClock): VectorClockComparison {
    let isLess = false;
    let isGreater = false;

    // Get all unique replica IDs
    const allReplicas = new Set([...this.clock.keys(), ...other.clock.keys()]);

    for (const replicaId of allReplicas) {
      const myTime = this.get(replicaId);
      const otherTime = other.get(replicaId);

      if (myTime < otherTime) {
        isLess = true;
      } else if (myTime > otherTime) {
        isGreater = true;
      }
    }

    if (isLess && isGreater) {
      return VectorClockComparison.Concurrent;
    } else if (isLess) {
      return VectorClockComparison.Before;
    } else if (isGreater) {
      return VectorClockComparison.After;
    } else {
      return VectorClockComparison.Equal;
    }
  }

  /** Check if this clock happens before another */
  happensBefore(other: VectorClock): boolean {
    return this.compare(other) === VectorClockComparison.Before;
  }

  /** Check if this clock happens after another */
  happensAfter(other: VectorClock): boolean {
    return this.compare(other) === VectorClockComparison.After;
  }

  /** Check if clocks are concurrent (no causal relationship) */
  isConcurrent(other: VectorClock): boolean {
    return this.compare(other) === VectorClockComparison.Concurrent;
  }

  /** Serialize to JSON */
  toJSON(): Record<string, number> {
    const obj: Record<string, number> = {};
    for (const [key, value] of this.clock) {
      obj[key] = value;
    }
    return obj;
  }

  /** Create from JSON */
  static fromJSON(json: Record<string, number>): VectorClock {
    const clock = new VectorClock();
    for (const [key, value] of Object.entries(json)) {
      clock.clock.set(key, value);
    }
    return clock;
  }

  /** Create a new vector clock with a single entry */
  static single(replicaId: ReplicaId, time: LogicalClock = 1): VectorClock {
    const clock = new VectorClock();
    clock.clock.set(replicaId, time);
    return clock;
  }
}

/** Conflict resolver interface */
export interface ConflictResolver<T> {
  /** Resolve a conflict between two values */
  resolve(local: T, remote: T, localClock: VectorClock, remoteClock: VectorClock): T;
}

/** Last-write-wins conflict resolver */
export class LastWriteWins<T extends { timestamp: number }> implements ConflictResolver<T> {
  resolve(local: T, remote: T): T {
    return local.timestamp >= remote.timestamp ? local : remote;
  }
}

/** Custom merge function conflict resolver */
export class MergeFunction<T> implements ConflictResolver<T> {
  constructor(private mergeFn: (local: T, remote: T) => T) {}

  resolve(local: T, remote: T): T {
    return this.mergeFn(local, remote);
  }
}

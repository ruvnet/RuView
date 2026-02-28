/**
 * Vector Clock Implementation
 * For conflict detection and resolution in distributed systems
 */
import type { ReplicaId, LogicalClock, VectorClockValue } from './types.js';
/** Comparison result between vector clocks */
export declare enum VectorClockComparison {
    /** First happens before second */
    Before = "before",
    /** First happens after second */
    After = "after",
    /** Clocks are concurrent (no causal relationship) */
    Concurrent = "concurrent",
    /** Clocks are equal */
    Equal = "equal"
}
/** Vector clock for tracking causality in distributed systems */
export declare class VectorClock {
    private clock;
    constructor(initial?: VectorClockValue | Map<ReplicaId, LogicalClock>);
    /** Get the clock value for a replica */
    get(replicaId: ReplicaId): LogicalClock;
    /** Increment the clock for a replica */
    increment(replicaId: ReplicaId): void;
    /** Update with a received clock (merge) */
    merge(other: VectorClock): void;
    /** Create a copy of this clock */
    clone(): VectorClock;
    /** Get the clock value as a Map */
    getValue(): VectorClockValue;
    /** Compare two vector clocks */
    compare(other: VectorClock): VectorClockComparison;
    /** Check if this clock happens before another */
    happensBefore(other: VectorClock): boolean;
    /** Check if this clock happens after another */
    happensAfter(other: VectorClock): boolean;
    /** Check if clocks are concurrent (no causal relationship) */
    isConcurrent(other: VectorClock): boolean;
    /** Serialize to JSON */
    toJSON(): Record<string, number>;
    /** Create from JSON */
    static fromJSON(json: Record<string, number>): VectorClock;
    /** Create a new vector clock with a single entry */
    static single(replicaId: ReplicaId, time?: LogicalClock): VectorClock;
}
/** Conflict resolver interface */
export interface ConflictResolver<T> {
    /** Resolve a conflict between two values */
    resolve(local: T, remote: T, localClock: VectorClock, remoteClock: VectorClock): T;
}
/** Last-write-wins conflict resolver */
export declare class LastWriteWins<T extends {
    timestamp: number;
}> implements ConflictResolver<T> {
    resolve(local: T, remote: T): T;
}
/** Custom merge function conflict resolver */
export declare class MergeFunction<T> implements ConflictResolver<T> {
    private mergeFn;
    constructor(mergeFn: (local: T, remote: T) => T);
    resolve(local: T, remote: T): T;
}
//# sourceMappingURL=vector-clock.d.ts.map
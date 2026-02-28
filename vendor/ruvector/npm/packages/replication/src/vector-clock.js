"use strict";
/**
 * Vector Clock Implementation
 * For conflict detection and resolution in distributed systems
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.MergeFunction = exports.LastWriteWins = exports.VectorClock = exports.VectorClockComparison = void 0;
/** Comparison result between vector clocks */
var VectorClockComparison;
(function (VectorClockComparison) {
    /** First happens before second */
    VectorClockComparison["Before"] = "before";
    /** First happens after second */
    VectorClockComparison["After"] = "after";
    /** Clocks are concurrent (no causal relationship) */
    VectorClockComparison["Concurrent"] = "concurrent";
    /** Clocks are equal */
    VectorClockComparison["Equal"] = "equal";
})(VectorClockComparison || (exports.VectorClockComparison = VectorClockComparison = {}));
/** Vector clock for tracking causality in distributed systems */
class VectorClock {
    constructor(initial) {
        this.clock = new Map(initial);
    }
    /** Get the clock value for a replica */
    get(replicaId) {
        return this.clock.get(replicaId) ?? 0;
    }
    /** Increment the clock for a replica */
    increment(replicaId) {
        const current = this.get(replicaId);
        this.clock.set(replicaId, current + 1);
    }
    /** Update with a received clock (merge) */
    merge(other) {
        for (const [replicaId, otherTime] of other.clock) {
            const myTime = this.get(replicaId);
            this.clock.set(replicaId, Math.max(myTime, otherTime));
        }
    }
    /** Create a copy of this clock */
    clone() {
        return new VectorClock(new Map(this.clock));
    }
    /** Get the clock value as a Map */
    getValue() {
        return new Map(this.clock);
    }
    /** Compare two vector clocks */
    compare(other) {
        let isLess = false;
        let isGreater = false;
        // Get all unique replica IDs
        const allReplicas = new Set([...this.clock.keys(), ...other.clock.keys()]);
        for (const replicaId of allReplicas) {
            const myTime = this.get(replicaId);
            const otherTime = other.get(replicaId);
            if (myTime < otherTime) {
                isLess = true;
            }
            else if (myTime > otherTime) {
                isGreater = true;
            }
        }
        if (isLess && isGreater) {
            return VectorClockComparison.Concurrent;
        }
        else if (isLess) {
            return VectorClockComparison.Before;
        }
        else if (isGreater) {
            return VectorClockComparison.After;
        }
        else {
            return VectorClockComparison.Equal;
        }
    }
    /** Check if this clock happens before another */
    happensBefore(other) {
        return this.compare(other) === VectorClockComparison.Before;
    }
    /** Check if this clock happens after another */
    happensAfter(other) {
        return this.compare(other) === VectorClockComparison.After;
    }
    /** Check if clocks are concurrent (no causal relationship) */
    isConcurrent(other) {
        return this.compare(other) === VectorClockComparison.Concurrent;
    }
    /** Serialize to JSON */
    toJSON() {
        const obj = {};
        for (const [key, value] of this.clock) {
            obj[key] = value;
        }
        return obj;
    }
    /** Create from JSON */
    static fromJSON(json) {
        const clock = new VectorClock();
        for (const [key, value] of Object.entries(json)) {
            clock.clock.set(key, value);
        }
        return clock;
    }
    /** Create a new vector clock with a single entry */
    static single(replicaId, time = 1) {
        const clock = new VectorClock();
        clock.clock.set(replicaId, time);
        return clock;
    }
}
exports.VectorClock = VectorClock;
/** Last-write-wins conflict resolver */
class LastWriteWins {
    resolve(local, remote) {
        return local.timestamp >= remote.timestamp ? local : remote;
    }
}
exports.LastWriteWins = LastWriteWins;
/** Custom merge function conflict resolver */
class MergeFunction {
    constructor(mergeFn) {
        this.mergeFn = mergeFn;
    }
    resolve(local, remote) {
        return this.mergeFn(local, remote);
    }
}
exports.MergeFunction = MergeFunction;
//# sourceMappingURL=vector-clock.js.map
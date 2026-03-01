"use strict";
/**
 * Raft State Management
 * Manages persistent and volatile state for Raft consensus
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.RaftState = void 0;
const log_js_1 = require("./log.js");
/** State manager for a Raft node */
class RaftState {
    constructor(nodeId, peers, options) {
        this.nodeId = nodeId;
        this.peers = peers;
        this._currentTerm = 0;
        this._votedFor = null;
        this._commitIndex = 0;
        this._lastApplied = 0;
        this._leaderState = null;
        this.log = new log_js_1.RaftLog({ onPersist: options?.onLogPersist });
        this.persistCallback = options?.onPersist;
    }
    /** Get current term */
    get currentTerm() {
        return this._currentTerm;
    }
    /** Get voted for */
    get votedFor() {
        return this._votedFor;
    }
    /** Get commit index */
    get commitIndex() {
        return this._commitIndex;
    }
    /** Get last applied */
    get lastApplied() {
        return this._lastApplied;
    }
    /** Get leader state (null if not leader) */
    get leaderState() {
        return this._leaderState;
    }
    /** Update term (with persistence) */
    async setTerm(term) {
        if (term > this._currentTerm) {
            this._currentTerm = term;
            this._votedFor = null;
            await this.persist();
        }
    }
    /** Record vote (with persistence) */
    async vote(term, candidateId) {
        this._currentTerm = term;
        this._votedFor = candidateId;
        await this.persist();
    }
    /** Update commit index */
    setCommitIndex(index) {
        if (index > this._commitIndex) {
            this._commitIndex = index;
        }
    }
    /** Update last applied */
    setLastApplied(index) {
        if (index > this._lastApplied) {
            this._lastApplied = index;
        }
    }
    /** Initialize leader state */
    initLeaderState() {
        const nextIndex = new Map();
        const matchIndex = new Map();
        for (const peer of this.peers) {
            // Initialize nextIndex to leader's last log index + 1
            nextIndex.set(peer, this.log.lastIndex + 1);
            // Initialize matchIndex to 0
            matchIndex.set(peer, 0);
        }
        this._leaderState = { nextIndex, matchIndex };
    }
    /** Clear leader state */
    clearLeaderState() {
        this._leaderState = null;
    }
    /** Update nextIndex for a peer */
    setNextIndex(peerId, index) {
        if (this._leaderState) {
            this._leaderState.nextIndex.set(peerId, Math.max(1, index));
        }
    }
    /** Update matchIndex for a peer */
    setMatchIndex(peerId, index) {
        if (this._leaderState) {
            this._leaderState.matchIndex.set(peerId, index);
        }
    }
    /** Get nextIndex for a peer */
    getNextIndex(peerId) {
        return this._leaderState?.nextIndex.get(peerId) ?? this.log.lastIndex + 1;
    }
    /** Get matchIndex for a peer */
    getMatchIndex(peerId) {
        return this._leaderState?.matchIndex.get(peerId) ?? 0;
    }
    /** Update commit index based on match indices (for leader) */
    updateCommitIndex() {
        if (!this._leaderState)
            return false;
        // Find the highest index N such that a majority have matchIndex >= N
        // and log[N].term == currentTerm
        const matchIndices = Array.from(this._leaderState.matchIndex.values());
        matchIndices.push(this.log.lastIndex); // Include self
        matchIndices.sort((a, b) => b - a); // Sort descending
        const majority = Math.floor((this.peers.length + 1) / 2) + 1;
        for (const index of matchIndices) {
            if (index <= this._commitIndex)
                break;
            const term = this.log.termAt(index);
            if (term === this._currentTerm) {
                // Count how many have this index or higher
                const count = matchIndices.filter((m) => m >= index).length + 1; // +1 for self
                if (count >= majority) {
                    this._commitIndex = index;
                    return true;
                }
            }
        }
        return false;
    }
    /** Get persistent state */
    getPersistentState() {
        return {
            currentTerm: this._currentTerm,
            votedFor: this._votedFor,
            log: this.log.getAll(),
        };
    }
    /** Get volatile state */
    getVolatileState() {
        return {
            commitIndex: this._commitIndex,
            lastApplied: this._lastApplied,
        };
    }
    /** Load persistent state */
    loadPersistentState(state) {
        this._currentTerm = state.currentTerm;
        this._votedFor = state.votedFor;
        this.log.load(state.log);
    }
    /** Persist state */
    async persist() {
        if (this.persistCallback) {
            await this.persistCallback(this.getPersistentState());
        }
    }
}
exports.RaftState = RaftState;
//# sourceMappingURL=state.js.map
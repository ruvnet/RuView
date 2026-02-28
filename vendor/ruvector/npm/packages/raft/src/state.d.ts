/**
 * Raft State Management
 * Manages persistent and volatile state for Raft consensus
 */
import type { NodeId, Term, LogIndex, PersistentState, VolatileState, LeaderState, LogEntry } from './types.js';
import { RaftLog } from './log.js';
/** State manager for a Raft node */
export declare class RaftState<T = unknown> {
    private readonly nodeId;
    private readonly peers;
    private _currentTerm;
    private _votedFor;
    private _commitIndex;
    private _lastApplied;
    private _leaderState;
    readonly log: RaftLog<T>;
    constructor(nodeId: NodeId, peers: NodeId[], options?: {
        onPersist?: (state: PersistentState<T>) => Promise<void>;
        onLogPersist?: (entries: LogEntry<T>[]) => Promise<void>;
    });
    private persistCallback?;
    /** Get current term */
    get currentTerm(): Term;
    /** Get voted for */
    get votedFor(): NodeId | null;
    /** Get commit index */
    get commitIndex(): LogIndex;
    /** Get last applied */
    get lastApplied(): LogIndex;
    /** Get leader state (null if not leader) */
    get leaderState(): LeaderState | null;
    /** Update term (with persistence) */
    setTerm(term: Term): Promise<void>;
    /** Record vote (with persistence) */
    vote(term: Term, candidateId: NodeId): Promise<void>;
    /** Update commit index */
    setCommitIndex(index: LogIndex): void;
    /** Update last applied */
    setLastApplied(index: LogIndex): void;
    /** Initialize leader state */
    initLeaderState(): void;
    /** Clear leader state */
    clearLeaderState(): void;
    /** Update nextIndex for a peer */
    setNextIndex(peerId: NodeId, index: LogIndex): void;
    /** Update matchIndex for a peer */
    setMatchIndex(peerId: NodeId, index: LogIndex): void;
    /** Get nextIndex for a peer */
    getNextIndex(peerId: NodeId): LogIndex;
    /** Get matchIndex for a peer */
    getMatchIndex(peerId: NodeId): LogIndex;
    /** Update commit index based on match indices (for leader) */
    updateCommitIndex(): boolean;
    /** Get persistent state */
    getPersistentState(): PersistentState<T>;
    /** Get volatile state */
    getVolatileState(): VolatileState;
    /** Load persistent state */
    loadPersistentState(state: PersistentState<T>): void;
    /** Persist state */
    private persist;
}
//# sourceMappingURL=state.d.ts.map
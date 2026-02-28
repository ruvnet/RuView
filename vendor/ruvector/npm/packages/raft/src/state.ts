/**
 * Raft State Management
 * Manages persistent and volatile state for Raft consensus
 */

import type {
  NodeId,
  Term,
  LogIndex,
  PersistentState,
  VolatileState,
  LeaderState,
  LogEntry,
} from './types.js';
import { RaftLog } from './log.js';

/** State manager for a Raft node */
export class RaftState<T = unknown> {
  private _currentTerm: Term = 0;
  private _votedFor: NodeId | null = null;
  private _commitIndex: LogIndex = 0;
  private _lastApplied: LogIndex = 0;
  private _leaderState: LeaderState | null = null;

  public readonly log: RaftLog<T>;

  constructor(
    private readonly nodeId: NodeId,
    private readonly peers: NodeId[],
    options?: {
      onPersist?: (state: PersistentState<T>) => Promise<void>;
      onLogPersist?: (entries: LogEntry<T>[]) => Promise<void>;
    },
  ) {
    this.log = new RaftLog({ onPersist: options?.onLogPersist });
    this.persistCallback = options?.onPersist;
  }

  private persistCallback?: (state: PersistentState<T>) => Promise<void>;

  /** Get current term */
  get currentTerm(): Term {
    return this._currentTerm;
  }

  /** Get voted for */
  get votedFor(): NodeId | null {
    return this._votedFor;
  }

  /** Get commit index */
  get commitIndex(): LogIndex {
    return this._commitIndex;
  }

  /** Get last applied */
  get lastApplied(): LogIndex {
    return this._lastApplied;
  }

  /** Get leader state (null if not leader) */
  get leaderState(): LeaderState | null {
    return this._leaderState;
  }

  /** Update term (with persistence) */
  async setTerm(term: Term): Promise<void> {
    if (term > this._currentTerm) {
      this._currentTerm = term;
      this._votedFor = null;
      await this.persist();
    }
  }

  /** Record vote (with persistence) */
  async vote(term: Term, candidateId: NodeId): Promise<void> {
    this._currentTerm = term;
    this._votedFor = candidateId;
    await this.persist();
  }

  /** Update commit index */
  setCommitIndex(index: LogIndex): void {
    if (index > this._commitIndex) {
      this._commitIndex = index;
    }
  }

  /** Update last applied */
  setLastApplied(index: LogIndex): void {
    if (index > this._lastApplied) {
      this._lastApplied = index;
    }
  }

  /** Initialize leader state */
  initLeaderState(): void {
    const nextIndex = new Map<NodeId, LogIndex>();
    const matchIndex = new Map<NodeId, LogIndex>();

    for (const peer of this.peers) {
      // Initialize nextIndex to leader's last log index + 1
      nextIndex.set(peer, this.log.lastIndex + 1);
      // Initialize matchIndex to 0
      matchIndex.set(peer, 0);
    }

    this._leaderState = { nextIndex, matchIndex };
  }

  /** Clear leader state */
  clearLeaderState(): void {
    this._leaderState = null;
  }

  /** Update nextIndex for a peer */
  setNextIndex(peerId: NodeId, index: LogIndex): void {
    if (this._leaderState) {
      this._leaderState.nextIndex.set(peerId, Math.max(1, index));
    }
  }

  /** Update matchIndex for a peer */
  setMatchIndex(peerId: NodeId, index: LogIndex): void {
    if (this._leaderState) {
      this._leaderState.matchIndex.set(peerId, index);
    }
  }

  /** Get nextIndex for a peer */
  getNextIndex(peerId: NodeId): LogIndex {
    return this._leaderState?.nextIndex.get(peerId) ?? this.log.lastIndex + 1;
  }

  /** Get matchIndex for a peer */
  getMatchIndex(peerId: NodeId): LogIndex {
    return this._leaderState?.matchIndex.get(peerId) ?? 0;
  }

  /** Update commit index based on match indices (for leader) */
  updateCommitIndex(): boolean {
    if (!this._leaderState) return false;

    // Find the highest index N such that a majority have matchIndex >= N
    // and log[N].term == currentTerm
    const matchIndices = Array.from(this._leaderState.matchIndex.values());
    matchIndices.push(this.log.lastIndex); // Include self
    matchIndices.sort((a, b) => b - a); // Sort descending

    const majority = Math.floor((this.peers.length + 1) / 2) + 1;

    for (const index of matchIndices) {
      if (index <= this._commitIndex) break;

      const term = this.log.termAt(index);
      if (term === this._currentTerm) {
        // Count how many have this index or higher
        const count =
          matchIndices.filter((m) => m >= index).length + 1; // +1 for self
        if (count >= majority) {
          this._commitIndex = index;
          return true;
        }
      }
    }

    return false;
  }

  /** Get persistent state */
  getPersistentState(): PersistentState<T> {
    return {
      currentTerm: this._currentTerm,
      votedFor: this._votedFor,
      log: this.log.getAll(),
    };
  }

  /** Get volatile state */
  getVolatileState(): VolatileState {
    return {
      commitIndex: this._commitIndex,
      lastApplied: this._lastApplied,
    };
  }

  /** Load persistent state */
  loadPersistentState(state: PersistentState<T>): void {
    this._currentTerm = state.currentTerm;
    this._votedFor = state.votedFor;
    this.log.load(state.log);
  }

  /** Persist state */
  private async persist(): Promise<void> {
    if (this.persistCallback) {
      await this.persistCallback(this.getPersistentState());
    }
  }
}

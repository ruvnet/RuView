/**
 * Raft Node Implementation
 * Core Raft consensus algorithm implementation
 */

import EventEmitter from 'eventemitter3';
import {
  NodeId,
  Term,
  LogIndex,
  NodeState,
  RaftNodeConfig,
  RequestVoteRequest,
  RequestVoteResponse,
  AppendEntriesRequest,
  AppendEntriesResponse,
  LogEntry,
  RaftError,
  RaftEvent,
  StateChangeEvent,
  LeaderElectedEvent,
  LogCommittedEvent,
  PersistentState,
} from './types.js';
import { RaftState } from './state.js';

/** Transport interface for sending RPCs to peers */
export interface RaftTransport<T = unknown> {
  /** Send RequestVote RPC to a peer */
  requestVote(peerId: NodeId, request: RequestVoteRequest): Promise<RequestVoteResponse>;
  /** Send AppendEntries RPC to a peer */
  appendEntries(peerId: NodeId, request: AppendEntriesRequest<T>): Promise<AppendEntriesResponse>;
}

/** State machine interface for applying committed entries */
export interface StateMachine<T = unknown, R = void> {
  /** Apply a committed command to the state machine */
  apply(command: T): Promise<R>;
}

/** Default configuration values */
const DEFAULT_CONFIG: Partial<RaftNodeConfig> = {
  electionTimeout: [150, 300],
  heartbeatInterval: 50,
  maxEntriesPerRequest: 100,
};

/** Raft consensus node */
export class RaftNode<T = unknown, R = void> extends EventEmitter {
  private readonly config: Required<RaftNodeConfig>;
  private readonly state: RaftState<T>;
  private nodeState: NodeState = NodeState.Follower;
  private leaderId: NodeId | null = null;
  private transport: RaftTransport<T> | null = null;
  private stateMachine: StateMachine<T, R> | null = null;

  private electionTimer: ReturnType<typeof setTimeout> | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private running = false;

  constructor(config: RaftNodeConfig) {
    super();
    this.config = { ...DEFAULT_CONFIG, ...config } as Required<RaftNodeConfig>;
    this.state = new RaftState<T>(config.nodeId, config.peers);
  }

  /** Get node ID */
  get nodeId(): NodeId {
    return this.config.nodeId;
  }

  /** Get current state */
  get currentState(): NodeState {
    return this.nodeState;
  }

  /** Get current term */
  get currentTerm(): Term {
    return this.state.currentTerm;
  }

  /** Get current leader ID */
  get leader(): NodeId | null {
    return this.leaderId;
  }

  /** Check if this node is the leader */
  get isLeader(): boolean {
    return this.nodeState === NodeState.Leader;
  }

  /** Get commit index */
  get commitIndex(): LogIndex {
    return this.state.commitIndex;
  }

  /** Set transport for RPC communication */
  setTransport(transport: RaftTransport<T>): void {
    this.transport = transport;
  }

  /** Set state machine for applying commands */
  setStateMachine(stateMachine: StateMachine<T, R>): void {
    this.stateMachine = stateMachine;
  }

  /** Start the Raft node */
  start(): void {
    if (this.running) return;
    this.running = true;
    this.resetElectionTimer();
  }

  /** Stop the Raft node */
  stop(): void {
    this.running = false;
    this.clearTimers();
  }

  /** Propose a command to be replicated (only works if leader) */
  async propose(command: T): Promise<LogEntry<T>> {
    if (this.nodeState !== NodeState.Leader) {
      throw RaftError.notLeader();
    }

    const entry = await this.state.log.appendCommand(this.state.currentTerm, command);
    this.emit(RaftEvent.LogAppended, entry);

    // Immediately replicate to followers
    await this.replicateToFollowers();

    return entry;
  }

  /** Handle RequestVote RPC from a candidate */
  async handleRequestVote(request: RequestVoteRequest): Promise<RequestVoteResponse> {
    // If request term is higher, update term and become follower
    if (request.term > this.state.currentTerm) {
      await this.state.setTerm(request.term);
      this.transitionTo(NodeState.Follower);
    }

    // Deny vote if request term is less than current term
    if (request.term < this.state.currentTerm) {
      return { term: this.state.currentTerm, voteGranted: false };
    }

    // Check if we can vote for this candidate
    const canVote =
      (this.state.votedFor === null || this.state.votedFor === request.candidateId) &&
      this.state.log.isUpToDate(request.lastLogTerm, request.lastLogIndex);

    if (canVote) {
      await this.state.vote(request.term, request.candidateId);
      this.resetElectionTimer();
      this.emit(RaftEvent.VoteGranted, { candidateId: request.candidateId, term: request.term });
      return { term: this.state.currentTerm, voteGranted: true };
    }

    return { term: this.state.currentTerm, voteGranted: false };
  }

  /** Handle AppendEntries RPC from leader */
  async handleAppendEntries(request: AppendEntriesRequest<T>): Promise<AppendEntriesResponse> {
    // If request term is higher, update term
    if (request.term > this.state.currentTerm) {
      await this.state.setTerm(request.term);
      this.transitionTo(NodeState.Follower);
    }

    // Reject if term is less than current term
    if (request.term < this.state.currentTerm) {
      return { term: this.state.currentTerm, success: false };
    }

    // Valid leader - reset election timer
    this.leaderId = request.leaderId;
    this.resetElectionTimer();

    // If not follower, become follower
    if (this.nodeState !== NodeState.Follower) {
      this.transitionTo(NodeState.Follower);
    }

    this.emit(RaftEvent.Heartbeat, { leaderId: request.leaderId, term: request.term });

    // Check if log contains entry at prevLogIndex with prevLogTerm
    if (request.prevLogIndex > 0 && !this.state.log.containsEntry(request.prevLogIndex, request.prevLogTerm)) {
      return { term: this.state.currentTerm, success: false };
    }

    // Append entries
    if (request.entries.length > 0) {
      await this.state.log.append(request.entries);
    }

    // Update commit index
    if (request.leaderCommit > this.state.commitIndex) {
      this.state.setCommitIndex(
        Math.min(request.leaderCommit, this.state.log.lastIndex),
      );
      await this.applyCommitted();
    }

    return {
      term: this.state.currentTerm,
      success: true,
      matchIndex: this.state.log.lastIndex,
    };
  }

  /** Load persistent state */
  loadState(state: PersistentState<T>): void {
    this.state.loadPersistentState(state);
  }

  /** Get current persistent state */
  getState(): PersistentState<T> {
    return this.state.getPersistentState();
  }

  // Private methods

  private transitionTo(newState: NodeState): void {
    const previousState = this.nodeState;
    if (previousState === newState) return;

    this.nodeState = newState;
    this.clearTimers();

    if (newState === NodeState.Leader) {
      this.state.initLeaderState();
      this.leaderId = this.config.nodeId;
      this.startHeartbeat();
      this.emit(RaftEvent.LeaderElected, {
        leaderId: this.config.nodeId,
        term: this.state.currentTerm,
      } as LeaderElectedEvent);
    } else {
      this.state.clearLeaderState();
      if (newState === NodeState.Follower) {
        this.leaderId = null;
        this.resetElectionTimer();
      }
    }

    this.emit(RaftEvent.StateChange, {
      previousState,
      newState,
      term: this.state.currentTerm,
    } as StateChangeEvent);
  }

  private getRandomElectionTimeout(): number {
    const [min, max] = this.config.electionTimeout;
    return min + Math.random() * (max - min);
  }

  private resetElectionTimer(): void {
    if (this.electionTimer) {
      clearTimeout(this.electionTimer);
    }
    if (!this.running) return;

    this.electionTimer = setTimeout(() => {
      this.startElection();
    }, this.getRandomElectionTimeout());
  }

  private clearTimers(): void {
    if (this.electionTimer) {
      clearTimeout(this.electionTimer);
      this.electionTimer = null;
    }
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  private async startElection(): Promise<void> {
    if (!this.running) return;

    // Increment term and become candidate
    await this.state.setTerm(this.state.currentTerm + 1);
    await this.state.vote(this.state.currentTerm, this.config.nodeId);
    this.transitionTo(NodeState.Candidate);

    this.emit(RaftEvent.VoteRequested, {
      term: this.state.currentTerm,
      candidateId: this.config.nodeId,
    });

    // Start with 1 vote (self)
    let votesReceived = 1;
    const majority = Math.floor((this.config.peers.length + 1) / 2) + 1;

    // Request votes from all peers
    if (!this.transport) {
      this.resetElectionTimer();
      return;
    }

    const votePromises = this.config.peers.map(async (peerId) => {
      try {
        const response = await this.transport!.requestVote(peerId, {
          term: this.state.currentTerm,
          candidateId: this.config.nodeId,
          lastLogIndex: this.state.log.lastIndex,
          lastLogTerm: this.state.log.lastTerm,
        });

        // If response term is higher, become follower
        if (response.term > this.state.currentTerm) {
          await this.state.setTerm(response.term);
          this.transitionTo(NodeState.Follower);
          return;
        }

        if (response.voteGranted && this.nodeState === NodeState.Candidate) {
          votesReceived++;
          if (votesReceived >= majority) {
            this.transitionTo(NodeState.Leader);
          }
        }
      } catch {
        // Peer unavailable, continue
      }
    });

    await Promise.allSettled(votePromises);

    // If still candidate, restart election timer
    if (this.nodeState === NodeState.Candidate) {
      this.resetElectionTimer();
    }
  }

  private startHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
    }

    // Send immediate heartbeat
    this.replicateToFollowers();

    // Start periodic heartbeat
    this.heartbeatTimer = setInterval(() => {
      if (this.nodeState === NodeState.Leader) {
        this.replicateToFollowers();
      }
    }, this.config.heartbeatInterval);
  }

  private async replicateToFollowers(): Promise<void> {
    if (!this.transport || this.nodeState !== NodeState.Leader) return;

    const replicationPromises = this.config.peers.map(async (peerId) => {
      await this.replicateToPeer(peerId);
    });

    await Promise.allSettled(replicationPromises);

    // Update commit index if majority have replicated
    if (this.state.updateCommitIndex()) {
      this.emit(RaftEvent.LogCommitted, {
        index: this.state.commitIndex,
        term: this.state.currentTerm,
      } as LogCommittedEvent);
      await this.applyCommitted();
    }
  }

  private async replicateToPeer(peerId: NodeId): Promise<void> {
    if (!this.transport || this.nodeState !== NodeState.Leader) return;

    const nextIndex = this.state.getNextIndex(peerId);
    const prevLogIndex = nextIndex - 1;
    const prevLogTerm = this.state.log.termAt(prevLogIndex) ?? 0;
    const entries = this.state.log.getFrom(nextIndex, this.config.maxEntriesPerRequest);

    try {
      const response = await this.transport.appendEntries(peerId, {
        term: this.state.currentTerm,
        leaderId: this.config.nodeId,
        prevLogIndex,
        prevLogTerm,
        entries,
        leaderCommit: this.state.commitIndex,
      });

      if (response.term > this.state.currentTerm) {
        await this.state.setTerm(response.term);
        this.transitionTo(NodeState.Follower);
        return;
      }

      if (response.success) {
        if (response.matchIndex !== undefined) {
          this.state.setNextIndex(peerId, response.matchIndex + 1);
          this.state.setMatchIndex(peerId, response.matchIndex);
        } else if (entries.length > 0) {
          const lastEntry = entries[entries.length - 1];
          this.state.setNextIndex(peerId, lastEntry.index + 1);
          this.state.setMatchIndex(peerId, lastEntry.index);
        }
      } else {
        // Decrement nextIndex and retry
        this.state.setNextIndex(peerId, nextIndex - 1);
      }
    } catch {
      // Peer unavailable, will retry on next heartbeat
    }
  }

  private async applyCommitted(): Promise<void> {
    while (this.state.lastApplied < this.state.commitIndex) {
      const nextIndex = this.state.lastApplied + 1;
      const entry = this.state.log.get(nextIndex);

      if (entry && this.stateMachine) {
        try {
          await this.stateMachine.apply(entry.command);
          this.state.setLastApplied(nextIndex);
          this.emit(RaftEvent.LogApplied, entry);
        } catch (error) {
          this.emit(RaftEvent.Error, error);
          break;
        }
      } else {
        this.state.setLastApplied(nextIndex);
      }
    }
  }
}

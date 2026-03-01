/**
 * Raft Node Implementation
 * Core Raft consensus algorithm implementation
 */
import EventEmitter from 'eventemitter3';
import { NodeId, Term, LogIndex, NodeState, RaftNodeConfig, RequestVoteRequest, RequestVoteResponse, AppendEntriesRequest, AppendEntriesResponse, LogEntry, PersistentState } from './types.js';
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
/** Raft consensus node */
export declare class RaftNode<T = unknown, R = void> extends EventEmitter {
    private readonly config;
    private readonly state;
    private nodeState;
    private leaderId;
    private transport;
    private stateMachine;
    private electionTimer;
    private heartbeatTimer;
    private running;
    constructor(config: RaftNodeConfig);
    /** Get node ID */
    get nodeId(): NodeId;
    /** Get current state */
    get currentState(): NodeState;
    /** Get current term */
    get currentTerm(): Term;
    /** Get current leader ID */
    get leader(): NodeId | null;
    /** Check if this node is the leader */
    get isLeader(): boolean;
    /** Get commit index */
    get commitIndex(): LogIndex;
    /** Set transport for RPC communication */
    setTransport(transport: RaftTransport<T>): void;
    /** Set state machine for applying commands */
    setStateMachine(stateMachine: StateMachine<T, R>): void;
    /** Start the Raft node */
    start(): void;
    /** Stop the Raft node */
    stop(): void;
    /** Propose a command to be replicated (only works if leader) */
    propose(command: T): Promise<LogEntry<T>>;
    /** Handle RequestVote RPC from a candidate */
    handleRequestVote(request: RequestVoteRequest): Promise<RequestVoteResponse>;
    /** Handle AppendEntries RPC from leader */
    handleAppendEntries(request: AppendEntriesRequest<T>): Promise<AppendEntriesResponse>;
    /** Load persistent state */
    loadState(state: PersistentState<T>): void;
    /** Get current persistent state */
    getState(): PersistentState<T>;
    private transitionTo;
    private getRandomElectionTimeout;
    private resetElectionTimer;
    private clearTimers;
    private startElection;
    private startHeartbeat;
    private replicateToFollowers;
    private replicateToPeer;
    private applyCommitted;
}
//# sourceMappingURL=node.d.ts.map
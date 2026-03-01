/**
 * Raft Consensus Types
 * Based on the Raft paper specification
 */

/** Unique identifier for a node in the cluster */
export type NodeId = string;

/** Monotonically increasing term number */
export type Term = number;

/** Index into the replicated log */
export type LogIndex = number;

/** Possible states of a Raft node */
export enum NodeState {
  Follower = 'follower',
  Candidate = 'candidate',
  Leader = 'leader',
}

/** Entry in the replicated log */
export interface LogEntry<T = unknown> {
  /** Term when entry was received by leader */
  term: Term;
  /** Index in the log */
  index: LogIndex;
  /** Command to be applied to state machine */
  command: T;
  /** Timestamp when entry was created */
  timestamp: number;
}

/** Persistent state on all servers (updated on stable storage before responding to RPCs) */
export interface PersistentState<T = unknown> {
  /** Latest term server has seen */
  currentTerm: Term;
  /** CandidateId that received vote in current term (or null if none) */
  votedFor: NodeId | null;
  /** Log entries */
  log: LogEntry<T>[];
}

/** Volatile state on all servers */
export interface VolatileState {
  /** Index of highest log entry known to be committed */
  commitIndex: LogIndex;
  /** Index of highest log entry applied to state machine */
  lastApplied: LogIndex;
}

/** Volatile state on leaders (reinitialized after election) */
export interface LeaderState {
  /** For each server, index of the next log entry to send to that server */
  nextIndex: Map<NodeId, LogIndex>;
  /** For each server, index of highest log entry known to be replicated on server */
  matchIndex: Map<NodeId, LogIndex>;
}

/** Configuration for a Raft node */
export interface RaftNodeConfig {
  /** Unique identifier for this node */
  nodeId: NodeId;
  /** List of all node IDs in the cluster */
  peers: NodeId[];
  /** Election timeout range in milliseconds [min, max] */
  electionTimeout: [number, number];
  /** Heartbeat interval in milliseconds */
  heartbeatInterval: number;
  /** Maximum entries per AppendEntries RPC */
  maxEntriesPerRequest: number;
}

/** Request for RequestVote RPC */
export interface RequestVoteRequest {
  /** Candidate's term */
  term: Term;
  /** Candidate requesting vote */
  candidateId: NodeId;
  /** Index of candidate's last log entry */
  lastLogIndex: LogIndex;
  /** Term of candidate's last log entry */
  lastLogTerm: Term;
}

/** Response for RequestVote RPC */
export interface RequestVoteResponse {
  /** Current term, for candidate to update itself */
  term: Term;
  /** True means candidate received vote */
  voteGranted: boolean;
}

/** Request for AppendEntries RPC */
export interface AppendEntriesRequest<T = unknown> {
  /** Leader's term */
  term: Term;
  /** So follower can redirect clients */
  leaderId: NodeId;
  /** Index of log entry immediately preceding new ones */
  prevLogIndex: LogIndex;
  /** Term of prevLogIndex entry */
  prevLogTerm: Term;
  /** Log entries to store (empty for heartbeat) */
  entries: LogEntry<T>[];
  /** Leader's commitIndex */
  leaderCommit: LogIndex;
}

/** Response for AppendEntries RPC */
export interface AppendEntriesResponse {
  /** Current term, for leader to update itself */
  term: Term;
  /** True if follower contained entry matching prevLogIndex and prevLogTerm */
  success: boolean;
  /** Hint for next index to try (optimization) */
  matchIndex?: LogIndex;
}

/** Raft error types */
export class RaftError extends Error {
  constructor(
    message: string,
    public readonly code: RaftErrorCode,
  ) {
    super(message);
    this.name = 'RaftError';
  }

  static notLeader(): RaftError {
    return new RaftError('Node is not the leader', RaftErrorCode.NotLeader);
  }

  static noLeader(): RaftError {
    return new RaftError('No leader available', RaftErrorCode.NoLeader);
  }

  static electionTimeout(): RaftError {
    return new RaftError('Election timeout', RaftErrorCode.ElectionTimeout);
  }

  static logInconsistency(): RaftError {
    return new RaftError('Log inconsistency detected', RaftErrorCode.LogInconsistency);
  }
}

export enum RaftErrorCode {
  NotLeader = 'NOT_LEADER',
  NoLeader = 'NO_LEADER',
  InvalidTerm = 'INVALID_TERM',
  InvalidLogIndex = 'INVALID_LOG_INDEX',
  ElectionTimeout = 'ELECTION_TIMEOUT',
  LogInconsistency = 'LOG_INCONSISTENCY',
  SnapshotFailed = 'SNAPSHOT_FAILED',
  ConfigError = 'CONFIG_ERROR',
  Internal = 'INTERNAL',
}

/** Event types emitted by RaftNode */
export enum RaftEvent {
  StateChange = 'stateChange',
  LeaderElected = 'leaderElected',
  LogAppended = 'logAppended',
  LogCommitted = 'logCommitted',
  LogApplied = 'logApplied',
  VoteRequested = 'voteRequested',
  VoteGranted = 'voteGranted',
  Heartbeat = 'heartbeat',
  Error = 'error',
}

/** State change event data */
export interface StateChangeEvent {
  previousState: NodeState;
  newState: NodeState;
  term: Term;
}

/** Leader elected event data */
export interface LeaderElectedEvent {
  leaderId: NodeId;
  term: Term;
}

/** Log committed event data */
export interface LogCommittedEvent {
  index: LogIndex;
  term: Term;
}

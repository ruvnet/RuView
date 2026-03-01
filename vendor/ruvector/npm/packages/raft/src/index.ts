/**
 * @ruvector/raft - Raft Consensus Implementation
 *
 * A TypeScript implementation of the Raft consensus algorithm for
 * distributed systems, providing leader election, log replication,
 * and fault tolerance.
 *
 * @example
 * ```typescript
 * import { RaftNode, RaftTransport, NodeState } from '@ruvector/raft';
 *
 * // Create a Raft node
 * const node = new RaftNode({
 *   nodeId: 'node-1',
 *   peers: ['node-2', 'node-3'],
 *   electionTimeout: [150, 300],
 *   heartbeatInterval: 50,
 *   maxEntriesPerRequest: 100,
 * });
 *
 * // Set up transport for RPC communication
 * node.setTransport(myTransport);
 *
 * // Set up state machine for applying commands
 * node.setStateMachine(myStateMachine);
 *
 * // Listen for events
 * node.on('stateChange', (event) => {
 *   console.log(`State changed: ${event.previousState} -> ${event.newState}`);
 * });
 *
 * node.on('leaderElected', (event) => {
 *   console.log(`New leader: ${event.leaderId} in term ${event.term}`);
 * });
 *
 * // Start the node
 * node.start();
 *
 * // Propose a command (only works if leader)
 * if (node.isLeader) {
 *   await node.propose({ type: 'SET', key: 'foo', value: 'bar' });
 * }
 * ```
 *
 * @packageDocumentation
 */

// Types
export {
  NodeId,
  Term,
  LogIndex,
  NodeState,
  LogEntry,
  PersistentState,
  VolatileState,
  LeaderState,
  RaftNodeConfig,
  RequestVoteRequest,
  RequestVoteResponse,
  AppendEntriesRequest,
  AppendEntriesResponse,
  RaftError,
  RaftErrorCode,
  RaftEvent,
  StateChangeEvent,
  LeaderElectedEvent,
  LogCommittedEvent,
} from './types.js';

// Log
export { RaftLog } from './log.js';

// State
export { RaftState } from './state.js';

// Node
export { RaftNode, RaftTransport, StateMachine } from './node.js';

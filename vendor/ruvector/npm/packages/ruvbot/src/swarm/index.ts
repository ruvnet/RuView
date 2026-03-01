/**
 * Swarm module exports
 *
 * Multi-agent swarm coordination with agentic-flow patterns.
 */

export {
  SwarmCoordinator,
  createSwarmCoordinator,
  WORKER_DEFAULTS,
  type SwarmTopology,
  type ConsensusProtocol,
  type WorkerType,
  type WorkerPriority,
  type SwarmConfig,
  type WorkerConfig,
  type SwarmTask,
  type SwarmAgent,
  type DispatchOptions,
} from './SwarmCoordinator.js';

export {
  ByzantineConsensus,
  createByzantineConsensus,
  type ConsensusPhase,
  type ConsensusConfig,
  type Proposal,
  type Vote,
  type ConsensusResult,
  type ReplicaInfo,
} from './ByzantineConsensus.js';

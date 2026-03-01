# ADR-011: Swarm Coordination (agentic-flow Integration)

## Status
Accepted (Implemented)

## Date
2026-01-27

## Context

Clawdbot has basic async processing. RuvBot integrates agentic-flow patterns for:
- Multi-agent swarm coordination
- 12 specialized background workers
- Byzantine fault-tolerant consensus
- Dynamic topology switching

## Decision

### Swarm Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    RuvBot Swarm Coordination                     │
├─────────────────────────────────────────────────────────────────┤
│  Topologies                                                      │
│    ├─ hierarchical       : Queen-worker (anti-drift)           │
│    ├─ mesh               : Peer-to-peer network                 │
│    ├─ hierarchical-mesh  : Hybrid for scalability              │
│    └─ adaptive           : Dynamic switching                    │
├─────────────────────────────────────────────────────────────────┤
│  Consensus Protocols                                             │
│    ├─ byzantine          : BFT (f < n/3 faulty)                 │
│    ├─ raft               : Leader-based (f < n/2)               │
│    ├─ gossip             : Eventually consistent                │
│    └─ crdt               : Conflict-free replication            │
├─────────────────────────────────────────────────────────────────┤
│  Background Workers (12)                                         │
│    ├─ ultralearn   [normal]   : Deep knowledge acquisition     │
│    ├─ optimize     [high]     : Performance optimization        │
│    ├─ consolidate  [low]      : Memory consolidation (EWC++)   │
│    ├─ predict      [normal]   : Predictive preloading           │
│    ├─ audit        [critical] : Security analysis               │
│    ├─ map          [normal]   : Codebase mapping                │
│    ├─ preload      [low]      : Resource preloading             │
│    ├─ deepdive     [normal]   : Deep code analysis              │
│    ├─ document     [normal]   : Auto-documentation              │
│    ├─ refactor     [normal]   : Refactoring suggestions         │
│    ├─ benchmark    [normal]   : Performance benchmarking        │
│    └─ testgaps     [normal]   : Test coverage analysis          │
└─────────────────────────────────────────────────────────────────┘
```

### Implementation

Located in `/npm/packages/ruvbot/src/swarm/`:
- `SwarmCoordinator.ts` - Main coordinator with task dispatch
- `ByzantineConsensus.ts` - PBFT-style consensus implementation

### SwarmCoordinator

```typescript
interface SwarmConfig {
  topology: SwarmTopology;        // 'hierarchical' | 'mesh' | 'hierarchical-mesh' | 'adaptive'
  maxAgents: number;              // default: 8
  strategy: 'specialized' | 'balanced' | 'adaptive';
  consensus: ConsensusProtocol;   // 'byzantine' | 'raft' | 'gossip' | 'crdt'
  heartbeatInterval?: number;     // default: 5000ms
  taskTimeout?: number;           // default: 60000ms
}

interface SwarmTask {
  id: string;
  worker: WorkerType;
  type: string;
  content: unknown;
  priority: WorkerPriority;
  status: 'pending' | 'running' | 'completed' | 'failed';
  assignedAgent?: string;
  result?: unknown;
  error?: string;
  createdAt: Date;
  startedAt?: Date;
  completedAt?: Date;
}

interface SwarmAgent {
  id: string;
  type: WorkerType;
  status: 'idle' | 'busy' | 'offline';
  currentTask?: string;
  completedTasks: number;
  failedTasks: number;
  lastHeartbeat: Date;
}
```

### Worker Configuration

```typescript
const WORKER_DEFAULTS: Record<WorkerType, WorkerConfig> = {
  ultralearn:   { priority: 'normal',   concurrency: 2, timeout: 60000,  retries: 3, backoff: 'exponential' },
  optimize:     { priority: 'high',     concurrency: 4, timeout: 30000,  retries: 2, backoff: 'exponential' },
  consolidate:  { priority: 'low',      concurrency: 1, timeout: 120000, retries: 1, backoff: 'linear' },
  predict:      { priority: 'normal',   concurrency: 2, timeout: 15000,  retries: 2, backoff: 'exponential' },
  audit:        { priority: 'critical', concurrency: 1, timeout: 45000,  retries: 3, backoff: 'exponential' },
  map:          { priority: 'normal',   concurrency: 2, timeout: 60000,  retries: 2, backoff: 'linear' },
  preload:      { priority: 'low',      concurrency: 4, timeout: 10000,  retries: 1, backoff: 'linear' },
  deepdive:     { priority: 'normal',   concurrency: 2, timeout: 90000,  retries: 2, backoff: 'exponential' },
  document:     { priority: 'normal',   concurrency: 2, timeout: 30000,  retries: 2, backoff: 'linear' },
  refactor:     { priority: 'normal',   concurrency: 2, timeout: 60000,  retries: 2, backoff: 'exponential' },
  benchmark:    { priority: 'normal',   concurrency: 1, timeout: 120000, retries: 1, backoff: 'linear' },
  testgaps:     { priority: 'normal',   concurrency: 2, timeout: 45000,  retries: 2, backoff: 'linear' },
};
```

### ByzantineConsensus (PBFT)

```typescript
interface ConsensusConfig {
  replicas: number;          // Total number of replicas (default: 5)
  timeout: number;           // Timeout per phase (default: 30000ms)
  retries: number;           // Retries before failing (default: 3)
  requireSignatures: boolean;
}

// Fault tolerance: f < n/3
// Quorum size: ceil(2n/3)
```

**Phases:**
1. `pre-prepare` - Leader broadcasts proposal
2. `prepare` - Replicas validate and send prepare messages
3. `commit` - Wait for quorum of commit messages
4. `decided` - Consensus reached
5. `failed` - Consensus failed (timeout/Byzantine fault)

### Usage Example

```typescript
import { SwarmCoordinator, ByzantineConsensus } from './swarm';

// Initialize swarm
const swarm = new SwarmCoordinator({
  topology: 'hierarchical',
  maxAgents: 8,
  strategy: 'specialized',
  consensus: 'raft'
});

await swarm.start();

// Spawn specialized agents
await swarm.spawnAgent('ultralearn');
await swarm.spawnAgent('optimize');

// Dispatch task
const task = await swarm.dispatch({
  worker: 'ultralearn',
  task: { type: 'deep-analysis', content: 'analyze this' },
  priority: 'normal'
});

// Wait for completion
const result = await swarm.waitForTask(task.id);

// Byzantine consensus for critical decisions
const consensus = new ByzantineConsensus({ replicas: 5, timeout: 30000 });
consensus.initializeReplicas(['node1', 'node2', 'node3', 'node4', 'node5']);
const decision = await consensus.propose({ action: 'deploy', version: '1.0.0' });
```

### Events

SwarmCoordinator emits:
- `started`, `stopped`
- `agent:spawned`, `agent:removed`, `agent:offline`
- `task:created`, `task:assigned`, `task:completed`, `task:failed`

ByzantineConsensus emits:
- `proposal:created`
- `phase:pre-prepare`, `phase:prepare`, `phase:commit`
- `vote:received`
- `consensus:decided`, `consensus:failed`, `consensus:no-quorum`
- `replica:faulty`, `view:changed`

## Consequences

### Positive
- Distributed task execution with priority queues
- Fault tolerance via PBFT consensus
- Specialized workers for different task types
- Heartbeat-based health monitoring
- Event-driven architecture

### Negative
- Coordination overhead
- Complexity of distributed systems
- Memory overhead for task/agent tracking

### RuvBot Advantages over Clawdbot
- 12 specialized workers vs basic async
- Byzantine fault tolerance vs none
- Multi-topology support vs single-threaded
- Learning workers (ultralearn, consolidate) vs static
- Priority-based task scheduling

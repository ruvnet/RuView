# Edge-Net Lifecycle Simulation - Technical Overview

## Architecture

This simulation is a comprehensive TypeScript-based system that models the complete lifecycle of the edge-net P2P network from genesis to full independence.

### Core Components

```
sim/
├── src/
│   ├── cell.ts          # Individual node simulation (6KB)
│   ├── network.ts       # Network state management (10KB)
│   ├── metrics.ts       # Performance tracking (10KB)
│   ├── phases.ts        # Phase transition logic (7KB)
│   ├── report.ts        # JSON report generation (8KB)
│   └── simulator.ts     # Main orchestration (6KB)
├── package.json         # Dependencies
├── tsconfig.json        # TypeScript config
├── README.md            # Project overview
├── USAGE.md             # Usage guide
└── SIMULATION_OVERVIEW.md  # This file
```

## Component Details

### 1. Cell (src/cell.ts)

Simulates individual network nodes with:

**Properties:**
- `id`: Unique identifier (UUID)
- `type`: Genesis or Regular node
- `state`: Active, Read-only, or Retired
- `capabilities`: Compute, bandwidth, reliability, storage (0-1 scale)
- `energy`: rUv (Resource Utility Voucher) balance
- `genesisMultiplier`: 10x for genesis nodes, decays over time
- `connectedCells`: Set of connected node IDs
- `metrics`: Task completion, energy earned/spent, success rate

**Key Methods:**
- `processTask()`: Execute tasks and earn energy
- `spendEnergy()`: Consume energy for operations
- `connectTo()` / `disconnectFrom()`: Manage connections
- `updateState()`: Transition between states based on network phase
- `tick()`: Simulate one time step
- `getFitnessScore()`: Calculate overall node fitness

**Energy Model:**
- Genesis nodes: Start with 1000 rUv, 10x earning multiplier
- Regular nodes: Start with 10 rUv, 1x multiplier
- Passive decay: 0.1 rUv per connection per tick
- Task rewards: Based on complexity × multiplier

### 2. Network (src/network.ts)

Manages the P2P network state:

**Properties:**
- `cells`: Map of all nodes (by ID)
- `currentPhase`: Current lifecycle phase
- `currentTick`: Simulation time step
- `genesisCells`: Set of genesis node IDs
- `taskQueue`: Pending tasks to distribute
- `config`: Network parameters

**Key Methods:**
- `initialize()`: Create genesis nodes and mesh topology
- `spawnNodes()`: Add regular nodes to network
- `connectNewNode()`: Preferential attachment algorithm
- `generateTasks()`: Create tasks based on network size
- `distributeTasks()`: Assign tasks to capable nodes
- `updatePhase()`: Check and trigger phase transitions
- `tick()`: Simulate one network time step
- `getStats()`: Aggregate network statistics

**Network Topology:**
- Genesis nodes: Full mesh (all connected)
- Regular nodes: Preferential attachment (5-10 connections)
- Max connections: 50 per node
- Connection cost: 0.5 rUv

**Task Distribution:**
- Tasks generated: 5 × node count × random factor
- Complexity: 0.1 - 1.0 (random)
- Routing: Fitness-based selection
- Rewards: Base reward × genesis multiplier

### 3. Metrics (src/metrics.ts)

Tracks network performance:

**Per-Phase Metrics:**
- Node count (start, end, peak)
- Energy economics (earned, spent, net, sustainability)
- Genesis node statistics (multiplier, state counts)
- Network health (connections, success rate, throughput)
- Validation results (pass/fail, reasons)

**Validation Criteria:**

**Genesis Phase:**
- ✅ Multiplier ≈ 10.0x
- ✅ Energy > 1000 rUv
- ✅ Avg connections > 5

**Growth Phase:**
- ✅ Genesis activity reducing
- ✅ Multiplier < 5.0x
- ✅ Success rate > 70%

**Maturation Phase:**
- ✅ Genesis > 80% read-only
- ✅ Sustainability > 1.0
- ✅ Avg connections > 10

**Independence Phase:**
- ✅ Genesis > 90% retired
- ✅ Multiplier ≈ 1.0
- ✅ Net energy > 0

### 4. Phases (src/phases.ts)

Manages lifecycle transitions:

**Phase Definitions:**

| Phase | Node Range | Duration | Key Events |
|-------|------------|----------|------------|
| Genesis | 0 - 10K | ~1,000 ticks | 10x multiplier, network formation |
| Growth | 10K - 50K | ~4,000 ticks | Multiplier decay, self-organization |
| Maturation | 50K - 100K | ~5,000 ticks | Genesis read-only, sustainability |
| Independence | 100K+ | ~2,500 ticks | Genesis retired, pure P2P |

**Transition Logic:**
1. Check node count thresholds
2. Validate custom conditions
3. Update all cell states
4. Trigger phase-specific events
5. Notify metrics collector

**Custom Checks:**
- Verify multiplier decay rates
- Confirm state transitions
- Validate sustainability metrics

### 5. Report (src/report.ts)

Generates comprehensive JSON reports:

**Report Structure:**
```typescript
{
  metadata: {
    timestamp: string,
    simulationVersion: string,
    duration: number,
    totalTicks: number
  },
  configuration: {
    genesisNodeCount: number,
    targetNodeCount: number,
    nodesPerTick: number,
    taskGenerationRate: number,
    baseTaskReward: number
  },
  summary: {
    phasesCompleted: number,
    totalPassed: boolean,
    phasesPassed: number,
    phasesTotal: number,
    finalNodeCount: number,
    finalPhase: string
  },
  phases: {
    [phaseName]: PhaseMetrics
  },
  finalState: {
    nodeCount: number,
    genesisNodes: object,
    economy: object,
    network: object,
    topPerformers: array
  },
  validation: {
    overallPassed: boolean,
    criticalIssues: string[],
    warnings: string[],
    successes: string[]
  }
}
```

**Analysis Features:**
- Top performer identification
- Validation issue categorization
- Economic sustainability analysis
- Network health assessment

### 6. Simulator (src/simulator.ts)

Main orchestration engine:

**Execution Flow:**
```
1. Initialize components
2. Create genesis network
3. Main loop:
   a. Spawn new nodes
   b. Generate tasks
   c. Distribute tasks
   d. Update all cells
   e. Check phase transitions
   f. Collect metrics
   g. Display progress
4. Finalize metrics
5. Generate report
6. Save to JSON
7. Exit with status
```

**Command Line Interface:**
- `--fast` / `-f`: Fast mode (100 nodes/tick)
- `--verbose` / `-v`: Detailed logging
- `--output=FILE`: Custom output path

**Progress Visualization:**
- Normal mode: Progress bar with key stats
- Verbose mode: Tick-by-tick detailed logs
- Phase transitions: Highlighted banners

## Simulation Parameters

### Default Configuration

```typescript
{
  genesisNodeCount: 100,         // Initial genesis nodes
  targetNodeCount: 120000,       // Final network size
  nodesPerTick: 10,              // Node spawn rate
  taskGenerationRate: 5,         // Tasks per node
  baseTaskReward: 1.0,           // Base rUv reward
  connectionCost: 0.5,           // Energy per connection
  maxConnectionsPerNode: 50      // Connection limit
}
```

### Performance Characteristics

**Normal Mode:**
- Duration: ~2-5 minutes
- Ticks: ~12,500
- Node spawn rate: 10/tick
- Progress updates: Every 100 ticks

**Fast Mode:**
- Duration: ~1-2 minutes
- Ticks: ~1,250
- Node spawn rate: 100/tick
- Progress updates: Every 1000 ticks

## Economic Model

### Energy (rUv) Flow

**Income:**
- Task completion: `baseReward × genesisMultiplier`
- Genesis boost: 10x initially → 1x by phase 2 end
- Success-based: Failed tasks earn nothing

**Expenses:**
- Connection maintenance: 0.1 rUv per connection per tick
- New connections: 0.5 rUv setup cost
- Network operations: Passive decay

**Sustainability:**
- Ratio: Total Earned / Total Spent
- Target: > 1.0 (earning more than spending)
- Critical threshold: Phase validation requires > 1.0 in maturation

### Genesis Node Economics

**Phase 1 (Genesis):**
- Multiplier: 10.0x
- Initial balance: 1000 rUv
- Role: Network bootstrap, high earning

**Phase 2 (Growth):**
- Multiplier: 10.0x → 1.0x (linear decay)
- Stops accepting connections
- Role: Task processing, guide network

**Phase 3 (Maturation):**
- Multiplier: 1.0x
- State: Read-only
- Role: Observation only, no new tasks

**Phase 4 (Independence):**
- Multiplier: 1.0x
- State: Retired
- Role: None (fully retired)

## Network Topology

### Genesis Mesh

All genesis nodes connect to each other:
```
Genesis nodes: 100
Connections: 100 × 99 / 2 = 4,950
```

### Preferential Attachment

New nodes connect based on:
1. Fitness score: `0.3×compute + 0.2×bandwidth + 0.3×reliability + 0.2×storage`
2. Existing connections: More connected = more attractive
3. Weighted selection: Higher fitness = higher probability

**Connection Count:**
- New nodes: 5-10 connections
- Target average: 10-15 connections
- Maximum: 50 connections per node

### Network Effects

**Small-world properties:**
- Short path lengths
- High clustering
- Hub formation

**Scale-free properties:**
- Power-law degree distribution
- Robust to random failures
- Vulnerable to targeted attacks (mitigated by security)

## Validation Framework

### Automatic Validation

Each phase is validated on completion:

1. **Quantitative Checks:**
   - Node count thresholds
   - Multiplier values
   - Energy sustainability ratios
   - Network connectivity

2. **Qualitative Checks:**
   - State transitions
   - Task success rates
   - System stability

3. **Custom Checks:**
   - Phase-specific logic
   - Economic viability
   - Network independence

### Success Criteria

Overall simulation passes if:
- All 4 phases reach completion
- All phase validations pass
- Final network is independent
- Economic sustainability achieved

### Failure Modes

**Critical Failures:**
- Phase validation fails
- Economic collapse (net energy < 0)
- Network fragmentation

**Warnings:**
- Low success rates (< 70%)
- Poor sustainability (< 1.0 ratio)
- Weak connectivity (< 5 avg)

## Output Analysis

### Console Output

**Progress Indicators:**
```
[████████████████████░░░░░░░░░░░░░░░░] growth | 25,000 nodes | 456,789 tasks | Genesis: 0/100 retired
```

**Phase Transitions:**
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🔄 PHASE TRANSITION: growth → maturation
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 Network Status:
   Nodes: 50,000
   Genesis Nodes: 100
   Avg Connections: 12.34
   Total Energy: 234,567.89 rUv
```

### JSON Report

**Key Sections:**
1. Metadata: Timestamp, version, duration
2. Configuration: All simulation parameters
3. Summary: High-level pass/fail
4. Phases: Detailed per-phase metrics
5. Final State: Network snapshot
6. Validation: All issues and successes

**Use Cases:**
- Automated testing (exit code)
- Performance analysis (metrics)
- Parameter tuning (validation)
- Research (detailed data)

## Testing Scenarios

### 1. Standard Lifecycle (Default)

Tests normal network growth:
- 100 genesis nodes
- 120K target nodes
- All 4 phases

### 2. Fast Growth (--fast)

Tests rapid expansion:
- Same configuration
- 10x spawn rate
- Stress test

### 3. Small Network (Custom)

Tests minimal viable network:
- 50 genesis nodes
- 20K target nodes
- Faster completion

### 4. Economic Stress (Custom)

Tests sustainability:
- Low base rewards
- High connection costs
- Economic viability

### 5. Network Resilience (Custom)

Tests robustness:
- Node failures (low reliability)
- Connection limits
- Recovery mechanisms

## Performance Optimization

### Computational Complexity

**Per Tick:**
- Node spawning: O(nodesPerTick)
- Task generation: O(nodeCount)
- Task distribution: O(taskCount)
- Cell updates: O(nodeCount)
- Phase checks: O(1)

**Overall:**
- Time: O(ticks × nodeCount)
- Space: O(nodeCount)

### Memory Usage

**Typical Simulation:**
- 120K nodes × ~2KB each = ~240MB
- Connection sets: ~60MB
- Metrics history: ~10MB
- Total: ~310MB

### Runtime Performance

**Bottlenecks:**
1. Task distribution (random selection)
2. Preferential attachment (weighted sampling)
3. Metrics collection (aggregation)

**Optimizations:**
- Fast mode: Fewer ticks via batch spawning
- Lazy evaluation: Metrics on-demand
- Efficient data structures: Maps, Sets

## Integration with Edge-Net

### Mapping to Real System

**Simulation → Edge-Net:**
- Cell → E2B sandbox instance
- Energy (rUv) → Real cryptocurrency/tokens
- Tasks → Distributed compute jobs
- Connections → P2P network links
- Phases → Actual deployment stages

### Design Validation

**What This Validates:**
1. Genesis sunset timing (when to retire?)
2. Economic parameters (rewards, costs)
3. Phase transition thresholds
4. Network topology (preferential attachment)
5. Sustainability requirements

### Parameter Tuning

**Use Simulation Results To:**
1. Set genesis multiplier decay rate
2. Determine phase transition points
3. Calibrate economic rewards
4. Optimize connection costs
5. Validate long-term viability

## Future Enhancements

### Potential Additions

1. **Node Churn:**
   - Random node failures
   - Recovery mechanisms
   - Resilience testing

2. **Adaptive Economics:**
   - Dynamic reward adjustment
   - Market-based pricing
   - Supply/demand modeling

3. **Security Simulation:**
   - Byzantine node behavior
   - Sybil attack modeling
   - Defense mechanisms

4. **Advanced Topology:**
   - Geographic constraints
   - Latency modeling
   - Bandwidth limitations

5. **Real-time Visualization:**
   - Web-based dashboard
   - Network graph rendering
   - Live metrics streaming

## References

### Related Files

- `/workspaces/ruvector/examples/edge-net/sim/README.md` - Project overview
- `/workspaces/ruvector/examples/edge-net/sim/USAGE.md` - Usage guide
- `/workspaces/ruvector/examples/edge-net/architecture.md` - Edge-net architecture
- `/workspaces/ruvector/examples/edge-net/economic-model.md` - Economic details

### Key Concepts

- **Preferential Attachment:** New nodes connect to well-connected nodes
- **Genesis Sunset:** Graceful retirement of bootstrap nodes
- **Economic Sustainability:** Self-sustaining token economy
- **Phase Transitions:** Automatic lifecycle stage progression
- **P2P Independence:** Fully decentralized operation

---

**Built for RuVector Edge-Net**
TypeScript simulation validating distributed compute network lifecycle.

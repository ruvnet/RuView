# Edge-Net Lifecycle Simulation - Usage Guide

## Quick Start

### 1. Install Dependencies

```bash
cd /workspaces/ruvector/examples/edge-net/sim
npm install
```

### 2. Run Full Simulation

```bash
# Standard simulation (120K nodes, ~2-5 minutes)
npm run simulate

# Fast mode (faster node spawning, ~1-2 minutes)
npm run simulate:fast

# Verbose mode (detailed tick-by-tick output)
npm run simulate:verbose
```

### 3. View Results

Results are saved to `simulation-report.json` in the sim directory.

## Command Line Options

```bash
# Custom output file
node --loader ts-node/esm src/simulator.ts --output=custom-report.json

# Combine options
node --loader ts-node/esm src/simulator.ts --fast --output=fast-run.json
```

Available options:
- `--fast` / `-f`: Faster node spawning (100 nodes/tick vs 10)
- `--verbose` / `-v`: Detailed tick-by-tick progress
- `--output=FILE`: Custom output file path

## Understanding the Output

### Console Output

```
╔════════════════════════════════════════════════════════════╗
║    EDGE-NET LIFECYCLE SIMULATION - Starting...            ║
╚════════════════════════════════════════════════════════════╝

⚙️  Configuration:
   Genesis Nodes: 100
   Target Nodes: 120,000
   Nodes/Tick: 10
   Mode: NORMAL

🌱 Genesis nodes deployed. Starting simulation...

[Progress Bar]

🔄 PHASE TRANSITION: genesis → growth (10,000 nodes)
  → Genesis nodes reducing 10x multiplier...

🔄 PHASE TRANSITION: growth → maturation (50,000 nodes)
  → Genesis nodes entering READ-ONLY mode...

🔄 PHASE TRANSITION: maturation → independence (100,000 nodes)
  → Genesis nodes RETIRED. Network is independent!

✨ Simulation complete!

   Total Ticks: 12,500
   Duration: 45.23s
   Final Nodes: 120,000
   Final Phase: INDEPENDENCE
```

### Summary Report

After simulation, you'll see:

1. **Overall Summary**
   - Duration and tick count
   - Final node count and phase
   - Pass/fail status for each phase

2. **Phase Results**
   - Node growth (start → end)
   - Energy economics (sustainability ratio)
   - Task completion and success rates

3. **Top Performers**
   - Highest earning nodes
   - Task completion leaders
   - Success rate champions

4. **Validation Results**
   - Critical issues (failures)
   - Warnings (potential issues)
   - Successes (passed validations)

### JSON Report Structure

```json
{
  "metadata": {
    "timestamp": "2025-12-31T...",
    "simulationVersion": "1.0.0",
    "duration": 45234,
    "totalTicks": 12500
  },
  "summary": {
    "phasesCompleted": 4,
    "totalPassed": true,
    "phasesPassed": 4,
    "phasesTotal": 4,
    "finalNodeCount": 120000,
    "finalPhase": "independence"
  },
  "phases": {
    "genesis": {
      "phase": "genesis",
      "startTick": 0,
      "endTick": 1000,
      "duration": 1000,
      "nodeCount": {
        "start": 100,
        "end": 10000,
        "peak": 10000
      },
      "energy": {
        "totalEarned": 15234.50,
        "totalSpent": 6234.20,
        "netEnergy": 9000.30,
        "avgPerNode": 1.52,
        "sustainability": 2.44
      },
      "genesis": {
        "avgMultiplier": 10.0,
        "activeCount": 100,
        "readOnlyCount": 0,
        "retiredCount": 0
      },
      "network": {
        "avgConnections": 15.2,
        "avgSuccessRate": 0.853,
        "taskThroughput": 45.678,
        "tasksCompleted": 45678
      },
      "validation": {
        "passed": true,
        "reasons": [
          "✓ Genesis multiplier active: 10.00x",
          "✓ Energy accumulated: 15234.50 rUv",
          "✓ Network connected: 15.20 avg connections"
        ]
      }
    },
    // ... other phases
  },
  "validation": {
    "overallPassed": true,
    "criticalIssues": [],
    "warnings": [],
    "successes": [...]
  }
}
```

## Phase Details

### Phase 1: Genesis (0 - 10K nodes)

**What happens:**
- 100 genesis nodes form initial network
- Genesis nodes have 10x energy multiplier
- Network establishes basic topology
- Nodes connect via preferential attachment

**Validation criteria:**
- ✅ Genesis multiplier ≈ 10.0x
- ✅ Energy accumulation > 1000 rUv
- ✅ Network connectivity (avg connections > 5)

**Typical duration:** ~1,000 ticks

### Phase 2: Growth (10K - 50K nodes)

**What happens:**
- Genesis multiplier decays from 10x → 1x
- Genesis nodes stop accepting new connections
- Network self-organizes around regular nodes
- Task routing optimizes based on node fitness

**Validation criteria:**
- ✅ Genesis activity reduction
- ✅ Multiplier decay (< 5.0x by end)
- ✅ Task success rate > 70%

**Typical duration:** ~4,000 ticks

### Phase 3: Maturation (50K - 100K nodes)

**What happens:**
- Genesis nodes enter READ-ONLY mode
- Network operates independently
- Economic sustainability achieved
- Adaptive security learning

**Validation criteria:**
- ✅ Genesis nodes > 80% read-only
- ✅ Economic sustainability (earned/spent > 1.0)
- ✅ Network connectivity > 10 avg connections

**Typical duration:** ~5,000 ticks

### Phase 4: Independence (100K+ nodes)

**What happens:**
- Genesis nodes fully RETIRED
- Pure P2P operation
- Long-term stability verification
- Economic equilibrium

**Validation criteria:**
- ✅ Genesis nodes > 90% retired
- ✅ Pure P2P (multiplier ≈ 1.0)
- ✅ Network stability (positive net energy)

**Typical duration:** ~2,500 ticks

## Customizing the Simulation

### Modify Network Parameters

Edit `src/simulator.ts`:

```typescript
this.network = new Network({
  genesisNodeCount: 100,        // Initial genesis count
  targetNodeCount: 120000,      // Total nodes to spawn
  nodesPerTick: 10,             // Growth rate
  taskGenerationRate: 5,        // Tasks per node
  baseTaskReward: 1.0,          // Energy reward
  connectionCost: 0.5,          // Connection energy cost
  maxConnectionsPerNode: 50,    // Max connections
});
```

### Test Smaller Networks

For faster testing:

```typescript
const network = new Network({
  genesisNodeCount: 50,
  targetNodeCount: 20000,
  nodesPerTick: 100,
});
```

### Adjust Phase Thresholds

Edit `src/phases.ts`:

```typescript
[NetworkPhase.GROWTH, {
  minNodes: 10000,  // Phase starts at 10K
  maxNodes: 50000,  // Phase ends at 50K
  customCheck: (net: Network) => {
    // Custom validation logic
  },
}]
```

## Interpreting Results

### Success Indicators

✅ **All phases passed validation**
- Genesis multiplier worked as expected
- Economic sustainability achieved
- Network remained connected
- Genesis sunset completed successfully

✅ **High success rates (> 70%)**
- Task routing is effective
- Node capabilities are well-matched
- Network is healthy

✅ **Positive net energy**
- More energy earned than spent
- Network is economically viable
- Sustainable long-term

### Warning Signs

⚠️ **Low success rates (< 70%)**
- Task routing may need optimization
- Node capabilities mismatch
- Network congestion

⚠️ **Economic sustainability < 1.0**
- Network losing energy
- Not sustainable long-term
- May need reward adjustments

⚠️ **Low connectivity (< 5 avg connections)**
- Network fragmentation risk
- Poor resilience
- Communication bottlenecks

### Critical Issues

❌ **Phase validation failures**
- Genesis multiplier not working
- Phase transitions not triggering
- Network instability

❌ **Negative net energy**
- Network is losing resources
- Economic model broken
- Unsustainable

❌ **Genesis retirement failed**
- Genesis nodes not retiring
- Network dependent on genesis
- Independence not achieved

## Performance Tips

### Faster Simulations

1. **Use fast mode:**
   ```bash
   npm run simulate:fast
   ```

2. **Reduce target node count:**
   ```typescript
   targetNodeCount: 50000  // Instead of 120000
   ```

3. **Increase nodes per tick:**
   ```typescript
   nodesPerTick: 100  // Instead of 10
   ```

### More Detailed Analysis

1. **Use verbose mode:**
   ```bash
   npm run simulate:verbose
   ```

2. **Lower progress interval:**
   ```typescript
   this.progressInterval = 10;  // Update every 10 ticks
   ```

3. **Add custom logging:**
   ```typescript
   // In simulator.ts
   if (this.network.currentTick % 100 === 0) {
     console.log('Custom metrics:', ...);
   }
   ```

## Troubleshooting

### Simulation hangs

- Check timeout (max 50,000 ticks)
- Reduce target node count
- Increase nodes per tick

### Out of memory

- Reduce target node count
- Increase node spawn rate (fewer total ticks)
- Run in fast mode

### TypeScript errors

```bash
npm run build
```

### Module errors

```bash
npm install
```

## Integration with Edge-Net

This simulation validates the edge-net architecture:

1. **Genesis Phase** - Corresponds to initial E2B swarm deployment
2. **Growth Phase** - Network expansion with guided self-organization
3. **Maturation** - Full autonomy with genesis oversight reduction
4. **Independence** - Pure P2P operation, genesis retired

Use simulation results to:
- Validate economic parameters
- Test phase transition logic
- Verify sustainability thresholds
- Optimize network topology
- Tune genesis sunset timing

## Next Steps

1. Run the simulation
2. Analyze the JSON report
3. Adjust parameters if needed
4. Test different scenarios
5. Integrate findings into edge-net design

## Support

For issues or questions about the simulation, refer to:
- `/workspaces/ruvector/examples/edge-net/sim/README.md`
- Edge-net architecture documentation
- RuVector project documentation

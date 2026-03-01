# Temporal Attractor Networks with MinCut Analysis

This example demonstrates how networks evolve toward stable "attractor states" and how minimum cut analysis helps detect convergence to these attractors.

## What are Temporal Attractors?

In **dynamical systems theory**, an **attractor** is a state toward which a system naturally evolves over time, regardless of initial conditions (within a basin).

### Real-World Analogies

```
🏔️ Gravitational Attractor
   ╱╲    ball
  ╱  ╲    ↓
 ╱____╲  valley (attractor)

🌊 Hydraulic Attractor
  ╱╲   ╱╲
 ╱  ╲_╱  ╲  ← water flows to lowest point

🕸️ Network Attractor
  Sparse → Dense
  ◯  ◯     ◯═◯
   ╲╱   →  ║╳║  (maximum connectivity)
   ◯       ◯═◯
```

### Three Types of Network Attractors

#### 1️⃣ Optimal Attractor (Maximum Connectivity)

**What it is**: Network evolves toward maximum connectivity and robustness.

```
Initial State (Ring):           Final State (Dense):
    ◯─◯─◯                          ◯═◯═◯
    │   │                          ║╳║╳║
    ◯─◯─◯                          ◯═◯═◯
MinCut: 1                        MinCut: 6+
```

**MinCut Evolution**:
```
Step:  0    10   20   30   40   50
       │    │    │    │    │    │
MinCut 1 ───2────4────5────6────6  (stable)
              ↑              ↑
          Adding edges   Converged!
```

**Why it matters for swarms**:
- ✅ Fault-tolerant communication
- ✅ Maximum information flow
- ✅ Robust against node failures
- ✅ Optimal for multi-agent coordination

#### 2️⃣ Fragmented Attractor (Network Collapse)

**What it is**: Network fragments into disconnected clusters.

```
Initial State (Connected):      Final State (Fragmented):
    ◯─◯─◯                          ◯─◯ ◯
    │   │                              ╲│
    ◯─◯─◯                            ◯ ◯─◯
MinCut: 1                        MinCut: 0
```

**MinCut Evolution**:
```
Step:  0    10   20   30   40   50
       │    │    │    │    │    │
MinCut 1 ───1────0────0────0────0  (stable)
              ↓              ↑
       Removing edges    Disconnected!
```

**Why it matters for swarms**:
- ❌ Communication breakdown
- ❌ Isolated agents
- ❌ Coordination failure
- ❌ Poor swarm performance

#### 3️⃣ Oscillating Attractor (Limit Cycle)

**What it is**: Network oscillates between states periodically.

```
State A:        State B:        State A:
  ◯═◯           ◯─◯             ◯═◯
  ║ ║     →     │ │       →     ║ ║  ...
  ◯═◯           ◯─◯             ◯═◯
```

**MinCut Evolution**:
```
Step:  0    10   20   30   40   50
       │    │    │    │    │    │
MinCut 1 ───3────1────3────1────3  (periodic)
          ↗ ↘ ↗ ↘ ↗ ↘ ↗ ↘
       Oscillating pattern!
```

**Why it matters for swarms**:
- ⚠️ Unstable equilibrium
- ⚠️ May indicate resonance
- ⚠️ Requires damping
- ⚠️ Unpredictable behavior

## How MinCut Detects Convergence

The **minimum cut value** serves as a "thermometer" for network health:

### Convergence Patterns

```
📈 INCREASING MinCut → Strengthening
    0─1─2─3─4─5─6─6─6  ✅ Converging to optimal
                  └─┴─ Stable (attractor reached)

📉 DECREASING MinCut → Fragmenting
    6─5─4─3─2─1─0─0─0  ❌ Network collapsing
                  └─┴─ Stable (disconnected)

🔄 OSCILLATING MinCut → Limit Cycle
    1─3─1─3─1─3─1─3─1  ⚠️ Periodic pattern
      └─┴─┴─┴─┴─┴─┴─── Oscillating attractor
```

### Mathematical Interpretation

**Variance Analysis**:
```
Variance = Σ(MinCut[i] - Mean)² / N

Low Variance (< 0.5):  STABLE → Attractor reached ✓
High Variance (> 5):   OSCILLATING → Limit cycle ⚠️
Medium Variance:       TRANSITIONING → Still evolving
```

## Why This Matters for Swarms

### Multi-Agent Systems Naturally Form Attractors

```
Agent Swarm Evolution:

t=0: Random deployment        t=20: Self-organizing       t=50: Converged
     🤖    🤖                       🤖─🤖                      🤖═🤖
  🤖    🤖    🤖                   ╱│ │╲                      ║╳║╳║
     🤖    🤖                     🤖─🤖─🤖                    🤖═🤖═🤖

MinCut: 0                     MinCut: 2                   MinCut: 6 (stable)
(disconnected)                (organizing)                (optimal attractor)
```

### Real-World Applications

1. **Drone Swarms**: Need optimal attractor for coordination
   - MinCut monitors communication strength
   - Detects when swarm has stabilized
   - Warns if swarm is fragmenting

2. **Distributed Computing**: Optimal attractor = efficient topology
   - MinCut shows network resilience
   - Identifies bottlenecks early
   - Validates load balancing

3. **Social Networks**: Understanding community formation
   - MinCut reveals cluster strength
   - Detects community splits
   - Predicts group stability

## Running the Example

```bash
# Build and run
cd /home/user/ruvector/examples/mincut/temporal_attractors
cargo run --release

# Expected output: 3 scenarios showing different attractor types
```

### Understanding the Output

```
Step  | MinCut | Edges | Avg Conn | Time(μs) | Status
------|--------|-------|----------|----------|------------------
    0 |      1 |    10 |     1.00 |       45 |   evolving...
    5 |      2 |    15 |     1.50 |       52 |   evolving...
   10 |      4 |    23 |     2.30 |       68 |   evolving...
   15 |      6 |    31 |     3.10 |       89 |   evolving...
   20 |      6 |    34 |     3.40 |       95 | ✓ CONVERGED
```

**Key Metrics**:
- **MinCut**: Network's bottleneck capacity
- **Edges**: Total connections
- **Avg Conn**: Average edges per node
- **Time**: Performance per evolution step
- **Status**: Convergence detection

## Code Structure

### Main Components

```rust
// 1. Network snapshot (state at each time step)
NetworkSnapshot {
    step: usize,
    mincut: u64,
    edge_count: usize,
    avg_connectivity: f64,
}

// 2. Attractor network (evolving system)
AttractorNetwork {
    graph: Graph,
    attractor_type: AttractorType,
    history: Vec<NetworkSnapshot>,
}

// 3. Evolution methods (dynamics)
evolve_toward_optimal()     // Add shortcuts, strengthen edges
evolve_toward_fragmented()  // Remove edges, weaken connections
evolve_toward_oscillating() // Alternate add/remove
```

### Key Methods

```rust
// Evolve one time step
network.evolve_step() -> NetworkSnapshot

// Check if converged to attractor
network.has_converged(window: usize) -> bool

// Get evolution history
network.history() -> &[NetworkSnapshot]

// Calculate current mincut
calculate_mincut() -> u64
```

## Key Insights

### 1. MinCut as Health Monitor

```
High MinCut (6+):  Healthy, robust network    ✅
Medium MinCut (2-5): Moderate connectivity    ⚠️
Low MinCut (1):     Fragile, single bottleneck ⚠️
Zero MinCut (0):    Disconnected, failed      ❌
```

### 2. Convergence Detection

```rust
// Stable variance → Attractor reached
variance < 0.5  ⟹  Equilibrium
variance > 5.0  ⟹  Oscillating
```

### 3. Evolution Speed

```
Optimal Attractor:     Fast convergence (10-20 steps)
Fragmented Attractor:  Medium speed (15-30 steps)
Oscillating Attractor: Never converges (limit cycle)
```

## Advanced Topics

### Basin of Attraction

```
       Optimal Basin           Fragmented Basin
    ╱                 ╲       ╱                ╲
   │   ┌─────────┐     │     │    ┌─────┐      │
   │   │ Optimal │     │     │    │ Frag│      │
   │   │Attractor│     │     │    │ment │      │
   │   └─────────┘     │     │    └─────┘      │
    ╲                 ╱       ╲                ╱
     Any initial state          Any initial state
     in this region  →          in this region  →
     converges here             converges here
```

### Bifurcation Points

Critical thresholds where attractor type changes:
```
Parameter (e.g., edge addition rate)
  │
  │  ┌─────────────  Optimal
  │ ╱
  ├─────────────────  Bifurcation point
  │ ╲
  │  └─────────────  Fragmented
  └───────────────────────────→
```

### Lyapunov Stability

MinCut variance measures stability:
```
dMinCut/dt < 0  →  Stable attractor
dMinCut/dt > 0  →  Unstable, moving away
dMinCut/dt ≈ 0  →  Near equilibrium
```

## References

- **Dynamical Systems Theory**: Strogatz, "Nonlinear Dynamics and Chaos"
- **Network Science**: Barabási, "Network Science"
- **Swarm Intelligence**: Bonabeau et al., "Swarm Intelligence"
- **MinCut Algorithms**: Stoer-Wagner (1997), Karger (2000)

## Performance Notes

- **Time Complexity**: O(V³) per step (dominated by mincut calculation)
- **Space Complexity**: O(V + E + H) where H is history length
- **Typical Runtime**: ~50-100μs per step for 10-node networks

## Educational Value

This example teaches:
1. ✅ What temporal attractors are and why they matter
2. ✅ How networks naturally evolve toward stable states
3. ✅ Using MinCut as a convergence detector
4. ✅ Interpreting attractor basins and stability
5. ✅ Applying these concepts to multi-agent swarms

Perfect for understanding how swarms self-organize and how to monitor their health!

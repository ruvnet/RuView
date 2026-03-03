# Quantum-Inspired & Autonomous Modules -- WiFi-DensePose Edge Intelligence

> Advanced algorithms inspired by quantum computing, neuroscience, and AI planning. These modules let the ESP32 make autonomous decisions, heal its own mesh network, interpret high-level scene semantics, and explore room states using quantum-inspired search.

## Quantum-Inspired

| Module | File | What It Does | Event IDs | Budget |
|--------|------|--------------|-----------|--------|
| Quantum Coherence | `qnt_quantum_coherence.rs` | Maps CSI phases onto a Bloch sphere to detect sudden environmental changes | 850-852 | H (<10 ms) |
| Interference Search | `qnt_interference_search.rs` | Grover-inspired multi-hypothesis room state classifier | 855-857 | H (<10 ms) |

---

### Quantum Coherence (`qnt_quantum_coherence.rs`)

**What it does**: Maps each subcarrier's phase onto a point on the quantum Bloch sphere and computes an aggregate coherence metric from the mean Bloch vector magnitude. When all subcarrier phases are aligned, the system is "coherent" (like a quantum pure state). When phases scatter randomly, it is "decoherent" (like a maximally mixed state). Sudden decoherence -- a rapid entropy spike -- indicates an environmental disturbance such as a door opening, a person entering, or furniture being moved.

**Algorithm**: Each subcarrier phase is mapped to a 3D Bloch vector:
- theta = |phase| (polar angle)
- phi = sign(phase) * pi/2 (azimuthal angle)

Since phi is always +/- pi/2, cos(phi) = 0 and sin(phi) = +/- 1. This eliminates 2 trig calls per subcarrier (saving 64+ cosf/sinf calls per frame for 32 subcarriers). The x-component of the mean Bloch vector is always zero.

Von Neumann entropy: S = -p*log(p) - (1-p)*log(1-p) where p = (1 + |bloch|) / 2. S=0 when perfectly coherent (|bloch|=1), S=ln(2) when maximally mixed (|bloch|=0). EMA smoothing with alpha=0.15.

#### Public API

```rust
use wifi_densepose_wasm_edge::qnt_quantum_coherence::QuantumCoherenceMonitor;

let mut mon = QuantumCoherenceMonitor::new();             // const fn
let events = mon.process_frame(&phases);                  // per-frame
let coh = mon.coherence();                                // [0, 1], 1=pure state
let ent = mon.entropy();                                  // [0, ln(2)]
let norm_ent = mon.normalized_entropy();                   // [0, 1]
let bloch = mon.bloch_vector();                           // [f32; 3]
let frames = mon.frame_count();                           // total frames
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 850 | `EVENT_ENTANGLEMENT_ENTROPY` | EMA-smoothed Von Neumann entropy [0, ln(2)] | Every 10 frames |
| 851 | `EVENT_DECOHERENCE_EVENT` | Entropy jump magnitude (> 0.3) | On detection |
| 852 | `EVENT_BLOCH_DRIFT` | Euclidean distance between consecutive Bloch vectors | Every 5 frames |

#### Configuration Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_SC` | 32 | Maximum subcarriers |
| `ALPHA` | 0.15 | EMA smoothing factor |
| `DECOHERENCE_THRESHOLD` | 0.3 | Entropy jump threshold |
| `ENTROPY_EMIT_INTERVAL` | 10 | Frames between entropy reports |
| `DRIFT_EMIT_INTERVAL` | 5 | Frames between drift reports |
| `LN2` | 0.693147 | Maximum binary entropy |

#### Example: Door Opening Detection via Decoherence

```
Frames 1-50: Empty room, phases stable at ~0.1 rad
  Bloch vector: (0, 0.10, 0.99) -> coherence = 0.995
  Entropy ~ 0.005 (near zero, pure state)

Frame 51: Door opens, multipath changes suddenly
  Phases scatter: [-2.1, 0.8, 1.5, -0.3, ...]
  Bloch vector: (0, 0.12, 0.34) -> coherence = 0.36
  Entropy jumps to 0.61
  -> EVENT_DECOHERENCE_EVENT = 0.605 (jump magnitude)
  -> EVENT_BLOCH_DRIFT = 0.65 (large Bloch vector displacement)

Frames 52-100: New stable multipath
  Phases settle at new values
  Entropy gradually decays via EMA
  No more decoherence events
```

#### Bloch Sphere Intuition

Think of each subcarrier as a compass needle. When the room is stable, all needles point roughly the same direction (high coherence, low entropy). When something changes the WiFi multipath -- a person enters, a door opens, furniture moves -- the needles scatter in different directions (low coherence, high entropy). The Bloch sphere formalism quantifies this in a way that is mathematically precise and computationally cheap.

---

### Interference Search (`qnt_interference_search.rs`)

**What it does**: Maintains 16 amplitude-weighted hypotheses for the current room state (empty, person in zone A/B/C/D, two persons, exercising, sleeping, etc.) and uses a Grover-inspired oracle+diffusion process to converge on the most likely state.

**Algorithm**: Inspired by Grover's quantum search algorithm, adapted for classical computation:

1. **Oracle**: CSI evidence (presence, motion, person count) multiplies hypothesis amplitudes by boost (1.3) or dampen (0.7) factors depending on consistency.
2. **Grover diffusion**: Reflects all amplitudes about their mean (a_i = 2*mean - a_i), concentrating probability mass on oracle-boosted hypotheses. Negative amplitudes are clamped to zero (classical approximation).
3. **Normalization**: Amplitudes are renormalized so sum-of-squares = 1.0 (probability conservation).

After enough iterations, the winner emerges with probability > 0.5 (convergence threshold).

#### The 16 Hypotheses

| Index | Hypothesis | Oracle Evidence |
|-------|-----------|----------------|
| 0 | Empty | presence=0 |
| 1-4 | Person in Zone A/B/C/D | presence=1, 1 person |
| 5 | Two Persons | n_persons=2 |
| 6 | Three Persons | n_persons>=3 |
| 7 | Moving Left | high motion, moving state |
| 8 | Moving Right | high motion, moving state |
| 9 | Sitting | low motion, present |
| 10 | Standing | low motion, present |
| 11 | Falling | high motion (transient) |
| 12 | Exercising | high motion, present |
| 13 | Sleeping | low motion, present |
| 14 | Cooking | moderate motion + moving |
| 15 | Working | low motion, present |

#### Public API

```rust
use wifi_densepose_wasm_edge::qnt_interference_search::{InterferenceSearch, Hypothesis};

let mut search = InterferenceSearch::new();               // const fn, uniform amplitudes
let events = search.process_frame(presence, motion_energy, n_persons);
let winner = search.winner();                             // Hypothesis enum
let prob = search.winner_probability();                   // [0, 1]
let converged = search.is_converged();                    // prob > 0.5
let amp = search.amplitude(Hypothesis::Sleeping);         // raw amplitude
let p = search.probability(Hypothesis::Exercising);       // amplitude^2
let iters = search.iterations();                          // total iterations
search.reset();                                           // back to uniform
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 855 | `EVENT_HYPOTHESIS_WINNER` | Winning hypothesis index (0-15) | Every 10 frames or on change |
| 856 | `EVENT_HYPOTHESIS_AMPLITUDE` | Winning hypothesis probability | Every 20 frames |
| 857 | `EVENT_SEARCH_ITERATIONS` | Total Grover iterations | Every 50 frames |

#### Configuration Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `N_HYPO` | 16 | Number of room-state hypotheses |
| `CONVERGENCE_PROB` | 0.5 | Threshold for declaring convergence |
| `ORACLE_BOOST` | 1.3 | Amplitude multiplier for supported hypotheses |
| `ORACLE_DAMPEN` | 0.7 | Amplitude multiplier for contradicted hypotheses |
| `MOTION_HIGH_THRESH` | 0.5 | Motion energy threshold for "high motion" |
| `MOTION_LOW_THRESH` | 0.15 | Motion energy threshold for "low motion" |

#### Example: Room State Classification

```
Initial state: All 16 hypotheses at probability 1/16 = 0.0625

Frames 1-30: presence=0, motion=0, n_persons=0
  Oracle boosts Empty (index 0), dampens all others
  Diffusion concentrates probability mass on Empty
  After 30 iterations: P(Empty) = 0.72, P(others) < 0.03
  -> EVENT_HYPOTHESIS_WINNER = 0 (Empty)

Frames 31-60: presence=1, motion=0.8, n_persons=1
  Oracle boosts Exercising, MovingLeft, MovingRight
  Oracle dampens Empty, Sitting, Sleeping
  After 30 more iterations: P(Exercising) = 0.45
  -> EVENT_HYPOTHESIS_WINNER = 12 (Exercising)
  Winner changed -> event emitted immediately

Frames 61-90: presence=1, motion=0.05, n_persons=1
  Oracle boosts Sitting, Sleeping, Working, Standing
  Oracle dampens Exercising, MovingLeft, MovingRight
  -> Convergence shifts to static hypotheses
```

---

## Autonomous Systems

| Module | File | What It Does | Event IDs | Budget |
|--------|------|--------------|-----------|--------|
| Psycho-Symbolic | `aut_psycho_symbolic.rs` | Context-aware inference using forward-chaining symbolic rules | 880-883 | H (<10 ms) |
| Self-Healing Mesh | `aut_self_healing_mesh.rs` | Monitors mesh node health and auto-reconfigures via min-cut analysis | 885-888 | S (<5 ms) |

---

### Psycho-Symbolic Inference (`aut_psycho_symbolic.rs`)

**What it does**: Interprets raw CSI-derived features into high-level semantic conclusions using a knowledge base of 16 forward-chaining rules. Given presence, motion energy, breathing rate, heart rate, person count, coherence, and time of day, it determines conclusions like "person resting", "possible intruder", "medical distress", or "social activity".

**Algorithm**: Forward-chaining rule evaluation. Each rule has 4 condition slots (feature_id, comparison_op, threshold). A rule fires when all non-disabled conditions match. Confidence propagation: the final confidence is the rule's base confidence multiplied by per-condition match-quality scores (how far above/below threshold the feature is, clamped to [0.5, 1.0]). Contradiction detection resolves mutually exclusive conclusions by keeping the higher-confidence one.

#### The 16 Rules

| Rule | Conclusion | Conditions | Base Confidence |
|------|-----------|------------|----------------|
| R0 | Possible Intruder | Presence + high motion (>=200) + night | 0.80 |
| R1 | Person Resting | Presence + low motion (<30) + breathing 10-22 BPM | 0.90 |
| R2 | Pet or Environment | No presence + motion (>=15) | 0.60 |
| R3 | Social Activity | Multi-person (>=2) + high motion (>=100) | 0.70 |
| R4 | Exercise | 1 person + high motion (>=150) + elevated HR (>=100) | 0.80 |
| R5 | Possible Fall | Presence + sudden stillness (motion<10, prev_motion>=150) | 0.70 |
| R6 | Interference | Low coherence (<0.4) + presence | 0.50 |
| R7 | Sleeping | Presence + very low motion (<5) + night + breathing (>=8) | 0.90 |
| R8 | Cooking Activity | Presence + moderate motion (40-120) + evening | 0.60 |
| R9 | Leaving Home | No presence + previous motion (>=50) + morning | 0.65 |
| R10 | Arriving Home | Presence + motion (>=60) + low prev_motion (<15) + evening | 0.70 |
| R11 | Child Playing | Multi-person (>=2) + very high motion (>=250) + daytime | 0.60 |
| R12 | Working at Desk | 1 person + low motion (<20) + good coherence (>=0.6) + morning | 0.75 |
| R13 | Medical Distress | Presence + very high HR (>=130) + low motion (<15) | 0.85 |
| R14 | Room Empty (Stable) | No presence + no motion (<5) + good coherence (>=0.6) | 0.95 |
| R15 | Crowd Gathering | Many persons (>=4) + high motion (>=120) | 0.70 |

#### Contradiction Pairs

These conclusions are mutually exclusive. When both fire, only the one with higher confidence survives:

| Pair A | Pair B |
|--------|--------|
| Sleeping | Exercise |
| Sleeping | Social Activity |
| Room Empty (Stable) | Possible Intruder |
| Person Resting | Exercise |

#### Input Features

| Index | Feature | Source | Range |
|-------|---------|--------|-------|
| 0 | Presence | Tier 2 DSP | 0 (absent) or 1 (present) |
| 1 | Motion Energy | Tier 2 DSP | 0 to ~1000 |
| 2 | Breathing BPM | Tier 2 vitals | 0-60 |
| 3 | Heart Rate BPM | Tier 2 vitals | 0-200 |
| 4 | Person Count | Tier 2 occupancy | 0-8 |
| 5 | Coherence | QuantumCoherenceMonitor or upstream | 0-1 |
| 6 | Time Bucket | Host clock | 0=morning, 1=afternoon, 2=evening, 3=night |
| 7 | Previous Motion | Internal (auto-tracked) | 0 to ~1000 |

#### Public API

```rust
use wifi_densepose_wasm_edge::aut_psycho_symbolic::PsychoSymbolicEngine;

let mut engine = PsychoSymbolicEngine::new();             // const fn
engine.set_coherence(0.8);                                // from upstream module
let events = engine.process_frame(
    presence, motion, breathing, heartrate, n_persons, time_bucket
);
let rules = engine.fired_rules();                         // u16 bitmap
let count = engine.fired_count();                         // number of rules that fired
let prev = engine.prev_conclusion();                      // last winning conclusion ID
let contras = engine.contradiction_count();                // total contradictions
engine.reset();                                           // clear state
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 880 | `EVENT_INFERENCE_RESULT` | Conclusion ID (1-16) | When any rule fires |
| 881 | `EVENT_INFERENCE_CONFIDENCE` | Confidence [0, 1] of the winning conclusion | Paired with result |
| 882 | `EVENT_RULE_FIRED` | Rule index (0-15) | For each rule that fired |
| 883 | `EVENT_CONTRADICTION` | Encoded pair: conclusion_a * 100 + conclusion_b | On contradiction |

#### Example: Fall Detection Sequence

```
Frame 1: Person walking briskly
  Features: presence=1, motion=200, breathing=20, HR=90, persons=1, time=1
  R4 (Exercise) fires: confidence = 0.80 * 0.75 = 0.60
  -> EVENT_INFERENCE_RESULT = 5 (Exercise)
  -> EVENT_INFERENCE_CONFIDENCE = 0.60

Frame 2: Sudden stillness (prev_motion=200, current motion=3)
  R5 (Possible Fall) fires: confidence = 0.70 * 0.85 = 0.595
  R1 (Person Resting) also fires: confidence = 0.90 * 0.50 = 0.45
  No contradiction between these two
  -> EVENT_RULE_FIRED = 5 (Fall rule)
  -> EVENT_RULE_FIRED = 1 (Resting rule)
  -> EVENT_INFERENCE_RESULT = 6 (Possible Fall, highest confidence)
  -> EVENT_INFERENCE_CONFIDENCE = 0.595
```

---

### Self-Healing Mesh (`aut_self_healing_mesh.rs`)

**What it does**: Monitors the health of an 8-node sensor mesh and automatically detects when the network topology becomes fragile. Uses the Stoer-Wagner minimum graph cut algorithm to find the weakest link in the mesh. When the min-cut value drops below a threshold, it identifies the degraded node and triggers a reconfiguration event.

**Algorithm**: Stoer-Wagner min-cut on a weighted graph of up to 8 nodes. Edge weights are the minimum quality score of the two endpoints (min(q_i, q_j)). Quality scores are EMA-smoothed (alpha=0.15) per-node CSI coherence values. O(n^3) complexity, which is only 512 operations for n=8. State machine transitions between healthy and healing modes.

#### Public API

```rust
use wifi_densepose_wasm_edge::aut_self_healing_mesh::SelfHealingMesh;

let mut mesh = SelfHealingMesh::new();                    // const fn
mesh.update_node_quality(0, coherence);                   // update single node
let events = mesh.process_frame(&node_qualities);         // process all nodes
let q = mesh.node_quality(2);                             // EMA quality for node 2
let n = mesh.active_nodes();                              // count
let mc = mesh.prev_mincut();                              // last min-cut value
let healing = mesh.is_healing();                          // fragile state?
let weak = mesh.weakest_node();                           // node ID or 0xFF
mesh.reset();                                             // clear state
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 885 | `EVENT_NODE_DEGRADED` | Index of the degraded node (0-7) | When min-cut < 0.3 |
| 886 | `EVENT_MESH_RECONFIGURE` | Min-cut value (measure of fragility) | Paired with degraded |
| 887 | `EVENT_COVERAGE_SCORE` | Mean quality across all active nodes [0, 1] | Every frame |
| 888 | `EVENT_HEALING_COMPLETE` | Min-cut value (now healthy) | When min-cut recovers >= 0.6 |

#### Configuration Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_NODES` | 8 | Maximum mesh nodes |
| `QUALITY_ALPHA` | 0.15 | EMA smoothing for node quality |
| `MINCUT_FRAGILE` | 0.3 | Below this, mesh is considered fragile |
| `MINCUT_HEALTHY` | 0.6 | Above this, healing is considered complete |

#### State Machine

```
                 mincut < 0.3
  [Healthy] ----------------------> [Healing]
      ^                                 |
      |         mincut >= 0.6           |
      +---------------------------------+
```

#### Stoer-Wagner Min-Cut Details

The algorithm finds the minimum weight of edges that, if removed, would disconnect the graph into two components. For an 8-node mesh:

1. Start with the full weighted adjacency matrix
2. For each phase (n-1 phases total):
   - Grow a set A by repeatedly adding the node with the highest total edge weight to A
   - The last two nodes added (prev, last) define a "cut of the phase" = weight to last
   - Track the global minimum cut across all phases
   - Merge the last two nodes (combine their edge weights)
3. Return (global_min_cut, node_on_lighter_side)

#### Example: Node Failure and Recovery

```
Frame 1: All 4 nodes healthy
  qualities = [0.9, 0.85, 0.88, 0.92]
  Coverage = 0.89
  Min-cut = 0.85 (well above 0.6)
  -> EVENT_COVERAGE_SCORE = 0.89

Frame 50: Node 1 starts degrading
  qualities = [0.9, 0.20, 0.88, 0.92]
  EMA-smoothed quality[1] drops gradually
  Min-cut drops to 0.20 (edge weights use min(q_i, q_j))
  Min-cut < 0.3 -> FRAGILE!
  -> EVENT_NODE_DEGRADED = 1
  -> EVENT_MESH_RECONFIGURE = 0.20
  -> Mesh enters healing mode

  Host firmware can now:
  - Increase node 1's transmit power
  - Route traffic around node 1
  - Wake up a backup node
  - Alert the operator

Frame 100: Node 1 recovers (antenna repositioned)
  qualities = [0.9, 0.85, 0.88, 0.92]
  Min-cut climbs back to 0.85
  Min-cut >= 0.6 -> HEALTHY!
  -> EVENT_HEALING_COMPLETE = 0.85
```

---

## How Quantum-Inspired Algorithms Help WiFi Sensing

These modules use quantum computing metaphors -- not because the ESP32 is a quantum computer, but because the mathematical frameworks from quantum mechanics map naturally onto CSI signal analysis:

**Bloch Sphere / Coherence**: WiFi subcarrier phases behave like quantum phases. When multipath is stable, all phases align (pure state). When the environment changes, phases randomize (mixed state). The Von Neumann entropy quantifies this exactly, providing a single scalar "change detector" that is more robust than tracking individual subcarrier phases.

**Grover's Algorithm / Hypothesis Search**: The oracle+diffusion loop is a principled way to combine evidence from multiple noisy sensors. Instead of hard-coding "if motion > 0.5 then exercising", the Grover-inspired search lets multiple hypotheses compete. Evidence gradually amplifies the correct hypothesis while suppressing incorrect ones. This is more robust to noisy CSI data than a single threshold.

**Why not just use classical statistics?** You could. But the quantum-inspired formulations have three practical advantages on embedded hardware:

1. **Fixed memory**: The Bloch vector is always 3 floats. The hypothesis array is always 16 floats. No dynamic allocation needed.
2. **Graceful degradation**: If CSI data is noisy, the Grover search does not crash or give a wrong answer immediately -- it just converges more slowly.
3. **Composability**: The coherence score from the Bloch sphere module feeds directly into the Temporal Logic Guard (rule 3: "no vital signs when coherence < 0.3") and the Psycho-Symbolic engine (feature 5: coherence). This creates a pipeline where quantum-inspired metrics inform classical reasoning.

---

## Memory Layout

| Module | State Size (approx) | Static Event Buffer |
|--------|---------------------|---------------------|
| Quantum Coherence | ~40 bytes (3D Bloch vector + 2 entropy floats + counter) | 3 entries |
| Interference Search | ~80 bytes (16 amplitudes + counters) | 3 entries |
| Psycho-Symbolic | ~24 bytes (bitmap + counters + prev_motion) | 8 entries |
| Self-Healing Mesh | ~360 bytes (8x8 adjacency + 8 qualities + state) | 6 entries |

All modules use fixed-size arrays and static event buffers. No heap allocation. Fully no_std compliant for WASM3 deployment on ESP32-S3.

---

## Cross-Module Integration

These modules are designed to work together in a pipeline:

```
CSI Frame (Tier 2 DSP)
    |
    v
[Quantum Coherence] --coherence--> [Psycho-Symbolic Engine]
    |                                     |
    v                                     v
[Interference Search]              [Inference Result]
    |                                     |
    v                                     v
[Room State Hypothesis]            [GOAP Planner]
                                         |
                                         v
                                   [Module Activate/Deactivate]
                                         |
                                         v
                                   [Self-Healing Mesh]
                                         |
                                         v
                                   [Reconfiguration Events]
```

The Quantum Coherence monitor feeds its coherence score to:
- **Psycho-Symbolic Engine**: As feature 5 (coherence), enabling rules R3 (interference) and R6 (low coherence)
- **Temporal Logic Guard**: Rule 3 checks "no vital signs when coherence < 0.3"
- **Self-Healing Mesh**: Node quality can be derived from coherence

The GOAP Planner uses inference results to decide which modules to activate (e.g., activate vitals monitoring when a person is present, enter low-power mode when the room is empty).

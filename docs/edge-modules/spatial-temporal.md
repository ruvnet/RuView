# Spatial & Temporal Intelligence -- WiFi-DensePose Edge Intelligence

> Location awareness, activity patterns, and autonomous decision-making running on the ESP32 chip. These modules figure out where people are, learn daily routines, verify safety rules, and let the device plan its own actions.

## Spatial Reasoning

| Module | File | What It Does | Event IDs | Budget |
|--------|------|--------------|-----------|--------|
| PageRank Influence | `spt_pagerank_influence.rs` | Finds the dominant person in multi-person scenes using cross-correlation PageRank | 760-762 | S (<5 ms) |
| Micro-HNSW | `spt_micro_hnsw.rs` | On-device approximate nearest-neighbor search for CSI fingerprint matching | 765-768 | S (<5 ms) |
| Spiking Tracker | `spt_spiking_tracker.rs` | Bio-inspired person tracking using LIF neurons with STDP learning | 770-773 | M (<8 ms) |

---

### PageRank Influence (`spt_pagerank_influence.rs`)

**What it does**: Figures out which person in a multi-person scene has the strongest WiFi signal influence, using the same math Google uses to rank web pages. Up to 4 persons are modelled as graph nodes; edge weights come from the normalized cross-correlation of their subcarrier phase groups (8 subcarriers per person).

**Algorithm**: 4x4 weighted adjacency graph built from abs(dot-product) / (norm_a * norm_b) cross-correlation. Standard PageRank power iteration with damping factor 0.85, 10 iterations, column-normalized transition matrix. Ranks are normalized to sum to 1.0 after each iteration.

#### Public API

```rust
use wifi_densepose_wasm_edge::spt_pagerank_influence::PageRankInfluence;

let mut pr = PageRankInfluence::new();          // const fn, zero-alloc
let events = pr.process_frame(&phases, 2);      // phases: &[f32], n_persons: usize
let score = pr.rank(0);                         // PageRank score for person 0
let dom = pr.dominant_person();                  // index of dominant person
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 760 | `EVENT_DOMINANT_PERSON` | Person index (0-3) | Every frame |
| 761 | `EVENT_INFLUENCE_SCORE` | PageRank score of dominant person [0, 1] | Every frame |
| 762 | `EVENT_INFLUENCE_CHANGE` | Encoded person_id + signed delta (fractional) | When rank shifts > 0.05 |

#### Configuration Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_PERSONS` | 4 | Maximum tracked persons |
| `SC_PER_PERSON` | 8 | Subcarriers assigned per person group |
| `DAMPING` | 0.85 | PageRank damping factor (standard) |
| `PR_ITERS` | 10 | Power-iteration rounds |
| `CHANGE_THRESHOLD` | 0.05 | Minimum rank change to emit change event |

#### Example: Detecting the Dominant Speaker in a Room

When multiple people are present, the person moving the most creates the strongest CSI disturbance. PageRank identifies which person's signal "influences" the others most strongly.

```
Frame 1: Person 0 speaking (active), Person 1 seated
  -> EVENT_DOMINANT_PERSON = 0, EVENT_INFLUENCE_SCORE = 0.62

Frame 50: Person 1 stands and walks
  -> EVENT_DOMINANT_PERSON = 1, EVENT_INFLUENCE_SCORE = 0.58
  -> EVENT_INFLUENCE_CHANGE (person 1 rank increased by 0.08)
```

#### How It Works (Step by Step)

1. Host reports `n_persons` and provides up to 32 subcarrier phases
2. Module groups subcarriers: person 0 gets phases[0..8], person 1 gets phases[8..16], etc.
3. Cross-correlation is computed between every pair of person groups (abs cosine similarity)
4. A 4x4 adjacency matrix is built (no self-loops)
5. PageRank power iteration runs 10 times with damping=0.85
6. The person with the highest rank is reported as the dominant person
7. If any person's rank changed by more than 0.05 since last frame, a change event fires

---

### Micro-HNSW (`spt_micro_hnsw.rs`)

**What it does**: Stores up to 64 reference CSI fingerprint vectors (8 dimensions each) in a single-layer navigable small-world graph, enabling fast approximate nearest-neighbor lookup. When the sensor sees a new CSI pattern, it finds the most similar stored reference and returns its classification label.

**Algorithm**: HNSW (Hierarchical Navigable Small World) simplified to a single layer for embedded use. 64 nodes, 4 neighbors per node, beam search width 4, maximum 8 hops. L2 (Euclidean) distance. Bidirectional edges with worst-neighbor replacement pruning when a node is full.

#### Public API

```rust
use wifi_densepose_wasm_edge::spt_micro_hnsw::MicroHnsw;

let mut hnsw = MicroHnsw::new();                     // const fn, zero-alloc
let idx = hnsw.insert(&features_8d, label);           // Option<usize>
let (nearest_id, distance) = hnsw.search(&query_8d);  // (usize, f32)
let events = hnsw.process_frame(&features);            // per-frame query
let label = hnsw.last_label();                         // u8 or 255=unknown
let dist = hnsw.last_match_distance();                 // f32
let n = hnsw.size();                                   // number of stored vectors
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 765 | `EVENT_NEAREST_MATCH_ID` | Index of nearest stored vector | Every frame |
| 766 | `EVENT_MATCH_DISTANCE` | L2 distance to nearest match | Every frame |
| 767 | `EVENT_CLASSIFICATION` | Label of nearest match (255 if too far) | Every frame |
| 768 | `EVENT_LIBRARY_SIZE` | Number of stored reference vectors | Every frame |

#### Configuration Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_VECTORS` | 64 | Maximum stored reference fingerprints |
| `DIM` | 8 | Dimensions per feature vector |
| `MAX_NEIGHBORS` | 4 | Edges per node in the graph |
| `BEAM_WIDTH` | 4 | Search beam width (quality vs speed) |
| `MAX_HOPS` | 8 | Maximum graph traversal depth |
| `MATCH_THRESHOLD` | 2.0 | Distance above which classification returns "unknown" |

#### Example: Room Location Fingerprinting

Pre-load reference CSI fingerprints for known locations, then classify new readings in real-time.

```
Setup:
  hnsw.insert(&kitchen_fingerprint, 1);   // label 1 = kitchen
  hnsw.insert(&bedroom_fingerprint, 2);   // label 2 = bedroom
  hnsw.insert(&bathroom_fingerprint, 3);  // label 3 = bathroom

Runtime:
  Frame arrives with features = [0.32, 0.15, ...]
  -> EVENT_NEAREST_MATCH_ID = 1 (kitchen reference)
  -> EVENT_MATCH_DISTANCE = 0.45
  -> EVENT_CLASSIFICATION = 1 (kitchen)
  -> EVENT_LIBRARY_SIZE = 3
```

#### How It Works (Step by Step)

1. **Insert**: New vector is added at position `n_vectors`. The module scans all existing nodes (N<=64, so linear scan is fine) to find the 4 nearest neighbors. Bidirectional edges are added; if a node already has 4 neighbors, the worst (farthest) is replaced if the new connection is shorter.
2. **Search**: Starting from the entry point, a beam search (width 4) explores neighbor nodes for up to 8 hops. Each hop expands unvisited neighbors of the current beam and inserts closer ones. Search terminates when no hop improves the beam.
3. **Classify**: If the nearest match distance is below `MATCH_THRESHOLD` (2.0), its label is returned. Otherwise, 255 (unknown).

---

### Spiking Tracker (`spt_spiking_tracker.rs`)

**What it does**: Tracks a person's location across 4 spatial zones using a biologically inspired spiking neural network. 32 Leaky Integrate-and-Fire (LIF) neurons (one per subcarrier) feed into 4 output neurons (one per zone). The zone with the highest spike rate indicates the person's location. Zone transitions measure velocity.

**Algorithm**: LIF neuron model with membrane leak factor 0.95, threshold 1.0, reset to 0.0. STDP (Spike-Timing-Dependent Plasticity) learning: potentiation LR=0.01 when pre+post fire within 1 frame, depression LR=0.005 when only pre fires. Weights clamped to [0, 2]. EMA smoothing on zone spike rates (alpha=0.1).

#### Public API

```rust
use wifi_densepose_wasm_edge::spt_spiking_tracker::SpikingTracker;

let mut st = SpikingTracker::new();                       // const fn
let events = st.process_frame(&phases, &prev_phases);     // returns events
let zone = st.current_zone();                             // i8, -1 if lost
let rate = st.zone_spike_rate(0);                         // f32 for zone 0
let vel = st.velocity();                                  // EMA velocity
let tracking = st.is_tracking();                          // bool
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 770 | `EVENT_TRACK_UPDATE` | Zone ID (0-3) | When tracked |
| 771 | `EVENT_TRACK_VELOCITY` | Zone transitions/frame (EMA) | When tracked |
| 772 | `EVENT_SPIKE_RATE` | Mean spike rate across zones [0, 1] | Every frame |
| 773 | `EVENT_TRACK_LOST` | Last known zone ID | When track lost |

#### Configuration Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `N_INPUT` | 32 | Input neurons (one per subcarrier) |
| `N_OUTPUT` | 4 | Output neurons (one per zone) |
| `THRESHOLD` | 1.0 | LIF firing threshold |
| `LEAK` | 0.95 | Membrane decay per frame |
| `STDP_LR_PLUS` | 0.01 | Potentiation learning rate |
| `STDP_LR_MINUS` | 0.005 | Depression learning rate |
| `W_MIN` / `W_MAX` | 0.0 / 2.0 | Weight bounds |
| `MIN_SPIKE_RATE` | 0.05 | Minimum rate to consider zone active |

#### Example: Tracking Movement Between Zones

```
Frames 1-30: Strong phase changes in subcarriers 0-7 (zone 0)
  -> EVENT_TRACK_UPDATE = 0, EVENT_SPIKE_RATE = 0.15

Frames 31-60: Activity shifts to subcarriers 16-23 (zone 2)
  -> EVENT_TRACK_UPDATE = 2, EVENT_TRACK_VELOCITY = 0.033
  STDP strengthens zone 2 connections, weakens zone 0

Frames 61-90: No activity
  -> Spike rates decay via EMA
  -> EVENT_TRACK_LOST = 2 (last known zone)
```

#### How It Works (Step by Step)

1. Phase deltas (|current - previous|) inject current into LIF neurons
2. Each neuron leaks (membrane *= 0.95), then adds current
3. If membrane >= threshold (1.0), the neuron fires and resets to 0
4. Input spikes propagate to output zones via weighted connections
5. Output neurons fire when cumulative input exceeds threshold
6. STDP adjusts weights: correlated pre+post firing strengthens connections, uncorrelated pre firing weakens them (sparse iteration skips silent neurons for 70-90% savings)
7. Zone spike rates are EMA-smoothed; the zone with the highest rate above `MIN_SPIKE_RATE` is reported as the tracked location

---

## Temporal Analysis

| Module | File | What It Does | Event IDs | Budget |
|--------|------|--------------|-----------|--------|
| Pattern Sequence | `tmp_pattern_sequence.rs` | Learns daily activity routines and detects deviations | 790-793 | S (<5 ms) |
| Temporal Logic Guard | `tmp_temporal_logic_guard.rs` | Verifies 8 LTL safety invariants on every frame | 795-797 | S (<5 ms) |
| GOAP Autonomy | `tmp_goap_autonomy.rs` | Autonomous module management via A* goal-oriented planning | 800-803 | S (<5 ms) |

---

### Pattern Sequence (`tmp_pattern_sequence.rs`)

**What it does**: Learns daily activity routines and alerts when something changes. Each minute is discretized into a motion symbol (Empty, Still, LowMotion, HighMotion, MultiPerson), stored in a 24-hour circular buffer (1440 entries). An hourly LCS (Longest Common Subsequence) comparison between today and yesterday yields a routine confidence score. If grandma usually goes to the kitchen by 8am but has not moved, it notices.

**Algorithm**: Two-row dynamic programming LCS with O(n) memory (60-entry comparison window). Majority-vote symbol selection from per-frame accumulation. Two-day history buffer with day rollover.

#### Public API

```rust
use wifi_densepose_wasm_edge::tmp_pattern_sequence::PatternSequenceAnalyzer;

let mut psa = PatternSequenceAnalyzer::new();            // const fn
psa.on_frame(presence, motion, n_persons);               // called per CSI frame (~20 Hz)
let events = psa.on_timer();                             // called at ~1 Hz
let conf = psa.routine_confidence();                     // [0, 1]
let n = psa.pattern_count();                             // stored patterns
let min = psa.current_minute();                          // 0-1439
let day = psa.day_offset();                              // days since start
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 790 | `EVENT_PATTERN_DETECTED` | LCS length of detected pattern | Hourly |
| 791 | `EVENT_PATTERN_CONFIDENCE` | Routine confidence [0, 1] | Hourly |
| 792 | `EVENT_ROUTINE_DEVIATION` | Minute index where deviation occurred | Per minute (when deviating) |
| 793 | `EVENT_PREDICTION_NEXT` | Predicted next-minute symbol (from yesterday) | Per minute |

#### Configuration Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DAY_LEN` | 1440 | Minutes per day |
| `MAX_PATTERNS` | 32 | Maximum stored pattern templates |
| `PATTERN_LEN` | 16 | Maximum symbols per pattern |
| `LCS_WINDOW` | 60 | Comparison window (1 hour) |
| `THRESH_STILL` / `THRESH_LOW` / `THRESH_HIGH` | 0.05 / 0.3 / 0.7 | Motion discretization thresholds |

#### Symbols

| Symbol | Value | Condition |
|--------|-------|-----------|
| Empty | 0 | No presence |
| Still | 1 | Present, motion < 0.05 |
| LowMotion | 2 | Present, 0.3 < motion <= 0.7 |
| HighMotion | 3 | Present, motion > 0.7 |
| MultiPerson | 4 | More than 1 person present |

#### Example: Elderly Care Routine Monitoring

```
Day 1: Learning phase
  07:00 - Still (person in bed)
  07:30 - HighMotion (getting ready)
  08:00 - LowMotion (breakfast)
  -> Patterns stored in history buffer

Day 2: Comparison active
  07:00 - Still (normal)
  07:30 - Still (DEVIATION! Expected HighMotion)
    -> EVENT_ROUTINE_DEVIATION = 450 (minute 7:30)
    -> EVENT_PREDICTION_NEXT = 3 (HighMotion expected)
  08:30 - Still (still no activity)
    -> Caregiver notified via DEVIATION events
```

---

### Temporal Logic Guard (`tmp_temporal_logic_guard.rs`)

**What it does**: Encodes 8 safety rules as Linear Temporal Logic (LTL) state machines. G-rules ("globally") are violated on any single frame. F-rules ("eventually") have deadlines. Every frame, the guard checks all rules and emits violations with counterexample frame indices.

**Algorithm**: State machine per rule (Satisfied/Pending/Violated). G-rules use immediate boolean checks. F-rules use deadline counters (frame-based). Counterexample tracking records the frame index when violation first occurs.

#### The 8 Safety Rules

| Rule | Type | Description | Violation Condition |
|------|------|-------------|---------------------|
| R0 | G | No fall alert when room is empty | `presence==0 AND fall_alert` |
| R1 | G | No intrusion alert when nobody present | `intrusion_alert AND presence==0` |
| R2 | G | No person ID active when nobody detected | `n_persons==0 AND person_id_active` |
| R3 | G | No vital signs when coherence is too low | `coherence<0.3 AND vital_signs_active` |
| R4 | F | Continuous motion must stop within 300s | Motion > 0.1 for 6000 consecutive frames |
| R5 | F | Fast breathing must trigger alert within 5s | Breathing > 40 BPM for 100 consecutive frames |
| R6 | G | Heart rate must not exceed 150 BPM | `heartrate_bpm > 150` |
| R7 | G-F | After seizure, no normal gait within 60s | Normal gait reported < 1200 frames after seizure |

#### Public API

```rust
use wifi_densepose_wasm_edge::tmp_temporal_logic_guard::{TemporalLogicGuard, FrameInput};

let mut guard = TemporalLogicGuard::new();               // const fn
let events = guard.on_frame(&input);                     // per-frame check
let satisfied = guard.satisfied_count();                 // how many rules OK
let state = guard.rule_state(4);                         // Satisfied/Pending/Violated
let vio = guard.violation_count(0);                      // total violations for rule 0
let frame = guard.last_violation_frame(3);               // frame index of last violation
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 795 | `EVENT_LTL_VIOLATION` | Rule index (0-7) | On violation |
| 796 | `EVENT_LTL_SATISFACTION` | Count of currently satisfied rules | Every 200 frames |
| 797 | `EVENT_COUNTEREXAMPLE` | Frame index when violation occurred | Paired with violation |

---

### GOAP Autonomy (`tmp_goap_autonomy.rs`)

**What it does**: Lets the ESP32 autonomously decide which sensing modules to activate or deactivate based on the current situation. Uses Goal-Oriented Action Planning (GOAP) with A* search over an 8-bit boolean world state to find the cheapest action sequence that achieves the highest-priority unsatisfied goal.

**Algorithm**: A* search over 8-bit world state. 6 prioritized goals, 8 actions with preconditions and effects encoded as bitmasks. Maximum plan depth 4, open set capacity 32. Replans every 60 seconds.

#### World State Properties

| Bit | Property | Meaning |
|-----|----------|---------|
| 0 | `has_presence` | Room occupancy detected |
| 1 | `has_motion` | Motion energy above threshold |
| 2 | `is_night` | Nighttime period |
| 3 | `multi_person` | More than 1 person present |
| 4 | `low_coherence` | Signal quality is degraded |
| 5 | `high_threat` | Threat score above threshold |
| 6 | `has_vitals` | Vital sign monitoring active |
| 7 | `is_learning` | Pattern learning active |

#### Goals (Priority Order)

| # | Goal | Priority | Condition |
|---|------|----------|-----------|
| 0 | Monitor Health | 0.9 | Achieve `has_vitals = true` |
| 1 | Secure Space | 0.8 | Achieve `has_presence = true` |
| 2 | Count People | 0.7 | Achieve `multi_person = false` |
| 3 | Learn Patterns | 0.5 | Achieve `is_learning = true` |
| 4 | Save Energy | 0.3 | Achieve `is_learning = false` |
| 5 | Self Test | 0.1 | Achieve `low_coherence = false` |

#### Actions

| # | Action | Precondition | Effect | Cost |
|---|--------|-------------|--------|------|
| 0 | Activate Vitals | Presence required | Sets `has_vitals` | 2 |
| 1 | Activate Intrusion | None | Sets `has_presence` | 1 |
| 2 | Activate Occupancy | Presence required | Clears `multi_person` | 2 |
| 3 | Activate Gesture Learn | Low coherence must be false | Sets `is_learning` | 3 |
| 4 | Deactivate Heavy | None | Clears `is_learning` + `has_vitals` | 1 |
| 5 | Run Coherence Check | None | Clears `low_coherence` | 2 |
| 6 | Enter Low Power | None | Clears `is_learning` + `has_motion` | 1 |
| 7 | Run Self Test | None | Clears `low_coherence` + `high_threat` | 3 |

#### Public API

```rust
use wifi_densepose_wasm_edge::tmp_goap_autonomy::GoapPlanner;

let mut planner = GoapPlanner::new();                    // const fn
planner.update_world(presence, motion, n_persons,
                     coherence, threat, has_vitals, is_night);
let events = planner.on_timer();                         // called at ~1 Hz
let ws = planner.world_state();                          // u8 bitmask
let goal = planner.current_goal();                       // goal index or 0xFF
let len = planner.plan_len();                            // steps in current plan
planner.set_goal_priority(0, 0.95);                      // dynamically adjust
```

#### Events

| Event ID | Constant | Value | Frequency |
|----------|----------|-------|-----------|
| 800 | `EVENT_GOAL_SELECTED` | Goal index (0-5) | On replan |
| 801 | `EVENT_MODULE_ACTIVATED` | Action index that activated a module | On plan step |
| 802 | `EVENT_MODULE_DEACTIVATED` | Action index that deactivated a module | On plan step |
| 803 | `EVENT_PLAN_COST` | Total cost of the planned action sequence | On replan |

#### Example: Autonomous Night-Mode Transition

```
18:00 - World state: presence=1, motion=0, night=0, vitals=1
  Goal 0 (Monitor Health) satisfied, Goal 1 (Secure Space) satisfied
  -> Goal 2 selected (Count People, prio 0.7)

22:00 - World state: presence=0, motion=0, night=1
  -> Goal 1 selected (Secure Space, prio 0.8)
  -> Plan: [Action 1: Activate Intrusion] (cost=1)
  -> EVENT_GOAL_SELECTED = 1
  -> EVENT_MODULE_ACTIVATED = 1 (intrusion detection)
  -> EVENT_PLAN_COST = 1

03:00 - No presence, low coherence detected
  -> Goal 5 selected (Self Test, prio 0.1)
  -> Plan: [Action 5: Run Coherence Check] (cost=2)
```

---

## Memory Layout Summary

All modules use fixed-size arrays and static event buffers. No heap allocation.

| Module | State Size (approx) | Static Event Buffer |
|--------|---------------------|---------------------|
| PageRank Influence | ~192 bytes (4x4 adj + 2x4 rank + meta) | 8 entries |
| Micro-HNSW | ~3.5 KB (64 nodes x 48 bytes + meta) | 4 entries |
| Spiking Tracker | ~1.1 KB (32x4 weights + membranes + rates) | 4 entries |
| Pattern Sequence | ~3.2 KB (2x1440 history + 32 patterns + LCS rows) | 4 entries |
| Temporal Logic Guard | ~120 bytes (8 rules + counters) | 12 entries |
| GOAP Autonomy | ~1.6 KB (32 open-set nodes + goals + plan) | 4 entries |

## Integration with Host Firmware

These modules receive data from the ESP32 Tier 2 DSP pipeline via the WASM3 host API:

```
ESP32 Firmware (C)          WASM3 Runtime            WASM Module (Rust)
       |                         |                         |
  CSI frame arrives              |                         |
  Tier 2 DSP runs                |                         |
       |--- csi_get_phase() ---->|--- host_get_phase() --->|
       |--- csi_get_presence() ->|--- host_get_presence()->|
       |                         |     process_frame()     |
       |<-- csi_emit_event() ----|<-- host_emit_event() ---|
       |                         |                         |
  Forward to aggregator          |                         |
```

Modules can be hot-loaded via OTA (ADR-040) without reflashing the firmware.

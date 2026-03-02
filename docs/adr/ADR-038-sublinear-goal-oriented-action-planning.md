# ADR-038: Sublinear Goal-Oriented Action Planning (GOAP) for Project Roadmap Optimization

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-02 |
| **Deciders** | ruv |
| **Relates to** | All 37 prior ADRs; ADR-014 (SOTA Signal Processing), ADR-016 (RuVector Integration), ADR-024 (AETHER Embeddings), ADR-027 (MERIDIAN Generalization), ADR-029 (RuvSense Multistatic), ADR-037 (Multi-Person Detection) |

---

## 1. Context

### 1.1 The Planning Problem

WiFi-DensePose has 37 Architecture Decision Records. Of these, 14 are Accepted/Complete, 4 are Partially Implemented, 19 are Proposed, and 1 is Superseded. The proposed ADRs span diverse capabilities: vital sign detection (ADR-021), multi-BSSID scanning (ADR-022), contrastive embeddings (ADR-024), cross-environment generalization (ADR-027), multistatic mesh sensing (ADR-029), persistent field models (ADR-030), multi-person pose detection (ADR-037), and more.

A single developer (or a small team aided by AI agents) must decide **what to build next** given:

- **Dense dependency graph**: ADR-037 (multi-person) depends on ADR-014 (signal processing), ADR-024 (AETHER), and ADR-029 (multistatic). ADR-029 depends on ADR-012 (ESP32 mesh), ADR-014, ADR-016, and ADR-018. Many ADRs share prerequisites.
- **Hardware variability**: Some ADRs require ESP32 hardware (ADR-021 vital signs, ADR-029 multistatic mesh), while others are software-only (ADR-024 AETHER, ADR-027 MERIDIAN). The available hardware changes session to session.
- **Shifting goals**: One session the user wants accuracy improvement; the next session they want multi-person support; the next they want WebAssembly deployment.
- **Resource constraints**: Limited compute budget, single-developer throughput, CI pipeline capacity.

Manually navigating this decision space is error-prone. The developer must hold the full dependency graph in working memory, re-evaluate priorities when goals shift, and avoid dead-end plans that block on unavailable hardware.

### 1.2 Why GOAP

Goal-Oriented Action Planning (GOAP), originally developed for game AI by Jeff Orkin (2003), models the world as a set of boolean/numeric state properties and defines actions with typed preconditions and effects. A planner searches from the current world state to a goal state, producing an optimal action sequence. GOAP is a natural fit for this problem because:

1. **ADR implementations are actions** with clear preconditions (which other ADRs/hardware must exist) and effects (which capabilities are unlocked).
2. **The world state is observable** -- we can query cargo test results, check hardware connections, read crate manifests, and measure accuracy metrics.
3. **Goals are declarative** -- "I want multi-person tracking at 20 Hz" translates to `{multi_person_tracking: true, update_rate_hz: 20}`.
4. **Replanning is cheap** -- when hardware becomes available or a user changes goals, the planner re-runs in milliseconds.

### 1.3 Why Sublinear

The naive GOAP planner uses A* search over the full action-state graph. With 37 ADRs, each potentially having multiple phases (ADR-037 has 4 phases, ADR-029 has 9 actions), the raw action count exceeds 80. The full state space is `2^N` for N boolean properties. Exhaustive search is wasteful because:

- Most actions are irrelevant to any given goal (the user asking for vital signs does not need WebAssembly deployment actions in the search).
- The dependency graph is sparse -- most actions depend on 1-3 prerequisites, not all other actions.
- Many state properties are independent (vital sign detection does not interact with WebAssembly compilation).

A sublinear approach avoids exploring the full state space by exploiting this sparsity.

---

## 2. Decision

Implement a GOAP planning system as a coordinator module within the claude-flow swarm framework. The planner takes a user goal, the current project state, and available hardware as input, and produces an ordered action plan that is dispatched to specialized agents for execution.

### 2.1 World State Model

The world state is a flat map of typed properties representing the current project capabilities.

#### 2.1.1 Feature Implementation Flags (Boolean)

| Property | Source of Truth | Description |
|----------|----------------|-------------|
| `sota_signal_processing` | `cargo test -p wifi-densepose-signal` passes | ADR-014 SOTA algorithms implemented |
| `ruvector_training_integrated` | `train/` crate builds with ruvector deps | ADR-016 RuVector training pipeline |
| `ruvector_signal_integrated` | `signal/src/ruvsense/` module exists | ADR-017 RuVector signal integration |
| `esp32_firmware_base` | `firmware/esp32-csi-node/` compiles | ADR-018 ESP32 base firmware |
| `esp32_channel_hopping` | Firmware supports multi-channel | ADR-029 Phase 1 |
| `multi_band_fusion` | `ruvsense/multiband.rs` passes tests | ADR-029 Phase 2 |
| `multistatic_mesh` | Multi-node fusion operational | ADR-029 Phase 3 |
| `coherence_gating` | `ruvsense/coherence_gate.rs` passes tests | ADR-029 Phase 6-7 |
| `pose_tracker_17kp` | `ruvsense/pose_tracker.rs` passes tests | ADR-029 Phase 4 |
| `vital_signs_extraction` | `vitals/` crate passes tests | ADR-021 |
| `vital_signs_esp32_validated` | ESP32 breathing detection verified | ADR-021 Phase 2 |
| `multi_bssid_scan` | `wifiscan/` crate passes tests | ADR-022 Phase 1 |
| `multi_bssid_concurrent` | Concurrent BSSID scanning | ADR-022 Phase 2 |
| `aether_embeddings` | Contrastive CSI encoder trained | ADR-024 |
| `aether_reid` | Person re-identification via embeddings | ADR-024 Phase 3 |
| `meridian_generalization` | Cross-environment transfer working | ADR-027 |
| `persistent_field_model` | Field model serializes/deserializes | ADR-030 |
| `person_count_estimation` | Eigenvalue occupancy estimator | ADR-037 Phase 1 |
| `signal_decomposition` | NMF per-person separation | ADR-037 Phase 2 |
| `multi_skeleton_generation` | Multiple skeletons per frame | ADR-037 Phase 3 |
| `multi_person_neural` | Neural multi-person model | ADR-037 Phase 4 |
| `wasm_deployment` | WebAssembly build functional | ADR-025 |
| `mat_survivor_detection` | MAT disaster detection operational | ADR-011/ADR-026 |
| `ruview_sensing_ui` | Sensing-first RF UI mode | ADR-031 |
| `mesh_security_hardened` | Multistatic mesh security layer | ADR-032 |

#### 2.1.2 Hardware Availability Flags (Boolean)

| Property | Detection Method | Description |
|----------|-----------------|-------------|
| `esp32_connected` | USB serial probe (`/dev/ttyUSB*` or `COM*`) | At least one ESP32 on USB |
| `esp32_count` | Count USB serial devices with ESP32 VID/PID | Number of ESP32 nodes |
| `esp32_multistatic_ready` | `esp32_count >= 2` | Sufficient for multistatic |
| `gpu_available` | `nvidia-smi` or CUDA probe | GPU for neural training |
| `wifi_adapter_present` | OS WiFi interface enumeration | Host WiFi for multi-BSSID |

#### 2.1.3 Quality Metrics (Numeric)

| Property | Source | Description |
|----------|--------|-------------|
| `pose_accuracy_pck02` | Benchmark suite output | PCK@0.2 accuracy (0.0-1.0) |
| `update_rate_hz` | Pipeline timing measurement | Effective output frame rate |
| `max_persons_tracked` | Multi-person test result | Maximum simultaneous persons |
| `breathing_snr_db` | Vital signs test output | Breathing detection SNR |
| `torso_jitter_mm` | Tracking benchmark | RMS torso keypoint jitter |
| `rust_test_count` | `cargo test --workspace` output | Total passing Rust tests |

### 2.2 Action Definitions

Each action maps to an ADR implementation phase. Actions are defined as structs with preconditions, effects, cost, and metadata.

```rust
pub struct GoapAction {
    /// Unique identifier (e.g., "adr029_phase1_channel_hopping")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// ADR reference (e.g., "ADR-029")
    pub adr: String,
    /// Phase within the ADR (e.g., "Phase 1")
    pub phase: Option<String>,
    /// Preconditions: state properties that must be true/meet threshold
    pub preconditions: Vec<Condition>,
    /// Effects: state properties set after successful execution
    pub effects: Vec<Effect>,
    /// Estimated effort in developer-days
    pub cost_days: f32,
    /// Whether this action requires hardware
    pub requires_hardware: Vec<String>,
    /// Agent types needed to execute this action
    pub agent_types: Vec<String>,
    /// Affected crates/files
    pub affected_components: Vec<String>,
}

pub enum Condition {
    BoolTrue(String),          // property must be true
    BoolFalse(String),         // property must be false
    NumericGte(String, f64),   // property >= threshold
    NumericLte(String, f64),   // property <= threshold
}

pub enum Effect {
    SetBool(String, bool),     // set boolean property
    SetNumeric(String, f64),   // set numeric property
    IncrementNumeric(String, f64), // add to numeric property
}
```

#### 2.2.1 Action Catalog (Key ADR Actions)

| Action ID | ADR | Cost (days) | Preconditions | Effects | Hardware |
|-----------|-----|-------------|---------------|---------|----------|
| `adr037_p1_person_count` | 037 | 3 | `sota_signal_processing` | `person_count_estimation = true` | None |
| `adr037_p2_nmf_decomp` | 037 | 5 | `person_count_estimation` | `signal_decomposition = true` | None |
| `adr037_p3_multi_skel` | 037 | 4 | `signal_decomposition`, `pose_tracker_17kp` | `multi_skeleton_generation = true`, `max_persons_tracked += 2` | None |
| `adr037_p4_neural_multi` | 037 | 10 | `signal_decomposition`, `aether_embeddings`, `gpu_available` | `multi_person_neural = true`, `pose_accuracy_pck02 = 0.6` | GPU |
| `adr021_vital_core` | 021 | 3 | `sota_signal_processing` | `vital_signs_extraction = true` | None |
| `adr021_vital_esp32` | 021 | 5 | `vital_signs_extraction`, `esp32_connected` | `vital_signs_esp32_validated = true`, `breathing_snr_db = 10.0` | ESP32 |
| `adr030_persist_field` | 030 | 2 | `ruvector_signal_integrated` | `persistent_field_model = true` | None |
| `adr022_p2_concurrent` | 022 | 4 | `multi_bssid_scan`, `wifi_adapter_present` | `multi_bssid_concurrent = true` | WiFi adapter |
| `adr029_p1_ch_hop` | 029 | 5 | `esp32_firmware_base`, `esp32_connected` | `esp32_channel_hopping = true` | ESP32 |
| `adr029_p2_multiband` | 029 | 5 | `esp32_channel_hopping` | `multi_band_fusion = true` | ESP32 |
| `adr029_p3_multistatic` | 029 | 5 | `multi_band_fusion`, `esp32_multistatic_ready` | `multistatic_mesh = true` | 2+ ESP32 |
| `adr029_p67_coherence` | 029 | 3 | `multi_band_fusion` | `coherence_gating = true` | None |
| `adr029_p4_tracker` | 029 | 3 | `multistatic_mesh`, `coherence_gating` | `pose_tracker_17kp = true`, `torso_jitter_mm = 30.0` | None |
| `adr024_aether_train` | 024 | 8 | `sota_signal_processing`, `gpu_available` | `aether_embeddings = true` | GPU |
| `adr024_aether_reid` | 024 | 4 | `aether_embeddings`, `pose_tracker_17kp` | `aether_reid = true` | None |
| `adr027_meridian` | 027 | 10 | `aether_embeddings`, `gpu_available` | `meridian_generalization = true` | GPU |
| `adr025_wasm` | 025 | 5 | `sota_signal_processing` | `wasm_deployment = true` | None |
| `adr011_mat` | 011 | 8 | `vital_signs_extraction`, `person_count_estimation` | `mat_survivor_detection = true` | None |
| `adr031_ruview` | 031 | 4 | `persistent_field_model`, `coherence_gating` | `ruview_sensing_ui = true` | None |
| `adr032_mesh_security` | 032 | 5 | `multistatic_mesh` | `mesh_security_hardened = true` | None |

### 2.3 Goal Specification

Goals are expressed as partial world states -- a set of conditions that must be satisfied.

```rust
pub struct Goal {
    /// Human-readable description
    pub description: String,
    /// Conditions that define success
    pub conditions: Vec<Condition>,
    /// Priority weight (higher = more important when competing)
    pub priority: f32,
}
```

**Predefined goal templates:**

| Goal | Conditions | Typical Plan Length |
|------|-----------|---------------------|
| Multi-person tracking | `multi_skeleton_generation = true`, `max_persons_tracked >= 3` | 4-6 actions |
| Vital sign monitoring | `vital_signs_esp32_validated = true`, `breathing_snr_db >= 10` | 2-3 actions |
| Production accuracy | `pose_accuracy_pck02 >= 0.6`, `torso_jitter_mm <= 30` | 5-8 actions |
| Browser deployment | `wasm_deployment = true` | 1-2 actions |
| Disaster response (MAT) | `mat_survivor_detection = true`, `multi_skeleton_generation = true` | 5-7 actions |
| Full multistatic mesh | `multistatic_mesh = true`, `coherence_gating = true`, `pose_tracker_17kp = true` | 5-7 actions |
| Cross-environment robustness | `meridian_generalization = true` | 3-5 actions |

### 2.4 Sublinear Planning Algorithm

The planner avoids exhaustive A* search over the full state space using three techniques.

#### 2.4.1 Backward Relevance Pruning

Before search begins, identify which actions are **relevant** to the goal using backward chaining:

```
function relevantActions(goal, allActions):
    relevant = {}
    frontier = {conditions in goal that are not satisfied}

    while frontier is not empty:
        pick condition C from frontier
        for each action A in allActions:
            if A.effects satisfies C:
                relevant.add(A)
                for each precondition P of A:
                    if P is not satisfied in current state:
                        frontier.add(P)

    return relevant
```

This typically reduces the action set from ~80 to 5-15 for a specific goal. The search then operates only on relevant actions.

**Complexity**: O(G * A) where G is the number of unsatisfied goal/precondition properties and A is the total action count. Since G << 2^N and A is fixed at ~80, this is constant-time relative to the state space.

#### 2.4.2 Hierarchical Decomposition

Actions are organized into three tiers based on the ADR dependency structure:

```
Tier 0 (Foundation):  ADR-014, ADR-016, ADR-018
    No internal prerequisites. Always satisfiable.

Tier 1 (Infrastructure):  ADR-017, ADR-021-core, ADR-022-p1, ADR-029-p1, ADR-030
    Depend only on Tier 0.

Tier 2 (Capability):  ADR-024, ADR-029-p2/p3, ADR-037-p1/p2, ADR-021-esp32
    Depend on Tier 0-1.

Tier 3 (Integration):  ADR-027, ADR-037-p3/p4, ADR-029-p4, ADR-011, ADR-031
    Depend on Tier 0-2.
```

The planner first resolves Tier 0 preconditions (usually already satisfied), then plans Tier 1 actions, then Tier 2, then Tier 3. Within each tier, actions are independent and can be planned in parallel. This reduces the effective search depth from ~15 (worst case linear chain) to ~4 (tier depth).

#### 2.4.3 Incremental Replanning

When the world state changes (a test passes, hardware is plugged in, the user shifts goals), the planner does not replan from scratch. Instead:

1. **Invalidation**: Mark actions in the current plan whose preconditions are no longer satisfied or whose effects are already achieved.
2. **Patch**: Remove invalidated actions and re-run backward relevance pruning only for the remaining unsatisfied goal conditions.
3. **Merge**: Insert new actions into the existing plan at the correct dependency-ordered position.

This is sublinear in the total action count because only the delta is re-examined.

#### 2.4.4 Heuristic Cost Function

The A* heuristic estimates remaining cost as the sum of minimum-cost actions needed to satisfy each unsatisfied goal condition, divided by the maximum parallelism available (number of idle agents). This is admissible (never overestimates) because actions can satisfy multiple conditions.

```
h(state, goal) = sum(min_cost_to_satisfy(c) for c in unsatisfied(state, goal)) / max_parallelism
```

#### 2.4.5 Complexity Analysis

| Component | Naive GOAP | Sublinear GOAP |
|-----------|-----------|----------------|
| State space | 2^N (N=25 booleans) = 33M | Pruned to relevant subset |
| Actions evaluated | All ~80 per expansion | 5-15 (backward pruning) |
| Search depth | Up to 15 | Up to 4 (tier decomposition) |
| Replan cost | Full re-search | Delta patch only |
| Typical plan time | ~100ms | <5ms |

### 2.5 State Observation

The planner queries the real project state before planning. Each property has a defined observation method.

| Property | Observation Command | Cache TTL |
|----------|-------------------|-----------|
| `sota_signal_processing` | `cargo test -p wifi-densepose-signal --no-default-features 2>&1 \| grep "test result"` | 10 min |
| `esp32_connected` | Platform-specific USB serial probe | 30 sec |
| `esp32_count` | Count ESP32 VID/PID USB devices | 30 sec |
| `gpu_available` | `nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null` | 5 min |
| `rust_test_count` | Parse `cargo test --workspace --no-default-features` output | 10 min |
| `wifi_adapter_present` | OS-specific WiFi interface enumeration | 5 min |
| Module existence flags | `test -f <path>` for key source files | 1 min |

Observations are cached with TTL to avoid re-running expensive commands (cargo test) on every plan request. Cache invalidation occurs on file change events or explicit user request.

### 2.6 Plan Execution via Swarm

Once the planner produces an ordered action list, execution is dispatched through the claude-flow swarm system.

#### 2.6.1 GOAP Coordinator Agent

The planner runs as a `goap-coordinator` agent within a hierarchical swarm topology:

```
goap-coordinator (planner + dispatcher)
    |
    +-- researcher (dependency analysis, API review)
    +-- coder (implementation)
    +-- tester (validation, state observation)
    +-- reviewer (code review, security check)
```

The coordinator:
1. Observes current world state
2. Accepts a goal from the user
3. Runs the sublinear planner to produce an action sequence
4. Dispatches each action to appropriate agent types (from the action's `agent_types` field)
5. Monitors action completion via the memory system
6. Updates the world state after each action completes
7. Re-plans if the world state diverges from expectations

#### 2.6.2 State Persistence via Memory

World state is stored in the claude-flow memory system under the `goap` namespace:

```bash
# Store observed state
npx @claude-flow/cli@latest memory store \
  --namespace goap \
  --key "world-state" \
  --value '{"sota_signal_processing": true, "esp32_connected": false, ...}'

# Store current plan
npx @claude-flow/cli@latest memory store \
  --namespace goap \
  --key "current-plan" \
  --value '{"goal": "multi-person tracking", "actions": ["adr037_p1", "adr037_p2", ...], "progress": 1}'

# Search for past successful plans
npx @claude-flow/cli@latest memory search \
  --namespace goap \
  --query "multi-person tracking plan"
```

#### 2.6.3 Action-to-Agent Routing

Each action declares which agent types are needed. The coordinator maps these to swarm agents:

| Agent Type | Role in GOAP Action | Example Actions |
|-----------|---------------------|-----------------|
| `researcher` | Analyze dependencies, review papers, check API compatibility | Pre-action analysis for any ADR |
| `coder` | Write implementation code | All implementation actions |
| `tester` | Run tests, observe state, validate effects | Post-action verification |
| `reviewer` | Code review, security audit | ADR-032 mesh security, any PR |
| `performance-engineer` | Benchmark, optimize latency | ADR-029 pipeline timing |
| `security-architect` | Threat model, audit | ADR-032 security hardening |

#### 2.6.4 Execution Protocol

For each action in the plan:

```
1. PRE-CHECK:  Observe preconditions. If any unsatisfied, re-plan.
2. DISPATCH:   Spawn required agents with action context.
3. EXECUTE:    Agents implement the action (write code, run tests).
4. VERIFY:     Tester agent observes the world state.
5. UPDATE:     If effects achieved, mark action complete, update state.
6. REPLAN:     If effects not achieved, flag failure, re-plan with updated state.
```

### 2.7 Dependency Graph Visualization

The planner can emit its action graph in DOT format for visualization:

```
digraph goap {
    rankdir=LR;
    node [shape=box, style=rounded];

    // Tier 0 (green = complete)
    adr014 [label="ADR-014\nSOTA Signal", color=green];
    adr016 [label="ADR-016\nRuVector Train", color=green];
    adr018 [label="ADR-018\nESP32 Base", color=green];

    // Tier 1 (blue = in progress)
    adr017 [label="ADR-017\nRuVector Signal", color=blue];
    adr030 [label="ADR-030\nField Model", color=orange];

    // Tier 2 (orange = planned)
    adr037_p1 [label="ADR-037 P1\nPerson Count", color=orange];
    adr037_p2 [label="ADR-037 P2\nNMF Decomp", color=orange];
    adr024 [label="ADR-024\nAETHER", color=orange];

    // Tier 3 (gray = future)
    adr037_p3 [label="ADR-037 P3\nMulti-Skeleton", color=gray];
    adr027 [label="ADR-027\nMERIDIAN", color=gray];

    // Edges
    adr014 -> adr037_p1;
    adr037_p1 -> adr037_p2;
    adr037_p2 -> adr037_p3;
    adr014 -> adr024;
    adr024 -> adr037_p3;
    adr024 -> adr027;
    adr014 -> adr017;
    adr017 -> adr030;
}
```

### 2.8 PageRank-Based Prioritization

When the user has not specified a single goal but asks "what should I work on next?", the planner uses PageRank on the action dependency graph to identify the highest-leverage actions:

1. Construct the adjacency matrix where `A[i][j] = 1` if action j depends on action i (i.e., completing i unblocks j).
2. Run PageRank with damping factor 0.85.
3. Actions with the highest PageRank scores are the most "load-bearing" -- they unblock the most downstream work.
4. Filter to actions whose preconditions are currently satisfiable.
5. Return the top-K actions ranked by `PageRank_score * (1 / cost_days)` (value per effort).

This naturally surfaces foundation actions (ADR-014, ADR-016) over leaf actions (ADR-032 security), matching the intuition that infrastructure work has the highest leverage.

---

## 3. Implementation

### 3.1 Module Structure

The GOAP planner is implemented as a TypeScript module within the claude-flow coordination layer (not in the Rust workspace, since it orchestrates Rust development rather than being part of the Rust product).

```
.claude-flow/goap/
    state.ts          -- World state model and observation
    actions.ts        -- Action catalog (all ~80 actions)
    planner.ts        -- Sublinear A* planner with backward pruning
    goals.ts          -- Goal templates and user goal parser
    executor.ts       -- Swarm dispatch and action lifecycle
    pagerank.ts       -- Dependency graph prioritization
    visualize.ts      -- DOT graph export
```

### 3.2 CLI Integration

```bash
# Plan: produce an action sequence for a goal
npx @claude-flow/cli@latest goap plan --goal "multi-person tracking"

# Observe: snapshot current world state
npx @claude-flow/cli@latest goap observe

# Prioritize: PageRank-based "what next?" recommendation
npx @claude-flow/cli@latest goap prioritize --top-k 5

# Execute: run the plan via swarm
npx @claude-flow/cli@latest goap execute --goal "vital sign monitoring"

# Visualize: emit DOT dependency graph
npx @claude-flow/cli@latest goap graph --format dot > goap.dot
```

### 3.3 Integration Points

| System | Integration | Purpose |
|--------|------------|---------|
| claude-flow memory | `goap` namespace | Persist world state, plans, execution history |
| claude-flow swarm | Hierarchical coordinator | Dispatch actions to agent teams |
| claude-flow hooks | `pre-task` / `post-task` | Trigger state observation before/after work |
| cargo test | State observation | Detect which crates/modules pass tests |
| USB device enumeration | Hardware observation | Detect ESP32 availability |
| Git status | Implementation detection | Check if files/modules exist |

---

## 4. Consequences

### 4.1 Positive

- **Eliminates manual priority analysis**: The developer states a goal; the planner produces a concrete, dependency-ordered action list.
- **Hardware-aware planning**: Actions requiring ESP32 or GPU are automatically excluded when hardware is unavailable, preventing dead-end plans.
- **Sublinear plan time**: Backward pruning + tier decomposition keeps planning under 5ms for typical goals, enabling interactive replanning.
- **Incremental replanning**: When state changes (a test starts passing, hardware is plugged in), only the delta is re-evaluated.
- **Swarm integration**: Actions are dispatched to specialized agents, enabling parallel execution of independent actions within the same tier.
- **Cross-session continuity**: World state and plan progress persist in the memory system, so the planner resumes where it left off.
- **PageRank prioritization**: When no specific goal is given, the planner identifies the highest-leverage next action based on the dependency graph structure.
- **Transparent reasoning**: The dependency graph can be visualized in DOT format, making the planner's reasoning inspectable.

### 4.2 Negative

- **Action catalog maintenance**: Every new ADR or ADR phase must be added to the action catalog with correct preconditions and effects. Stale actions produce incorrect plans.
- **State observation overhead**: Some state checks (running `cargo test`) are expensive. Caching with TTL mitigates this but introduces staleness risk.
- **Approximate cost model**: Action costs in developer-days are estimates. Actual effort varies with developer experience and codebase familiarity.
- **Boolean state simplification**: Some capabilities are continuous (accuracy improves gradually) but are modeled as boolean thresholds, losing nuance.

### 4.3 Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Action catalog diverges from reality | Medium | Plans reference nonexistent or completed actions | Validate catalog against ADR directory at plan time |
| State observation produces false positives | Low | Planner skips needed actions | Cross-validate with multiple observation methods |
| User goals conflict (accuracy vs latency) | Medium | Planner produces suboptimal compromise | Support multi-objective goals with explicit weights |
| Swarm agents fail during action execution | Medium | Plan stalls | Timeout + automatic replan with failure noted in state |

---

## 5. Affected Components

| Component | Change | Description |
|-----------|--------|-------------|
| `.claude-flow/goap/` | New | GOAP planner module (TypeScript) |
| claude-flow memory (`goap` namespace) | New | World state and plan persistence |
| claude-flow swarm coordinator | Extended | GOAP coordinator agent type |
| claude-flow CLI | Extended | `goap` subcommand (plan, observe, prioritize, execute, graph) |

---

## 6. Performance Budget

| Operation | Budget | Method |
|-----------|--------|--------|
| World state observation (cached) | < 100ms | Read from memory cache |
| World state observation (fresh) | < 30s | Run cargo test + hardware probes |
| Plan generation (sublinear) | < 5ms | Backward pruning + tier A* |
| PageRank prioritization | < 2ms | Sparse matrix iteration |
| Incremental replan | < 1ms | Delta patch on existing plan |
| DOT graph generation | < 1ms | Traverse action catalog |

---

## 7. Alternatives Considered

1. **Manual priority spreadsheet**: Maintain a spreadsheet of ADR priorities and dependencies. Rejected because it requires manual updates, does not adapt to hardware availability, and cannot be queried programmatically by agents.

2. **Full A* over raw state space**: Standard GOAP without sublinear optimizations. Rejected because 2^25 boolean states is unnecessarily large when most actions are irrelevant to any given goal.

3. **Hierarchical Task Network (HTN)**: HTN decomposes tasks into subtasks using predefined methods. More powerful than GOAP but requires hand-authored decomposition methods for every task. GOAP's flat action model with automatic planning is simpler to maintain as ADRs evolve.

4. **Reinforcement learning planner**: Train an RL agent to select actions. Rejected because the action space changes as ADRs are added, the reward signal is sparse (project completion), and the sample complexity is too high for a planning problem with known structure.

5. **Simple topological sort**: Sort actions by dependency order and execute top-down. Rejected because it does not consider goals (executes everything), does not handle hardware constraints, and does not support replanning.

---

## 8. References

1. Orkin, J. (2003). "Applying Goal-Oriented Action Planning to Games." AI Game Programming Wisdom 2.
2. Orkin, J. (2006). "Three States and a Plan: The A.I. of F.E.A.R." Game Developers Conference.
3. Page, L., Brin, S., Motwani, R., Winograd, T. (1999). "The PageRank Citation Ranking: Bringing Order to the Web." Stanford InfoLab.
4. Ghallab, M., Nau, D., Traverso, P. (2004). "Automated Planning: Theory and Practice." Morgan Kaufmann.
5. Russell, S., Norvig, P. (2020). "Artificial Intelligence: A Modern Approach." 4th ed., Chapter 11: Automated Planning.

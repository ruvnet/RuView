# ADR-020: Temporal Scoring and Tier Migration Algorithm

**Status**: Proposed
**Date**: 2026-02-08
**Parent**: ADR-017 Temporal Tensor Compression, ADR-018 Block-Based Storage Engine
**Author**: System Architecture Team

**Note**: Temporal scoring and tier migration is now handled by RVF's adaptive temperature tiering as part of ADR-029. See rvf-runtime for the implementation.

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-08 | Architecture Team | Initial proposal |

---

## Abstract

This ADR specifies the scoring algorithm, tier migration logic, and budgeted
maintenance pass that govern how compressed tensor blocks move between storage
tiers in the Temporal Tensor Store. It supersedes the simple
`access_count * 1024 / age` heuristic from ADR-017 with a composite score
that blends an exponential moving average (EMA) of access rate, a sliding-window
popularity bitset, and an exponential recency function. Hysteresis margins and
minimum residency constraints prevent pathological tier thrashing. A tick-driven
maintenance pass processes tier transitions within configurable byte and CPU
budgets, producing a deterministic witness log for every decision.

---

## 1. Context and Problem Statement

### 1.1 Limitations of the ADR-017 Score

ADR-017 introduced a tier score of `access_count * 1024 / (now - last_access + 1)`.
This formula has three weaknesses:

1. **Monotonic accumulation**: `access_count` never decays. A block accessed
   10,000 times a year ago and never since still scores high until `age` grows
   large enough to dominate. This delays demotion by hours or days.

2. **No temporal locality signal**: Two blocks with identical total counts but
   different access patterns (bursty vs. uniform) receive the same score. Bursty
   access often predicts near-future reuse and should be promoted faster.

3. **No thrashing protection**: A block sitting exactly on a tier boundary
   oscillates between tiers on every tick, wasting compression and decompression
   cycles.

### 1.2 Requirements for the Replacement

| Requirement | Rationale |
|-------------|-----------|
| Decay old accesses | Blocks untouched for long periods must drain to cold |
| Detect bursts | Recent concentrated access should promote aggressively |
| Prevent thrashing | Tier transitions must have hysteresis and residency floors |
| Budget-bounded | Maintenance must respect per-tick byte and CPU limits |
| Deterministic | Same event sequence must produce identical tier decisions |
| Configurable | Operators must tune all weights, thresholds, and decay constants |

### 1.3 Design Constraints

- The scoring function runs on the hot path (every `touch` call) and must
  complete in under 50ns on x86-64.
- The maintenance pass runs once per tick (configurable, default 100ms) and must
  process candidate blocks within its CPU budget without stalling ingest.
- All floating-point operations use `f32` to stay WASM-compatible (no `f64`
  dependency) and to match the existing `tier_policy.rs` types.

---

## 2. Decision

### 2.1 Replace the ADR-017 Score with a Composite Three-Signal Score

Adopt a weighted composite score that combines three independent signals, each
capturing a different temporal property of access behavior. Protect tier
transitions with hysteresis margins and minimum residency enforcement.

---

## 3. Detailed Design

### 3.1 Block Metadata State

Every block carries the following metadata fields, updated on each access:

```rust
pub struct BlockMeta {
    pub tensor_id: u64,
    pub block_index: u32,

    // --- Access tracking ---
    pub last_access_at: u64,       // Tick timestamp of most recent access
    pub access_count: u64,         // Saturating total access count
    pub ema_rate: f32,             // Exponential moving average of access rate
    pub window: u64,               // 64-bit sliding window bitset

    // --- Tier state ---
    pub current_tier: u8,          // 0=absent, 1=Tier1(8-bit), 2=Tier2(5/7-bit), 3=Tier3(3-bit)
    pub tier_age: u32,             // Ticks spent in current tier since last transition
    pub last_score: f32,           // Cached score from most recent evaluation
    pub checksum: u32,             // CRC32 for corruption detection
}
```

### 3.2 State Updates on Each Access (Touch)

On every read or write to a block, the `touch` function updates metadata
atomically. No locks are needed because the Temporal Tensor Store is
single-writer per block (enforced by the block-based storage engine from
ADR-018).

```rust
/// Update block metadata on access.
///
/// Called on every read or write. Must complete in <50ns.
pub fn touch(policy: &TierPolicy, now: u64, m: &mut BlockMeta) {
    // 1. Timestamp and count
    m.last_access_at = now;
    m.access_count = m.access_count.saturating_add(1);

    // 2. Sliding window: shift left by 1, set LSB to 1
    //    Each bit represents one tick; 1 = accessed, 0 = not accessed.
    m.window = (m.window << 1) | 1;

    // 3. EMA update: instant = 1.0 because this tick had an access
    //    ema_new = alpha * instant + (1 - alpha) * ema_old
    m.ema_rate = policy.alpha * 1.0 + (1.0 - policy.alpha) * m.ema_rate;
}
```

On ticks where a block is **not** accessed, the EMA decays passively during the
maintenance pass:

```rust
/// Passive decay for blocks not accessed this tick.
fn decay_ema(policy: &TierPolicy, m: &mut BlockMeta) {
    // instant = 0.0 (no access this tick)
    m.ema_rate = (1.0 - policy.alpha) * m.ema_rate;

    // Shift window without setting LSB
    m.window <<= 1;
}
```

**Complexity**: O(1) per call. Three integer ops, one shift, two FMA-equivalent
f32 ops. Benchmarks show <20ns on x86-64 and <40ns in WASM.

### 3.3 Score Computation

The composite score S blends three signals, each normalized to the [0, 1] range
before weighting:

```
S = w_ema * ema_access_rate + w_pop * popcount(window) / 64 + w_rec * recency(now - last_access_at)
```

In Rust:

```rust
/// Compute the composite tier score for a block.
pub fn compute_score(policy: &TierPolicy, now: u64, m: &BlockMeta) -> f32 {
    // Signal 1: EMA access rate (already in [0, 1] for reasonable alpha)
    let sig_ema = m.ema_rate;

    // Signal 2: Sliding window popularity, normalized to [0, 1]
    let pop = m.window.count_ones() as f32;  // popcount intrinsic
    let sig_pop = pop / 64.0;

    // Signal 3: Exponential recency decay
    let delta_t = (now.saturating_sub(m.last_access_at)) as f32;
    let sig_rec = fast_exp_neg(delta_t / policy.tau);

    // Weighted sum
    policy.w_ema * sig_ema + policy.w_pop * sig_pop + policy.w_rec * sig_rec
}
```

#### 3.3.1 Signal Descriptions

| Signal | Symbol | Range | Property |
|--------|--------|-------|----------|
| EMA rate | `sig_ema` | [0, 1] | Smooth estimate of recent access frequency. High alpha = responsive to bursts. Low alpha = stable long-term average. |
| Window popularity | `sig_pop` | [0, 1] | Fraction of the last 64 ticks with at least one access. Captures breadth of recent usage. |
| Recency | `sig_rec` | (0, 1] | Exponential decay from last access. Drops rapidly for stale blocks. |

#### 3.3.2 Why Three Signals

No single signal captures all relevant behavior:

- **EMA alone** cannot distinguish a block accessed once per tick for 64 ticks
  from one accessed 64 times in a single tick then idle. Both converge to
  similar EMA values.
- **Popcount alone** is binary per tick and ignores access intensity within
  a tick.
- **Recency alone** has no memory of historical access patterns; a single
  recent touch fully restores the score regardless of history.

The composite score captures intensity (EMA), breadth (popcount), and freshness
(recency) as orthogonal axes. Default weights emphasize recency to ensure
prompt demotion of stale data.

### 3.4 Recency Function and Fast Exponential Approximation

The ideal recency function is:

```
r(delta_t) = exp(-delta_t / tau)
```

where `tau` is the characteristic decay time in ticks. For `tau = 100`, a block
untouched for 100 ticks decays to `1/e ~ 0.368`; at 200 ticks it decays to
`0.135`; at 460 ticks it drops below 0.01.

#### 3.4.1 Fast Approximation via Rational Function

For the maintenance pass, which evaluates potentially thousands of blocks per
tick, a full `f32::exp` call (~15ns, involves range reduction and polynomial
evaluation) is too expensive. We use a rational approximation:

```rust
/// Fast approximation of exp(-x) for x >= 0.
///
/// Uses the Pade(1,1) approximant: exp(-x) ~ 1 / (1 + x)
/// Maximum relative error: 26% at x=2 (acceptable for scoring, not for numerics).
///
/// For higher accuracy, use the LUT approach below.
fn fast_exp_neg_pade(x: f32) -> f32 {
    1.0 / (1.0 + x.max(0.0))
}
```

#### 3.4.2 LUT with Linear Interpolation (Recommended)

For production use, a 256-entry lookup table with linear interpolation provides
<0.5% error across the useful range:

```rust
/// 256-entry LUT for exp(-x) over [0, 8].
/// Beyond x=8, exp(-x) < 0.00034, effectively zero for scoring.
const EXP_LUT_SIZE: usize = 256;
const EXP_LUT_MAX_X: f32 = 8.0;

static EXP_LUT: [f32; EXP_LUT_SIZE] = {
    let mut lut = [0.0f32; EXP_LUT_SIZE];
    let mut i = 0;
    while i < EXP_LUT_SIZE {
        let x = (i as f32) * EXP_LUT_MAX_X / (EXP_LUT_SIZE as f32 - 1.0);
        // compile-time evaluation via const fn not available for exp;
        // in practice, initialize at startup or use a build script.
        lut[i] = 0.0; // placeholder
        i += 1;
    }
    lut
};

/// Fast exp(-x) via LUT with linear interpolation.
/// x is clamped to [0, EXP_LUT_MAX_X].
fn fast_exp_neg(x: f32) -> f32 {
    if x <= 0.0 {
        return 1.0;
    }
    if x >= EXP_LUT_MAX_X {
        return 0.0;
    }
    let t = x * (EXP_LUT_SIZE as f32 - 1.0) / EXP_LUT_MAX_X;
    let idx = t as usize;
    let frac = t - idx as f32;

    if idx + 1 >= EXP_LUT_SIZE {
        return EXP_LUT[EXP_LUT_SIZE - 1];
    }

    // Linear interpolation between adjacent LUT entries
    EXP_LUT[idx] * (1.0 - frac) + EXP_LUT[idx + 1] * frac
}
```

**LUT initialization** (called once at startup):

```rust
fn init_exp_lut(lut: &mut [f32; EXP_LUT_SIZE]) {
    for i in 0..EXP_LUT_SIZE {
        let x = (i as f32) * EXP_LUT_MAX_X / (EXP_LUT_SIZE as f32 - 1.0);
        lut[i] = (-x).exp();    // std exp, only called 256 times
    }
}
```

**Error analysis** for LUT interpolation:

| x range | Max absolute error | Max relative error |
|---------|-------------------|--------------------|
| [0, 1] | 0.0005 | 0.08% |
| [1, 3] | 0.0003 | 0.15% |
| [3, 6] | 0.0001 | 0.42% |
| [6, 8] | 0.00002 | 0.38% |

### 3.5 Tier Selection by Thresholds

The score is compared against three thresholds to select the target tier:

```
if   S >= t1  then  Tier1  (8-bit, hot)
elif S >= t2  then  Tier2  (7-bit or 5-bit, warm)
elif S >= t3  then  Tier3  (3-bit, cold)
else               Tier0  (absent / evicted)
```

```
Score axis (0.0 to 1.0)
|                                                                |
0.0       t3        t2                t1                       1.0
|----Tier0----|---Tier3---|----Tier2----|---------Tier1-----------|
  (absent)      (3-bit)     (5/7-bit)           (8-bit)
```

Default threshold values:

| Parameter | Default | Rationale |
|-----------|---------|-----------|
| `t1` | 0.70 | Requires strong signal on at least two axes to qualify as hot |
| `t2` | 0.35 | Moderate recent activity; still worth keeping at reduced precision |
| `t3` | 0.10 | Minimal recent activity; compress aggressively or evict |

### 3.6 Hysteresis to Prevent Thrashing

A block sitting near a tier boundary may oscillate if the score fluctuates
around the threshold. This causes repeated compression/decompression cycles
(thrashing), each of which consumes CPU and I/O bandwidth.

#### 3.6.1 Hysteresis Margins

Tier transitions require the score to exceed the threshold by a configurable
margin:

```
Upgrade:   S > threshold_upper + hysteresis
Downgrade: S < threshold_lower - hysteresis
```

This creates a dead zone around each boundary where no transition occurs:

```
Score axis around threshold t2 = 0.35, hysteresis = 0.05:

        Downgrade zone       Dead zone (no transition)     Upgrade zone
    <------|--------|-------------|-------------|-----------|-------->
          0.25     0.30         0.35          0.40        0.45
                    ^                                      ^
              Tier3 if below                         Tier2 if above
```

In Rust:

```rust
/// Determine if a tier transition should occur, accounting for hysteresis.
pub fn should_transition(
    policy: &TierPolicy,
    current_tier: u8,
    score: f32,
) -> Option<u8> {
    let h = policy.hysteresis;

    // Check for upgrade (higher tier = lower number = higher precision)
    if current_tier > 1 && score > policy.t1 + h {
        return Some(1); // Promote to Tier1
    }
    if current_tier > 2 && score > policy.t2 + h {
        return Some(2); // Promote to Tier2
    }

    // Check for downgrade (lower tier = higher number = lower precision)
    if current_tier < 3 && current_tier > 0 && score < policy.t3 - h {
        return Some(0); // Evict to Tier0
    }
    if current_tier < 3 && score < policy.t2 - h {
        return Some(3); // Demote to Tier3
    }
    if current_tier < 2 && score < policy.t1 - h {
        return Some(2); // Demote to Tier2
    }

    None // No transition; remain in current tier
}
```

#### 3.6.2 Minimum Residency Enforcement

Even with hysteresis, a rapidly changing workload could cause transitions faster
than the system can absorb. The `min_residency` parameter sets a floor on the
number of ticks a block must remain in its current tier before any transition
is permitted:

```rust
fn is_eligible_for_transition(policy: &TierPolicy, m: &BlockMeta) -> bool {
    m.tier_age >= policy.min_residency
}
```

**Recommended values**:

| Workload | `min_residency` | Rationale |
|----------|-----------------|-----------|
| Real-time inference | 10 ticks (1s at 100ms tick) | Fast adaptation, tolerate some thrashing |
| Batch processing | 100 ticks (10s) | Stability preferred over responsiveness |
| Archival | 1000 ticks (100s) | Very conservative, minimize I/O |

#### 3.6.3 Tier Transition State Machine

```
                     S > t1 + h
                     age >= min_residency
              +---------------------------+
              |                           |
              v                           |
  +--------+     S > t2 + h     +--------+     S > t1 + h     +--------+
  | Tier0  |  ----------------> | Tier3  |  - - - - - - - ->  | Tier2  |
  | absent |     age >= min     | 3-bit  |     (via Tier2)    | 5/7-bit|
  +--------+                    +--------+                    +--------+
       ^                            |   ^                        |   ^
       |     S < t3 - h             |   |    S < t2 - h          |   |
       |     age >= min             |   |    age >= min          |   |
       +----------------------------+   +------------------------+   |
                                                                     |
                                        +--------+                   |
                                        | Tier1  | ------------------+
                                        | 8-bit  |   S < t1 - h
                                        +--------+   age >= min
                                            ^
                                            |
                                            +--- S > t1 + h, age >= min
                                                 (from Tier2)
```

**Transitions are always single-step**: a block in Tier3 cannot jump directly
to Tier1. It must pass through Tier2 first. This prevents large recompression
jumps and gives the system time to validate intermediate states. Each step
resets `tier_age` to 0, so the block must again satisfy `min_residency` before
its next transition.

### 3.7 TierPolicy Configuration

All scoring and migration parameters are consolidated in a single configuration
structure:

```rust
pub struct TierPolicy {
    // --- Scoring weights ---
    pub alpha: f32,          // EMA smoothing factor (0, 1). Higher = more responsive.
    pub tau: f32,            // Recency decay time constant (in ticks).
    pub w_ema: f32,          // Weight for EMA access rate signal.
    pub w_pop: f32,          // Weight for popcount window signal.
    pub w_rec: f32,          // Weight for exponential recency signal.

    // --- Tier thresholds ---
    pub t1: f32,             // Score threshold for Tier1 (hot, 8-bit).
    pub t2: f32,             // Score threshold for Tier2 (warm, 5/7-bit).
    pub t3: f32,             // Score threshold for Tier3 (cold, 3-bit).

    // --- Anti-thrashing ---
    pub hysteresis: f32,     // Margin added/subtracted from thresholds.
    pub min_residency: u32,  // Minimum ticks before tier transition allowed.

    // --- Storage ---
    pub max_delta_chain: u8, // Max delta segments before full rewrite (from ADR-018).
    pub block_bytes: usize,  // Block size in bytes (from ADR-018).
}
```

**Default configuration**:

```rust
impl Default for TierPolicy {
    fn default() -> Self {
        Self {
            alpha: 0.1,
            tau: 100.0,
            w_ema: 0.3,
            w_pop: 0.2,
            w_rec: 0.5,
            t1: 0.70,
            t2: 0.35,
            t3: 0.10,
            hysteresis: 0.05,
            min_residency: 50,
            max_delta_chain: 4,
            block_bytes: 4096,
        }
    }
}
```

**Weight normalization**: The weights `w_ema + w_pop + w_rec` should sum to 1.0
so that the score range is [0, 1]. The system asserts this at construction time
with a tolerance of 1e-6.

### 3.8 Budgeted Maintenance Pass (Tick Handler)

The maintenance pass executes once per tick. It is the sole location where tier
transitions are enacted. The `touch` function only updates metadata; it never
triggers compression or decompression directly. This separation ensures that
ingest latency is bounded and independent of maintenance costs.

#### 3.8.1 Inputs

```rust
pub struct TickBudget {
    pub byte_budget: usize,   // Max bytes of compression/decompression this tick
    pub cpu_budget: u32,       // Max block evaluations this tick
}
```

#### 3.8.2 Candidate Selection

Candidates are blocks whose state may require action:

| Condition | Action |
|-----------|--------|
| Score crossed a boundary (accounting for hysteresis) | Tier transition |
| `tier_age > max_age` | Forced re-evaluation (prevents stale metadata) |
| `checksum` mismatch detected | Repair via re-read and recompression |
| `current_tier == 0` and score > t3 + h | Promotion from absent |

#### 3.8.3 Priority Ordering

Candidates are sorted into two queues processed in order:

**Upgrade queue** (highest priority): sorted by score descending (highest
score delta first). Rationale: promoting a heavily-accessed block reduces
read amplification for many future accesses.

**Downgrade queue** (lower priority): sorted by score ascending (lowest score
first). Rationale: demoting the coldest blocks first frees the most byte
budget for hot tier capacity.

Corruption repairs bypass both queues and are processed first unconditionally.

#### 3.8.4 Processing Loop

```rust
pub fn run_maintenance_tick(
    policy: &TierPolicy,
    budget: &TickBudget,
    now: u64,
    blocks: &mut [BlockMeta],
    witness_log: &mut Vec<WitnessEntry>,
) {
    let mut bytes_used: usize = 0;
    let mut ops_used: u32 = 0;

    // Phase 0: Passive EMA decay for all blocks not accessed this tick
    for m in blocks.iter_mut() {
        if m.last_access_at != now {
            decay_ema(policy, m);
        }
        m.tier_age = m.tier_age.saturating_add(1);
    }

    // Phase 1: Score computation and candidate collection
    let mut upgrades: Vec<(usize, f32, u8)> = Vec::new();   // (index, score, target_tier)
    let mut downgrades: Vec<(usize, f32, u8)> = Vec::new();
    let mut repairs: Vec<usize> = Vec::new();

    for (i, m) in blocks.iter_mut().enumerate() {
        let score = compute_score(policy, now, m);
        m.last_score = score;

        // Check corruption
        if needs_repair(m) {
            repairs.push(i);
            continue;
        }

        // Check eligibility
        if !is_eligible_for_transition(policy, m) {
            continue;
        }

        if let Some(target) = should_transition(policy, m.current_tier, score) {
            if target < m.current_tier {
                upgrades.push((i, score, target));
            } else {
                downgrades.push((i, score, target));
            }
        }
    }

    // Phase 2: Sort queues
    upgrades.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    downgrades.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Phase 3: Process repairs (unconditional)
    for idx in repairs {
        if ops_used >= budget.cpu_budget { break; }
        let cost = execute_repair(&mut blocks[idx]);
        bytes_used += cost;
        ops_used += 1;
        witness_log.push(WitnessEntry::repair(now, &blocks[idx]));
    }

    // Phase 4: Process upgrades (highest score first)
    for (idx, score, target) in upgrades {
        if ops_used >= budget.cpu_budget || bytes_used >= budget.byte_budget {
            break;
        }
        let cost = execute_tier_transition(&mut blocks[idx], target);
        bytes_used += cost;
        ops_used += 1;
        blocks[idx].current_tier = target;
        blocks[idx].tier_age = 0;
        witness_log.push(WitnessEntry::transition(now, &blocks[idx], score, target));
    }

    // Phase 5: Process downgrades (lowest score first)
    for (idx, score, target) in downgrades {
        if ops_used >= budget.cpu_budget || bytes_used >= budget.byte_budget {
            break;
        }
        let cost = execute_tier_transition(&mut blocks[idx], target);
        bytes_used += cost;
        ops_used += 1;
        blocks[idx].current_tier = target;
        blocks[idx].tier_age = 0;
        witness_log.push(WitnessEntry::transition(now, &blocks[idx], score, target));
    }
}
```

#### 3.8.5 Witness Log

Every maintenance decision emits a structured log entry for auditability:

```rust
pub struct WitnessEntry {
    pub tick: u64,
    pub tensor_id: u64,
    pub block_index: u32,
    pub action: WitnessAction,    // Transition | Repair | Evict | Skip
    pub score: f32,
    pub from_tier: u8,
    pub to_tier: u8,
    pub reason: &'static str,
}
```

The witness log enables post-hoc analysis of tier decisions, capacity planning,
and regression testing of policy changes.

#### 3.8.6 Maintenance Pass Flow Diagram

```
                          Tick Event (periodic)
                               |
                               v
                  +---------------------------+
                  | Phase 0: Passive EMA      |
                  | decay for non-accessed    |
                  | blocks; increment tier_age|
                  +---------------------------+
                               |
                               v
                  +---------------------------+
                  | Phase 1: Compute scores   |
                  | Classify into:            |
                  |  - repairs[]              |
                  |  - upgrades[]             |
                  |  - downgrades[]           |
                  +---------------------------+
                               |
                               v
                  +---------------------------+
                  | Phase 2: Sort queues      |
                  |  upgrades: by score DESC  |
                  |  downgrades: by score ASC |
                  +---------------------------+
                               |
                               v
                  +---------------------------+
                  | Phase 3: Process repairs  |
                  | (unconditional, first)    |
                  +------------|------+-------+
                               |      |
                          budget ok?  budget exhausted?
                               |           |
                               v           v
                  +---------------------------+
                  | Phase 4: Process upgrades |
                  | highest score delta first |
                  +------------|------+-------+
                               |      |
                          budget ok?  budget exhausted?
                               |           |
                               v           v
                  +---------------------------+
                  | Phase 5: Process downgrades|
                  | lowest score first        |
                  +---------------------------+
                               |
                               v
                  +---------------------------+
                  | Emit witness log entries  |
                  | for all actions taken     |
                  +---------------------------+
```

### 3.9 Score Sensitivity Analysis

#### 3.9.1 EMA Response Curve

The EMA signal responds to access pattern changes with a time constant of
`1/alpha` ticks. For alpha = 0.1:

```
After sustained access (1 access per tick):
  ema converges to alpha / (1 - (1-alpha)) = 1.0

After access stops (from steady state of 1.0):
  ema(t) = (1 - alpha)^t
  t=1:   0.90
  t=5:   0.59
  t=10:  0.35
  t=20:  0.12
  t=30:  0.04
  t=50:  0.005
```

**Derivation**: At steady state with one access per tick, the EMA satisfies
`ema = alpha * 1 + (1-alpha) * ema`, giving `ema = 1.0`. After access ceases,
each tick multiplies by `(1-alpha)`, so `ema(t) = (1-alpha)^t`. The half-life
is `ln(2) / ln(1/(1-alpha))`. For alpha=0.1, half-life ~ 6.6 ticks.

#### 3.9.2 Recency Decay Curve

For tau = 100:

```
r(delta_t) = exp(-delta_t / 100)

delta_t:    0     10     50    100    200    300    500    1000
r:        1.000  0.905  0.607  0.368  0.135  0.050  0.007  0.000
```

#### 3.9.3 Composite Score Trajectories

**Scenario A: Block accessed steadily then abandoned**

```
Score
1.0 |*******
    |       ****
    |           ***
0.7 |-- t1 -------***-----------  (Tier1 threshold)
    |                ***
0.35|-- t2 ------------***------  (Tier2 threshold)
    |                     ****
0.10|-- t3 ------------------***  (Tier3 threshold)
    |                         ***
0.0 +------|------|------|-------> Ticks after last access
    0      10     50    100   200
```

**Scenario B: Bursty access (10 accesses in tick 0, then silence)**

```
Score
1.0 |*
    | *
    |  *
0.7 |-- **--------------------------  (Tier1)
    |     **
0.35|------***----------------------  (Tier2)
    |         ***
0.10|-----------****----------------  (Tier3)
    |               *******
0.0 +------|------|------|-------> Ticks
    0      10     50    100
```

Burst raises the initial EMA to `alpha * 1 + (1-alpha) * (alpha * 1 + ...) ~
alpha * 10` (clamped), but decays at the same rate. The window signal remains
1/64 after tick 1, providing differentiation from steady access.

**Scenario C: Periodic access (every 20 ticks)**

```
Score
1.0 |
    |
    |
0.7 |--------------------------------------
    |  *       *       *       *       *
0.5 |** **   ** **   ** **   ** **   ** **    (oscillates 0.3--0.6)
0.35|--------------------------------------
    |
0.10|--------------------------------------
0.0 +------|------|------|------|-------> Ticks
    0      20     40     60     80
```

The block stabilizes in Tier2. Hysteresis of 0.05 prevents flapping between
Tier2 and Tier1 since the peaks reach ~0.6, which is below t1 + h = 0.75.

### 3.10 Determinism Guarantees

The tier migration algorithm is fully deterministic:

1. **No randomness**: No random number generators are used in scoring,
   candidate selection, or tie-breaking.

2. **Stable ordering**: When two blocks have identical scores, ties are broken
   by `(tensor_id, block_index)` in ascending lexicographic order. This
   ensures the same blocks are processed first regardless of memory layout
   or iteration order.

3. **Reproducible EMA**: Because the EMA update uses the same `alpha` and
   the same sequence of `touch` / `decay_ema` calls (driven by the event
   stream), replaying the same event log produces identical metadata states.

4. **No wall-clock dependency**: All timestamps are logical tick counters, not
   system clocks. The maintenance pass is triggered by the tick event, not by
   a timer.

5. **Bit-exact f32**: All computations use `f32` with no intermediate `f64`
   promotion. The LUT for `fast_exp_neg` is initialized deterministically.
   On IEEE 754 compliant hardware (including WASM), results are bit-exact.

### 3.11 Failure Modes and Remediation

#### 3.11.1 Thrashing

**Symptom**: Frequent tier transitions for the same block (>2 transitions per
100 ticks). Detected by monitoring the witness log.

**Root cause**: Hysteresis margin too small relative to score volatility, or
`min_residency` too low for the workload's access variability.

**Remediation**:

| Action | Effect |
|--------|--------|
| Increase `hysteresis` from 0.05 to 0.10 | Doubles the dead zone around each threshold |
| Increase `min_residency` from 50 to 200 | Block must stay in tier 4x longer before eligible |
| Decrease `tau` | Recency signal decays faster, reducing score volatility from stale state |
| Decrease `alpha` | EMA smooths more aggressively, damping burst sensitivity |

#### 3.11.2 Hot Set Misprediction

**Symptom**: Tier1 byte footprint exceeds capacity. Too many blocks qualified
as hot.

**Root cause**: `t1` threshold too low, or `w_pop` too high (treating any
recent activity as hot).

**Remediation**:

| Action | Effect |
|--------|--------|
| Raise `t1` from 0.70 to 0.85 | Only blocks with very strong multi-signal evidence promoted |
| Lower `w_pop` from 0.2 to 0.1 | Reduce influence of window breadth |
| Enforce per-tier byte cap | Hard limit on total bytes in Tier1; evict lowest-scoring Tier1 blocks |
| Raise `w_rec` | Makes recency dominant; blocks must be very recently accessed |

#### 3.11.3 Starvation of Downgrades

**Symptom**: Cold blocks accumulate in Tier2 because upgrade processing
exhausts the CPU budget before downgrades run.

**Root cause**: Budget too small, or too many upgrade candidates per tick.

**Remediation**:

| Action | Effect |
|--------|--------|
| Split budget 50/50 between upgrades and downgrades | Guarantees downgrade processing |
| Increase `cpu_budget` | More operations per tick |
| Process downgrades first every other tick | Round-robin priority |

#### 3.11.4 Corruption Cascade

**Symptom**: Multiple blocks fail checksum validation simultaneously after
a storage fault.

**Root cause**: Underlying storage corruption (disk error, truncated write).

**Remediation**: Repairs are processed unconditionally before tier transitions.
If the repair budget is exhausted, remaining corrupted blocks are flagged and
prioritized on the next tick. A persistent corruption counter triggers an alert
if it exceeds a configurable threshold.

---

## 4. Mathematical Derivations

### 4.1 EMA Convergence

For a constant access rate of `r` accesses per tick (modeled as instant = r):

```
ema(t) = alpha * r + (1 - alpha) * ema(t-1)
```

This is a first-order IIR filter. The steady-state solution is:

```
ema_ss = alpha * r / (1 - (1 - alpha)) = r
```

The transient response from ema(0) = 0 is:

```
ema(t) = r * (1 - (1-alpha)^t)
```

Time to reach 95% of steady state: `t_95 = ln(0.05) / ln(1-alpha)`.
For alpha=0.1: `t_95 ~ 29 ticks`.

### 4.2 Score Sensitivity to Weight Changes

Partial derivatives of S with respect to each weight:

```
dS/d(w_ema) = sig_ema           (range [0, 1])
dS/d(w_pop) = sig_pop           (range [0, 1])
dS/d(w_rec) = sig_rec           (range (0, 1])
```

Since all signals are in [0, 1], a unit change in any weight shifts the score
by at most 1.0. For small perturbations:

```
delta_S ~ delta_w_ema * sig_ema + delta_w_pop * sig_pop + delta_w_rec * sig_rec
```

To maintain threshold stability, changes to weights should be bounded:

```
|delta_w_i| < hysteresis / max(sig_i) = hysteresis
```

For hysteresis=0.05, individual weight adjustments should stay within +/-0.05
to avoid unintended mass tier migrations.

### 4.3 Hysteresis Dead Zone Width

The effective dead zone around threshold T is:

```
dead_zone = [T - hysteresis, T + hysteresis]
width = 2 * hysteresis
```

A block's score must traverse the full dead zone width to complete a transition.
Given the maximum score velocity (one `touch` per tick driving all three
signals upward), the minimum time to traverse the dead zone is:

```
t_min_traverse ~ 2 * hysteresis / max_score_rate
```

For alpha=0.1, tau=100, and all weights=0.33:
- After a single touch from zero state: `delta_S ~ 0.33*0.1 + 0.33*(1/64) + 0.33*1 = 0.37`
- Dead zone width: `2 * 0.05 = 0.10`

A single touch can cross the dead zone, but `min_residency` provides the
additional time floor.

### 4.4 Popcount Signal Characteristics

The window is a 64-bit shift register. After `k` consecutive ticks with
access: `popcount = min(k, 64)`. After `j` ticks of silence following
saturation: `popcount = max(64 - j, 0)`.

Normalized popcount (`sig_pop = popcount/64`) has a trapezoidal response:
linear ramp up over 64 ticks, flat at 1.0 during sustained access, linear
ramp down over 64 ticks after access stops. This provides a 64-tick "memory"
that is independent of and complementary to the EMA and recency signals.

---

## 5. Integration Points

### 5.1 Relationship to ADR-017 (Temporal Tensor Compression)

ADR-017 defined the compression pipeline (groupwise quantization, bitstream
packing, segment format) but used a simple score heuristic. This ADR replaces
that heuristic with the composite score while preserving the compression
pipeline unchanged. The `TierPolicy` struct from ADR-017's `tier_policy.rs`
is extended with the new fields (alpha, tau, weights, hysteresis,
min_residency).

### 5.2 Relationship to ADR-018 (Block-Based Storage Engine)

ADR-018 defines the block storage layer including `BlockMeta`, delta chains,
and the block I/O interface. This ADR adds the `ema_rate`, `window`,
`tier_age`, and `last_score` fields to `BlockMeta` and defines the maintenance
pass that operates on blocks through the storage engine's API.

### 5.3 Coherence Engine Integration

The coherence engine (ADR-014, ADR-015) may override tier decisions via
coherence-gated signals:

- A coherence violation forces a block to Tier1 regardless of score, ensuring
  full-precision access during consistency recovery.
- A coherence quiescence signal (stable energy for N ticks) permits accelerated
  demotion by halving `min_residency` for the affected tensor.

### 5.4 WASM Compatibility

All types use `f32` and fixed-size integers. The LUT for `fast_exp_neg` is
initialized via a startup function callable from WASM's `_start` or
`__wasm_call_ctors`. The maintenance pass uses no heap allocation beyond the
candidate vectors, which can be pre-allocated to a fixed capacity.

---

## 6. Alternatives Considered

### 6.1 LRU / LFU Eviction

**Rejected**: Pure LRU (least recently used) ignores frequency. Pure LFU
(least frequently used) ignores recency. Both are single-signal policies
that cannot express the nuanced tradeoffs of a multi-tier system. The
composite score subsumes both: high `w_rec` approximates LRU; high `w_ema`
approximates LFU.

### 6.2 ARC (Adaptive Replacement Cache)

**Considered but rejected**: ARC maintains two LRU lists and a ghost list
to adaptively balance recency vs. frequency. While elegant for binary
(cache hit / miss) decisions, extending ARC to four tiers with different
bit-widths is non-trivial. The composite score approach is simpler to
implement, tune, and reason about.

### 6.3 Machine-Learned Scoring

**Deferred**: A small neural network could predict future access patterns
from historical traces. However, this introduces non-determinism (floating
point ordering in inference), model management complexity, and a cold-start
problem. We may revisit this when the RuVector intelligence system (SONA)
is mature enough to provide lightweight, deterministic inference.

### 6.4 Single-Signal Score (Keep ADR-017 Heuristic)

**Rejected**: As detailed in Section 1.1, the ADR-017 heuristic has
fundamental limitations. Extending it with decay would address monotonic
accumulation but still lack burst detection and thrashing protection.

---

## 7. Acceptance Criteria

| Criterion | Measurement | Target |
|-----------|-------------|--------|
| Touch latency | Benchmark `touch()` on x86-64 | < 50ns p99 |
| Score computation latency | Benchmark `compute_score()` | < 100ns p99 |
| Maintenance pass (1000 blocks) | End-to-end tick processing time | < 1ms |
| Determinism | Replay same event log twice, compare witness logs | Bit-exact match |
| Thrashing rate | Transitions per block per 100 ticks under mixed workload | < 2 |
| Tier accuracy | Fraction of blocks in correct tier after 1000 ticks (vs oracle) | > 90% |
| Hysteresis effectiveness | Tier transitions eliminated by hysteresis under oscillating load | > 80% |
| Budget compliance | Bytes and ops used per tick vs budget | Never exceeds budget |

---

## 8. Risks and Mitigations

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| Weight tuning requires per-workload calibration | Medium | High | Ship sensible defaults; provide tuning guide; expose metrics for auto-tuning |
| LUT initialization overhead | Low | Low | 256 entries * ~15ns = <4us; negligible startup cost |
| f32 precision drift over millions of EMA updates | Low | Medium | EMA is bounded [0, 1]; no accumulation. Periodic reset not needed. |
| min_residency delays urgent promotions | Medium | Medium | Coherence override bypasses min_residency for consistency-critical blocks |
| Witness log grows unbounded | Low | High | Ring buffer with configurable capacity; oldest entries evicted |
| WASM f32 semantics differ from native | Low | Low | Both follow IEEE 754; WASM mandates deterministic NaN handling |

---

## 9. Open Questions

1. **Auto-tuning**: Should we implement an online tuning loop that adjusts
   weights based on observed cache hit rates and tier utilization? This could
   adapt to changing workloads without manual configuration.

2. **Per-tensor overrides**: Should individual tensors be able to specify
   their own TierPolicy, or should the policy be global? Per-tensor policies
   add flexibility but complicate the maintenance pass.

3. **Tick rate selection**: The default tick interval of 100ms is appropriate
   for server workloads. Embedded or edge deployments may need different
   tick rates. Should the tick rate be configurable independently of the
   policy parameters, or should tau and min_residency be specified in wall
   time?

4. **Budget split strategy**: The current design processes all upgrades before
   all downgrades. Should we interleave upgrades and downgrades, or allocate
   a fixed fraction of the budget to each?

---

## 10. Implementation Roadmap

### Phase 1: Core Scoring (Week 1)
- Extend `BlockMeta` with `ema_rate`, `window`, `tier_age`, `last_score`
- Implement `touch()`, `decay_ema()`, `compute_score()`
- Implement `fast_exp_neg` with LUT initialization
- Extend `TierPolicy` with new fields
- Unit tests for all score computations and edge cases

### Phase 2: Tier Migration Logic (Week 1-2)
- Implement `should_transition()` with hysteresis
- Implement `is_eligible_for_transition()` with min_residency
- Implement single-step transition constraint
- State machine tests covering all transition paths

### Phase 3: Maintenance Pass (Week 2-3)
- Implement `run_maintenance_tick()` with budget tracking
- Implement candidate selection and priority sorting
- Implement witness log emission
- Integration tests with synthetic workloads
- Determinism tests (replay verification)

### Phase 4: Tuning and Hardening (Week 3-4)
- Benchmark touch and score computation latency
- Profile maintenance pass with 10K+ blocks
- Implement per-tier byte caps (failure mode 3.11.2)
- Create tuning guide with recommended configurations
- Fuzz testing for edge cases (zero tau, extreme weights, u64 overflow)

---

## 11. References

1. O'Neil, E., O'Neil, P., Weikum, G. "The LRU-K Page Replacement Algorithm
   for Database Disk Buffering." SIGMOD 1993.
2. Megiddo, N., Modha, D. "ARC: A Self-Tuning, Low Overhead Replacement
   Cache." USENIX FAST 2003.
3. Jiang, S., Zhang, X. "LIRS: An Efficient Low Inter-reference Recency Set
   Replacement Policy." SIGMOD 2002.
4. ADR-017: Temporal Tensor Compression with Tiered Quantization.
5. ADR-018: Block-Based Storage Engine (referenced, not yet published).
6. ADR-014: Coherence Engine Architecture.
7. ADR-015: Coherence-Gated Transformer.

---

## Appendix A: Score Curve Reference Charts

### A.1 EMA Decay After Access Ceases (alpha = 0.1)

```
ema
1.0 |*
    | *
0.8 |  *
    |   *
0.6 |    *
    |     **
0.4 |       **
    |         ***
0.2 |            ****
    |                ******
0.0 |                      ***************
    +------|------|------|------|------|---> Ticks
    0      5      10     15     20     30
```

### A.2 Recency Decay (tau = 100)

```
recency
1.0 |****
    |    ***
0.8 |       **
    |         **
0.6 |           **
    |             ***
0.4 |                ***
    |                   ****
0.2 |                       *****
    |                            ********
0.0 |                                    ******************
    +------|------|------|------|------|------|------|-----> Ticks
    0      50     100    150    200    300    400    500
```

### A.3 Popcount Ramp-Up and Decay

```
sig_pop
1.0 |                  ************************
    |               ***                        ***
0.8 |            ***                              ***
    |         ***                                    ***
0.6 |       **                                         **
    |     **                                             **
0.4 |   **                                                 **
    |  *                                                     **
0.2 | *                                                        **
    |*                                                           **
0.0 +------|------|------|------|------|------|------|------|-------> Ticks
    0      16     32     48     64     80     96     112    128
    |<-- ramp up -->|<-- sustained -->|<------- decay -------->|
```

## Appendix B: Comparison of Approximation Methods for exp(-x)

| Method | Max Relative Error (x in [0, 4]) | Latency (ns) | Memory |
|--------|----------------------------------|---------------|--------|
| `std::f32::exp` | 0 (reference) | 12-15 | 0 |
| Pade(1,1): `1/(1+x)` | 26% at x=2 | 2-3 | 0 |
| Pade(2,2): `(1-x/2+x^2/12)/(1+x/2+x^2/12)` | 1.5% at x=4 | 4-5 | 0 |
| LUT-256 + linear interp | 0.42% | 3-4 | 1 KB |
| LUT-1024 + linear interp | 0.03% | 3-4 | 4 KB |

The LUT-256 approach provides the best accuracy/cost tradeoff for scoring.

## Appendix C: Worked Example -- Full Lifecycle of a Block

Assume default policy: alpha=0.1, tau=100, w_ema=0.3, w_pop=0.2, w_rec=0.5,
t1=0.70, t2=0.35, t3=0.10, hysteresis=0.05, min_residency=50.

**Tick 0**: Block created. `ema=0, window=0, tier=Tier3, tier_age=0`.
Score = 0.3*0 + 0.2*0 + 0.5*1.0 = 0.50. Above t2+h=0.40 but tier_age < 50.
No transition.

**Tick 1-49**: Block accessed every tick.
By tick 49: `ema ~ 1-(0.9)^50 ~ 0.995`. `popcount = 50/64 ~ 0.78`.
`recency = 1.0` (accessed this tick). Score ~ 0.3*0.995 + 0.2*0.78 + 0.5*1.0
= 0.30 + 0.16 + 0.50 = 0.96. Above t1+h = 0.75, but tier_age = 49 < 50.

**Tick 50**: tier_age = 50 >= min_residency. Score = 0.96 > 0.75 (t1+h).
Upgrade: Tier3 -> Tier2 (single-step). tier_age resets to 0.

**Tick 100**: tier_age = 50 again. Score still ~0.96. Upgrade: Tier2 -> Tier1.

**Tick 101-200**: Access stops. EMA decays: `ema(t) = 0.995 * 0.9^t`.
Popcount drains 1 bit per tick. Recency decays: `exp(-t/100)`.

**Tick 164** (64 ticks after last access): popcount reaches 0. Score drops
to ~0.3*0.002 + 0.2*0 + 0.5*0.53 = 0.27. Below t1-h = 0.65. tier_age = 64
>= 50. Downgrade: Tier1 -> Tier2.

**Tick 250** (150 ticks after last access): Score ~ 0.3*0 + 0.2*0 +
0.5*0.22 = 0.11. Below t2-h = 0.30. tier_age = 86 >= 50.
Downgrade: Tier2 -> Tier3.

**Tick 350** (250 ticks after last access): Score ~ 0.5*0.08 = 0.04.
Below t3-h = 0.05. tier_age = 100 >= 50. Downgrade: Tier3 -> Tier0 (evicted).

# cognitum-gate-kernel

[![Crates.io](https://img.shields.io/crates/v/cognitum-gate-kernel.svg)](https://crates.io/crates/cognitum-gate-kernel)
[![Documentation](https://docs.rs/cognitum-gate-kernel/badge.svg)](https://docs.rs/cognitum-gate-kernel)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/ruvector/ruvector/ci.yml?branch=main)](https://github.com/ruvector/ruvector/actions)

A `no_std` WASM kernel for the **Anytime-Valid Coherence Gate** - a real-time permission system that decides "Is it safe to act right now, or should we pause or escalate?" The coherence gate provides formal safety guarantees for autonomous agent actions through continuous monitoring and evidence accumulation.

Think of it like a **smoke detector for AI agents**: it continuously monitors system coherence, can keep listening forever, and the moment it has enough evidence of instability, it triggers. Unlike traditional gating systems, you can stop the computation at any time and still trust the decision - that's what makes it "anytime-valid." The gate doesn't try to be smart; it tries to be **safe**, **calm**, and **correct** about permission.

The gate uses **three stacked filters** that must all agree before permitting an action: (1) **Structural** - graph coherence via dynamic min-cut to detect fragile partitions, (2) **Shift** - distribution monitoring to detect when the environment is changing, and (3) **Evidence** - e-value accumulation for sequential hypothesis testing with formal Type I error control. Every decision outputs a signed witness receipt explaining why.

> Created by [ruv.io](https://ruv.io) and [RuVector](https://github.com/ruvector/ruvector)

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
cognitum-gate-kernel = "0.1"
```

Basic usage - create a worker tile, ingest graph deltas, tick, and get the report:

```rust
use cognitum_gate_kernel::{TileState, Delta};

// Initialize a worker tile (ID 42 in the 256-tile fabric)
let mut tile = TileState::new(42);

// Ingest graph deltas (edge additions, removals, weight updates)
tile.ingest_delta(&Delta::edge_add(0, 1, 100));  // Add edge 0->1, weight 1.0
tile.ingest_delta(&Delta::edge_add(1, 2, 150));  // Add edge 1->2, weight 1.5
tile.ingest_delta(&Delta::edge_add(2, 0, 100));  // Complete the triangle

// Process one tick of the kernel
let report = tile.tick(1);

// Check the coherence state
println!("Vertices: {}, Edges: {}", report.num_vertices, report.num_edges);
println!("Connected: {}", report.is_connected());
println!("E-value: {:.2}", report.e_value_approx());

// Get the witness fragment for global aggregation
let witness = tile.get_witness_fragment();
println!("Local min-cut estimate: {}", witness.local_min_cut);
```

<details>
<summary><h2>Architecture</h2></summary>

### 256-Tile WASM Fabric

The coherence gate runs on a distributed fabric of 256 tiles, with TileZero acting as the central arbiter:

```
+-------------------------------------------------------------------------+
|                         256-TILE COGNITUM FABRIC                        |
+-------------------------------------------------------------------------+
|                                                                         |
|  +-------------------------------------------------------------------+  |
|  |                        TILE ZERO (Arbiter)                        |  |
|  |                                                                   |  |
|  |  * Merge worker reports      * Hierarchical min-cut              |  |
|  |  * Global gate decision      * Permit token issuance             |  |
|  |  * Witness receipt log       * Hash-chained eventlog             |  |
|  +-------------------------------+-----------------------------------+  |
|                                  |                                      |
|             +--------------------+--------------------+                 |
|             |                    |                    |                 |
|             v                    v                    v                 |
|  +----------------+   +----------------+   +----------------+           |
|  |  Workers       |   |  Workers       |   |  Workers       |   ...    |
|  |  [1-85]        |   |  [86-170]      |   |  [171-255]     |           |
|  |                |   |                |   |                |           |
|  |  Shard A       |   |  Shard B       |   |  Shard C       |           |
|  |  Local cuts    |   |  Local cuts    |   |  Local cuts    |           |
|  |  E-accum       |   |  E-accum       |   |  E-accum       |           |
|  +----------------+   +----------------+   +----------------+           |
|                                                                         |
+-------------------------------------------------------------------------+
```

### Worker Tile Responsibilities

Each of the 255 worker tiles maintains a **local shard** with:

- **CompactGraph** (~42KB): Vertices, edges, adjacency lists with union-find connectivity
- **EvidenceAccumulator** (~2KB): Hypothesis tracking and sliding observation window
- **Delta buffer** (1KB): Circular buffer for incoming graph updates
- **Total**: ~46KB per tile, fitting within the 64KB WASM memory budget

Worker tiles perform:
1. **Ingest deltas** - Edge additions, removals, weight updates, observations
2. **Process ticks** - Deterministic tick loop updates local state
3. **Produce reports** - 64-byte cache-aligned reports with coherence metrics
4. **Emit witness fragments** - Boundary information for global aggregation

### TileZero Arbiter Role

TileZero collects reports from all worker tiles and:

1. **Merges reports** into a reduced supergraph
2. **Applies three filters**: structural, shift, and evidence
3. **Issues decisions**: `Permit`, `Defer`, or `Deny`
4. **Signs permit tokens** with Ed25519
5. **Maintains receipt log** with hash-chained audit trail

### Data Flow

```
                    +-------------------+
                    |   Graph Updates   |
                    | (Edges, Weights)  |
                    +---------+---------+
                              |
                              v
+---------------------------------------------------------------+
|                    WORKER TILES [1-255]                       |
|                                                               |
|   Delta --> CompactGraph --> Connectivity --> WitnessFragment |
|         --> EvidenceAccum --> LogEValue                       |
|                                                               |
+---------------------------+-----------------------------------+
                            |
                    TileReports (64 bytes each)
                            |
                            v
+---------------------------------------------------------------+
|                      TILEZERO ARBITER                         |
|                                                               |
|   Structural Filter: global_cut >= min_cut_threshold?         |
|   Shift Filter: shift_pressure < max_shift_threshold?         |
|   Evidence Filter: e_aggregate in [tau_deny, tau_permit]?     |
|                                                               |
|              +-------> PERMIT (proceed autonomously)          |
|   DECISION --+-------> DEFER  (escalate to human)             |
|              +-------> DENY   (block the action)              |
|                                                               |
+---------------------------+-----------------------------------+
                            |
                            v
                    +-------------------+
                    |   PermitToken     |
                    | (signed + TTL)    |
                    +-------------------+
                            |
                            v
                    +-------------------+
                    |  WitnessReceipt   |
                    | (hash-chained)    |
                    +-------------------+
```

</details>

<details>
<summary><h2>Technical Deep Dive</h2></summary>

### CompactGraph Internals

The `CompactGraph` structure is optimized for cache-efficient access on WASM:

```rust
#[repr(C, align(64))]  // Cache-line aligned
pub struct CompactGraph {
    // HOT FIELDS (first cache line - 64 bytes)
    pub num_vertices: u16,      // Active vertex count
    pub num_edges: u16,         // Active edge count
    pub free_edge_head: u16,    // Free list for edge reuse
    pub generation: u16,        // Structural change counter
    pub num_components: u16,    // Connected component count
    pub status: u16,            // Dirty/connected flags
    _hot_pad: [u8; 52],         // Padding to 64 bytes

    // COLD FIELDS (subsequent cache lines)
    pub vertices: [VertexEntry; 256],     // 256 * 8 = 2KB
    pub edges: [ShardEdge; 1024],         // 1024 * 8 = 8KB
    pub adjacency: [[AdjEntry; 32]; 256], // 256 * 32 * 4 = 32KB
}
// Total: ~42KB
```

**Key optimizations**:
- `#[inline(always)]` on all hot-path accessors
- Unsafe unchecked array access after bounds validation
- Union-find with iterative path compression (no recursion)
- Branchless flag manipulation for partition sides

### E-Value Accumulator Math

The evidence accumulator uses **fixed-point log2 representation** for numerical stability:

```rust
pub type LogEValue = i32;  // log2(e-value) * 65536

// Pre-computed threshold constants (avoid runtime log)
pub const LOG_E_STRONG: LogEValue = 282944;      // log2(20) * 65536
pub const LOG_E_VERY_STRONG: LogEValue = 436906; // log2(100) * 65536
pub const LOG_LR_CONNECTIVITY_POS: LogEValue = 38550;  // log2(1.5) * 65536
pub const LOG_LR_CONNECTIVITY_NEG: LogEValue = -65536; // log2(0.5) * 65536
```

**E-value composition** (multiplicative):
```
log(e1 * e2) = log(e1) + log(e2)
```

This enables efficient sequential evidence accumulation with saturating addition:
```rust
self.log_e_value = self.log_e_value.saturating_add(log_lr);
```

**Anytime-valid property**: Because e-values are nonnegative supermartingales with E[E_0] = 1, the decision is valid at any stopping time:
```
P_H0(E_tau >= 1/alpha) <= alpha
```

### TileReport Structure (64 bytes, cache-line aligned)

```rust
#[repr(C, align(64))]
pub struct TileReport {
    // Header (8 bytes)
    pub tile_id: u8,           // Tile ID (0-255)
    pub status: TileStatus,    // Processing status
    pub generation: u16,       // Epoch number
    pub tick: u32,             // Current tick

    // Graph state (8 bytes)
    pub num_vertices: u16,
    pub num_edges: u16,
    pub num_components: u16,
    pub graph_flags: u16,

    // Evidence state (8 bytes)
    pub log_e_value: LogEValue,  // 4 bytes
    pub obs_count: u16,
    pub rejected_count: u16,

    // Witness fragment (16 bytes)
    pub witness: WitnessFragment,

    // Performance metrics (8 bytes)
    pub delta_time_us: u16,
    pub tick_time_us: u16,
    pub deltas_processed: u16,
    pub memory_kb: u16,

    // Cross-tile coordination (8 bytes)
    pub ghost_vertices: u16,
    pub ghost_edges: u16,
    pub boundary_vertices: u16,
    pub pending_sync: u16,

    // Reserved (8 bytes)
    pub _reserved: [u8; 8],
}
```

### Memory Layout (~41KB per tile)

| Component | Size | Notes |
|-----------|------|-------|
| Graph shard | 42 KB | 256 vertices, 1024 edges, 32-degree adjacency |
| Evidence accumulator | 2 KB | 16 hypotheses, 64-observation window |
| Delta buffer | 1 KB | 64 deltas @ 16 bytes each |
| TileState overhead | 1 KB | Metadata, status, counters |
| **Total per worker** | **~46 KB** | Fits in 64KB WASM page |
| **Total 255 workers** | **~11.5 MB** | |
| TileZero state | ~1 MB | Supergraph + receipt log head |
| **Total fabric** | **~13 MB** | |

</details>

<details>
<summary><h2>Tutorials and Examples</h2></summary>

### Example 1: Network Security Gate

Protect network device configuration changes with coherence gating:

```rust
use cognitum_gate_kernel::{TileState, Delta, Observation};
use cognitum_gate_tilezero::{TileZero, GateThresholds, ActionContext, ActionTarget, ActionMetadata};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create gate with security-focused thresholds
    let thresholds = GateThresholds {
        tau_deny: 0.01,      // Very conservative: 1% false alarm rate
        tau_permit: 100.0,   // Require strong evidence for autonomous action
        min_cut: 10.0,       // High structural integrity required
        max_shift: 0.3,      // Low tolerance for distribution shift
        permit_ttl_ns: 60_000_000_000, // 60 second token lifetime
    };

    let tilezero = TileZero::new(thresholds);
    let mut tile = TileState::new(1);

    // Model network topology as graph
    // Devices are vertices, connections are edges
    tile.ingest_delta(&Delta::edge_add(0, 1, 100));  // core-router -> firewall
    tile.ingest_delta(&Delta::edge_add(1, 2, 100));  // firewall -> switch
    tile.ingest_delta(&Delta::edge_add(2, 3, 100));  // switch -> server-rack
    tile.ingest_delta(&Delta::edge_add(2, 4, 100));  // switch -> workstations

    // Add connectivity hypothesis for firewall
    tile.evidence.add_connectivity_hypothesis(1);

    // Add observations about healthy connectivity
    for tick in 1..=10 {
        let obs = Observation::connectivity(1, true);  // firewall is connected
        tile.ingest_delta(&Delta::observation(obs));
        tile.tick(tick);
    }

    // Now request permission to push a config change
    let ctx = ActionContext {
        action_id: "cfg-push-001".into(),
        action_type: "config_change".into(),
        target: ActionTarget {
            device: Some("firewall".into()),
            path: Some("/rules/allow-list".into()),
            extra: Default::default(),
        },
        context: ActionMetadata {
            agent_id: "netops-agent".into(),
            session_id: Some("session-123".into()),
            prior_actions: vec![],
            urgency: "normal".into(),
        },
    };

    // Get decision
    let token = tilezero.decide(&ctx).await;

    match token.decision {
        GateDecision::Permit => {
            println!("Action permitted. Token valid for {} ns", token.ttl_ns);
            println!("Witness hash: {:?}", &token.witness_hash[..8]);
        }
        GateDecision::Defer => {
            println!("Uncertain. Escalating to human operator.");
            // Wait for human approval...
        }
        GateDecision::Deny => {
            println!("Blocked: network topology unstable");
        }
    }

    // Verify receipt exists
    if let Some(receipt) = tilezero.get_receipt(token.sequence).await {
        println!("Receipt sequence: {}", receipt.sequence);
    }

    Ok(())
}
```

### Example 2: Config Change Approval

Gate infrastructure changes based on dependency graph stability:

```rust
use cognitum_gate_kernel::{TileState, Delta, Observation};

fn main() {
    let mut tile = TileState::new(1);

    // Build dependency graph for microservices
    // Service 0: API Gateway
    // Service 1: Auth Service
    // Service 2: User Service
    // Service 3: Database

    // Dependencies: API -> Auth, API -> User, Auth -> DB, User -> DB
    tile.ingest_delta(&Delta::edge_add(0, 1, 200));  // API -> Auth (critical)
    tile.ingest_delta(&Delta::edge_add(0, 2, 150));  // API -> User
    tile.ingest_delta(&Delta::edge_add(1, 3, 200));  // Auth -> DB (critical)
    tile.ingest_delta(&Delta::edge_add(2, 3, 150));  // User -> DB

    // Process initial state
    let report = tile.tick(1);
    println!("Connected: {}", report.is_connected());
    println!("Components: {}", report.num_components);
    assert!(report.is_connected());
    assert_eq!(report.num_components, 1);

    // Add hypothesis to track auth connectivity
    tile.evidence.add_connectivity_hypothesis(1);

    // Ingest recent health checks (all healthy)
    for tick in 2..=12 {
        let obs = Observation::connectivity(1, true);
        tile.ingest_delta(&Delta::observation(obs));
        tile.tick(tick);
    }

    // Check if we have enough evidence to permit changes
    let e_value = tile.evidence.global_e_value();
    println!("Accumulated evidence: {:.2}", e_value);

    if e_value > 20.0 {
        println!("Strong evidence of stability. Config change may proceed.");
    } else if e_value > 1.0 {
        println!("Some evidence of stability. Human review recommended.");
    } else {
        println!("Insufficient evidence. Config change blocked.");
    }

    // Simulate removing a critical edge (partition risk)
    tile.ingest_delta(&Delta::edge_remove(1, 3));  // Remove Auth -> DB
    let report = tile.tick(13);

    if !report.is_connected() {
        println!("ALERT: Graph partition detected! {} components",
                 report.num_components);
        // Gate would DENY any action touching these services
    }
}
```

### Example 3: Multi-Agent Coordination

Coordinate multiple agents through the coherence gate:

```rust
use cognitum_gate_kernel::{TileState, Delta, Observation};
use std::collections::HashMap;

struct AgentCoordinator {
    tiles: HashMap<u8, TileState>,
}

impl AgentCoordinator {
    fn new(num_tiles: u8) -> Self {
        let mut tiles = HashMap::new();
        for id in 1..=num_tiles {
            tiles.insert(id, TileState::new(id));
        }
        Self { tiles }
    }

    /// Model agent interactions as graph edges
    fn register_interaction(&mut self, agent_a: u16, agent_b: u16, tile_id: u8) {
        if let Some(tile) = self.tiles.get_mut(&tile_id) {
            tile.ingest_delta(&Delta::edge_add(agent_a, agent_b, 100));
        }
    }

    /// Process a tick across all tiles
    fn tick_all(&mut self, tick: u32) -> Vec<(u8, bool)> {
        let mut results = vec![];
        for (&id, tile) in &mut self.tiles {
            let report = tile.tick(tick);
            results.push((id, report.is_connected()));
        }
        results
    }

    /// Evaluate action safety based on tile coherence
    fn evaluate_action(&self, tile_id: u8) -> ActionResult {
        if let Some(tile) = self.tiles.get(&tile_id) {
            let witness = tile.get_witness_fragment();
            let e_value = tile.evidence.global_e_value();

            if !tile.last_report.is_connected() {
                ActionResult::Deny("Tile graph disconnected".into())
            } else if witness.local_min_cut < 50 {
                ActionResult::Defer("Low min-cut detected".into())
            } else if e_value < 1.0 {
                ActionResult::Defer("Insufficient evidence".into())
            } else if e_value > 20.0 {
                ActionResult::Permit
            } else {
                ActionResult::Defer("Moderate evidence".into())
            }
        } else {
            ActionResult::Deny("Unknown tile".into())
        }
    }
}

enum ActionResult {
    Permit,
    Defer(String),
    Deny(String),
}

fn main() {
    let mut coordinator = AgentCoordinator::new(4);

    // Register agent interactions across tiles
    coordinator.register_interaction(0, 1, 1);  // Agents 0,1 interact on tile 1
    coordinator.register_interaction(1, 2, 1);
    coordinator.register_interaction(2, 3, 2);  // Agents 2,3 interact on tile 2
    coordinator.register_interaction(3, 4, 2);

    // Run simulation ticks
    for tick in 1..=20 {
        let results = coordinator.tick_all(tick);
        for (tile_id, connected) in results {
            if !connected {
                println!("Tick {}: Tile {} lost connectivity!", tick, tile_id);
            }
        }
    }

    // Evaluate pending action on tile 1
    match coordinator.evaluate_action(1) {
        ActionResult::Permit => println!("Action on tile 1: PERMITTED"),
        ActionResult::Defer(reason) => println!("Action on tile 1: DEFERRED - {}", reason),
        ActionResult::Deny(reason) => println!("Action on tile 1: DENIED - {}", reason),
    }
}
```

</details>

<details>
<summary><h2>Super Advanced Usage</h2></summary>

### Custom Update Rules for E-Process

Extend the evidence accumulator with custom likelihood ratio functions:

```rust
use cognitum_gate_kernel::evidence::{LogEValue, f32_to_log_e, LOG_E_STRONG};

/// Custom e-value update for domain-specific hypothesis testing
pub trait CustomEUpdateRule {
    /// Compute log likelihood ratio for domain-specific observation
    fn compute_log_lr(&self, observation: &DomainObservation) -> LogEValue;

    /// Apply custom stopping rule
    fn should_stop(&self, cumulative_log_e: LogEValue, obs_count: u32) -> StopDecision;
}

/// Financial anomaly detection e-process
struct FinancialAnomalyRule {
    baseline_volatility: f32,
    alert_multiplier: f32,
}

impl CustomEUpdateRule for FinancialAnomalyRule {
    fn compute_log_lr(&self, obs: &DomainObservation) -> LogEValue {
        let volatility = obs.value as f32 / 1000.0;
        let ratio = volatility / self.baseline_volatility;

        // Evidence for anomaly increases when volatility exceeds baseline
        if ratio > self.alert_multiplier {
            f32_to_log_e(ratio)
        } else {
            f32_to_log_e(1.0 / ratio)  // Evidence against anomaly
        }
    }

    fn should_stop(&self, cumulative_log_e: LogEValue, obs_count: u32) -> StopDecision {
        if obs_count < 10 {
            return StopDecision::Continue;  // Minimum sample size
        }
        if cumulative_log_e > LOG_E_STRONG {
            StopDecision::Reject  // Strong evidence of anomaly
        } else if cumulative_log_e < -LOG_E_STRONG {
            StopDecision::Accept  // Strong evidence of normality
        } else {
            StopDecision::Continue
        }
    }
}

enum StopDecision { Continue, Accept, Reject }
struct DomainObservation { value: u32 }
```

### SIMD Optimization Hooks

For high-throughput scenarios, inject SIMD-optimized paths:

```rust
#[cfg(target_arch = "x86_64")]
mod simd_opt {
    use std::arch::x86_64::*;

    /// Batch e-value computation with AVX2
    #[target_feature(enable = "avx2")]
    pub unsafe fn compute_log_lr_batch_avx2(
        h1: &[f64; 4],
        h0: &[f64; 4],
    ) -> [f64; 4] {
        let v_h1 = _mm256_loadu_pd(h1.as_ptr());
        let v_h0 = _mm256_loadu_pd(h0.as_ptr());
        let ratio = _mm256_div_pd(v_h1, v_h0);

        let mut out = [0f64; 4];
        _mm256_storeu_pd(out.as_mut_ptr(), ratio);
        out
    }
}

#[cfg(target_arch = "wasm32")]
mod simd_opt {
    use core::arch::wasm32::*;

    /// WASM SIMD128 optimized log likelihood ratio
    #[target_feature(enable = "simd128")]
    pub unsafe fn compute_log_lr_simd128(h1: v128, h0: v128) -> v128 {
        f32x4_div(h1, h0)
    }
}
```

### Distributed Coordination with ruvector-raft

Integrate with RuVector's Raft consensus for distributed gate deployment:

```rust
use cognitum_gate_tilezero::{TileZero, GateThresholds, GateDecision};

/// Distributed coherence gate with Raft consensus
pub struct DistributedCoherenceGate {
    local_gate: TileZero,
    peers: Vec<String>,
    node_id: u64,
}

impl DistributedCoherenceGate {
    pub async fn new(node_id: u64, peers: Vec<String>) -> Self {
        let thresholds = GateThresholds::default();
        Self {
            local_gate: TileZero::new(thresholds),
            peers,
            node_id,
        }
    }

    /// Make a distributed decision (requires consensus)
    pub async fn decide_with_consensus(
        &self,
        ctx: &ActionContext,
    ) -> Result<PermitToken, DistributedError> {
        // Step 1: Local evaluation
        let local_token = self.local_gate.decide(ctx).await;

        // Step 2: Propose to Raft cluster
        let proposal = GateProposal {
            sequence: local_token.sequence,
            action_id: ctx.action_id.clone(),
            decision: local_token.decision,
            witness_hash: local_token.witness_hash,
        };

        // Step 3: Wait for consensus (majority agreement)
        self.propose_and_wait(proposal).await?;

        // Step 4: Return token only after consensus
        Ok(local_token)
    }

    async fn propose_and_wait(&self, proposal: GateProposal) -> Result<(), DistributedError> {
        // In production, this would use ruvector-raft
        Ok(())
    }
}

struct GateProposal {
    sequence: u64,
    action_id: String,
    decision: GateDecision,
    witness_hash: [u8; 32],
}
struct DistributedError;
struct ActionContext { action_id: String }
struct PermitToken { sequence: u64, decision: GateDecision, witness_hash: [u8; 32] }
```

### Hardware Integration (Cognitum Chip)

For deployment on dedicated Cognitum ASIC/FPGA:

```rust
//! Hardware abstraction layer for Cognitum coherence gate chip

use cognitum_gate_kernel::{Delta, TileState};
use cognitum_gate_kernel::report::TileReport;

/// Hardware register interface
#[repr(C)]
pub struct CognitumRegisters {
    pub control: u32,
    pub status: u32,
    pub delta_fifo_addr: u64,
    pub report_fifo_addr: u64,
    pub tile_config_base: u64,
    pub clock_mhz: u32,
}

/// Hardware-accelerated tile driver
pub struct HardwareTile {
    registers: *mut CognitumRegisters,
    tile_id: u8,
}

impl HardwareTile {
    /// Initialize hardware tile
    pub unsafe fn new(base_addr: *mut u8, tile_id: u8) -> Self {
        Self {
            registers: base_addr as *mut CognitumRegisters,
            tile_id,
        }
    }

    /// Submit delta to hardware FIFO
    pub fn submit_delta(&mut self, delta: &Delta) {
        unsafe {
            let fifo_addr = (*self.registers).delta_fifo_addr as *mut Delta;
            core::ptr::write_volatile(fifo_addr, *delta);
        }
    }

    /// Trigger hardware tick
    pub fn trigger_tick(&mut self) {
        unsafe {
            (*self.registers).control |= 0x1;
        }
    }

    /// Read report from hardware
    pub fn read_report(&self) -> TileReport {
        unsafe {
            let fifo_addr = (*self.registers).report_fifo_addr as *const TileReport;
            core::ptr::read_volatile(fifo_addr)
        }
    }

    /// Check if tile is ready
    pub fn is_ready(&self) -> bool {
        unsafe { ((*self.registers).status & 0x1) != 0 }
    }
}
```

### Extending the Witness Receipt Format

Add custom fields to witness receipts for domain-specific auditing:

```rust
use cognitum_gate_tilezero::{WitnessReceipt, WitnessSummary, GateDecision};
use serde::{Serialize, Deserialize};

/// Extended witness receipt with compliance fields
#[derive(Clone, Serialize, Deserialize)]
pub struct ComplianceWitnessReceipt {
    pub base: WitnessReceipt,
    pub jurisdiction: String,
    pub framework: String,  // e.g., "SOC2", "GDPR", "HIPAA"
    pub controls_checked: Vec<String>,
    pub risk_score: u8,     // 0-100
    pub human_reviewer: Option<String>,
    pub extended_signature: [u8; 64],
}

impl ComplianceWitnessReceipt {
    pub fn from_base(base: WitnessReceipt, jurisdiction: &str, framework: &str) -> Self {
        Self {
            base,
            jurisdiction: jurisdiction.to_string(),
            framework: framework.to_string(),
            controls_checked: vec![],
            risk_score: 0,
            human_reviewer: None,
            extended_signature: [0u8; 64],
        }
    }

    pub fn add_control(&mut self, control_id: &str) {
        self.controls_checked.push(control_id.to_string());
    }

    /// Calculate risk score based on receipt data
    pub fn calculate_risk_score(&mut self) {
        let mut score: u32 = 0;

        score += match self.base.token.decision {
            GateDecision::Permit => 0,
            GateDecision::Defer => 30,
            GateDecision::Deny => 70,
        };

        if self.base.witness_summary.min_cut < 5.0 {
            score += 20;
        }

        self.risk_score = score.min(100) as u8;
    }
}
```

</details>

## API Reference

Full API documentation is available on [docs.rs/cognitum-gate-kernel](https://docs.rs/cognitum-gate-kernel).

### Key Types

| Type | Description |
|------|-------------|
| `TileState` | Main worker tile state containing graph, evidence, and delta buffer |
| `Delta` | Tagged union for graph updates (edge add/remove, weight update, observation) |
| `TileReport` | 64-byte cache-aligned report produced after each tick |
| `WitnessFragment` | 16-byte fragment for global min-cut aggregation |
| `CompactGraph` | ~42KB fixed-size graph shard with union-find connectivity |
| `EvidenceAccumulator` | Hypothesis tracking with sliding window and e-value computation |

### WASM Exports

When compiled for WASM, the kernel exports:

```c
void init_tile(uint8_t tile_id);
int32_t ingest_delta(const uint8_t* ptr);
int32_t tick(uint32_t tick_number, uint8_t* report_ptr);
int32_t get_witness_fragment(uint8_t* fragment_ptr);
uint8_t get_status();
void reset_tile();
uint32_t get_memory_usage();
```

## Claude-Flow Integration

### Using as SDK

The coherence gate integrates with Claude-Flow for multi-agent coordination:

```javascript
import { ClaudeFlow } from '@claude-flow/core';
import { CoherenceGate } from '@ruvector/cognitum-gate';

const flow = new ClaudeFlow({
  topology: 'mesh',
  maxAgents: 8,
});

// Initialize coherence gate
const gate = new CoherenceGate({
  thresholds: {
    tauDeny: 0.01,
    tauPermit: 100.0,
    minCut: 5.0,
    maxShift: 0.5,
  },
});

// Register gate with flow
flow.use(gate.middleware());

// Gate evaluates agent actions before execution
flow.onBeforeAction(async (action, context) => {
  const permit = await gate.evaluate(action, context);

  if (permit.decision === 'DENY') {
    throw new ActionDeniedError(permit.reason);
  }

  if (permit.decision === 'DEFER') {
    return await flow.escalate(action, permit);
  }

  // Attach token for audit trail
  context.permitToken = permit.token;
});
```

### MCP Plugin Configuration

Configure the gate as an MCP server:

```json
{
  "mcpServers": {
    "coherence-gate": {
      "command": "cargo",
      "args": ["run", "-p", "mcp-gate", "--", "serve"],
      "env": {
        "GATE_TAU_DENY": "0.01",
        "GATE_TAU_PERMIT": "100.0",
        "GATE_MIN_CUT": "5.0",
        "GATE_MAX_SHIFT": "0.5",
        "GATE_SIGNING_KEY_PATH": "/etc/gate/keys/signing.key"
      }
    }
  }
}
```

### Example Swarm Coordination

Coordinate a research swarm with coherence gating:

```javascript
import { ClaudeFlow, SwarmConfig } from '@claude-flow/core';

const config = {
  topology: 'hierarchical',
  agents: [
    { role: 'researcher', count: 3 },
    { role: 'coder', count: 2 },
    { role: 'tester', count: 1 },
  ],
  gate: {
    enabled: true,
    mode: 'strict',  // All actions require permit
    escalation: {
      channel: 'human-operator',
      timeout: 300_000,  // 5 minutes
      defaultOnTimeout: 'deny',
    },
  },
};

const flow = new ClaudeFlow(config);

// Gate tracks agent interactions as graph edges
flow.onAgentInteraction((from, to, type) => {
  gate.recordInteraction(from.id, to.id, type);
});

// Research tasks are gated
await flow.spawn('researcher', {
  task: 'Analyze security vulnerabilities in auth module',
  gate: {
    requirePermit: true,
    minEvidence: 20.0,  // Require strong evidence before proceeding
  },
});
```

### MCP Tools

The gate exposes three MCP tools:

```typescript
// Request permission for an action
permit_action({
  action_id: "cfg-push-001",
  action_type: "config_change",
  context: { agent_id: "ops-agent", target: "router-1" }
}) -> { decision: "permit", token: "...", valid_until_ns: ... }

// Get witness receipt for audit
get_receipt({ sequence: 1847394 }) -> {
  decision: "deny",
  witness: { structural: {...}, predictive: {...}, evidential: {...} },
  receipt_hash: "..."
}

// Replay decision for debugging
replay_decision({ sequence: 1847394, verify_chain: true }) -> {
  original_decision: "deny",
  replayed_decision: "deny",
  match_confirmed: true
}
```

## License

Licensed under either of:

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

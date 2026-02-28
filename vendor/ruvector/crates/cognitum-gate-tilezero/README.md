# cognitum-gate-tilezero: The Central Arbiter

<p align="center">
  <a href="https://ruv.io"><img src="https://img.shields.io/badge/ruv.io-coherence_gate-blueviolet?style=for-the-badge" alt="ruv.io"></a>
  <a href="https://github.com/ruvnet/ruvector"><img src="https://img.shields.io/badge/RuVector-monorepo-orange?style=for-the-badge&logo=github" alt="RuVector"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/crates/v/cognitum-gate-tilezero" alt="Crates.io">
  <img src="https://img.shields.io/badge/latency-<100μs-blue" alt="Latency">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-green" alt="License">
  <img src="https://img.shields.io/badge/rust-1.77%2B-orange?logo=rust" alt="Rust">
</p>

<p align="center">
  <strong>Native arbiter for the Anytime-Valid Coherence Gate in a 256-tile WASM fabric</strong>
</p>

<p align="center">
  <em>TileZero merges worker reports, makes gate decisions, and issues cryptographically signed permit tokens.</em>
</p>

<p align="center">
  <a href="#what-is-tilezero">What is TileZero?</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#key-capabilities">Capabilities</a> •
  <a href="#tutorials">Tutorials</a> •
  <a href="https://ruv.io">ruv.io</a>
</p>

---

## What is TileZero?

**TileZero** is the central coordinator in a distributed coherence assessment system. In a 256-tile WASM fabric, TileZero (tile 0) acts as the arbiter that:

1. **Merges** worker tile reports into a unified supergraph
2. **Decides** whether to Permit, Defer, or Deny actions
3. **Signs** cryptographic permit tokens with Ed25519
4. **Logs** every decision in a Blake3 hash-chained receipt log

### Architecture Overview

```
         Worker Tiles (1-255)              TileZero (Tile 0)
    ┌─────────┐ ┌─────────┐ ┌─────────┐      ┌─────────────┐
    │ Tile 1  │ │ Tile 2  │ │Tile 255 │      │  TileZero   │
    │ ─────── │ │ ─────── │ │ ─────── │      │  Arbiter    │
    │ Local   │ │ Local   │ │ Local   │ ───► │ ─────────── │
    │ Graph   │ │ Graph   │ │ Graph   │      │ Supergraph  │
    │ Report  │ │ Report  │ │ Report  │      │ Decision    │
    └────┬────┘ └────┬────┘ └────┬────┘      │ PermitToken │
         │           │           │           │ ReceiptLog  │
         └───────────┴───────────┴──────────►└─────────────┘
```

### The Three-Filter Decision Pipeline

TileZero applies three stacked filters to every action request:

| Filter | Question | Pass Condition |
|--------|----------|----------------|
| **Structural** | Is the graph well-connected? | Min-cut ≥ threshold |
| **Shift** | Is the distribution stable? | Shift pressure < max |
| **Evidence** | Have we accumulated enough confidence? | E-value in safe range |

```
Action Request → [Structural] → [Shift] → [Evidence] → PERMIT/DEFER/DENY
                      ↓            ↓           ↓
                 Graph cut    Distribution  E-value
                 healthy?     stable?       confident?
```

---

## Quick Start

### Installation

```toml
[dependencies]
cognitum-gate-tilezero = "0.1"

# With min-cut integration
cognitum-gate-tilezero = { version = "0.1", features = ["mincut"] }
```

### Basic Usage

```rust
use cognitum_gate_tilezero::{
    TileZero, GateThresholds, ActionContext, ActionTarget, ActionMetadata,
    GateDecision,
};

#[tokio::main]
async fn main() {
    // Create TileZero with default thresholds
    let thresholds = GateThresholds::default();
    let tilezero = TileZero::new(thresholds);

    // Define an action to evaluate
    let action = ActionContext {
        action_id: "action-001".to_string(),
        action_type: "config_change".to_string(),
        target: ActionTarget {
            device: Some("router-1".to_string()),
            path: Some("/config/firewall".to_string()),
            extra: Default::default(),
        },
        context: ActionMetadata {
            agent_id: "agent-42".to_string(),
            session_id: Some("session-abc".to_string()),
            prior_actions: vec![],
            urgency: "normal".to_string(),
        },
    };

    // Get a decision
    let token = tilezero.decide(&action).await;

    match token.decision {
        GateDecision::Permit => println!("✅ Action permitted"),
        GateDecision::Defer => println!("⚠️ Action deferred - escalate"),
        GateDecision::Deny => println!("🛑 Action denied"),
    }

    // Token is cryptographically signed
    println!("Sequence: {}", token.sequence);
    println!("Witness hash: {:x?}", &token.witness_hash[..8]);
}
```

---

## Key Capabilities

### Core Features

| Capability | Description |
|------------|-------------|
| **Report Merging** | Combine 255 worker tile reports into unified supergraph |
| **Three-Filter Pipeline** | Structural + Shift + Evidence decision making |
| **Ed25519 Signing** | Cryptographic permit tokens that can't be forged |
| **Blake3 Hash Chain** | Tamper-evident receipt log for audit compliance |
| **Async/Await** | Full Tokio async support for concurrent operations |

### Decision Outcomes

| Decision | Meaning | Recommended Action |
|----------|---------|-------------------|
| `Permit` | All filters pass, action is safe | Proceed immediately |
| `Defer` | Uncertainty detected | Escalate to human or wait |
| `Deny` | Structural issue detected | Block action, quarantine region |

---

## Tutorials

<details>
<summary><strong>Tutorial 1: Processing Worker Reports</strong></summary>

### Collecting and Merging Tile Reports

Worker tiles continuously monitor their local patch of the coherence graph. TileZero collects these reports and maintains a global view.

```rust
use cognitum_gate_tilezero::{TileZero, TileReport, WitnessFragment, GateThresholds};

#[tokio::main]
async fn main() {
    let tilezero = TileZero::new(GateThresholds::default());

    // Simulate reports from worker tiles
    let reports = vec![
        TileReport {
            tile_id: 1,
            coherence: 0.95,
            boundary_moved: false,
            suspicious_edges: vec![],
            e_value: 1.0,
            witness_fragment: None,
        },
        TileReport {
            tile_id: 2,
            coherence: 0.87,
            boundary_moved: true,
            suspicious_edges: vec![42, 43],
            e_value: 0.8,
            witness_fragment: Some(WitnessFragment {
                tile_id: 2,
                boundary_edges: vec![42, 43],
                cut_value: 5.2,
            }),
        },
    ];

    // Merge reports into supergraph
    tilezero.collect_reports(&reports).await;

    println!("Reports collected from {} tiles", reports.len());
}
```

**Key Concepts:**

- **boundary_moved**: Indicates structural change requiring supergraph update
- **witness_fragment**: Contains boundary information for witness computation
- **e_value**: Local evidence accumulator for statistical testing

</details>

<details>
<summary><strong>Tutorial 2: Verifying Permit Tokens</strong></summary>

### Token Verification and Validation

Permit tokens are Ed25519 signed and time-bounded. Recipients should verify before acting.

```rust
use cognitum_gate_tilezero::{TileZero, GateThresholds, Verifier};

#[tokio::main]
async fn main() {
    let tilezero = TileZero::new(GateThresholds::default());

    // Get the verifier (contains public key)
    let verifier: Verifier = tilezero.verifier();

    // Later, when receiving a token...
    let action = create_action();
    let token = tilezero.decide(&action).await;

    // Verify signature
    match verifier.verify(&token) {
        Ok(()) => println!("✅ Valid signature"),
        Err(e) => println!("❌ Invalid: {:?}", e),
    }

    // Check time validity
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    if token.timestamp + token.ttl_ns > now_ns {
        println!("⏰ Token still valid");
    } else {
        println!("⏰ Token expired");
    }
}
```

</details>

<details>
<summary><strong>Tutorial 3: Audit Trail with Receipt Log</strong></summary>

### Tamper-Evident Decision Logging

Every decision is logged in a Blake3 hash chain for compliance and debugging.

```rust
use cognitum_gate_tilezero::{TileZero, GateThresholds};

#[tokio::main]
async fn main() {
    let tilezero = TileZero::new(GateThresholds::default());

    // Make several decisions
    for i in 0..5 {
        let action = ActionContext {
            action_id: format!("action-{}", i),
            action_type: "test".to_string(),
            target: Default::default(),
            context: Default::default(),
        };
        let _ = tilezero.decide(&action).await;
    }

    // Retrieve specific receipt
    if let Some(receipt) = tilezero.get_receipt(2).await {
        println!("Receipt #2:");
        println!("  Decision: {:?}", receipt.token.decision);
        println!("  Timestamp: {}", receipt.token.timestamp);
        println!("  Previous hash: {:x?}", &receipt.previous_hash[..8]);
    }

    // Verify chain integrity
    match tilezero.verify_receipt_chain().await {
        Ok(()) => println!("✅ Hash chain intact"),
        Err(e) => println!("❌ Chain broken: {:?}", e),
    }

    // Export for audit
    let json = tilezero.export_receipts_json().await.unwrap();
    println!("Exported {} bytes of audit data", json.len());
}
```

</details>

<details>
<summary><strong>Tutorial 4: Custom Thresholds Configuration</strong></summary>

### Tuning the Decision Pipeline

Adjust thresholds based on your security requirements and system characteristics.

```rust
use cognitum_gate_tilezero::{TileZero, GateThresholds};

fn main() {
    // Conservative settings (more DENY/DEFER)
    let conservative = GateThresholds {
        min_cut: 10.0,           // Higher min-cut requirement
        max_shift: 0.1,          // Lower tolerance for distribution shift
        tau_deny: 0.001,         // Lower e-value triggers DENY
        tau_permit: 1000.0,      // Higher e-value needed for PERMIT
        permit_ttl_ns: 100_000,  // Shorter token validity (100μs)
    };

    // Permissive settings (more PERMIT)
    let permissive = GateThresholds {
        min_cut: 3.0,            // Lower connectivity requirement
        max_shift: 0.5,          // Higher tolerance for shift
        tau_deny: 0.0001,        // Very low e-value for DENY
        tau_permit: 10.0,        // Lower e-value sufficient for PERMIT
        permit_ttl_ns: 10_000_000, // Longer validity (10ms)
    };

    // Production defaults
    let default = GateThresholds::default();

    println!("Conservative min_cut: {}", conservative.min_cut);
    println!("Permissive min_cut: {}", permissive.min_cut);
    println!("Default min_cut: {}", default.min_cut);
}
```

**Threshold Guidelines:**

| Parameter | Low Value Effect | High Value Effect |
|-----------|------------------|-------------------|
| `min_cut` | More permissive | More conservative |
| `max_shift` | More conservative | More permissive |
| `tau_deny` | More permissive | More conservative |
| `tau_permit` | More conservative | More permissive |
| `permit_ttl_ns` | Tighter security | Looser security |

</details>

<details>
<summary><strong>Tutorial 5: Human Escalation for DEFER Decisions</strong></summary>

### Handling Uncertain Situations

When TileZero returns DEFER, escalate to a human operator.

```rust
use cognitum_gate_tilezero::{TileZero, GateDecision, EscalationInfo};

async fn handle_action(tilezero: &TileZero, action: ActionContext) {
    let token = tilezero.decide(&action).await;

    match token.decision {
        GateDecision::Permit => {
            // Auto-approve
            execute_action(&action).await;
        }
        GateDecision::Deny => {
            // Auto-reject
            log_rejection(&action, "Structural issue detected");
        }
        GateDecision::Defer => {
            // Escalate to human
            let escalation = EscalationInfo {
                to: "security-team@example.com".to_string(),
                context_url: format!("https://dashboard/actions/{}", action.action_id),
                timeout_ns: 60_000_000_000, // 60 seconds
                default_on_timeout: "deny".to_string(),
            };

            match await_human_decision(&escalation).await {
                HumanDecision::Approve => execute_action(&action).await,
                HumanDecision::Reject => log_rejection(&action, "Human rejected"),
                HumanDecision::Timeout => log_rejection(&action, "Escalation timeout"),
            }
        }
    }
}
```

</details>

---

## API Reference

<details>
<summary><strong>Core Types</strong></summary>

### GateDecision

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateDecision {
    /// All filters pass - action is permitted
    Permit,
    /// Uncertainty - defer to human or wait
    Defer,
    /// Structural issue - deny action
    Deny,
}
```

### GateThresholds

```rust
pub struct GateThresholds {
    /// Minimum global min-cut value for PERMIT
    pub min_cut: f64,
    /// Maximum allowed shift pressure
    pub max_shift: f64,
    /// E-value below which to DENY
    pub tau_deny: f64,
    /// E-value above which to PERMIT
    pub tau_permit: f64,
    /// Permit token time-to-live in nanoseconds
    pub permit_ttl_ns: u64,
}
```

### PermitToken

```rust
pub struct PermitToken {
    /// The gate decision
    pub decision: GateDecision,
    /// ID of the action this token authorizes
    pub action_id: ActionId,
    /// Unix timestamp in nanoseconds
    pub timestamp: u64,
    /// Time-to-live in nanoseconds
    pub ttl_ns: u64,
    /// Blake3 hash of witness state
    pub witness_hash: [u8; 32],
    /// Sequence number in receipt log
    pub sequence: u64,
    /// Ed25519 signature
    pub signature: [u8; 64],
}
```

</details>

<details>
<summary><strong>TileZero API</strong></summary>

### Constructor

```rust
impl TileZero {
    /// Create a new TileZero arbiter with given thresholds
    pub fn new(thresholds: GateThresholds) -> Self;
}
```

### Core Methods

```rust
impl TileZero {
    /// Collect reports from worker tiles
    pub async fn collect_reports(&self, reports: &[TileReport]);

    /// Make a gate decision for an action
    pub async fn decide(&self, action_ctx: &ActionContext) -> PermitToken;

    /// Get a receipt by sequence number
    pub async fn get_receipt(&self, sequence: u64) -> Option<WitnessReceipt>;

    /// Verify hash chain integrity
    pub async fn verify_chain_to(&self, sequence: u64) -> Result<(), ChainVerifyError>;

    /// Get the token verifier (public key)
    pub fn verifier(&self) -> Verifier;

    /// Export receipts as JSON for audit
    pub async fn export_receipts_json(&self) -> Result<String, serde_json::Error>;
}
```

</details>

---

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `mincut` | Enable ruvector-mincut integration for real min-cut | No |
| `audit-replay` | Enable decision replay for debugging | No |

```toml
# Full features
cognitum-gate-tilezero = { version = "0.1", features = ["mincut", "audit-replay"] }
```

---

## Security

### Cryptographic Guarantees

| Component | Algorithm | Purpose |
|-----------|-----------|---------|
| Token signing | **Ed25519** | Unforgeable authorization tokens |
| Hash chain | **Blake3** | Tamper-evident audit trail |
| Key derivation | **Deterministic** | Reproducible in test environments |

### Security Considerations

- **Private keys** are generated at TileZero creation and never exported
- **Tokens expire** after `permit_ttl_ns` nanoseconds
- **Hash chain** allows detection of any receipt tampering
- **Constant-time comparison** used for signature verification

---

## Integration with ruQu

TileZero is designed to work with [ruQu](../ruQu/README.md), the quantum coherence assessment system:

```rust
// ruQu provides the coherence data
let ruqu_fabric = ruqu::QuantumFabric::new(config);

// TileZero makes authorization decisions
let tilezero = TileZero::new(thresholds);

// Integration loop
loop {
    // ruQu assesses coherence
    let reports = ruqu_fabric.collect_tile_reports();

    // TileZero merges and decides
    tilezero.collect_reports(&reports).await;

    // Gate an action
    let token = tilezero.decide(&action).await;
}
```

---

## Benchmarks

Run the benchmarks:

```bash
cargo bench -p cognitum-gate-tilezero
```

### Expected Performance

| Operation | Typical Latency |
|-----------|-----------------|
| Token signing (Ed25519) | ~50μs |
| Decision evaluation | ~10μs |
| Receipt append (Blake3) | ~5μs |
| Report merge (per tile) | ~1μs |

---

## Related Crates

| Crate | Purpose |
|-------|---------|
| [ruQu](../ruQu/README.md) | Quantum coherence assessment |
| [ruvector-mincut](../ruvector-mincut/README.md) | Subpolynomial dynamic min-cut |
| [cognitum-gate-kernel](../cognitum-gate-kernel/README.md) | WASM kernel for worker tiles |

---

## License

MIT OR Apache-2.0

---

<p align="center">
  <em>"The arbiter sees all tiles. The arbiter decides."</em>
</p>

<p align="center">
  <strong>cognitum-gate-tilezero — Central coordination for distributed coherence.</strong>
</p>

<p align="center">
  <a href="https://ruv.io">ruv.io</a> •
  <a href="https://github.com/ruvnet/ruvector">RuVector</a> •
  <a href="https://crates.io/crates/cognitum-gate-tilezero">crates.io</a>
</p>

<p align="center">
  <sub>Built with care by the <a href="https://ruv.io">ruv.io</a> team</sub>
</p>

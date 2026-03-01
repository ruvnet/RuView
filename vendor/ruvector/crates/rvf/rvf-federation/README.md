# rvf-federation

[![Crates.io](https://img.shields.io/crates/v/rvf-federation.svg)](https://crates.io/crates/rvf-federation)
[![docs.rs](https://img.shields.io/docsrs/rvf-federation)](https://docs.rs/rvf-federation)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust 1.87+](https://img.shields.io/badge/rust-1.87%2B-orange.svg)](https://www.rust-lang.org)

**Privacy-preserving federated transfer learning for the RVF format.**

```toml
rvf-federation = "0.1"
```

RuVector users independently accumulate learning patterns -- SONA weight trajectories, policy kernel configurations, domain expansion priors, HNSW tuning parameters. Today that learning is siloed. `rvf-federation` implements the inter-user federation layer defined in [ADR-057](../../../docs/adr/ADR-057-federated-rvf-transfer-learning.md): it strips PII, injects differential privacy noise, packages transferable learning as RVF segments, and merges incoming learning with formal privacy guarantees.

| | rvf-federation | Siloed learning | Manual sharing |
|---|---|---|---|
| **Privacy** | 3-stage PII stripping + calibrated DP noise | N/A -- nothing leaves the machine | Trust the sender |
| **Knowledge reuse** | New users bootstrap from community priors | Every deployment starts cold | Copy-paste config files |
| **Integrity** | Witness chain + Ed25519/ML-DSA-65 signatures | N/A | No verification |
| **Aggregation** | FedAvg, FedProx, Byzantine-tolerant averaging | N/A | Manual merge |
| **Privacy accounting** | RDP composition with formal epsilon budget | N/A | N/A |

## Quick Start

```rust
use rvf_federation::{
    ExportBuilder, DiffPrivacyEngine, FederationPolicy,
    TransferPriorSet, TransferPriorEntry, BetaParams,
};

// 1. Build an export from local learning
let priors = TransferPriorSet {
    source_domain: "code_review".into(),
    entries: vec![TransferPriorEntry {
        bucket_id: "medium_algorithm".into(),
        arm_id: "arm_0".into(),
        params: BetaParams::new(10.0, 5.0),
        observation_count: 50,
    }],
    cost_ema: 0.85,
};

// 2. Configure differential privacy (epsilon=1.0, delta=1e-5)
let mut dp = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 1.0).unwrap();

// 3. Build: PII strip -> DP noise -> assemble manifest
let export = ExportBuilder::new("alice_pseudo".into(), "code_review".into())
    .with_policy(FederationPolicy::default())
    .add_priors(priors)
    .add_string_field("config_path".into(), "/home/alice/project/.config".into())
    .build(&mut dp)
    .unwrap();

assert_eq!(export.manifest.format_version, 1);
assert!(export.redaction_log.total_redactions >= 1); // PII was stripped
assert!(export.privacy_proof.epsilon > 0.0);         // DP noise was applied
```

## Key Features

| Feature | What It Does | Why It Matters |
|---|---|---|
| **PII stripping** | 3-stage pipeline: detect, redact, attest | No personal data leaves the local machine |
| **Differential privacy** | Gaussian/Laplace noise with RDP accounting | Formal mathematical privacy guarantee per export |
| **Gradient clipping** | Bound L2 norms before aggregation | Limits any single user's influence on the aggregate |
| **FedAvg / FedProx** | Federated averaging with optional proximal term | Industry-standard aggregation (McMahan et al. 2017) |
| **Byzantine tolerance** | Outlier detection by L2-norm z-score | Malicious contributions are excluded automatically |
| **Version-aware merging** | Dampened confidence for cross-version imports | Older learning still helps, with reduced weight |
| **Selective sharing** | Allowlist/denylist for segments and domains | Users control exactly what they share |

## Architecture

```
Local Engine                                             Remote
  +------------------+    +------------+    +---------+     +----------+
  | TransferPriors   |--->|            |--->|         |---->|          |
  | PolicyKernels    |    | PII Strip  |    | DP      |    | RVF      |     Registry
  | CostCurves       |    | (3-stage)  |    | Noise   |    | Export   |---->  (GCS)
  | LoRA Weights     |    |            |    |         |    | Builder  |       |
  +------------------+    +------------+    +---------+    +----------+       |
                                                                             v
  +------------------+    +------------+    +---------+     +----------+  +--------+
  | Merged Learning  |<---| Version-   |<---| Import  |<----| Validate |<-| Import |
  | (local engines)  |    | Aware      |    | Merger  |    | (sig +   |  | (pull) |
  |                  |    | Merge      |    |         |    | witness) |  +--------+
  +------------------+    +------------+    +---------+    +----------+
```

## Modules

| Module | Description |
|---|---|
| `types` | Four new RVF segment payload types (0x33-0x36) plus federation data structures |
| `error` | 15 error variants covering privacy, validation, aggregation, and I/O failures |
| `pii_strip` | Three-stage PII stripping pipeline with 12 built-in detection rules |
| `diff_privacy` | Gaussian/Laplace noise engines, gradient clipping, RDP privacy accountant |
| `federation` | `ExportBuilder` and `ImportMerger` implementing the ADR-057 transfer protocol |
| `aggregate` | `FederatedAggregator` with FedAvg, FedProx, and Byzantine-tolerant strategies |
| `policy` | `FederationPolicy` for selective sharing with allowlists, denylists, and rate limits |

## Segment Types

Four new RVF segment types extend the `0x30-0x32` domain expansion range:

| Code | Name | Purpose |
|---|---|---|
| `0x33` | `FederatedManifest` | Describes the export: contributor pseudonym, timestamp, included segments, privacy budget spent |
| `0x34` | `DiffPrivacyProof` | Privacy attestation: epsilon/delta, mechanism, sensitivity, clipping norm, noise scale |
| `0x35` | `RedactionLog` | PII stripping attestation: redaction counts by category, pre-redaction content hash, rules fired |
| `0x36` | `AggregateWeights` | Federated-averaged LoRA deltas with participation count, round number, confidence scores |

Readers that do not recognize these segment types skip them per the RVF forward-compatibility rule. Existing `TransferPrior (0x30)`, `PolicyKernel (0x31)`, `CostCurve (0x32)`, `Witness`, and `Crypto` segments are reused as-is.

## PII Stripping Pipeline

`PiiStripper` runs a three-stage pipeline on every string field before it leaves the local machine.

**Stage 1 -- Detection.** Twelve built-in regex rules scan for:

- Unix and Windows file paths (`/home/user/...`, `C:\Users\...`)
- IPv4 and IPv6 addresses
- Email addresses
- API keys (`sk-...`, `AKIA...`, `ghp_...`, Bearer tokens)
- Environment variable references (`$HOME`, `%USERPROFILE%`)
- Usernames (`@handle`)

Custom rules can be registered with `add_rule()`.

**Stage 2 -- Redaction.** Detected PII is replaced with deterministic pseudonyms (`<PATH_1>`, `<IP_2>`, `<REDACTED_KEY>`). The same original value always maps to the same pseudonym within a single export, preserving structural relationships without revealing content.

**Stage 3 -- Attestation.** A `RedactionLog (0x35)` segment is generated containing redaction counts by category, the SHAKE-256 hash of the pre-redaction content (proves scanning happened without revealing it), and the rules that fired.

```rust
use rvf_federation::PiiStripper;

let mut stripper = PiiStripper::new();
let fields = vec![
    ("config", "/home/alice/project/.env"),
    ("server", "connecting to 10.0.0.1:8080"),
    ("note", "no pii here"),
];
let (redacted, log) = stripper.strip_fields(&fields);
assert_eq!(log.fields_scanned, 3);
assert!(log.total_redactions >= 2);
assert!(redacted[2].1 == "no pii here"); // clean fields pass through
```

## Differential Privacy

### Noise Mechanisms

| Mechanism | Privacy Model | Noise Distribution | Use Case |
|---|---|---|---|
| Gaussian | (epsilon, delta)-DP | N(0, sigma^2) where sigma = S * sqrt(2 ln(1.25/delta)) / epsilon | Default; tighter for large parameter counts |
| Laplace | Pure epsilon-DP | Laplace(0, S/epsilon) | Stronger guarantee; no delta term |

### Gradient Clipping

Before noise injection, all parameter vectors are clipped to a configurable L2 norm bound. This limits the sensitivity of the aggregation to any single user's contribution.

### Privacy Accountant

`PrivacyAccountant` tracks cumulative privacy loss using Renyi Differential Privacy (RDP) composition across 16 alpha orders. RDP composition is tighter than naive (epsilon, delta)-DP composition, meaning more exports fit within the same budget.

```rust
use rvf_federation::PrivacyAccountant;

let mut accountant = PrivacyAccountant::new(10.0, 1e-5); // budget: eps=10, delta=1e-5
accountant.record_gaussian(1.0, 1.0, 1e-5, 100);
assert!(accountant.remaining_budget() > 0.0);
assert!(!accountant.is_exhausted());
```

## Federation Strategies

| Strategy | Algorithm | Weighting | When to Use |
|---|---|---|---|
| `FedAvg` | Federated Averaging (McMahan et al.) | Trajectory count | Default; most scenarios |
| `FedProx` | Proximal regularization | Trajectory count + mu penalty | Heterogeneous data distributions |
| `WeightedAverage` | Simple weighted mean | Quality/reputation score | When contributor reputation varies widely |
| Byzantine detection | L2-norm z-score filtering | Outliers > 2 std removed | Always runs before aggregation |

```rust
use rvf_federation::{FederatedAggregator, AggregationStrategy};
use rvf_federation::aggregate::Contribution;

let mut agg = FederatedAggregator::new("code_review".into(), AggregationStrategy::FedAvg)
    .with_min_contributions(2)
    .with_byzantine_threshold(2.0);

agg.add_contribution(Contribution {
    contributor: "alice".into(),
    weights: vec![1.0, 2.0, 3.0],
    quality_weight: 0.9,
    trajectory_count: 100,
});
agg.add_contribution(Contribution {
    contributor: "bob".into(),
    weights: vec![1.2, 1.8, 3.1],
    quality_weight: 0.85,
    trajectory_count: 80,
});

let result = agg.aggregate().unwrap();
assert_eq!(result.participation_count, 2);
assert_eq!(result.lora_deltas.len(), 3);
```

## Performance Benchmarks

Measured on an AMD64 Linux system with Criterion.

| Benchmark | Time |
|---|---|
| PII detect (single string) | 756 ns |
| PII strip (10 fields) | 44 us |
| PII strip (100 fields) | 303 us |
| Gaussian noise (100 params) | 4.7 us |
| Gaussian noise (10k params) | 334 us |
| Gradient clipping (1k params) | 487 ns |
| Privacy accountant (100 rounds) | 1.0 us |
| FedAvg (10 contrib, 100 dim) | 3.9 us |
| FedAvg (100 contrib, 1k dim) | 365 us |
| Byzantine detection (50 contrib) | 12 us |
| Full export pipeline | 1.2 ms |
| Merge 100 priors | 28 us |

## Feature Flags

| Flag | Default | What It Enables |
|---|---|---|
| `std` | Yes | Standard library support (required) |
| `serde` | No | Derive `Serialize`/`Deserialize` on all public types |

```toml
[dependencies]
rvf-federation = { version = "0.1", features = ["serde"] }
```

## API Overview

### Core Types

| Type | Description |
|---|---|
| `FederatedManifest` | Export metadata: contributor pseudonym, domain, timestamp, privacy budget spent |
| `DiffPrivacyProof` | Privacy attestation: epsilon, delta, mechanism, sensitivity, noise scale |
| `RedactionLog` | PII stripping attestation: entries by category, pre-redaction hash, field count |
| `AggregateWeights` | Federated-averaged LoRA deltas with round number, participation count, confidences |
| `BetaParams` | Beta distribution parameters for Thompson Sampling priors (merge, dampen, mean) |

### Transfer Types

| Type | Description |
|---|---|
| `TransferPriorEntry` | Single context bucket prior: bucket ID, arm ID, Beta params, observation count |
| `TransferPriorSet` | Collection of priors from a trained domain with cost EMA |
| `PolicyKernelSnapshot` | Snapshot of tunable policy knob values with fitness score |
| `CostCurveSnapshot` | Ordered (step, cost) points with acceleration factor |

### Aggregation Types

| Type | Description |
|---|---|
| `FederatedAggregator` | Aggregation server: collects contributions, detects outliers, produces `AggregateWeights` |
| `AggregationStrategy` | `FedAvg`, `FedProx { mu }`, or `WeightedAverage` |
| `Contribution` | Single participant's weight vector with quality and trajectory metadata |

### Protocol Types

| Type | Description |
|---|---|
| `ExportBuilder` | Builder pattern: add priors/kernels/weights, PII-strip, DP-noise, produce `FederatedExport` |
| `ImportMerger` | Validate imports, merge priors with version-aware dampening, merge weights |
| `FederatedExport` | Completed export: manifest + redaction log + privacy proof + learning data |
| `FederationPolicy` | Selective sharing: allowlists, denylists, quality gate, rate limit, privacy budget |
| `PiiStripper` | Three-stage PII pipeline: detect, redact, attest |
| `DiffPrivacyEngine` | Noise injection with Gaussian or Laplace mechanism and gradient clipping |
| `PrivacyAccountant` | RDP-based cumulative privacy loss tracker |

### Error Types

`FederationError` covers 15 variants:

| Variant | Trigger |
|---|---|
| `PrivacyBudgetExhausted` | Cumulative epsilon exceeds limit |
| `InvalidEpsilon` | Epsilon <= 0 |
| `InvalidDelta` | Delta outside (0, 1) |
| `SegmentValidation` | Malformed segment data |
| `VersionMismatch` | Incompatible format version |
| `SignatureVerification` | Ed25519/ML-DSA-65 signature check failed |
| `WitnessChainBroken` | Witness chain has a gap or tampered entry |
| `InsufficientObservations` | Prior has too few observations for export |
| `QualityBelowThreshold` | Trajectory quality below policy minimum |
| `RateLimited` | Export rate limit exceeded |
| `PiiLeakDetected` | PII found after stripping (defense-in-depth) |
| `ByzantineOutlier` | Contribution flagged as adversarial |
| `InsufficientContributions` | Not enough participants for aggregation round |
| `Serialization` | Encoding/decoding failure |
| `Io` | I/O operation failure |

## Related Crates

| Crate | Relationship |
|---|---|
| [`rvf-types`](../rvf-types) | Core RVF segment definitions; `rvf-federation` defines its own payload types to avoid circular deps |
| [`ruvector-domain-expansion`](../../ruvector-domain-expansion) | Source of `TransferPrior`, `PolicyKernel`, `CostCurve`; federation exports these as RVF segments |
| [`sona`](../../sona) | SONA learning engine; `FederatedCoordinator` handles intra-deployment aggregation, `rvf-federation` handles inter-user |
| [`rvf-crypto`](../rvf-crypto) | Ed25519 signatures and SHAKE-256 hashing used for witness chains and segment integrity |

## Testing

54 tests across all modules:

```bash
cargo test -p rvf-federation
```

Benchmarks:

```bash
cargo bench -p rvf-federation
```

## License

MIT OR Apache-2.0

---

Part of [RuVector](https://github.com/ruvnet/ruvector) -- the self-learning vector database.

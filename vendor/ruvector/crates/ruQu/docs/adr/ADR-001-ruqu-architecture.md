# ADR-001: ruQu Architecture - Classical Nervous System for Quantum Machines

**Status**: Proposed
**Date**: 2026-01-17
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-17 | ruv.io | Initial architecture proposal |

---

## Context

### The Quantum Operability Problem

Quantum computers in 2025 have achieved remarkable milestones:
- Google Willow: Below-threshold error correction (0.143% per cycle)
- Quantinuum Helios: 98 qubits with 48 logical qubits at 2:1 ratio
- Riverlane: 240ns ASIC decoder latency
- IonQ: 99.99%+ two-qubit gate fidelity

Yet these systems remain **fragile laboratory instruments**, not **operable production systems**.

The gap is not in the quantum hardware or the decoders. The gap is in the **classical control intelligence** that mediates between hardware and algorithms.

### Current Limitations

| Limitation | Impact |
|------------|--------|
| **Monolithic treatment** | Entire device treated as one object per cycle |
| **Reactive control** | Decoders react after errors accumulate |
| **Static policies** | Fixed decoder, schedule, cadence |
| **Superlinear overhead** | Control infrastructure scales worse than qubit count |

### The Missing Primitive

Current systems can ask:
> "What is the most likely correction?"

They cannot ask:
> "Is this system still internally consistent enough to trust action?"

**That question, answered continuously at microsecond timescales, is the missing primitive.**

---

## Decision

### Introduce ruQu: A Two-Layer Classical Nervous System

We propose ruQu, a classical control layer combining:

1. **RuVector Memory Layer**: Pattern recognition and historical mitigation retrieval
2. **Dynamic Min-Cut Gate**: Real-time structural coherence assessment

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ruQu FABRIC                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐ │
│  │                         TILE ZERO (Coordinator)                       │ │
│  │  • Supergraph merge                  • Global min-cut evaluation     │ │
│  │  • Permit token issuance             • Hash-chained receipt log      │ │
│  └───────────────────────────────────────────────────────────────────────┘ │
│                                      │                                      │
│         ┌────────────────────────────┼────────────────────────────┐        │
│         ▼                            ▼                            ▼         │
│  ┌─────────────┐            ┌─────────────┐            ┌─────────────┐     │
│  │ WORKER TILE │            │ WORKER TILE │            │ WORKER TILE │     │
│  │   [1-85]    │   × 85     │  [86-170]   │   × 85     │ [171-255]   │× 85 │
│  │             │            │             │            │             │     │
│  │ • Patch     │            │ • Patch     │            │ • Patch     │     │
│  │ • Syndromes │            │ • Syndromes │            │ • Syndromes │     │
│  │ • Local cut │            │ • Local cut │            │ • Local cut │     │
│  │ • E-accum   │            │ • E-accum   │            │ • E-accum   │     │
│  └─────────────┘            └─────────────┘            └─────────────┘     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. Operational Graph Model

The operational graph includes all elements that can affect quantum coherence:

| Node Type | Examples | Edge Type |
|-----------|----------|-----------|
| **Qubits** | Data, ancilla, flag | Coupling strength |
| **Couplers** | ZZ, XY, tunable | Crosstalk correlation |
| **Readout** | Resonators, amplifiers | Signal path dependency |
| **Control** | Flux, microwave, DC | Control line routing |
| **Classical** | Clocks, temperature, calibration | State dependency |

#### 2. Dynamic Min-Cut as Coherence Metric

The min-cut between "healthy" and "unhealthy" partitions provides:

- **Structural fragility**: Low cut value = boundary forming
- **Localization**: Cut edges identify the fracture point
- **Early warning**: Cut value drops before logical errors spike

**Complexity**: O(n^{o(1)}) update time via SubpolynomialMinCut from ruvector-mincut

#### 3. Three-Filter Decision Logic

```
┌─────────────────────────────────────────────────────────────────┐
│                    FILTER 1: STRUCTURAL                         │
│  Local fragility detection → Global cut confirmation            │
│  Cut ≥ threshold → Coherent                                     │
│  Cut < threshold → Boundary forming → Quarantine                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    FILTER 2: SHIFT                              │
│  Nonconformity scores → Aggregated shift pressure               │
│  Shift < threshold → Distribution stable                        │
│  Shift ≥ threshold → Drift detected → Conservative mode        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    FILTER 3: EVIDENCE                           │
│  Running e-value accumulators → Anytime-valid testing           │
│  E ≥ τ_permit → Accept (permit immediately)                     │
│  E ≤ τ_deny → Reject (deny immediately)                         │
│  Otherwise → Continue (gather more evidence)                    │
└─────────────────────────────────────────────────────────────────┘
```

#### 4. Tile Architecture

Each worker tile (64KB memory budget):

| Component | Size | Purpose |
|-----------|------|---------|
| Patch Graph | ~32KB | Local graph shard (vertices, edges, adjacency) |
| Syndrome Ring | ~16KB | Rolling syndrome history (1024 rounds) |
| Evidence Accumulator | ~4KB | E-value computation |
| Local Min-Cut | ~8KB | Boundary candidates, cut cache, witness fragments |
| Control/Scratch | ~4KB | Delta buffer, report scratch, stack |

#### 5. Decision Output

The coherence gate outputs a decision every cycle:

```rust
enum GateDecision {
    Safe {
        region_mask: RegionMask,     // Which regions are stable
        permit_token: PermitToken,   // Signed authorization
    },
    Cautious {
        region_mask: RegionMask,     // Which regions need care
        lead_time: Cycles,           // Estimated cycles before degradation
        recommendations: Vec<Action>, // Suggested mitigations
    },
    Unsafe {
        quarantine_mask: RegionMask, // Which regions to isolate
        recovery_mode: RecoveryMode, // How to recover
        witness: WitnessReceipt,     // Audit trail
    },
}
```

---

## Rationale

### Why Min-Cut for Coherence?

1. **Graph structure captures dependencies**: Qubits, couplers, and control lines form a natural graph
2. **Cut value quantifies fragility**: Low cut = system splitting into incoherent partitions
3. **Edges identify the boundary**: Know exactly which connections are failing
4. **Subpolynomial updates**: O(n^{o(1)}) enables real-time tracking

### Why Three Filters?

| Filter | What It Catches | Timescale |
|--------|-----------------|-----------|
| **Structural** | Partition formation, hardware failures | Immediate |
| **Shift** | Calibration drift, environmental changes | Gradual |
| **Evidence** | Statistical anomalies, rare events | Cumulative |

All three must agree for PERMIT. Any one can trigger DENY or DEFER.

### Why 256 Tiles?

- Maps to practical FPGA/ASIC fabric sizes
- 255 workers can cover ~512 qubits each (130K qubit system)
- Single TileZero keeps coordination simple
- Power of 2 enables efficient addressing

### Why Not Just Improve Decoders?

Decoders answer: "What correction should I apply?"

ruQu answers: "Should I apply any correction right now?"

These are complementary, not competing. ruQu tells decoders when to work hard and when to relax.

---

## Alternatives Considered

### Alternative 1: Purely Statistical Approach

Use only statistical tests on syndrome streams without graph structure.

**Rejected because**:
- Cannot identify *where* problems are forming
- Cannot leverage structural dependencies
- Cannot provide localized quarantine

### Alternative 2: Post-Hoc Analysis

Analyze syndrome logs offline to detect patterns.

**Rejected because**:
- No real-time intervention possible
- Problems detected after logical failures
- Cannot enable adaptive control

### Alternative 3: Hardware-Only Solution

Implement all logic in quantum hardware or cryogenic electronics.

**Rejected because**:
- Inflexible to algorithm changes
- High development cost
- Limited to simple policies

### Alternative 4: Single-Level Evaluation

No tile hierarchy, evaluate whole system each cycle.

**Rejected because**:
- Does not scale beyond ~1000 qubits
- Cannot provide regional policies
- Single point of failure

---

## Consequences

### Benefits

1. **Localized Recovery**: Quarantine smallest region, keep rest running
2. **Early Warning**: Detect correlated failures before logical errors
3. **Selective Overhead**: Extra work only where needed
4. **Bounded Latency**: Constant-time decision every cycle
5. **Audit Trail**: Cryptographic proof of every decision
6. **Scalability**: Effort scales with structure, not system size

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Graph model mismatch | Medium | High | Learn graph from trajectories |
| Threshold tuning difficulty | Medium | Medium | Adaptive thresholds via meta-learning |
| FPGA latency exceeds budget | Low | High | ASIC path for production |
| Correlated noise overwhelms detection | Low | High | Multiple detection modalities |

### Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| Gate decision latency | < 4 μs p99 | Compatible with 1 MHz syndrome rate |
| Memory per tile | < 64 KB | Fits in FPGA BRAM |
| Power consumption | < 100 mW | Cryo-compatible ASIC path |
| Lead time for correlation | > 100 cycles | Actionable warning |

---

## Implementation Status

### Completed (v0.1.0)

**Core Implementation** (340+ tests passing):

| Module | Status | Description |
|--------|--------|-------------|
| `ruqu::types` | ✅ Complete | GateDecision, RegionMask, Verdict, FilterResults |
| `ruqu::syndrome` | ✅ Complete | DetectorBitmap (SIMD-ready), SyndromeBuffer, SyndromeDelta |
| `ruqu::filters` | ✅ Complete | StructuralFilter, ShiftFilter, EvidenceFilter, FilterPipeline |
| `ruqu::tile` | ✅ Complete | WorkerTile (64KB), TileZero, PatchGraph, ReceiptLog |
| `ruqu::fabric` | ✅ Complete | QuantumFabric, FabricBuilder, CoherenceGate, PatchMap |
| `ruqu::error` | ✅ Complete | RuQuError with thiserror |

**Security Review** (see `docs/SECURITY-REVIEW.md`):
- 3 Critical findings fixed (signature length, verification, hash chain)
- 5 High findings fixed (bounds validation, hex panic, TTL validation)
- Ed25519 64-byte signatures implemented
- Bounds checking in release mode

**Test Coverage**:
- 90 library unit tests
- 66 integration tests
- Property-based tests with proptest
- Memory budget verification (64KB per tile)

**Benchmarks** (see `benches/`):
- `latency_bench.rs` - Gate decision latency profiling
- `throughput_bench.rs` - Syndrome ingestion rates
- `scaling_bench.rs` - Code distance/qubit scaling
- `memory_bench.rs` - Memory efficiency verification

---

## Implementation Phases

### Phase 1: Simulation Demo (v0.1) ✅ COMPLETE

- Stim simulation stream
- Baseline decoder (PyMatching)
- ruQu gate + partition only
- Controller switches fast/slow decode

**Deliverables**:
- Gate latency distribution
- Correlation detection lead time
- Logical error vs overhead curve

### Phase 2: FPGA Prototype (v0.2)

- AMD VU19P or equivalent
- Full 256-tile fabric
- Real syndrome stream from hardware
- Integration with existing decoder

### Phase 3: ASIC Design (v1.0)

- Custom 256-tile fabric
- < 250 ns latency target
- ~100 mW power budget
- 4K operation capable

---

## Integration Points

### RuVector Components Used

| Component | Purpose |
|-----------|---------|
| `ruvector-mincut::SubpolynomialMinCut` | O(n^{o(1)}) dynamic cut |
| `ruvector-mincut::WitnessTree` | Cut certificates |
| `cognitum-gate-kernel` | Worker tile implementation |
| `cognitum-gate-tilezero` | Coordinator implementation |
| `rvlite` | Pattern memory storage |

### External Interfaces

| Interface | Protocol | Purpose |
|-----------|----------|---------|
| Syndrome input | Streaming binary | Hardware syndrome data |
| Decoder control | gRPC/REST | Switch decoder modes |
| Calibration | gRPC | Trigger targeted calibration |
| Monitoring | Prometheus | Export metrics |
| Audit | Log files / API | Receipt chain export |

---

## Open Questions

1. **Optimal patch size**: How many qubits per worker tile?
2. **Overlap band width**: How much redundancy at tile boundaries?
3. **Threshold initialization**: How to set thresholds for new hardware?
4. **Multi-chip coordination**: How to extend to federated systems?
5. **Learning integration**: How to update graph model online?

---

## References

1. El-Hayek, Henzinger, Li. "Dynamic Min-Cut with Subpolynomial Update Time." arXiv:2512.13105, 2025.
2. Google Quantum AI. "Quantum error correction below the surface code threshold." Nature, 2024.
3. Riverlane. "Collision Clustering Decoder." Nature Communications, 2025.
4. RuVector Team. "ADR-001: Anytime-Valid Coherence Gate." 2026.

---

## Appendix A: Latency Analysis

### Critical Path Breakdown

```
Syndrome Arrival        → 0 ns
  │
  ▼ Ring buffer append  → +50 ns
Delta Dispatch
  │
  ▼ Graph update        → +200 ns (amortized O(n^{o(1)}))
Worker Tick
  │
  ▼ Local cut eval      → +500 ns
  ▼ Report generation   → +100 ns
Worker Report Complete
  │
  ▼ Report collection   → +500 ns (parallel from 255 tiles)
TileZero Merge
  │
  ▼ Global cut          → +300 ns
  ▼ Three-filter eval   → +100 ns
Gate Decision
  │
  ▼ Token signing       → +500 ns (Ed25519)
  ▼ Receipt append      → +100 ns
Decision Complete       → ~2,350 ns total

Margin                  → ~1,650 ns (to 4 μs budget)
```

---

## Appendix B: Memory Layout

### Worker Tile (64 KB)

```
0x0000 - 0x7FFF : Patch Graph (32 KB)
  0x0000 - 0x1FFF : Vertex array (512 vertices × 16 bytes)
  0x2000 - 0x5FFF : Edge array (2048 edges × 8 bytes)
  0x6000 - 0x7FFF : Adjacency lists

0x8000 - 0xBFFF : Syndrome Ring (16 KB)
  1024 rounds × 16 bytes per round

0xC000 - 0xCFFF : Evidence Accumulator (4 KB)
  Hypothesis states, log e-values, window stats

0xD000 - 0xEFFF : Local Min-Cut State (8 KB)
  Boundary candidates, cut cache, witness fragments

0xF000 - 0xFFFF : Control (4 KB)
  Delta buffer, report scratch, stack
```

---

## Appendix C: Decision Flow Pseudocode

```python
def gate_evaluate(tile_reports: List[TileReport]) -> GateDecision:
    # Merge reports into supergraph
    supergraph = merge_reports(tile_reports)

    # Filter 1: Structural
    global_cut = supergraph.min_cut()
    if global_cut < THRESHOLD_STRUCTURAL:
        boundary = supergraph.cut_edges()
        return GateDecision.Unsafe(
            quarantine_mask=identify_regions(boundary),
            recovery_mode=RecoveryMode.LocalReset,
            witness=generate_witness(supergraph, boundary)
        )

    # Filter 2: Shift
    shift_pressure = supergraph.aggregate_shift()
    if shift_pressure > THRESHOLD_SHIFT:
        affected = supergraph.high_shift_regions()
        return GateDecision.Cautious(
            region_mask=affected,
            lead_time=estimate_lead_time(shift_pressure),
            recommendations=[
                Action.IncreaseSyndromeRounds(affected),
                Action.SwitchToConservativeDecoder(affected)
            ]
        )

    # Filter 3: Evidence
    e_value = supergraph.aggregate_evidence()
    if e_value < THRESHOLD_DENY:
        return GateDecision.Unsafe(...)
    elif e_value < THRESHOLD_PERMIT:
        return GateDecision.Cautious(
            lead_time=evidence_to_lead_time(e_value),
            ...
        )

    # All filters pass
    return GateDecision.Safe(
        region_mask=RegionMask.all(),
        permit_token=sign_permit(supergraph.hash())
    )
```

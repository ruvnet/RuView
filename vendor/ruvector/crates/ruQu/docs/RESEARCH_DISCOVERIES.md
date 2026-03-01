# Research Discoveries for ruQu Enhancement

*Compiled: January 2026*

This document captures state-of-the-art research findings that can inform further improvements to ruQu's coherence gate architecture.

---

## 1. Real-Time Decoding at Scale

### DECONET System (April 2025)
**Source**: [arXiv:2504.11805](https://arxiv.org/abs/2504.11805)

DECONET is a first-of-its-kind decoding system that scales to **thousands of logical qubits** with lattice surgery support. Key innovations:

- **Network-integrated hybrid tree-grid structure**: O(log(l)) latency increase as system grows
- **Resource scaling**: O(l × log(l)) compute, O(l) I/O for l logical qubits
- **Union-Find decoder**: 100× higher accuracy than greedy algorithms
- **Prototype**: 100 logical qubits on 5 VMK-180 FPGAs

**Relevance to ruQu**: Our `ParallelFabric` uses flat parallelism. Consider hierarchical tree-grid topology for 1000+ tile scaling.

### Google Below-Threshold (2025)
**Source**: [Nature 2024](https://www.nature.com/articles/s41586-024-08449-y)

Google achieved Λ = 2.14 ± 0.02 error suppression when increasing code distance by 2, with a 101-qubit distance-7 code achieving **0.143% error per cycle**.

**Relevance to ruQu**: Our three-filter decision pipeline should target similar sub-0.2% false positive rates.

---

## 2. Hardware-Accelerated Decoding

### Riverlane Collision Clustering Decoder
**Source**: [Riverlane Blog](https://www.riverlane.com/news/introducing-the-world-s-first-low-latency-qec-experiment)

| Platform | Qubits | Latency | Power |
|----------|--------|---------|-------|
| FPGA | 881 | 810 ns | - |
| ASIC | 1,057 | **240 ns** | 8 mW |

The ASIC fits in 0.06 mm² - suitable for cryogenic deployment.

**Relevance to ruQu**: Our coherence simulation achieves 468ns P99. ASIC compilation of the hot path could reach 240ns.

### QASBA: Sparse Blossom on FPGA
**Source**: [ACM TRETS](https://dl.acm.org/doi/10.1145/3723168)

- **25× performance** vs software baseline
- **304× energy efficiency** improvement

**Relevance to ruQu**: Our min-cut computation is the hot path. FPGA synthesis of `SubpolynomialMinCut` could yield similar gains.

---

## 3. Adaptive Syndrome Extraction

### PRX Quantum (July 2025)
**Source**: [PRX Quantum](https://doi.org/10.1103/ps3r-wf84)

Adaptive syndrome extraction measures **only stabilizers likely to provide useful information**:

- **10× lower logical error rates** vs non-adaptive
- Fewer CNOT gates and physical qubits
- Uses [[4,2,2]] concatenated with hypergraph product code

**Relevance to ruQu**: This validates our coherence gate philosophy - don't process everything, focus on what matters. Consider:
- Tracking which detectors fire frequently (already in `stim.rs`)
- Skip syndrome processing for "quiet" regions
- Adaptive measurement scheduling

### Multi-Agent RL for QEC
**Source**: [arXiv:2509.03974](https://arxiv.org/pdf/2509.03974)

Uses **reinforcement learning bandits** to:
- Evaluate fidelity after recovery
- Determine when retraining is necessary
- Optimize encoder, syndrome measurement, and recovery jointly

**Relevance to ruQu**: Our `AdaptiveThresholds` uses EMA-based learning. Consider upgrading to bandit-based exploration for threshold optimization.

### Window-Based Drift Estimation (Nov 2025)
**Source**: [arXiv:2511.09491](https://arxiv.org/html/2511.09491)

Estimates noise drift profiles **from syndrome data alone**, then adapts decoder parameters.

**Relevance to ruQu**: Integrate drift detection into `adaptive.rs`:
```rust
pub fn detect_drift(&mut self, window: &[SyndromeStats]) -> Option<DriftProfile> {
    // Detect if noise characteristics are shifting
    // Adjust thresholds proactively
}
```

---

## 4. Mixture-of-Depths for Efficiency

### MoD (DeepMind, 2024)
**Source**: [arXiv:2404.02258](https://arxiv.org/html/2404.02258v1)

- **50% FLOPs reduction** while matching dense transformer performance
- Per-token dynamic routing (skip middle layers for "resolved" tokens)
- Different from early-exit: tokens can skip middle layers then attend

**Status**: Already implemented in `attention.rs` via `MincutDepthRouter` integration.

### Mixture-of-Recursions (NeurIPS 2025)
**Source**: [arXiv:2507.10524](https://arxiv.org/html/2507.10524v1)

Combines parameter sharing + adaptive computation:
- Reuses shared layer stack across recursion steps
- Lightweight routers assign recursion depth per-token
- Token-level early exiting for simple predictions

**Relevance to ruQu**: Consider recursive tile processing:
```rust
pub fn process_recursive(&mut self, syndrome: &SyndromeDelta, max_depth: usize) -> GateDecision {
    for depth in 0..max_depth {
        let decision = self.process_at_depth(syndrome, depth);
        if decision.confidence > EARLY_EXIT_THRESHOLD {
            return decision;  // Exit early for clear cases
        }
    }
    decision
}
```

---

## 5. Fusion Blossom Performance

### Fusion Blossom Decoder
**Source**: [arXiv:2305.08307](https://arxiv.org/abs/2305.08307), [GitHub](https://github.com/yuewuo/fusion-blossom)

- **1 million measurement rounds/second** at d=33
- **0.7 ms latency** in stream mode at d=21
- **58 ns per non-trivial measurement** on 64-core machine
- O(N) complexity for defect vertices N

**Status**: Already integrated via `decoder.rs` feature. Consider:
- Enabling parallel fusion mode in production
- Streaming mode for real-time applications

### PyMatching V2 Comparison
PyMatching V2 achieves 5-20× single-thread speedup over Fusion Blossom. The algorithms are compatible - combining them could yield another 5-20× improvement.

---

## 6. Graph Neural Networks for QEC

### QSeer (May 2025)
**Source**: [arXiv:2505.06810](https://arxiv.org/abs/2505.06810)

GNN for QAOA parameter prediction:
- 6-68% improvement in approximation ratio
- 5-10× convergence speedup
- Supports variable-depth circuits and weighted Max-Cut

**Relevance to ruQu**: Train a small GNN to predict optimal thresholds from syndrome graph structure:
```rust
pub struct ThresholdPredictor {
    model: OnnxModel,  // Export trained model
}

impl ThresholdPredictor {
    pub fn predict(&self, graph_embedding: &[f32]) -> GateThresholds {
        // Use learned model for threshold prediction
    }
}
```

---

## Implementation Priority Matrix

| Enhancement | Impact | Effort | Priority |
|-------------|--------|--------|----------|
| Hierarchical tree-grid topology | High | High | P2 |
| Drift detection in adaptive.rs | High | Medium | P1 |
| Recursive early-exit processing | Medium | Low | P1 |
| Bandit-based threshold exploration | Medium | Medium | P2 |
| FPGA synthesis of min-cut | Very High | Very High | P3 |
| GNN threshold predictor | Medium | High | P3 |
| Streaming Fusion mode | High | Low | P1 |

---

## Immediate Next Steps

1. **Drift Detection**: Add window-based drift estimation to `adaptive.rs`
2. **Early-Exit Depth**: Implement confidence-based early exit in tile processing
3. **Streaming Decoder**: Enable Fusion Blossom streaming mode for <1ms latency
4. **Parallel Fusion**: Configure parallel fusion on 64+ core systems

---

## References

1. DECONET: [arxiv.org/abs/2504.11805](https://arxiv.org/abs/2504.11805)
2. Google Below-Threshold: [nature.com/articles/s41586-024-08449-y](https://www.nature.com/articles/s41586-024-08449-y)
3. Riverlane CC Decoder: [riverlane.com](https://www.riverlane.com/news/introducing-the-world-s-first-low-latency-qec-experiment)
4. Adaptive Syndrome Extraction: [doi.org/10.1103/ps3r-wf84](https://doi.org/10.1103/ps3r-wf84)
5. Multi-Agent RL QEC: [arxiv.org/pdf/2509.03974](https://arxiv.org/pdf/2509.03974)
6. Drift Estimation: [arxiv.org/html/2511.09491](https://arxiv.org/html/2511.09491)
7. Mixture-of-Depths: [arxiv.org/html/2404.02258v1](https://arxiv.org/html/2404.02258v1)
8. Mixture-of-Recursions: [arxiv.org/html/2507.10524v1](https://arxiv.org/html/2507.10524v1)
9. Fusion Blossom: [arxiv.org/abs/2305.08307](https://arxiv.org/abs/2305.08307)
10. QSeer GNN: [arxiv.org/abs/2505.06810](https://arxiv.org/abs/2505.06810)
11. QASBA FPGA: [dl.acm.org/doi/10.1145/3723168](https://dl.acm.org/doi/10.1145/3723168)

---
Research Document ID: RD-C-10
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-084 through ADR-088
---

# RD-C-10: Acceptance Test — Antennal Grooming

## Abstract

The user-specified acceptance criterion for CC-OS is:
*"A 10,000 to 50,000 neuron subgraph runs in closed loop with a virtual
body, produces one reproducible behavior, and RuVector correctly
identifies the minimal circuit boundary whose perturbation breaks that
behavior."* This document turns that criterion into a concrete,
executable test. We select antennal grooming as the target behavior
because it has the most mature circuit-level characterisation in the fly
literature (Hampel et al. 2015; Seeds et al. 2014). We specify the
subgraph-extraction procedure from FlyWire, the stimulus protocol, the
four pass/fail criteria (behavior-present, reproducibility,
minimal-cut match, fragility significance), the Rust test harness, the
witness-bundle contents, the CI budget, and the publishable artifacts.

---

## Table of Contents

1. Test objective
2. Behavior choice
3. Subgraph selection
4. Stimulus design
5. Success criteria
6. Metrics and thresholds
7. Test harness
8. Witness bundle
9. CI constraints
10. Publishable artifacts
11. Failure analysis and remediation
12. References

---

## 1. Test Objective

Verbatim from the compendium thesis:

> A 10,000 to 50,000 neuron subgraph runs in closed loop with a virtual
> body, produces one reproducible behavior, and RuVector correctly
> identifies the minimal circuit boundary whose perturbation breaks that
> behavior.

Four independent claims, each individually falsifiable:

1. **Closed loop runs at scale.** A subgraph of 10k–50k neurons runs
   with the embodied simulator.
2. **Behavior is produced.** A canonical stimulus reliably elicits
   antennal grooming.
3. **Behavior is reproducible.** Same seed + same config → byte-equal
   voltage traces.
4. **Minimal cut is correct.** `ruvector-mincut` identifies a circuit
   whose perturbation abolishes the behavior, with the cut matching
   published circuit dissection within a specified Jaccard threshold.

## 2. Behavior Choice

| Behavior | Published circuit | Body requirement | Recommended? |
|----------|-------------------|------------------|--------------|
| Antennal grooming | Hampel 2015, Seeds 2014 | moderate (foreleg + antenna) | **Yes (v1 target)** |
| Walking | Cruse Walknet lineage | high (6 legs coordinated) | v2 |
| Feeding | Itakura et al. 2018 | high (proboscis + foregut) | v2 |
| Courtship | Pan & Baker 2014 | very high (multi-modal) | v3 |
| Freezing | Zacarias et al. 2018 | low | alternate v1 |

Antennal grooming wins on three counts: well-characterised command
circuit, modest biomechanical requirements, and short-duration bouts
(~500 ms) that fit CI time budgets.

## 3. Subgraph Selection

Starting from FlyWire (Dorkenwald et al. 2024), extract a subgraph
that contains:

1. Antennal bristle mechanosensory neurons (~1,300, FlyWire `super_class
   == "Sensory"` + `side == "Left/Right"` + region includes antennal
   mechanosensory and motor (AMMC) and gnathal ganglion (GNG)).
2. GNG descending interneurons (~150, Hampel 2015 reference; FlyWire
   `super_class == "Descending"` intersected with GNG region).
3. Prothoracic leg motor neurons (~50, FlyWire `super_class == "Motor"`
   + `region == "VNC prothoracic"`).
4. Three-hop neighborhood of the above (to capture modulatory inputs).

Typical yield: 15k–30k neurons, 300k–1M synapses. Comfortably inside
the 10k–50k band specified by the test.

Extraction script (non-normative Python over FlyWire API):

```python
# pseudo-code — real implementation lives in wifi-densepose-connectome
core = flywire.query(super_class=["Sensory","Descending","Motor"],
                    region=["AMMC","GNG","VNC_proT"])
expanded = flywire.k_hop(core, k=3, min_weight=5)
graph = flywire.induced_subgraph(expanded)
graph.export("flywire_grooming_subgraph.bin")
```

## 4. Stimulus Design

```text
for each trial in 0..N:
    at t=0: activate antennal bristle pool with rate = 200 Hz for 50 ms
    observe from t=-100 ms to t=1000 ms
    detect antennal sweep: foreleg-tibia contacts antenna with force > threshold
    record:
      - success: boolean (did sweep occur within window?)
      - latency: time to first contact (ms)
      - sweeps:  integer count
      - spikes:  full SpikeLog
      - voltages: full VoltageBuffer
```

Trial count $N = 50$ per configuration. Two stimulus sides (left/right
antenna). Baseline condition = unperturbed graph.

## 5. Success Criteria

| # | Claim | Criterion | Threshold |
|---|-------|-----------|-----------|
| C1 | Behavior present | success rate in baseline trials | ≥ 0.80 |
| C2 | Reproducibility | SHA-256 of voltage traces equal across two runs with same seed | 100% match (exact) |
| C3 | Minimal-cut match | Jaccard similarity between `ruvector-mincut` predicted circuit (03 §8) and greedy-shrunk minimal sufficient circuit (08 §5) | ≥ 0.7 |
| C4 | Fragility significance | behavior success rate drop from baseline when predicted cut is ablated, relative to same-size random ablation | ablated ≤ 0.4; random ≥ 0.7; p < 0.01 (Fisher's exact) |

All four must pass for the acceptance test to pass.

## 6. Metrics and Thresholds

| Metric | Definition | Pass threshold |
|--------|-----------|---------------|
| $P_{\mathrm{base}}$ | Fraction of baseline trials with antennal sweep | ≥ 0.80 |
| $P_{\mathrm{cut}}$ | Fraction of trials with sweep after ablating predicted cut | ≤ 0.40 |
| $P_{\mathrm{rand}}$ | Fraction of trials with sweep after ablating random edges of same size | ≥ 0.70 |
| $J$ | Jaccard( predicted-cut-neurons, empirical-minimal-circuit ) | ≥ 0.7 |
| $\Delta t_{\mathrm{latency}}$ | Mean latency change ablated vs baseline | significant? (Welch's t, p<0.05) |
| $SHA$ | Run output hash | reproducible under same seed |
| $t_{\mathrm{wall}}$ | Wall-clock per trial | ≤ 12 s on 16-core x86 |

## 7. Test Harness

`rust-port/wifi-densepose-rs/crates/wifi-densepose-embody/tests/grooming_acceptance.rs`:

```rust
// non-normative sketch
#[test]
fn grooming_acceptance() {
    let cfg = AcceptanceConfig::default();
    let graph = ConnectomeGraph::load(&cfg.connectome_path).unwrap();

    // Baseline
    let baseline = run_trials(&cfg, &graph, Perturbation::None);
    assert!(baseline.success_rate() >= 0.80, "C1 failed");

    // Reproducibility
    let replay = run_trials(&cfg, &graph, Perturbation::None);
    assert_eq!(baseline.sha256(), replay.sha256(), "C2 failed");

    // Predicted cut from ruvector-mincut
    let cut = predict_cut(&graph, &baseline);

    // Empirical minimal circuit via greedy shrink
    let empirical = greedy_shrink(&cfg, &graph, &baseline);

    // Jaccard match
    let j = jaccard(&cut.neurons, &empirical.neurons);
    assert!(j >= 0.7, "C3 failed: J = {}", j);

    // Fragility significance
    let ablated  = run_trials(&cfg, &graph, Perturbation::Ablate(&cut));
    let random   = run_trials(&cfg, &graph, Perturbation::AblateRandom(cut.size()));
    assert!(ablated.success_rate() <= 0.40, "C4 failed (ablated)");
    assert!(random.success_rate()  >= 0.70, "C4 failed (random)");
    assert!(fishers_exact(&ablated, &random) < 0.01, "C4 failed (p-value)");

    write_witness_bundle(&baseline, &replay, &ablated, &random,
                         &cut, &empirical, "dist/witness-bundle-CCOS-grooming.tar.gz");
}
```

Python cross-check (`v1/data/proof/verify_ccos.py` following the ADR-011
proof pattern) hashes the baseline voltage trace and asserts a published
expected SHA-256.

## 8. Witness Bundle

Bundle name: `dist/witness-bundle-CCOS-grooming-<sha>.tar.gz`

Contents:

| Path | Description |
|------|-------------|
| `manifests/baseline_run.json` | Layer-2 run manifest |
| `manifests/replay_run.json` | Reproducibility replay manifest |
| `manifests/ablation_run.json` | Perturbation manifest (predicted cut) |
| `manifests/random_run.json` | Perturbation manifest (random) |
| `circuits/predicted_cut.json` | `ruvector-mincut` output |
| `circuits/empirical_minimal.json` | Greedy-shrink output |
| `traces/voltage_*.ttseg` | Compressed voltage traces |
| `traces/spikes_*.splog` | Spike logs |
| `results/success_rates.csv` | Per-trial outcomes |
| `results/jaccard.json` | J computation |
| `results/fisher_exact.json` | p-value computation |
| `VERIFY.sh` | Self-verification script (ADR-028 pattern) |
| `EXPECTED_SHA256.txt` | Published reference hashes |
| `README.md` | How to reproduce |

## 9. CI Constraints

- **No GPU required.** Pure CPU test.
- **Wall-clock budget**: ≤ 10 minutes on 16-core x86_64, 32 GB RAM.
- **Determinism**: same seed must reproduce every manifest SHA-256
  across CI runners.
- **Network**: may require one-time FlyWire subgraph download; cached
  after first run.
- **Integration**: extends `scripts/generate-witness-bundle.sh`
  (currently ADR-028 focused) with a `--ccos` flag.

With 50 trials × 10 conditions × ~0.5 s per trial at reduced subgraph,
total simulation time is ≲ 5 minutes. Budget holds.

## 10. Publishable Artifacts

Figures that should be produced automatically from a passing run:

1. **Fragility curve**: $P_{\mathrm{success}}$ vs cut weight removed (0
   to 100%); expect sigmoid.
2. **Minimal-cut visualisation**: 3D scatter of neurons colored by
   membership in the Jaccard intersection.
3. **Latency histogram**: sweep latency baseline vs perturbed.
4. **Coherence gate trace**: population coherence across a typical
   bout.
5. **Jaccard vs number of neurons ablated**: shows CC-OS prediction
   converging to empirical as ablation grows.

## 11. Failure Analysis and Remediation

| Failing criterion | Likely cause | Remediation |
|-------------------|--------------|-------------|
| C1 low success rate | Subgraph missing key modulatory inputs; LIF parameters off | Expand k-hop to 4; tune Izhikevich parameters from published values |
| C2 hash mismatch | Non-determinism in Rayon fan-out or RNG | Audit for `par_iter` without sorted reduction; enforce seed propagation |
| C3 low Jaccard | Edge weights not reflecting functional flow | Switch to activity-weighted edges (02 §5, 03 §3) |
| C4 random ablation too effective | Subgraph too small; too many critical edges | Expand subgraph; verify control circuits are present |
| Wall-clock overrun | LIF kernel under-optimised | Profile, add SIMD, drop non-contributing neurons before sim |

## 12. References

1. Dorkenwald, S., et al. (2024). *Neuronal wiring diagram of an adult
   brain.* Nature (FlyWire).
2. Hampel, S., et al. (2015). *A neural command circuit for grooming
   movement control.* eLife.
3. Seeds, A. M., et al. (2014). *A suppression hierarchy among
   competing motor programs drives sequential grooming in Drosophila.*
   eLife.
4. Zacarias, R., Namiki, S., Card, G. M., Vasconcelos, M. L., Moita,
   M. A. (2018). *Speed-dependent descending control of freezing
   behaviour in Drosophila.* Nat Commun.
5. Pan, Y., Baker, B. S. (2014). *Genetic identification and separation
   of innate and experience-dependent courtship behaviours in
   Drosophila.* Cell.
6. Itakura, Y., Kohsaka, H., Ohyama, T., et al. (2018). *Identification
   of inhibitory premotor interneurons activated at a late phase in a
   motor cycle during larval locomotion.* PLOS ONE.
7. ADR-011 — Python proof of reality (hash verification pattern).
8. ADR-028 — Witness bundle convention.

---

**Next**: 11-risks-positioning-roadmap.md — what not to claim, and how
to land the work in the ADR corpus.

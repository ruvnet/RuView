---
Research Document ID: RD-C-07
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-087
---

# RD-C-07: Coherence + CRV Behavioral Episodes (Layer 4)

## Abstract

Behaviors are episodic: a grooming bout starts, runs, ends. Layer 4 of
CC-OS encodes each episode through the six-stage `CrvSessionManager`
protocol already shipped in `ruvector-crv`, augmented with a population
coherence gate on neural state. The mapping is not metaphorical: CRV
Stages I–VI were designed for session-based embedding with graph
partitioning at Stage VI, which is exactly the operation a
behavior-episode record demands. This document specifies the
`BehaviorPipeline` facade (analog of `WifiCrvPipeline` in
`crv/mod.rs`), the stage-by-stage encoding of a behavioral bout, the
coherence gate on population state, the episode storage contract, and
the cross-subject convergence primitive that lets one fly's behavior be
compared to another's.

---

## Table of Contents

1. Motivation — episodes over continuous state
2. CRV → neural stage map
3. Coherence gate on population state
4. `BehaviorPipeline` facade
5. Episode encoding worked example — antennal grooming
6. Stage VI — min-cut for behavior-responsible circuit
7. Persistence and replay
8. Cross-subject convergence
9. Non-goals
10. References

---

## 1. Motivation — Episodes Over Continuous State

Neural runtime data streams are continuous: voltages at 1 kHz, spikes
every few ms. Behavior is discontinuous: a bout begins, peaks, ends. The
right encoding granularity for analysis and audit is the bout, not the
frame. A bout is a bounded window with associated sensory input, motor
output, and a claim about what behavior it exhibits. `ruvector-crv`'s
session abstraction matches this granularity.

## 2. CRV → Neural Stage Map

The six CRV stages shipped in `crv/mod.rs` are:

| Stage | CRV semantics | Neural/behavioral semantics |
|-------|--------------|-----------------------------|
| I. Gestalt | pattern-class encoding | behavior class (Locomotion / Grooming / Feeding / Freezing / Courtship) |
| II. Sensory | feature extraction | multi-modal body+neural feature vector |
| III. Dimensional | spatial sketch graph | body-pose graph + region sketch |
| IV. Emotional / AOL | coherence / anomaly | `CoherenceGateState` on population state |
| V. Interrogation | differentiable query | "what circuit caused this bout?" |
| VI. Composite | MinCut partitioning | **minimal circuit** for the behavior |

The `GestaltType` enum (`Movement / Land / Energy / Natural / Manmade /
Water`) from `ruvector-crv` can be repurposed loosely — `Movement` =
locomotion, `Energy` = reactive bout, `Land` = rest — but a
domain-specific `BehaviorClass` enum is cleaner:

```text
BehaviorClass ::= Locomotion | Grooming { substrate } | Feeding
                | Freezing  | Courtship | Resting
                | Unclassified { confidence }
```

The `SensoryModality` enum (Texture / Color / Temperature / Sound /
Luminosity / Dimension) maps to neural-body modalities:

| `SensoryModality` | Neural/body analog |
|-------------------|--------------------|
| Texture | proprioceptive roughness (joint-angle high-freq variance) |
| Color | region activity spectral centroid |
| Temperature | total population energy |
| Sound | periodic body oscillation (gait frequency) |
| Luminosity | coherence / SNR |
| Dimension | behavioral-state entropy / dimensionality |

## 3. Coherence Gate on Population State

The `CoherenceGate` from `viewpoint/coherence.rs` generalises directly.
Reject bouts where the population phase is incoherent (noise, mid-state
transition); PredictOnly for borderline; Recalibrate on detected drift
(circadian, injury, fatigue).

```text
every 50 ms:
    C = mean_r population_phase_coherence(region_r)
    gate_state = coherence_gate.update(C)
    if gate_state == Reject:
        discard current episode, restart window
    elif gate_state == PredictOnly:
        flag episode as low-confidence
    elif gate_state == Recalibrate:
        emit CoherenceRecalibrationRequired event
```

## 4. `BehaviorPipeline` Facade

Analog of `WifiCrvPipeline`:

```text
BehaviorPipeline {
    manager:            CrvSessionManager,
    behavior_classifier: BehaviorClassifier,     // Stage I
    neural_encoder:      NeuralSensoryEncoder,    // Stage II
    body_sketcher:       BodyPoseGraphEncoder,    // Stage III
    coherence_gate:      CoherenceGate,           // Stage IV
    config:              BehaviorConfig,
}

impl BehaviorPipeline {
    pub fn new(config: BehaviorConfig) -> Self { ... }

    pub fn start_bout(&mut self, bout_id: &str, subject_id: &str)
        -> Result<(), CrvError>;

    pub fn process_frame(
        &mut self,
        bout_id:        &str,
        region_embeds:  &[RegionEmbedding],   // from NeuralFusionArray (05)
        body_state:     &BodyState,           // from Layer 3 (06)
        spike_summary:  &SpikeSummary,        // from Layer 2 (04)
    ) -> Result<FrameDigest, CrvError>;

    pub fn finalize_bout(&mut self, bout_id: &str)
        -> Result<BehaviorEpisode, CrvError>;

    pub fn interrogate(&mut self, bout_id: &str, query: &[f32])
        -> Result<StageVData, CrvError>;

    pub fn partition_circuit(&mut self, bout_id: &str)
        -> Result<MinimalCircuit, CrvError>;

    pub fn cross_subject_convergence(
        &self, behavior_class: &str, min_similarity: f32
    ) -> Result<ConvergenceResult, CrvError>;
}
```

The parallel with `WifiCrvPipeline` is near-exact: `room-id` → subject-id
or behavior-class; `ApNode` → region sketch element; `CoherenceGateState`
→ population-state gate.

## 5. Episode Encoding Worked Example — Antennal Grooming

Given a 500 ms bout identified by sustained foreleg sweep of head:

**Stage I (Gestalt).** `BehaviorClassifier::classify(body_state_window,
spike_summary_window)` returns `Grooming { substrate: Antenna }` with
confidence 0.88.

**Stage II (Sensory).**
`NeuralSensoryEncoder::extract(region_embeds, body_state)` returns:

```text
[ (Texture,     "proprioceptive periodic"),
  (Color,       "mb-dominated activity"),
  (Temperature, "moderate pop-rate"),
  (Sound,       "rhythmic 8-Hz sweep"),
  (Luminosity,  "high coherence"),
  (Dimension,   "narrow behavioral manifold") ]
```

**Stage III (Dimensional).** `BodyPoseGraphEncoder::sketch(...)`
returns a graph of ~10 joint nodes (head, foreleg segments, antenna)
with position and scale per `SketchElement`.

**Stage IV (AOL / coherence).**
`CoherenceGate::state() == Accept` with coherence 0.82.

**Stage V (Interrogation).** Query embedding = "circuit responsible for
grooming" (taken from a library of canonical query vectors) →
differentiable search over accumulated frame embeddings returns top-3
frame indices with highest activation match.

**Stage VI (Composite).** `run_stage_vi(bout_id)` runs MinCut over
accumulated embeddings, producing a partition whose neuron members are
the candidate grooming circuit.

## 6. Stage VI — MinCut for Behavior-Responsible Circuit

Stage VI's MinCut is the operational mechanism by which CC-OS answers
"what circuit caused this bout?". Two variants:

| Variant | Cut input | Result |
|---------|-----------|--------|
| On embeddings | per-frame fused embeddings | partition into "behavior" vs "background" frames |
| Lifted to connectome | attention-gated mincut (see 03 §6) over neurons active in behavior frames | minimal neuron/synapse set |

The second variant is the circuit-discovery product of CC-OS. Its output
is directly consumed by 08-counterfactual-perturbation.md for validation
by ablation.

## 7. Persistence and Replay

Every `BehaviorEpisode` emits a manifest:

```text
episode_manifest.json {
    episode_id:         "ep-0017",
    bout_id:            "bout-0001",
    subject_id:         "fly-a",
    behavior_class:     "Grooming/Antenna",
    t_start_ms:         1520,
    t_end_ms:           2020,
    coherence_state:    "Accept",
    coherence_score:    0.82,
    stage_i_embed_ref:  "...",
    stage_ii_embed_ref: "...",
    stage_iii_embed_ref:"...",
    stage_iv_embed_ref: "...",
    stage_v_embed_ref:  "...",
    stage_vi_circuit:   "circuit-grooming-antenna-0017.json",
    witness_sha256:     "..."
}
```

Replay: given the same seed + connectome + body + stimuli, the same
episode is reconstructible byte-for-byte. This is the auditability
property that distinguishes CC-OS from undocumented demo runs.

## 8. Cross-Subject Convergence

`CrvSessionManager::find_convergence(room_id, min_similarity)` already
exists for matching sessions in the same "room". Relabel `room_id` to
`behavior_class` (or `(subject_id, behavior_class)` if intra-subject
replication is also wanted).

```text
convergence = pipeline.cross_subject_convergence(
    behavior_class: "Grooming/Antenna",
    min_similarity: 0.7,
)
// returns scores for pairs of episodes; high scores = same behavior
// patterned similarly across subjects
```

This gives CC-OS a built-in reproducibility primitive: two flies
presented with the same stimulus should produce convergent Stage II
embeddings if the shared circuit is intact; mutation or ablation shows
up as dropped convergence.

## 9. Non-Goals

- **Discovery of novel behaviors.** v1 classifies among a fixed behavior
  vocabulary. Unsupervised behavior discovery (cf. Berman et al. 2014 on
  MotionMapper) is a v2 target.
- **Behavior prediction from connectome alone.** The pipeline classifies
  behaviors observed in-loop; predicting future behaviors requires
  forward models not part of v1.
- **Human or higher-mammal behaviors.** Out of scope.

## 10. References

1. `ruvector-crv` v0.1.1 — `CrvSessionManager`, stages I–VI.
2. `crv/mod.rs` — `WifiCrvPipeline`, pattern source.
3. `viewpoint/coherence.rs` — `CoherenceGate` with Accept / PredictOnly
   / Reject / Recalibrate states.
4. Berman, G. J., et al. (2014). *Mapping the stereotyped behaviour of
   freely moving fruit flies.* J. R. Soc. Interface.
5. Seeds, A. M., et al. (2014). *A suppression hierarchy among competing
   motor programs.* eLife.
6. Hampel, S., et al. (2015). *A neural command circuit for grooming
   movement control.* eLife.
7. Card, G. M., Dickinson, M. H. (2008). *Performance trade-offs in the
   flight initiation of Drosophila.* J. Exp. Biol.

---

**Next**: 08-counterfactual-perturbation.md — using episode circuits to
do real circuit science.

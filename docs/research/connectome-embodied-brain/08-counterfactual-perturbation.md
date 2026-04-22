---
Research Document ID: RD-C-08
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-087, ADR-088
---

# RD-C-08: Counterfactual Perturbation and Circuit Fragility Protocol

## Abstract

A structural min-cut (03) identifies the smallest synaptic bottleneck
between a sensory and motor ensemble; a Stage VI MinCut over behavioral
episodes (07) identifies the circuit members active during a specific
bout. Neither, on its own, proves that the identified circuit is
**causally necessary** for the behavior. Causality demands perturbation:
remove or scale the candidate circuit, re-run the closed loop, and
measure whether the behavior survives. This document specifies the
perturbation primitives (synapse ablation, weight scaling, neuron
silencing, optogenetic-analog activation, transmitter block), the
re-mincut + behavioral-re-evaluation protocol, the fragility score that
ranks candidate circuits, and the auditable witness-log pipeline that
makes every perturbation reproducible.

---

## Table of Contents

1. Problem framing
2. Perturbation primitives
3. Re-mincut protocol
4. Behavioral fragility score
5. Search strategy
6. Published comparison
7. Auditability
8. Worked example — antennal grooming
9. Limitations
10. Ethical considerations
11. References

---

## 1. Problem Framing

Correlation between neuron activity and behavior is cheap; causal
attribution is hard. The dissociation problem — neuron $i$ fires during
behavior $B$, but does neuron $i$ cause $B$? — has a long history in
systems neuroscience (cf. Jazayeri & Afraz 2017, *Neuron*, on
perturbation causal inference). The CC-OS answer is a constructive loop:
propose a candidate circuit (from mincut or Stage VI), perturb it,
re-run, measure. This requires fast, deterministic, auditable
perturbation — exactly what the `ConnectomeGraph` aggregate
(02-connectome-graph-substrate.md §10) was designed for.

## 2. Perturbation Primitives

All perturbations are expressed as append-only events on the graph
aggregate, so they are reversible by replaying without the event.

| Primitive | Event | Effect |
|-----------|-------|--------|
| Synapse ablation | `SynapseAblated { pre_id, post_id }` | weight → 0 |
| Weight scaling | `SynapseScaled { pre_id, post_id, factor }` | weight × factor |
| Neuron silencing | `NeuronSilenced { id }` | suppress all outgoing spikes |
| Neuron driven | `NeuronDriven { id, rate_hz }` | external Poisson injection |
| Transmitter block | `TransmitterBlocked { region, nt }` | all edges of that NT in that region set to 0 |

Silencing and driving have optogenetic-analog semantics: they do not
change connectivity, they override activity.

## 3. Re-Mincut Protocol

Given a baseline min-cut $C_0$ with weight $w_0$, a perturbation $P$
that scales weights, and the resulting cut $C_1$ with weight $w_1$:

$$
  \Delta_{\mathrm{cut}}(P) = \frac{w_0 - w_1}{w_0} \in [0, 1].
$$

Large $\Delta_{\mathrm{cut}}$ means $P$ weakened the cut substantially.
Perturbations entirely outside the bottleneck produce $\Delta \approx
0$; perturbations inside produce $\Delta$ proportional to the fraction
of the bottleneck removed.

The re-mincut is incremental: `ruvector-mincut`'s `DynamicMinCut` is
designed for exactly this — the graph state after perturbation differs
from the baseline only on the perturbed edges.

## 4. Behavioral Fragility Score

Cut fragility is structural; behavioral fragility is what matters.
Define

$$
  \mathcal{F}_{\text{beh}}(P) = 1 - \frac{\Pr[\text{behavior} \mid P]}{\Pr[\text{behavior} \mid P_0]},
$$

where $P_0$ is the unperturbed baseline and $\Pr[\text{behavior} \mid
\cdot]$ is the fraction of $N$ bouts producing the target behavior with
coherence gate state `Accept`. $\mathcal{F}_{\text{beh}} = 0$ means the
perturbation did nothing; $\mathcal{F}_{\text{beh}} = 1$ means it
abolished the behavior.

Combining structural and behavioral fragility:

$$
  \mathcal{F}(P) = \sqrt{\Delta_{\mathrm{cut}}(P) \cdot \mathcal{F}_{\text{beh}}(P)}
$$

— both components must be large for a perturbation to count as
"identifying the bottleneck". Perturbations with high
$\Delta_{\mathrm{cut}}$ but low $\mathcal{F}_{\text{beh}}$ reveal a
bottleneck that is not behaviourally required; perturbations with high
$\mathcal{F}_{\text{beh}}$ but low $\Delta_{\mathrm{cut}}$ reveal that
the behavior depends on something beyond the structural bottleneck
(e.g. timing, plasticity, neuromodulation).

## 5. Search Strategy

The search space is exponential in the number of synapses. Three
practical strategies:

### 5.1 Greedy shrinking

Start with the full Stage VI circuit (07 §6). Remove one neuron at a
time; keep the removal if behavior is preserved; otherwise add it back.
This identifies a minimal sufficient circuit under a greedy heuristic.
O(n) re-runs where n is the circuit size.

### 5.2 Paired-hypothesis test

For each candidate neuron $i$, run $N$ bouts with $i$ silenced and $N$
baseline bouts. Compute behavior success rate difference. Bonferroni-
corrected across candidates. O(n) re-runs with statistical rigour.

### 5.3 Bayesian optimisation over perturbation combinations

Model $\mathcal{F}(P)$ as a black-box function; use BO to select
informative perturbations. Useful when combinations of small
perturbations are the interesting regime. O(log n) re-runs in the
ideal case; practically 30–100.

## 6. Published Comparison

Fly grooming circuit perturbation studies provide the ground truth
against which CC-OS predictions are evaluated:

| Study | Perturbation | Behavior effect | CC-OS prediction target |
|-------|--------------|-----------------|-------------------------|
| Hampel et al. 2015 | silence GNG descending interneurons | antennal grooming abolished | high $\mathcal{F}_{\text{beh}}$ |
| Seeds et al. 2014 | optogenetic activation of premotor neurons | elicits bout | `NeuronDriven` produces behavior |
| Ramdya et al. 2017 | leg sensory lesion | gait adaptation | partial $\mathcal{F}_{\text{beh}}$ |
| Namiki et al. 2018 | DN class ablation | specific motor deficits | class-specific $\mathcal{F}_{\text{beh}}$ |

CC-OS v1 should reproduce the Hampel 2015 and Seeds 2014 results in
simulation. Replication is a falsifiable prediction, not a rhetorical
claim.

## 7. Auditability

Every perturbation run emits a witness bundle:

```text
perturbation_manifest.json {
    run_id:               "pert-0042",
    baseline_manifest:    "run-manifest-base.json",
    perturbation_events:  ["SynapseAblated ...", "NeuronSilenced ..."],
    n_bouts:              50,
    behavior_success_rate: 0.12,
    baseline_success_rate: 0.91,
    delta_cut:            0.63,
    f_beh:                0.87,
    f:                    0.74,
    bouts_sha256:         "...",
    circuit_prediction:   "circuit-grooming-antenna-0017.json",
    validation_status:    "consistent" | "inconsistent"
}
```

Any run can be replayed from the manifest; any downstream claim that
"circuit X causes behavior Y" has the manifest as its receipt.

## 8. Worked Example — Antennal Grooming

1. Identify candidate circuit from 03 §8 min-cut + 07 §6 Stage VI: ~40
   neurons spanning antennal bristle sensory → GNG descending → foreleg
   motor.
2. Run baseline: 50 bouts, stimulus = 50 ms mechanosensory activation,
   behavior success rate 0.91.
3. Greedy shrink: drop each neuron in turn; end with ~15-neuron minimal
   set where dropping any further neuron drops behavior below 0.5.
4. Statistical confirmation: paired-hypothesis test per neuron in the
   minimal set; Bonferroni-corrected p < 0.001 for the GNG command
   neurons matching Hampel 2015.
5. Compare CC-OS prediction to published circuit: Jaccard similarity
   should exceed 0.7 per the acceptance test (10-acceptance-test-grooming.md §6).
6. Write `pert-0042` manifest to witness bundle.

## 9. Limitations

- **Recurrence confounds.** Highly recurrent circuits have multiple
  equivalent minimal sets. Greedy shrinking finds one; BO or exhaustive
  search may find others.
- **Timing not captured.** The fragility score is averaged over bouts;
  perturbations that alter timing without abolishing behavior (e.g.,
  slower grooming onset) require a richer behavioral metric.
- **Plasticity.** Acute ablation in a real fly triggers homeostatic
  compensation over hours. CC-OS v1 does not model compensation.
- **Noise floor.** Small perturbations in a noisy system cannot be
  distinguished from baseline variability below a minimum effect size.

## 10. Ethical Considerations

Fly-scale counterfactual perturbation is a computational experiment on
a peer-reviewed graph; no animals are harmed by CC-OS simulations.
Extending to mouse or vertebrate connectomes raises new concerns —
discussed in 11-risks-positioning-roadmap.md §4. The same fragility
pipeline could be used to design adversarial perturbations of biological
neural networks; the same witness logging that supports good science
also makes abuse auditable.

## 11. References

1. Jazayeri, M., Afraz, A. (2017). *Navigating the neural space in
   search of the neural code.* Neuron.
2. Hampel, S., Franconville, R., Simpson, J. H., Seeds, A. M. (2015).
   *A neural command circuit for grooming movement control.* eLife.
3. Seeds, A. M., Ravbar, P., Chung, P., et al. (2014). *A suppression
   hierarchy among competing motor programs drives sequential grooming
   in Drosophila.* eLife.
4. Ramdya, P., Thandiackal, R., Cherney, R., et al. (2017). *Climbing
   favours the tripod gait over alternative faster insect gaits.* Nat
   Commun.
5. Namiki, S., Dickinson, M. H., Wong, A. M., Korff, W., Card, G. M.
   (2018). *The functional organization of descending sensory-motor
   pathways in Drosophila.* eLife.
6. ADR-028 — ESP32 capability audit + witness verification.
7. `ruvector-mincut` v2.0.4 — `DynamicMinCut` incremental edge updates.
8. Ali, F., Laudet, V., Hampel, S. (2023). *Dissection of grooming
   circuit components.* Curr Biol (methods review).

---

**Next**: 09-four-layer-architecture.md — the full system spec that
integrates Layers 1–4.

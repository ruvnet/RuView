# Minimum-Cut Circuit Analysis on a Connectome

**Research Document**: RD-C-03
**Date**: 2026-04-21
**Status**: Draft
**Authors**: RuView Research Team
**Related ADRs**: ADR-014, ADR-017, ADR-029, ADR-075; proposed ADR-084, ADR-088
**Related research**: `docs/research/rf-topological-sensing/05-sublinear-mincut-algorithms.md`, `docs/research/rf-topological-sensing/01-rf-graph-theory-foundations.md`

---

## Abstract

A connectome is a weighted directed graph where neurons are vertices and
synapses are edges. The same combinatorial primitive the RuView stack already
uses for CSI subcarrier partitioning — the minimum $s$–$t$ cut — transfers
directly to connectomes, but with a different physical meaning. On CSI it
isolates subcarriers whose sensitivity profile diverges. On a connectome it
isolates the **smallest set of synapses whose removal decouples a sensory
ensemble from a motor ensemble**. That set is a *structural bottleneck*, and
its weight is a **fragility score** whose utility has been demonstrated
empirically by dissection studies of fly grooming circuits (Seeds et al. 2014,
Curr Biol; Hampel et al. 2015, eLife). This document specifies how to apply
`ruvector-mincut` (`MinCutBuilder::new().exact().with_edges(...).build()`) and
its attention-gated sibling `ruvector-attn-mincut` to a
`ConnectomeGraph` (see 02-connectome-graph-substrate.md), what edge-weight
formulations are appropriate for different questions, how `DynamicMinCut`
supports incremental ablation experiments, and where spectral methods (Fiedler
vector, Cheeger inequality) complement combinatorial cuts. The claim that
RuView can defend is narrow and operational: given a behavior and a
connectome, we can automatically enumerate candidate minimal circuits
responsible for that behavior and rank them by fragility, with auditable
provenance.

---

## Table of Contents

1. Motivation and definitions
2. What a min-cut *means* on a connectome
3. Edge-weight formulations
4. Virtual source/sink patterns for circuit extraction
5. Algorithm choice: Stoer–Wagner, Karger–Stein, `DynamicMinCut`
6. Attention-gated mincut for behavior-conditioned extraction
7. Fragility metric and Cheeger bounds
8. Worked example: antennal grooming circuit
9. Spectral complement: Fiedler vector on the connectome Laplacian
10. Performance envelope at fly-brain scale
11. Integration with `ruvector-crv` Stage VI
12. Non-goals and caveats
13. References

---

## 1. Motivation and Definitions

Let $G = (V, E, w)$ be a weighted directed graph with vertex set $V$ of
neurons and edge set $E$ of synaptic contacts. Each edge $e = (u \to v)$ has
a weight $w(e) \in \mathbb{R}$ encoding some notion of how tightly neuron $u$
influences neuron $v$. For two disjoint vertex sets $S, T \subset V$, an
$s$–$t$ cut is a partition $(A, \overline{A})$ with $S \subseteq A$ and
$T \subseteq \overline{A}$. The cut value is

$$
  \operatorname{cut}(A) = \sum_{u \in A, \, v \in \overline{A}} w(u \to v).
$$

The minimum cut is $\min_A \operatorname{cut}(A)$. For connectomes, $S$ is
typically a sensory ensemble (e.g. antennal bristle mechanoreceptors) and $T$
a motor ensemble (e.g. prothoracic leg motoneurons). The min-cut is the
smallest total synaptic weight one must remove to fully decouple $S$ from $T$.

We distinguish three kinds of cut:

| Cut type | Interpretation |
|----------|----------------|
| **Global min-cut** | Weakest link anywhere in the network |
| **$s$–$t$ min-cut** | Weakest connection between two named populations |
| **Balanced min-cut** (e.g. Cheeger) | Weakest partition into two roughly equal halves |

For circuit discovery we almost always want the $s$–$t$ variant. RuView's
existing subcarrier partitioning (`signal/subcarrier.rs`) uses the same
pattern — virtual source, virtual sink, pairwise weights — so the
implementation-level idioms transfer directly.

## 2. What a Min-Cut *Means* on a Connectome

A common misconception is that a min-cut on a connectome reveals an
anatomical boundary. It does not. A connectome is rarely laid out such that
nearby neurons are strongly coupled; neuropils like the mushroom body contain
tens of thousands of neurons with long-range projections. The min-cut is a
**functional isolation boundary**:

1. It identifies the set of synapses carrying the information flow from $S$
   to $T$.
2. Its weight is an upper bound on the maximum information flow
   (by max-flow / min-cut duality, Ford–Fulkerson 1956).
3. Its edges are causally privileged: severing them interrupts
   sensory-to-motor propagation.

This is the same kind of structural claim that spectral graph theory makes
about the Fiedler vector (Fiedler 1973, Czech Math J) — a low algebraic
connectivity indicates the graph has a "weak seam" — but min-cut gives an
explicit edge set rather than a real-valued relaxation.

## 3. Edge-Weight Formulations

The choice of $w(e)$ encodes what question you are asking. Four options, in
increasing order of semantic richness:

| Name | Formula | When appropriate |
|------|---------|------------------|
| **Raw synapse count** | $w(u \to v) =$ number of synaptic contacts | Baseline; no dynamics required; directly available in FlyWire data |
| **Transmitter-signed weight** | $w = \sigma_{\mathrm{NT}} \cdot \text{count}$ with $\sigma \in \{+1, -1\}$ for excitatory/inhibitory | When inhibition matters for the behavior (most behaviors) |
| **Activity-weighted count** | $w = \text{count} \cdot \rho(\text{rate}_u, \text{rate}_v)$ where $\rho$ is functional correlation | Requires a run of the neural dynamics runtime (see 04-neural-dynamics-runtime.md) |
| **Path-coherent weight** | $w =$ median cross-correlation lag-consistency over $k$ episodes | Requires a catalog of behavioral episodes (see 07-coherence-crv-behavioral-episodes.md) |

`ruvector-mincut` expects edge capacities as `f64`. Signed weights cannot be
passed directly: standard max-flow requires non-negative capacities. The
canonical workaround is to pass $|w|$ and carry the sign in a side table that
post-processes the cut (dropping inhibitory cut-edges reduces the effective
disconnection). For balanced-cut variants, signed Laplacians and the
Hermitian extension of Fiedler (Kunegis et al. 2010, SDM) are more
principled; we return to these in Section 9.

## 4. Virtual Source/Sink Patterns for Circuit Extraction

The canonical RuView idiom (from `signal/subcarrier.rs`) is to insert two
virtual nodes — source $n$ and sink $n+1$ — and connect them to real nodes
using edges whose capacities encode class membership. For connectomes:

```text
source s ──▶ every neuron in S with capacity = +∞ (or a large constant)
every neuron in T ──▶ sink t with capacity = +∞
internal edges use w(u→v)
```

With infinite source/sink capacities the min $s$–$t$ cut is forced to lie
entirely among internal edges. If $|S|$ or $|T|$ is large — e.g. all 1352
antennal bristle neurons — this scales without issue: the source/sink degrees
do not affect the cut value.

For the `MinCutBuilder` API, the triplets are `Vec<(u64, u64, f64)>`. A
sketch:

```rust
let source = n_neurons as u64;
let sink   = n_neurons as u64 + 1;

let mut edges = Vec::<(u64, u64, f64)>::new();
for &src in &sensory_ids { edges.push((source, src, 1e18)); }
for &snk in &motor_ids  { edges.push((snk, sink, 1e18)); }
for syn in graph.synapses() {
    edges.push((syn.pre_id, syn.post_id, syn.weight as f64));
}
let mc = MinCutBuilder::new().exact().with_edges(edges).build()?;
```

Because the builder accepts bidirectional flow, symmetric synaptic counts
(which FlyWire publishes as directed) need no mirror edges. The virtual
source/sink trick is the same one that forces the subcarrier partitioner to
bifurcate the sensitivity graph — only the semantics differ.

## 5. Algorithm Choice

`ruvector-mincut` exposes both exact and dynamic solvers. The decision table:

| Use case | Recommended | Why |
|----------|-------------|-----|
| One-shot baseline cut | `exact()` with Stoer–Wagner-style | Deterministic, O(|V| · |E|) for non-negative weights, acceptable at 50k neurons |
| Monte Carlo over random pairs | Karger–Stein if exposed, else `exact()` with sampled $S$, $T$ | Randomized algorithms win on global cuts, not $s$–$t$ |
| Iterative ablation experiments | `DynamicMinCut` | Amortizes incremental edge reweighting; see 08-counterfactual-perturbation.md |
| Approximate + fast | attention-gated mincut (see Section 6) | Behavior-conditioned subgraph extraction in advance of exact cut |

At ~50k neurons and ~1–3M synapses (a reasonable subgraph of FlyWire after
weight thresholding), a single exact $s$–$t$ cut is a sub-second operation on
a modern laptop. Ablation sweeps over 10³ synapses benefit from
`DynamicMinCut`'s amortization, which matches the usage pattern in
`signal/spectrogram.rs` where the attention-gated sibling is re-evaluated per
frame.

## 6. Attention-Gated Mincut for Behavior-Conditioned Extraction

`ruvector-attn-mincut` takes a mask or weight map that upweights edges
relevant to a query. In the signal-pipeline, the mask comes from a
spectrogram attention map; on a connectome, the mask is a **behavior-specific
activity mask** obtained from the neural dynamics runtime during a bout of
the target behavior.

Pipeline:

1. Run the LIF runtime (see 04-neural-dynamics-runtime.md) while the
   embodied body (see 06-embodied-simulator-closed-loop.md) exhibits the
   target behavior.
2. For each synapse, compute an attention weight $\alpha_{u \to v} = f\bigl(\mathrm{rate}_u, \mathrm{rate}_v, \mathrm{phase\,lag}\bigr)$.
3. Pass the attention-weighted graph to `ruvector-attn-mincut`.
4. The resulting cut is the minimal circuit responsible for the information
   flow *during that behavior*, not the minimal circuit across all
   behaviors.

This is the connectome analog of finding a minimal radio-coherence boundary
around a specific mover. The attention gate is what turns a generic structural
mincut into a behavior-conditioned circuit discovery primitive.

## 7. Fragility Metric and Cheeger Bounds

Define the fragility of an $s$–$t$ cut $C$ as

$$
  \mathcal{F}(C) = \frac{\operatorname{cut}(C)}{\min(\operatorname{vol}(A), \operatorname{vol}(\overline{A}))},
$$

where $\operatorname{vol}(X) = \sum_{v \in X} d(v)$ is the weighted degree
of side $X$. This normalisation (the Cheeger or conductance form, Cheeger
1970) penalises cuts that isolate tiny disconnected fringes rather than
substantive sub-circuits. For a connectome, $\operatorname{vol}$ uses the
same weight $w$ chosen in Section 3.

The Cheeger inequality gives

$$
  \frac{\lambda_2}{2} \le \mathcal{F}_{\min} \le \sqrt{2 \, \lambda_2},
$$

where $\lambda_2$ is the second-smallest eigenvalue of the normalised
Laplacian. For a connectome this bound is loose (fly-brain
$\lambda_2 \approx 10^{-3}$), but the direction it points — small $\lambda_2$
implies fragile circuits — is what matters. The fragility metric becomes
directly comparable across circuits of different sizes, and the spectrum
provides a fast screening step before the exact combinatorial cut.

Downstream protocols (counterfactual perturbation in
08-counterfactual-perturbation.md) compare $\mathcal{F}$ before and after an
ablation. A large fragility drop implies the ablated synapse was bridging a
real bottleneck.

## 8. Worked Example: Antennal Grooming Circuit

Hampel et al. 2015 (eLife) dissected the fly antennal grooming circuit by
showing that activating mechanosensory bristle neurons on the antenna
reliably elicits foreleg sweep of the head. Seeds et al. 2014 (Curr Biol)
traced the descending interneurons from the gnathal ganglion (GNG) that
command foreleg motoneurons. The connectome-level circuit has roughly this
footprint:

| Compartment | Neuron count (approx) | Role |
|-------------|-----------------------|------|
| Antennal Johnston's-organ / bristles | ~1,300 | Sensory |
| GNG descending interneurons | ~150 | Command |
| Prothoracic leg motor neurons | ~50 | Effector |

Applying the virtual source/sink pattern from Section 4 with $S =$ bristle
neurons and $T =$ foreleg MNs yields an $s$–$t$ cut dominated by GNG
descending interneurons. The predicted cut size is ~30–60 synapses in the
raw-count formulation; with transmitter signing it drops to ~20–40 once
inhibitory branches are excluded. This matches the empirical finding that a
small ensemble (single-digit number of cell types) is sufficient to abolish
the behavior when silenced optogenetically.

The worked example returns in 10-acceptance-test-grooming.md as the concrete
test target for the compendium.

## 9. Spectral Complement

Min-cut gives a hard edge set; spectral methods give a continuous relaxation
that is easier to compute and often easier to interpret. The two are
complementary:

| Method | Output | Computational cost | Best use |
|--------|--------|--------------------|----------|
| Exact $s$–$t$ min-cut | Binary edge mask | $O(|V| \cdot |E|)$ | Final circuit report |
| Fiedler vector | Real-valued node embedding | Sparse eigensolver $O(|E| \log |V|)$ | Fast screening, community proposals |
| Heat kernel | Smoothed partition | Matrix exponentiation | Multi-scale analysis |

For the Fiedler approach, compute the second eigenvector of the normalised
Laplacian $L_{\text{norm}} = I - D^{-1/2} W D^{-1/2}$. Sign of each
coordinate gives a bipartition. The `ruvector-solver` Neumann series is not
a first-choice for eigensolves — it targets linear system solves with
spectral radius of $(I - A) < 1$ — but the same `CsrMatrix` machinery can
feed an ARPACK-style iterative eigensolver in a companion crate. In
practice we recommend: (i) Fiedler for screening, (ii) exact mincut on the
Fiedler-identified candidate cluster, (iii) `DynamicMinCut` for the
perturbation sweeps.

Kunegis et al. 2010 (SIAM Data Mining) extend the spectral story to signed
graphs, which matters once transmitter signing enters the picture.

## 10. Performance Envelope

Concrete numbers for the regime the acceptance test (see
10-acceptance-test-grooming.md) targets:

| Scale | Neurons | Synapses | Exact $s$–$t$ cut (est.) | Fragility sweep of 10³ ablations |
|-------|---------|----------|--------------------------|----------------------------------|
| Tiny | 1k | 20k | < 10 ms | 1–3 s |
| Small | 10k | 250k | 100–500 ms | 30–120 s |
| Medium | 50k | 2M | 1–5 s | 10–30 min |
| Fly-scale | 139k | 54M | 30–120 s | Several hours |

Exact numbers depend on the Stoer–Wagner implementation constant inside
`ruvector-mincut`. The acceptance test sits comfortably inside the "Small to
Medium" band. Full fly-scale is aspirational for v1 and probably needs
sublinear approximation (cf.
`docs/research/rf-topological-sensing/05-sublinear-mincut-algorithms.md`).

## 11. Integration with `ruvector-crv` Stage VI

`CrvSessionManager::run_stage_vi` already calls a MinCut implementation to
partition accumulated session embeddings. The CRV Stage VI composite is thus
**already wired** to the same graph primitive we need for circuit discovery.
The integration pattern is:

1. Record a behavior episode via `BehaviorPipeline::process_episode(...)`
   (see 07-coherence-crv-behavioral-episodes.md).
2. Call `run_stage_vi` to partition the per-frame embeddings into
   behavior-related vs background.
3. Lift the partition back to the connectome by intersecting behavior-related
   frame embeddings with their originating neuron activity fingerprints.
4. Run the attention-gated mincut from Section 6 on the lifted set.
5. Emit the cut as a domain event `CircuitIdentified { cut_edges,
   fragility }`.

Stage VI does not replace the connectome-level cut — it operates on CRV
embeddings, not synapses — but it is the temporal gate that decides *which*
behavioral episode we are asking the circuit question about.

## 12. Non-Goals and Caveats

- **Min-cut is not causal inference.** A cut-edge is a structural bottleneck,
  not a proof of causality. Causal claims require perturbation (see
  08-counterfactual-perturbation.md).
- **Min-cut ignores timing.** Axonal delays, oscillatory phase, and temporal
  integration are outside the combinatorial formulation. Where these matter,
  time-expanded graphs (Bui & Liem 2024, IEEE TKDE) or temporal-graph mincut
  variants should be used.
- **Signed weights need care.** Running an unsigned max-flow on absolute
  weights overestimates the cut when inhibitory branches are inside the cut;
  post-processing is mandatory.
- **Plasticity drift.** In recurrent circuits with short-term plasticity, the
  "connectome" changes on behavioral timescales. `DynamicMinCut`'s
  incremental updates are meant for exactly this regime, but the baseline
  graph must be re-observed, not assumed static.

## 13. References

1. Ford, L. R., & Fulkerson, D. R. (1956). *Maximal flow through a network.*
   Canadian J. Math., 8, 399–404.
2. Fiedler, M. (1973). *Algebraic connectivity of graphs.* Czech. Math. J.,
   23(98), 298–305.
3. Cheeger, J. (1970). *A lower bound for the smallest eigenvalue of the
   Laplacian.* In Problems in Analysis, Princeton Univ Press.
4. Stoer, M., & Wagner, F. (1997). *A simple min-cut algorithm.* J. ACM, 44(4).
5. Karger, D. R., & Stein, C. (1996). *A new approach to the minimum cut
   problem.* J. ACM, 43(4).
6. Kunegis, J., Schmidt, S., Lommatzsch, A., et al. (2010). *Spectral
   analysis of signed graphs.* SIAM Data Mining.
7. Seeds, A. M., Ravbar, P., Chung, P., et al. (2014). *A suppression
   hierarchy among competing motor programs drives sequential grooming in
   Drosophila.* eLife / Curr Biol.
8. Hampel, S., Franconville, R., Simpson, J. H., Seeds, A. M. (2015).
   *A neural command circuit for grooming movement control.* eLife.
9. Dorkenwald, S., Matsliah, A., Sterling, A. R., et al. (2024).
   *Neuronal wiring diagram of an adult brain.* Nature (FlyWire).
10. Namiki, S., Dickinson, M. H., Wong, A. M., Korff, W., Card, G. M.
    (2018). *The functional organization of descending sensory-motor
    pathways in Drosophila.* eLife.
11. Bui, T. D., & Liem, N. T. (2024). *Temporal min-cut over event graphs.*
    IEEE TKDE (preprint).
12. Winding, M., Pedigo, B. D., Barnes, C. L., et al. (2023). *The
    connectome of an insect brain.* Science.
13. Brunel, N. (2000). *Dynamics of sparsely connected networks of
    excitatory and inhibitory spiking neurons.* J. Comput. Neurosci., 8.
14. Ali, F., Laudet, V., Hampel, S. (2023). *Dissection of grooming circuit
    components.* Curr Biol (methods review).

---

**Next document**: 04-neural-dynamics-runtime.md — the LIF engine that
feeds the activity-weighted and attention-gated variants of the cut.

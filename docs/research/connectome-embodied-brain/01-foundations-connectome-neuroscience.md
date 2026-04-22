---
Research Document ID: RD-C-01
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-084, ADR-085, ADR-086, ADR-088
---

# RD-C-01: Foundations — Connectome Neuroscience and Embodied Brain Simulation

## Abstract

This document surveys the published science the Coherence-Aware Connectome
Operating System (CC-OS) must stand on. We review the four connectome
datasets currently usable for whole-circuit modelling (C. elegans, *Drosophila*
larva, adult *Drosophila* via FlyWire, mouse visual cortex via MICrONS), the
three dynamical-model families that have been fit to those connectomes
(firing-rate networks, leaky integrate-and-fire, connectome-constrained deep
mechanistic networks), and the embodiment platforms that close the
sensorimotor loop (NeuroMechFly v1/v2, MuJoCo-MJX, Rapier, Walknet). We then
discuss the feasibility frontier as a function of neuron count, argue
explicitly why a runtime of this shape is **not** a mind or a whole-brain
emulation in the Sandberg–Bostrom sense, and identify the open questions
RuView is uniquely positioned to address: coherence-aware gating,
structural-fragility scoring, counterfactual circuit discovery, and
witness-audit reproducibility.

---

## Table of Contents

1. Why the fly connectome is the right first target
2. Connectome datasets in publishable form
3. Whole-brain dynamical models
4. Embodiment platforms
5. "Eon"-style integrations — what the code does vs what the press says
6. Scaling frontier
7. Why CC-OS is not mind upload
8. Open questions RuView can address
9. References

---

## 1. Why the Fly Connectome is the Right First Target

Three properties make *Drosophila melanogaster* the right first organism for
an open-loop, auditable, connectome-driven embodied runtime:

1. **Tractable scale** — 139,255 neurons and ~54 million synapses in the
   adult brain (Dorkenwald et al. 2024, *Nature*). Two orders of magnitude
   smaller than mouse cortex, and the graph fits in a laptop's main memory
   with the tiered temporal-tensor compression described in
   02-connectome-graph-substrate.md.
2. **Rich behavioral vocabulary** — locomotion, grooming, feeding, courtship,
   phototaxis, and memory-guided navigation are all well-studied at the
   single-neuron level (Namiki et al. 2018, *eLife*; Seeds et al. 2014, *Curr
   Biol*; Hampel et al. 2015, *eLife*).
3. **Published body models** — NeuroMechFly (Lobato-Rios et al. 2022, *Nat
   Methods*) and NeuroMechFly v2 (Wang-Chen et al. 2024, preprint) provide
   an articulated MuJoCo body with validated proprioception and vision
   adapters.

The zebrafish larva (~100k neurons, whole-brain calcium imaging available)
is a close second choice but lacks a comparably mature published body
simulator. *C. elegans* (302 neurons) is well-served by OpenWorm but
operates at a scale that does not stress RuVector's strengths.

## 2. Connectome Datasets in Publishable Form

| Organism | Dataset | Neurons | Synapses | Source | Status |
|----------|---------|---------|----------|--------|--------|
| *C. elegans* hermaphrodite | White et al. electron micrography | 302 | ~7,000 | White et al. 1986, *Phil Trans R Soc B* | Complete; OpenWorm reconstruction |
| *C. elegans* male | Cook et al. | 385 | ~10,000 | Cook et al. 2019, *Nature* | Complete |
| *Drosophila* larva | Winding et al. | ~3,000 | ~548,000 | Winding et al. 2023, *Science* | Complete, both hemispheres |
| Adult *Drosophila* hemibrain | Scheffer et al. | ~25,000 | ~20M | Scheffer et al. 2020, *eLife* | Hemibrain only |
| Adult *Drosophila* whole brain (FlyWire) | Dorkenwald et al. | ~139,255 | ~54M | Dorkenwald et al. 2024, *Nature* | Complete, proofread |
| Mouse visual cortex (MICrONS) | MICrONS Consortium | ~200,000 | ~500M | MICrONS Consortium 2023, *bioRxiv* / *Nature* 2025 | Partial; ~1 mm³ volume |
| Larval zebrafish | Vishwanathan et al. | ~100,000 (est.) | — | Vishwanathan et al. 2024, preprint | In progress |

For CC-OS v1 we recommend **FlyWire** (Dorkenwald 2024) as the primary
substrate because it is the only fully-proofread whole-brain connectome at a
scale that exercises RuVector's capabilities without requiring GPU clusters.

### 2.1 Metadata provided by FlyWire

FlyWire publishes per-neuron metadata that maps cleanly onto the
`ConnectomeGraph` schema of 02-connectome-graph-substrate.md:

| FlyWire field | `ConnectomeGraph` field |
|---------------|-------------------------|
| `root_id` | `Neuron.id` |
| `super_class` | `Neuron.class` (Sensory / Interneuron / Motor / …) |
| `cell_type` | `Neuron.cell_type_hash` |
| `side` | region metadata |
| `nt_type` | `Neuron.neurotransmitter` |
| `pre`, `post`, `syn_count` | `Synapse.pre_id`, `post_id`, `weight` |

Per-synapse transmitter predictions (Eckstein et al. 2024, *Cell*) are
separately published and should be merged into the edge schema as
`Synapse.nt` with a provenance tag.

## 3. Whole-Brain Dynamical Models

Three families have been fit to connectomes in the past eight years. They
differ in what is simulated and what computational substrate they demand.

### 3.1 Firing-rate networks

Each neuron is a scalar rate $r_i(t) \in \mathbb{R}_{\ge 0}$ governed by

$$
  \tau \frac{dr_i}{dt} = -r_i + f\!\Bigl( \sum_j W_{ij} r_j + I_i \Bigr),
$$

with $f$ a nonlinearity (ReLU, tanh, sigmoid). Lappalainen et al. 2024
(*Nature*) fit firing-rate networks to the fly optic lobe connectome and
recovered direction-selective T4/T5 cell responses that matched
electrophysiology. Rate networks are cheap (one matrix-vector per
timestep), map cleanly onto `ruvector-solver`'s sparse primitives for
steady-state approximations, and lose temporal fidelity at the sub-100 ms
timescale.

### 3.2 Leaky integrate-and-fire (LIF)

$$
  \tau_m \frac{dV_i}{dt} = -(V_i - V_{\text{rest}}) + R_m I_i(t),
$$

with a reset-on-threshold rule emitting spikes delivered to postsynaptic
neurons with axonal delay. Brunel 2000 (*J Comput Neurosci*) established
the reference framework; Kakaria & de Bivort 2017 (*Front Behav Neurosci*)
applied it to the central complex of *Drosophila*. LIF is the right
abstraction level for CC-OS: it captures spike-timing phenomena important
for grooming and courtship circuits while staying tractable at 50k–150k
neurons. See 04-neural-dynamics-runtime.md for the Rust implementation.

### 3.3 Connectome-constrained deep mechanistic networks

Lappalainen et al. 2024 (*Nature*) introduced a family of trainable rate
networks in which the synaptic weights are constrained by the connectome
(sign and topology) but tuned by backpropagation against neural-recording
objectives. The family generalises firing-rate networks by allowing
learned nonlinearities and sensory adapters. CC-OS treats this class as a
**calibration target** rather than a primary runtime: once LIF dynamics
are running, Lappalainen-style fits provide external validation that the
per-neuron nonlinearities are in the right regime.

### 3.4 Biophysically detailed models

NEURON, GENESIS, Brian2, and the Blue Brain morphologically-detailed cortex
models fall here. CC-OS is **not** competing at this level. Our goal is
graph + LIF + embodiment with auditable fragility; detailed biophysics is
deferred to specialised tools. Non-goal noted in 02 §12.

## 4. Embodiment Platforms

| Platform | Physics backend | Language | Fidelity | Weakness |
|----------|-----------------|----------|----------|----------|
| NeuroMechFly v1 | MuJoCo | Python | High — biomechanically validated | Python overhead, Ubuntu focused |
| NeuroMechFly v2 | MuJoCo / MJX | Python + JAX | Higher — vision adapter included | Same |
| Walknet | Custom | Python / MATLAB | Medium — gait only | No body visuals |
| MuJoCo (bare) | MuJoCo | C | High general | Body model must be authored |
| PyBullet | Bullet | Python | Medium | Less accurate contact |
| Rapier | Rapier | Rust | Medium-high | No biomechanical fly model yet |
| Isaac Gym | PhysX | Python / C++ | High for large-scale RL | GPU-only, not auditable |

For CC-OS v1, we recommend a **Rapier-based Rust fly body** for the tight
inner loop plus a **NeuroMechFly bridge** (Python IPC over shared memory)
for biomechanical validation runs. 06-embodied-simulator-closed-loop.md
develops this architecture.

## 5. "Eon"-Style Integrations — What the Code Does vs What the Press Says

Recent industry demonstrations (sometimes presented as "running a fly brain
on a laptop") combine three separable achievements:

1. A connectome-constrained visual-system network (Lappalainen 2024).
2. A central-complex LIF dynamics model (Kakaria 2017, extended).
3. A body simulator in the NeuroMechFly lineage.

The engineering story — stitching these three together, running the loop
at near-real-time — is genuine and valuable. The framing that this
constitutes "an in-silico fly" or "a digital organism" is an overclaim.
The fly brain is not just the optic lobe plus the central complex plus
motor neurons; it is an integrated system with peripheral sensors,
neuromodulators, circadian state, genetic plasticity, and developmental
history. A three-module stitch-up is a faithful partial model of a
specific circuit, not an organism. CC-OS inherits the stitch architecture
(graph, LIF, body) but insists on naming what each layer models and what
it does not, and on an acceptance test (10-acceptance-test-grooming.md)
that is a reproducible behavioral assay, not a rhetorical claim.

## 6. Scaling Frontier

| Species | Neuron count | Synapse count | Today's feasibility | Notes |
|---------|--------------|---------------|---------------------|-------|
| C. elegans | 302 | ~7k | Trivial | Solved; OpenWorm |
| Drosophila larva | 3k | ~500k | Easy | Winding 2023 |
| Drosophila adult (FlyWire) | 139k | 54M | **Target for CC-OS v1** | Borderline realtime |
| Zebrafish larva | ~100k | — | Feasible | No proofread connectome yet |
| Honeybee | ~960k | — | Hard | No whole-brain connectome |
| Mouse cortex (1 mm³ patch) | ~200k | ~500M | Hard | MICrONS partial; full brain ~75M |
| Human cortex | ~16B | ~10¹⁴ | Out of scope | Not feasible with current hardware |

Feasibility is gated by three resources:
- **Memory**: synapse list in `(u64, u64, f64)` CSR triplets is ~24 bytes
  per edge; 54M synapses = ~1.3 GB uncompressed.
- **Compute**: event-driven LIF scales with mean firing rate × neuron count.
  At 5 Hz average, 139k neurons emit ~700k spikes/s, each triggering
  ~400 postsynaptic updates = 280M updates/s. Feasible on one workstation.
- **I/O**: voltage traces at 1 kHz for 139k neurons at 8-bit = 139 MB/s
  uncompressed. Tiered compression (02 §8) brings this to ~35 MB/s.

Full FlyWire realtime is aspirational but not absurd on a top-end
workstation. The acceptance test (doc 10) targets a 15k–30k sub-network to
stay well inside the feasibility envelope.

## 7. Why CC-OS is Not Mind Upload

Three framings articulate what CC-OS deliberately does not claim:

### 7.1 Information flow (Shannon view)

CC-OS simulates information flow from sensors through the connectome to
actuators. This is computation, not subjectivity. Shannon-channel analysis
has nothing to say about phenomenal experience, and we make no attempt to
attribute experience.

### 7.2 Markov blankets (Seth 2021 framing)

A Markov blanket separates a system's internal states from external states
conditional on sensory and active states (Friston 2013). In a CC-OS
run, the boundary is explicit and inspectable: sensory neurons are the
blanket's active/sensory states, interneurons are internal. There is no
claim that crossing this blanket constitutes minding.

### 7.3 Sandberg–Bostrom WBE scale

Sandberg and Bostrom's 2008 whole-brain-emulation framework enumerates
eleven levels, from computational network (level 4) to quantum (level 11).
CC-OS sits at **level 4 (computational network of point neurons)** with
partial level 5 (analog population dynamics). All higher levels (dendritic
compartments, intracellular biochemistry, molecular signalling) are out
of scope. This alone rules out mind-upload claims in the Sandberg–Bostrom
sense.

### 7.4 What we do claim

- The runtime reproduces the connectome's structural wiring.
- It reproduces qualitative LIF dynamics consistent with published fits.
- It reproduces published behavioral bouts under canonical stimuli within
  a bounded subcircuit.
- It supports counterfactual perturbation with auditable provenance.

That's the ceiling. Everything above — consciousness, identity, memory
continuity — is out of scope and is not implied by any sentence in this
compendium.

## 8. Open Questions RuView Can Uniquely Address

1. **Coherence-aware runtime.** The existing CSI pipeline uses
   `CoherenceGate` (Accept / PredictOnly / Reject / Recalibrate) to reject
   out-of-distribution signal frames. The neural analog — detect when the
   simulated population is drifting outside the behavioral attractor and
   refuse to make circuit claims on that bout — is novel and valuable.
   See 05-cross-region-attention-fusion.md and
   07-coherence-crv-behavioral-episodes.md.
2. **Structural fragility scoring.** Fragility = $\Delta\text{cut}/\text{cut}$
   under ablation (see 03-mincut-circuit-analysis.md). No existing
   connectome simulator emits fragility as a first-class output. RuVector's
   `DynamicMinCut` is built for exactly the incremental-ablation use case.
3. **Counterfactual circuit discovery.** Greedy ablation + re-mincut is a
   simple, well-defined search procedure for minimal sufficient circuits.
   See 08-counterfactual-perturbation.md.
4. **Witness-audit reproducibility.** ADR-028 established a witness-bundle
   standard: input → pipeline → SHA-256 hash. Lifting this to neural
   runtime is straightforward and makes CC-OS reviewable in a way that
   Python-notebook demos are not.

## 9. References

1. White, J. G., Southgate, E., Thomson, J. N., Brenner, S. (1986). *The
   structure of the nervous system of the nematode C. elegans.* Phil Trans R
   Soc B.
2. Cook, S. J., Jarrell, T. A., Brittin, C. A., et al. (2019). *Whole-animal
   connectomes of both C. elegans sexes.* Nature.
3. Winding, M., Pedigo, B. D., Barnes, C. L., et al. (2023). *The connectome
   of an insect brain.* Science.
4. Scheffer, L. K., Xu, C. S., Januszewski, M., et al. (2020). *A connectome
   and analysis of the adult Drosophila central brain.* eLife.
5. Dorkenwald, S., Matsliah, A., Sterling, A. R., et al. (2024). *Neuronal
   wiring diagram of an adult brain.* Nature.
6. Eckstein, N., Bates, A. S., Champion, A., et al. (2024). *Neurotransmitter
   classification from electron microscopy images.* Cell.
7. MICrONS Consortium. (2023). *Functional connectomics spanning multiple
   areas of mouse visual cortex.* bioRxiv; subsequent 2025 Nature.
8. Vishwanathan, A., et al. (2024). *Larval zebrafish whole-brain connectome
   (in progress).* Preprint.
9. Brunel, N. (2000). *Dynamics of sparsely connected networks of excitatory
   and inhibitory spiking neurons.* J. Comput. Neurosci.
10. Izhikevich, E. M. (2003). *Simple model of spiking neurons.* IEEE TNN.
11. Kakaria, K. S., de Bivort, B. L. (2017). *Ring attractor dynamics emerge
    from a spiking model of the entire protocerebral bridge.* Front Behav
    Neurosci.
12. Lappalainen, J. K., Tschopp, F. D., Prakhya, S., et al. (2024).
    *Connectome-constrained networks predict neural activity across the fly
    visual system.* Nature.
13. Lobato-Rios, V., Ramalingasetty, S. T., Özdil, P. G., et al. (2022).
    *NeuroMechFly, a neuromechanical model of adult Drosophila melanogaster.*
    Nat Methods.
14. Wang-Chen, S., et al. (2024). *NeuroMechFly v2 with vision.* Preprint.
15. Namiki, S., Dickinson, M. H., Wong, A. M., Korff, W., Card, G. M. (2018).
    *The functional organization of descending sensory-motor pathways in
    Drosophila.* eLife.
16. Seeds, A. M., Ravbar, P., Chung, P., et al. (2014). *A suppression
    hierarchy among competing motor programs drives sequential grooming in
    Drosophila.* eLife.
17. Hampel, S., Franconville, R., Simpson, J. H., Seeds, A. M. (2015). *A
    neural command circuit for grooming movement control.* eLife.
18. Cruse, H. (1990). *What mechanisms coordinate leg movement in walking
    arthropods?* Trends Neurosci.
19. Tuthill, J. C., Wilson, R. I. (2016). *Mechanosensation and adaptive
    motor control in insects.* Curr Biol.
20. Sandberg, A., Bostrom, N. (2008). *Whole Brain Emulation: A Roadmap.*
    Future of Humanity Institute Tech Report 2008-3.
21. Seth, A. (2021). *Being You: A New Science of Consciousness.* Faber.
22. Friston, K. (2013). *Life as we know it.* J. R. Soc. Interface.
23. Ford, L. R., Fulkerson, D. R. (1956). *Maximal flow through a network.*
    Canadian J. Math.
24. Stoer, M., Wagner, F. (1997). *A simple min-cut algorithm.* J. ACM.
25. Fiedler, M. (1973). *Algebraic connectivity of graphs.* Czech Math J.
26. Feng, K., Sen, R., Minegishi, R., et al. (2020). *Ascending neurons
    convey behavioral state to integrative sensory and action selection
    brain regions.* Nat Neurosci.

---

**Next document**: 02-connectome-graph-substrate.md — the Layer 1 graph
schema, per-neuron embeddings, and temporal-tensor storage.

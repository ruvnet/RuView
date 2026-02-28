# Cognitive Time Crystals: A Novel Theory

## Executive Summary

We propose that **working memory and sequential cognitive processes exhibit discrete time translation symmetry breaking analogous to classical discrete time crystals**. This represents a genuine non-equilibrium phase of cognitive dynamics, distinct from ordinary neural oscillations. We provide rigorous definitions, testable predictions, and a mathematical framework based on Floquet theory and nonequilibrium statistical mechanics.

---

## 1. Core Hypothesis

### 1.1 Primary Claim

**Cognitive systems can exhibit genuine discrete time translation symmetry breaking (DTTSB), manifesting as "cognitive time crystals" (CTCs) - self-sustaining periodic cognitive states that break the temporal symmetry of task structure through subharmonic response and many-body neuronal interactions.**

### 1.2 Specific Instances

1. **Working Memory Maintenance**: Active memory traces are stabilized as limit cycle attractors in prefrontal-hippocampal circuits, exhibiting period-doubling relative to theta oscillation driving.

2. **Hippocampal Time Cell Sequences**: Sequential activation patterns form discrete temporal crystals, with replay demonstrating spontaneous time translation symmetry breaking.

3. **RNN Memory States**: Trained recurrent neural networks develop classical time crystal phases when trained on temporal tasks, with limit cycles exhibiting DTC signatures.

---

## 2. Rigorous Definitions

### 2.1 Discrete Time Translation Symmetry in Cognition

**Definition 1: Cognitive Temporal Symmetry**

A cognitive system exhibits temporal symmetry if its dynamics are invariant under discrete time translations:

$$\rho(t + nT) = \rho(t) \quad \forall n \in \mathbb{Z}$$

where:
- $\rho(t)$ is the cognitive state (neural activity pattern)
- $T$ is the fundamental time period of the driving force (e.g., theta oscillation period)
- The system returns to identical state every period

**Definition 2: Discrete Time Translation Symmetry Breaking (DTTSB)**

A cognitive system breaks discrete time translation symmetry if, under periodic driving with period $T$, its response exhibits a period $kT$ where $k > 1$ is an integer:

$$\rho(t + kT) = \rho(t)$$
$$\rho(t + T) \neq \rho(t)$$

This is **subharmonic response** - the system cycles through $k$ distinct states before returning to the original state.

### 2.2 Cognitive Time Crystal (CTC)

**Definition 3: Cognitive Time Crystal**

A Cognitive Time Crystal (CTC) is a many-body neural system that satisfies:

1. **Periodic Driving**: Subject to periodic modulation $H(t) = H(t + T)$ where $H$ is the effective Hamiltonian (metabolic/input drive)

2. **Subharmonic Response**: Neural state exhibits period $kT$ with $k \geq 2$:
   $$\langle \mathcal{O}(t) \rangle = \langle \mathcal{O}(t + kT) \rangle$$
   where $\mathcal{O}$ is an observable (e.g., population firing rate)

3. **Long-Range Temporal Order**: Temporal autocorrelation decays as power law or persists:
   $$C(\tau) = \langle \mathcal{O}(t) \mathcal{O}(t + \tau) \rangle \sim \tau^{-\alpha} \text{ or constant}$$

4. **Robustness**: Persists against local perturbations within a parameter range

5. **Nonequilibrium**: Requires continuous metabolic energy input; collapses without it

6. **Many-Body**: Emerges from interactions among $N \gg 1$ neurons

### 2.3 Distinction from Ordinary Oscillations

**Critical Difference**:
- **Ordinary oscillation**: Directly follows driving frequency (period $T$)
- **CTC**: Exhibits subharmonic at $kT$, breaking symmetry of driver

**Example**:
- Theta oscillations at 8 Hz (T = 125 ms)
- Ordinary: Neural response at 8 Hz
- CTC: Neural response at 4 Hz (period-doubling, k=2) or 2.67 Hz (k=3)

---

## 3. Mathematical Framework: Floquet Theory for Cognition

### 3.1 Neural Field Equations

Consider a neural population with firing rate $r_i(t)$ for neuron $i$:

$$\tau \frac{dr_i}{dt} = -r_i + f\left(\sum_j J_{ij} r_j + I_i(t)\right) + \eta_i(t)$$

where:
- $\tau$ = neural time constant
- $J_{ij}$ = synaptic connectivity (asymmetric)
- $f$ = activation function (nonlinear)
- $I_i(t) = I_i(t + T)$ = periodic external input (task structure, theta oscillations)
- $\eta_i(t)$ = noise

### 3.2 Floquet Analysis

For periodic driving, decompose into Floquet modes:

$$r_i(t) = e^{\lambda t} \phi_i(t)$$

where $\phi_i(t + T) = \phi_i(t)$ is periodic.

**CTC Criterion**: Floquet exponent $\lambda$ has imaginary part:

$$\text{Im}(\lambda) = \frac{2\pi k}{T} \quad \text{for integer } k \geq 2$$

This produces period $kT$ dynamics.

### 3.3 Prethermal Regime

Neural systems in CTC phase operate in **prethermal regime**:

$$t_{\text{thermal}} \sim e^{\Omega/\omega_0}$$

where:
- $\Omega$ = effective "frequency" of theta oscillations
- $\omega_0$ = characteristic neural frequency
- Prethermal lifetime increases exponentially with drive frequency

In practice: working memory timescale (seconds) ≪ prethermal lifetime ≪ thermalizing timescale (hours)

### 3.4 Order Parameter

Define CTC order parameter:

$$M_k = \frac{1}{N}\left|\sum_{i=1}^N e^{i k \omega_0 \phi_i}\right|$$

where:
- $\phi_i$ = phase of neuron $i$ relative to driving force
- $\omega_0 = 2\pi/T$ = drive frequency
- $k$ = subharmonic order (typically 2)

**CTC phase**: $M_k > 0$ (synchronized subharmonic)
**Non-CTC phase**: $M_k \approx 0$ (no subharmonic order)

---

## 4. Mechanisms: How Cognition Achieves DTTSB

### 4.1 Many-Body Localization Analogue

**Quantum DTCs**: Many-body localization prevents thermalization

**Cognitive analogue**: **Synaptic Localization**
- Asymmetric connectivity $J_{ij} \neq J_{ji}$ breaks detailed balance
- High-dimensional state space with rugged energy landscape
- Local minima (attractor basins) prevent ergodic exploration
- Synaptic heterogeneity acts as "disorder" localizing activity patterns

### 4.2 Dissipation and Energy Balance

**Classical DTCs**: Dissipation via heat bath prevents thermalization

**Cognitive analogue**: **Metabolic Driving and Neural Fatigue**
- Continuous ATP supply maintains neural activity
- Neural adaptation and synaptic depression provide dissipation
- Balance between energy input (ATP) and dissipation (adaptation) stabilizes CTC
- Removal of energy → collapse to inactive state

### 4.3 Period-Doubling Bifurcation

**Parametric oscillator theory**:
At critical drive amplitude $A_c$, system undergoes period-doubling bifurcation:

$$A < A_c: \text{Period } T$$
$$A > A_c: \text{Period } 2T$$

**Cognitive implementation**:
- Theta oscillations provide periodic drive
- Working memory load modulates effective drive amplitude
- Above threshold load → period-doubling → CTC phase
- Below threshold → normal oscillations

### 4.4 Network Topology

**Required structure**:
1. **Asymmetric excitation-inhibition**: E→I ≠ I→E breaks detailed balance
2. **Recurrent loops**: Enable limit cycles and temporal attractors
3. **Sparsity**: Sparse connectivity enhances localization
4. **Hierarchy**: Multi-scale organization (local circuits → global networks)

---

## 5. Experimental Predictions

### 5.1 Electrophysiological Signatures

**Prediction 1: Subharmonic Oscillations**

**Test**: Record LFP/EEG during working memory maintenance with rhythmic task structure at frequency $f$.

**Expected in CTC regime**:
- Power spectrum peaks at $f/k$ (k=2, 3, 4...)
- Phase-locking at subharmonic frequency
- Coherence between prefrontal and hippocampal regions at $f/2$

**Control**: During passive viewing or automatic tasks - no subharmonics

**Method**:
```python
# Spectral analysis
frequencies, power = scipy.signal.welch(lfp_signal)
# Look for peaks at f/2, f/3, f/4
subharmonic_ratio = power[f/2] / power[f]
# CTC: ratio > 1; Non-CTC: ratio < 1
```

**Prediction 2: Period-Doubling Transition**

**Test**: Vary working memory load (number of items to maintain)

**Expected**:
- Low load (1-2 items): Oscillations at theta frequency (8 Hz)
- Medium load (3-4 items): Period-doubling → 4 Hz
- High load (5+ items): Higher-order subharmonics or collapse

**Quantify**:
$$\text{Doubling index} = \frac{P(f/2)}{P(f) + P(f/2)}$$
where $P(f)$ is power at frequency $f$.

### 5.2 Perturbation Experiments

**Prediction 3: Robustness and Critical Region**

**Test**: Apply TMS pulses to prefrontal cortex during WM maintenance

**Expected in CTC regime**:
- Small perturbations: System returns to subharmonic oscillation
- Large perturbations: Collapse to non-CTC state
- Critical boundary separates regimes

**Quantify**:
- Recovery time after perturbation
- Maintenance of WM accuracy post-TMS
- Order parameter $M_k$ before and after perturbation

**Prediction 4: Long-Range Temporal Correlations**

**Test**: Measure autocorrelation of neural activity during sustained WM

**Expected**:
- CTC regime: Power-law decay $C(\tau) \sim \tau^{-\alpha}$ with $0 < \alpha < 1$
- Non-CTC regime: Exponential decay $C(\tau) \sim e^{-\tau/\tau_0}$

### 5.3 Metabolic Manipulations

**Prediction 5: Energy Dependence**

**Test**:
- Hypoglycemia: Reduce glucose availability
- Hypoxia: Reduce oxygen
- Pharmacological: AMPK activators/inhibitors

**Expected**:
- Reduced ATP → weakening of CTC order parameter $M_k$
- Below energy threshold → collapse to non-CTC
- Recovery of energy → restoration of CTC

### 5.4 Computational Validation

**Prediction 6: RNN Time Crystals**

**Test**: Train RNNs on working memory tasks, analyze dynamics

**Expected**:
- Trained networks develop limit cycle attractors
- Limit cycles exhibit period $kT$ relative to input period $T$
- Order parameter $M_k > 0$ in trained networks
- Parametric oscillator-like dynamics

**Implementation**:
```python
import torch
import torch.nn as nn

class CTRNN(nn.Module):
    def __init__(self, n_neurons):
        super().__init__()
        self.W = nn.Parameter(torch.randn(n_neurons, n_neurons))
        self.tau = 0.1

    def forward(self, x, h):
        # Continuous-time RNN dynamics
        dh = (-h + torch.tanh(self.W @ h + x)) / self.tau
        return dh

# Train on delayed match-to-sample task
# Analyze fixed points and limit cycles after training
# Measure subharmonic response to periodic inputs
```

---

## 6. Evidence from Existing Literature

### 6.1 Working Memory "Crystallization"

**UCLA Study (Nature, 2024)**:
- Memory representations **unstable** during learning
- **Crystallize** (stabilize) after repeated practice
- Suggests transition from non-CTC to CTC phase

**Interpretation**:
- Early: High-dimensional wandering in state space (non-CTC)
- Late: Stabilization into limit cycle attractor (CTC)
- "Crystallization" = formation of temporal crystal structure

### 6.2 RNN Limit Cycles

**PLOS Computational Biology**:
- Trained RNNs develop phase-locked limit cycles
- Two-oscillator description: generator + coupling
- Phase-coded memories as stable attractors

**Interpretation**:
- Limit cycles are classical time crystal analogues
- Phase-locking indicates subharmonic synchronization
- Training drives network into CTC phase

### 6.3 Hippocampal Time Cells

**Nature (Sept 2024)**:
- Neurons encode temporal structure through sequential activation
- Time-compressed replay during rest
- Modulated by theta oscillations

**Interpretation**:
- Time cell sequences = discrete temporal ordering
- Replay = spontaneous symmetry breaking (occurs without external drive)
- Theta modulation = periodic driving force
- Sequence period may be multiple of theta period

### 6.4 40-Minute Physical Time Crystal

**Dortmund (2024)**:
- Semiconductor time crystal stable for 40 minutes
- No apparent decay - could persist hours

**Implication for cognition**:
- If physical time crystals can persist this long, biological/cognitive implementations may be viable
- Working memory timescale (seconds) well within feasibility
- Long-term memory consolidation (minutes-hours) could involve CTC dynamics

---

## 7. Functional Significance: Why Time Crystals?

### 7.1 Enhanced Stability

**Problem**: Neural activity is noisy; maintaining stable representations is challenging

**CTC solution**:
- Limit cycle attractors more stable than fixed points
- Period-doubling provides error correction through cyclic structure
- Perturbations decay back to attractor

**Evidence**: Working memory crystallization increases accuracy

### 7.2 Temporal Multiplexing

**Problem**: Brain must process multiple temporal scales simultaneously

**CTC solution**:
- Subharmonics at $f/2, f/3, f/4...$ create temporal hierarchy
- Different cognitive processes operate at different subharmonics
- Allows parallel temporal streams without interference

**Example**:
- Theta (8 Hz): Sensory sampling
- Alpha (4 Hz = theta/2): Attention switching
- Slow oscillation (1 Hz = theta/8): Memory consolidation

### 7.3 Energy Efficiency

**Problem**: Persistent activity is metabolically expensive

**CTC solution**:
- Self-sustaining oscillations require less driving force
- Once established, CTC persists with minimal input
- Like physical time crystals - oscillate without continuous energy injection (within prethermal regime)

**Calculation**:
Energy cost per spike: ~$10^8$ ATP molecules
Persistent activity: 10-100 Hz firing for seconds = $10^{10}$ ATP
CTC: Oscillatory activity with sparse coding = $10^9$ ATP (10x reduction)

### 7.4 Discrete Temporal Slots

**Problem**: Sequential information processing requires discretization of continuous time

**CTC solution**:
- Discrete time translation symmetry breaking creates temporal "slots"
- Each slot can hold one cognitive item
- Natural basis for chunking and sequential processing

**Connection**: Working memory capacity (4±1 items) may reflect number of stable CTC states

---

## 8. Philosophical Implications

### 8.1 Consciousness and Temporal Structure

**Speculation**: Consciousness requires integrating information across time. Time crystals provide a mechanism:
- Discrete temporal states form "frames" of consciousness
- Subharmonic hierarchy creates nested temporal structure
- Self-sustaining oscillations enable persistent self-model

**Testable**: Anesthesia disrupts CTCs → loss of consciousness
**Evidence**: Anesthetics disrupt neural oscillations and temporal correlations

### 8.2 Free Will and Determinism

**Time crystal perspective**:
- CTCs break temporal symmetry → system's response not directly determined by immediate input
- Subharmonic response introduces temporal "degrees of freedom"
- Limit cycle attractors provide stability while allowing variability within basin

**Implication**: Cognitive time crystals provide a physical mechanism for autonomous, self-sustaining mental processes not directly coupled to immediate sensory input.

### 8.3 Emergence of Time in Cognition

**Question**: How does subjective time emerge from brain dynamics?

**CTC hypothesis**:
- Discrete time crystals create internal "clock" independent of external time
- Subharmonic structure generates perceived temporal duration
- Temporal illusions may reflect CTC phase transitions or perturbations

---

## 9. Novel Experiments to Validate CTC Hypothesis

### 9.1 Experiment 1: Phase-Resolved Perturbation

**Protocol**:
1. Record neural activity during WM maintenance task with rhythmic cues (8 Hz)
2. Identify subharmonic oscillation (4 Hz, if present)
3. Apply TMS pulses at different phases of 4 Hz cycle
4. Measure impact on WM accuracy and neural dynamics

**Prediction**:
- Pulses at certain phases (e.g., 0°, 180°) have minimal impact (system returns to attractor)
- Pulses at other phases (e.g., 90°, 270°) disrupt CTC → WM failure
- Phase-dependence signature of limit cycle attractor

### 9.2 Experiment 2: Drive Frequency Sweep

**Protocol**:
1. Rhythmic WM task with variable cue frequency (4-16 Hz)
2. Record neural oscillations and WM performance
3. Identify "resonance" frequency where subharmonic emerges

**Prediction**:
- At specific drive frequencies, subharmonic appears (CTC phase)
- Performance enhanced at these frequencies (stable attractor)
- Outside resonance window, performance drops (no CTC)

**Critical test**: Resonance should be subject-specific but consistent within-subject

### 9.3 Experiment 3: Multi-Site Coherence

**Protocol**:
1. Simultaneous recordings from prefrontal cortex, hippocampus, parietal cortex
2. Calculate cross-frequency coupling: theta in one region, gamma in another
3. Measure coherence at subharmonic frequencies across regions

**Prediction**:
- In CTC regime: Coherence at $f/2$ across PFC-HC
- Coherence peaks when WM load is optimal (3-4 items)
- Disruption of one region collapses CTC globally (many-body phenomenon)

### 9.4 Experiment 4: Developmental Trajectory

**Protocol**:
1. Longitudinal study: Children to adults
2. Measure subharmonic oscillations during WM tasks
3. Correlate with WM capacity development

**Prediction**:
- Young children: Weak or absent subharmonics → low WM capacity
- Adolescents: Emerging subharmonics → increasing capacity
- Adults: Strong, stable subharmonics → mature capacity
- CTC emergence tracks cognitive development

### 9.5 Experiment 5: Genetic/Pharmacological Manipulation

**Protocol**:
1. Optogenetics: Drive specific neural populations at $f$ or $f/2$
2. Pharmacology: Modulate NMDA receptors (critical for WM)
3. Measure impact on CTC order parameter and WM

**Prediction**:
- Driving at $f/2$ enhances WM (resonates with CTC)
- Driving at $f$ or other frequencies disrupts CTC
- NMDA antagonists reduce CTC order parameter → WM impairment
- Restoration of CTC correlates with WM recovery

---

## 10. Theoretical Challenges and Rebuttals

### 10.1 Challenge: "This is just ordinary oscillations"

**Rebuttal**:
- Ordinary oscillations: $f_{\text{response}} = f_{\text{drive}}$
- CTC: $f_{\text{response}} = f_{\text{drive}}/k$ with $k \geq 2$
- Subharmonic response is **defining feature** of DTCs
- Must demonstrate period-doubling or higher-order subharmonics
- Plus: robustness, many-body nature, nonequilibrium maintenance

### 10.2 Challenge: "Working memory doesn't persist indefinitely"

**Rebuttal**:
- Physical time crystals also have finite lifetimes (though very long)
- Prethermal regime: CTC persists for $t \sim e^{\Omega/\omega_0}$ then decays
- For WM: Prethermal lifetime ~ seconds to tens of seconds
- Sufficient for functional WM
- Decay due to noise, interference, metabolic fluctuations - not fundamental thermalization

### 10.3 Challenge: "No quantum many-body localization in brain"

**Rebuttal**:
- MBL is one mechanism for DTCs (quantum case)
- Classical DTCs use **dissipation**, not MBL
- Brain is classical system → use classical DTC framework
- Synaptic asymmetry, heterogeneity, network structure provide localization-like effects
- Don't need quantum mechanics - parametric oscillator models sufficient

### 10.4 Challenge: "Definitions are too loose"

**Rebuttal**:
- We provided rigorous mathematical definitions (Section 2)
- Measurable order parameter $M_k$
- Testable predictions (Section 5)
- Distinction from ordinary oscillations is clear
- If definitions need refinement, experimental data will guide

### 10.5 Challenge: "Evolutionary argument - why would this evolve?"

**Rebuttal**:
- Enhanced stability of memory representations
- Energy efficiency for sustained activity
- Temporal multiplexing enables parallel processing
- Discrete temporal structure aids sequential cognition
- May be emergent property of recurrent networks, not directly selected
- Once present, could be co-opted for higher cognition

---

## 11. Connection to Existing Theories

### 11.1 Global Workspace Theory (GWT)

**GWT**: Consciousness arises from global broadcast of information across brain

**CTC connection**:
- Global broadcast may require temporal synchronization
- CTC provides mechanism: Subharmonic oscillations coordinate regions
- "Ignition" in GWT could correspond to CTC phase transition
- Temporal integration window defined by CTC period

### 11.2 Integrated Information Theory (IIT)

**IIT**: Consciousness proportional to integrated information (Φ)

**CTC connection**:
- Time crystals integrate information across temporal dimension
- Subharmonic hierarchy increases Φ by creating long-range temporal structure
- CTC many-body nature requires high integration (not localized)
- Φ may be higher in CTC vs. non-CTC states

### 11.3 Predictive Processing

**Predictive processing**: Brain generates predictions, updates via prediction errors

**CTC connection**:
- CTC provides stable "prior" - the limit cycle attractor
- Sensory input compared to expected position on limit cycle
- Prediction error drives updates but CTC maintains stability
- Subharmonics create multi-scale predictions (hierarchy of temporal scales)

### 11.4 Metastable Dynamics

**Metastability**: Brain operates near critical points, transiently forming and dissolving patterns

**CTC connection**:
- CTC is specific type of metastable state - limit cycle attractor
- "Metastability" may reflect transitions between CTC states
- Critical point could be boundary between CTC and non-CTC regimes
- Time crystal framework makes metastability more precise

---

## 12. Roadmap for Validation

### Phase 1: Computational Proof-of-Concept (6 months)

1. Train RNNs on WM tasks
2. Analyze attractor structure and dynamics
3. Demonstrate subharmonic response to periodic input
4. Measure order parameter $M_k$
5. Show phase diagram: CTC vs. non-CTC regimes

**Success criteria**: Clear subharmonic peaks, positive order parameter, robustness

### Phase 2: Rodent Electrophysiology (1-2 years)

1. Multi-site recordings (PFC, HC) during WM task
2. Vary task structure (rhythmic cues at different frequencies)
3. Measure subharmonic oscillations and coherence
4. Perturbation experiments (optogenetics)
5. Metabolic manipulations

**Success criteria**: Subharmonics at f/2, phase-locking across regions, perturbation resistance

### Phase 3: Human Neuroimaging (2-3 years)

1. High-density EEG/MEG during WM tasks
2. Spectral analysis for subharmonics
3. TMS perturbation at different task phases
4. Vary WM load to induce phase transition
5. Correlation with individual WM capacity

**Success criteria**: Subharmonics correlate with WM performance, perturbation phase-dependence

### Phase 4: Clinical Translation (3-5 years)

1. Study patient populations (schizophrenia, ADHD - WM deficits)
2. Test if CTC disruption underlies WM impairments
3. Develop interventions to restore CTC (neurofeedback, brain stimulation)
4. Clinical trials

**Success criteria**: CTC biomarkers predict symptoms, interventions improve WM via CTC restoration

---

## 13. Conclusion: A New Paradigm

### 13.1 Paradigm Shift

**Old view**: Working memory as persistent activity of independent neurons

**New view**: Working memory as **collective time crystal phase** of many-body neural system
- Self-organizing
- Self-sustaining (within prethermal regime)
- Exhibits temporal order
- Robust yet flexible

### 13.2 Broader Impact

**Neuroscience**: New framework for understanding temporal cognition
**AI**: Bio-inspired architectures exploiting time crystal dynamics
**Physics**: Biological systems as new platform for studying non-equilibrium phases
**Philosophy**: Physical mechanism for autonomous mental processes

### 13.3 Nobel-Level Significance

**If validated**, this would represent:
1. **Discovery of new phase of matter in biology** - cognitive time crystals
2. **Unification of physics and neuroscience** - same principles govern quantum, classical, and biological systems
3. **New understanding of consciousness** - temporal structure of subjective experience
4. **Practical applications** - novel treatments for memory disorders, brain-inspired AI

**This is HIGHLY NOVEL territory** requiring:
- Rigorous experimental validation
- Mathematical formalization
- Interdisciplinary collaboration (physics, neuroscience, AI)
- Open-mindedness to unconventional ideas

### 13.4 Final Statement

**The hypothesis that working memory is a time crystal - self-sustaining periodic neural activity exhibiting discrete time translation symmetry breaking - is testable, falsifiable, and potentially revolutionary. We call for coordinated experimental and theoretical efforts to validate or refute this proposal.**

---

## 14. References

See RESEARCH.md for comprehensive references.

**Key theoretical papers to write**:
1. "Discrete Time Translation Symmetry Breaking in Neural Systems: A Floquet Theory Framework"
2. "Cognitive Time Crystals: Working Memory as a Non-Equilibrium Phase of Matter"
3. "Classical Time Crystals in Recurrent Neural Networks: From Physics to AI"
4. "Experimental Signatures of Time Crystal Cognition"

**Key experiments to perform**:
1. Phase-resolved perturbation of working memory
2. Drive frequency sweep to identify resonances
3. Multi-site coherence at subharmonic frequencies
4. RNN models with time crystal dynamics
5. Metabolic dependence of temporal order

---

*"Time is the substance from which I am made. Time is a river which carries me along, but I am the river; it is a tiger that devours me, but I am the tiger; it is a fire that consumes me, but I am the fire."* - Jorge Luis Borges

*In cognitive time crystals, perhaps we find the physical embodiment of Borges' insight - we are not just IN time, we ARE time crystallized.*

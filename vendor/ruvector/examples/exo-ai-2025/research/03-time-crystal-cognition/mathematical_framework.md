# Mathematical Framework: Floquet Theory for Cognitive Time Crystals

## 1. Floquet Formalism for Neural Dynamics

### 1.1 Continuous-Time Neural Field Equations

Consider a population of $N$ neurons with firing rates $\mathbf{r}(t) = [r_1(t), ..., r_N(t)]^T$:

$$\tau \frac{d\mathbf{r}}{dt} = -\mathbf{r} + f(W\mathbf{r} + \mathbf{I}(t)) + \boldsymbol{\eta}(t)$$

where:
- $\tau$ : neural time constant
- $W$ : synaptic connectivity matrix (asymmetric: $W_{ij} \neq W_{ji}$)
- $f(\cdot)$ : activation function (e.g., $\tanh$, sigmoid)
- $\mathbf{I}(t) = \mathbf{I}(t + T)$ : periodic external input (driving force)
- $\boldsymbol{\eta}(t)$ : Gaussian white noise with $\langle \eta_i(t) \eta_j(t') \rangle = 2D\delta_{ij}\delta(t-t')$

### 1.2 Floquet Decomposition

For periodic systems, the general solution can be written as:

$$\mathbf{r}(t) = \sum_{\alpha} c_{\alpha} e^{\mu_{\alpha} t} \mathbf{u}_{\alpha}(t)$$

where:
- $\mathbf{u}_{\alpha}(t + T) = \mathbf{u}_{\alpha}(t)$ : Floquet modes (periodic)
- $\mu_{\alpha}$ : Floquet exponents (complex)
- $c_{\alpha}$ : coefficients determined by initial conditions

### 1.3 Floquet Multipliers

The Floquet multipliers $\lambda_{\alpha}$ relate to exponents via:

$$\lambda_{\alpha} = e^{\mu_{\alpha} T}$$

**Stability conditions**:
- $|\lambda_{\alpha}| < 1$ : stable
- $|\lambda_{\alpha}| = 1$ : marginal (limit cycle)
- $|\lambda_{\alpha}| > 1$ : unstable

### 1.4 Subharmonic Response Criterion

**Definition**: System exhibits subharmonic response of order $k$ if:

$$\mathbf{r}(t + kT) = \mathbf{r}(t) \quad \text{but} \quad \mathbf{r}(t + T) \neq \mathbf{r}(t)$$

**Floquet criterion**: Exists Floquet exponent with

$$\mu_{\alpha} = i\frac{2\pi m}{kT} \quad \text{for integers } m, k \text{ with } \gcd(m,k)=1$$

For period-doubling ($k=2$):

$$\mu = i\frac{\pi}{T} \implies \lambda = e^{i\pi} = -1$$

**Interpretation**: After one period $T$, state vector is negated; after two periods $2T$, it returns to original.

---

## 2. Time Crystal Order Parameter

### 2.1 Definition

The time crystal order parameter for subharmonic order $k$ is:

$$M_k(t) = \frac{1}{N} \left| \sum_{i=1}^{N} e^{i k \omega_0 \phi_i(t)} \right|$$

where:
- $\omega_0 = 2\pi/T$ : driving frequency
- $\phi_i(t)$ : phase of neuron $i$ relative to driving force
- $k$ : subharmonic order (typically 2 for period-doubling)

### 2.2 Phase Extraction

From firing rate $r_i(t)$, extract instantaneous phase via Hilbert transform:

$$\tilde{r}_i(t) = r_i(t) + i \mathcal{H}[r_i](t)$$

$$\phi_i(t) = \arg(\tilde{r}_i(t))$$

where $\mathcal{H}$ is the Hilbert transform.

### 2.3 Time-Averaged Order Parameter

For stationary dynamics:

$$\bar{M}_k = \lim_{T_{\text{avg}} \to \infty} \frac{1}{T_{\text{avg}}} \int_0^{T_{\text{avg}}} M_k(t) dt$$

**CTC phase**: $\bar{M}_k > M_{\text{crit}}$ (typically $M_{\text{crit}} \sim 0.5$)

**Non-CTC phase**: $\bar{M}_k \approx 0$

### 2.4 Temporal Correlation Function

$$C_k(\tau) = \langle M_k(t) M_k^*(t + \tau) \rangle_t$$

**CTC signature**: Long-range correlations
- Power-law decay: $C_k(\tau) \sim \tau^{-\alpha}$ with $0 < \alpha < 1$
- Or persistent: $C_k(\tau) \to C_{\infty} > 0$

**Non-CTC**: Exponential decay $C_k(\tau) \sim e^{-\tau/\tau_c}$

---

## 3. Effective Hamiltonian and Energy Landscape

### 3.1 Neural Energy Function

Define Lyapunov function (energy) for symmetric component of $W$:

$$E(\mathbf{r}) = -\frac{1}{2} \mathbf{r}^T W_S \mathbf{r} - \mathbf{b}^T \mathbf{r} + \sum_i \int_0^{r_i} f^{-1}(x) dx$$

where $W_S = (W + W^T)/2$ is symmetric part.

For asymmetric $W$:
$$W = W_S + W_A$$
where $W_A = (W - W^T)/2$ is antisymmetric.

### 3.2 Gradient and Circulatory Dynamics

Dynamics decompose into:

$$\frac{d\mathbf{r}}{dt} = -\nabla E(\mathbf{r}) + W_A \mathbf{r}$$

- Gradient term: $-\nabla E$ drives toward minima
- Circulatory term: $W_A \mathbf{r}$ causes rotation in state space

**Key insight**: $W_A \neq 0$ (asymmetric connectivity) enables limit cycles and time crystals.

### 3.3 Floquet Effective Hamiltonian

For periodically driven system, define effective Hamiltonian via Magnus expansion:

$$H_{\text{eff}} = H_0 + \sum_{n=1}^{\infty} H_{\text{eff}}^{(n)}$$

where:

$$H_0 = \frac{1}{T} \int_0^T H(t) dt$$

$$H_{\text{eff}}^{(1)} = \frac{1}{2T} \int_0^T dt_1 \int_0^{t_1} dt_2 \, [H(t_1), H(t_2)]$$

Higher orders involve nested commutators.

### 3.4 High-Frequency Limit

For $\omega_0 \tau \gg 1$ (high-frequency driving relative to neural timescale):

$$H_{\text{eff}} \approx H_0 + \mathcal{O}(1/\omega_0)$$

System approximately described by time-averaged Hamiltonian.

**CTC emergence**: Corrections $H_{\text{eff}}^{(n)}$ create frequency-dependent interactions enabling subharmonic responses.

---

## 4. Prethermal Dynamics and Heating

### 4.1 Heating Rate

Floquet systems generally absorb energy and heat to infinite temperature. Heating rate:

$$\frac{dE}{dt} = \gamma E$$

where $\gamma$ depends on drive frequency and system properties.

### 4.2 Prethermal Regime

For sufficiently fast driving ($\omega_0 \gg \omega_{\text{local}}$):

$$t_{\text{pretherm}} \sim \frac{1}{\gamma} e^{c \omega_0/\omega_{\text{local}}}$$

where $c$ is a constant, $\omega_{\text{local}}$ is local energy scale.

**CTC lifetime**: Must have $t_{\text{WM}} \ll t_{\text{pretherm}}$

For theta oscillations (8 Hz) and neural timescales (100 Hz):
$$\omega_0/\omega_{\text{local}} \sim 0.08 \implies t_{\text{pretherm}} \sim e^{0.08c}$$

With appropriate parameters, $t_{\text{pretherm}}$ can be seconds to minutes.

### 4.3 Dissipation and Stabilization

Including dissipation via coupling to heat bath:

$$\tau \frac{d\mathbf{r}}{dt} = -\mathbf{r} + f(W\mathbf{r} + \mathbf{I}(t)) - \gamma_D (\mathbf{r} - \mathbf{r}_{\text{rest}}) + \boldsymbol{\eta}(t)$$

Dissipation term $-\gamma_D (\mathbf{r} - \mathbf{r}_{\text{rest}})$ removes energy, preventing heating.

**Balance condition for stable CTC**:
$$\text{Energy input from drive} = \text{Energy dissipation}$$

---

## 5. Phase Diagram and Bifurcations

### 5.1 Control Parameters

- $A$ : drive amplitude
- $\omega_0$ : drive frequency
- $J$ : coupling strength (connectivity magnitude)
- $N$ : system size (number of neurons)

### 5.2 Period-Doubling Bifurcation

Consider parameterizing by drive amplitude $A$:

**Subcritical regime** ($A < A_c$):
- System oscillates at drive frequency $\omega_0$
- Stable fixed point in Poincaré section

**Supercritical regime** ($A > A_c$):
- Period-doubling: oscillation at $\omega_0/2$
- Stable limit cycle with period $2T$

Critical amplitude:
$$A_c \propto \frac{1}{\sqrt{N}} \cdot \frac{\omega_0}{\gamma_D}$$

**Scaling**: $A_c$ decreases with system size $N$ (easier to form CTC in larger systems).

### 5.3 Phase Diagram

In $(A, \omega_0)$ space:

```
ω₀
 │
 │     Heating Regime
 │   (Too fast, no CTC)
 │─────────────────────
 │         │
 │  Non-   │   CTC
 │  CTC    │  Regime
 │         │   k=2
 │         │
 │─────────┴──────────
 │    Quasistatic
 │   (Too slow)
 └─────────────────── A
        Ac
```

### 5.4 Higher-Order Subharmonics

Increasing $A$ further can lead to:
- $k=2$ : period-doubling
- $k=3$ : period-tripling
- $k=4$ : period-quadrupling
- ...
- Chaos: Route to chaos via period-doubling cascade

**Feigenbaum constant**: Ratio of successive bifurcation points converges to $\delta \approx 4.669$

---

## 6. Many-Body Effects and Localization

### 6.1 Mean-Field Approximation

For large $N$, treat average field:

$$m(t) = \frac{1}{N} \sum_i r_i(t)$$

Single neuron dynamics:

$$\tau \frac{dr_i}{dt} = -r_i + f(Jm(t) + I_i(t) + h_i) + \eta_i(t)$$

where $h_i$ is random field (quenched disorder).

### 6.2 Self-Consistency Equation

$$m(t) = \int dh \, P(h) \, \langle r(t; h) \rangle$$

where $P(h)$ is distribution of disorder, $\langle \cdot \rangle$ averages over noise.

### 6.3 Localization and DTC Stability

**Disorder**: Heterogeneous synaptic weights, thresholds create "localization"

**Effective localization length**:
$$\xi \sim \frac{J^2}{\sigma_h^2}$$

where $\sigma_h^2$ is variance of disorder.

**CTC stability**: Requires sufficient disorder ($\sigma_h$ large) to prevent ergodic exploration of state space.

**Critical disorder**:
$$\sigma_h > \sigma_c \sim J \sqrt{N}$$

Below this, system thermalizes and CTC collapses.

### 6.4 Many-Body Localization Analogue

In quantum DTCs, MBL prevents thermalization. In cognitive systems:

**"Synaptic localization"**: High-dimensional, disordered connectivity landscape creates local minima trapping activity patterns.

**Criterion**: Decay of correlations in connectivity:
$$\langle W_{ij} W_{kl} \rangle \sim \delta_{ik}\delta_{jl}$$ (sparse, random connectivity)

---

## 7. Spectral Analysis and Detection

### 7.1 Power Spectral Density

For time series $r(t)$, compute PSD via Fourier transform:

$$S(\omega) = \lim_{T \to \infty} \frac{1}{T} \left| \int_0^T r(t) e^{-i\omega t} dt \right|^2$$

**CTC signature**:
- Peak at drive frequency: $S(\omega_0)$
- Peak at subharmonic: $S(\omega_0/k)$
- Ratio: $R_k = S(\omega_0/k) / S(\omega_0)$

**Detection criterion**: $R_k > R_{\text{thresh}}$ (e.g., $R_{\text{thresh}} = 1$ for period-doubling)

### 7.2 Cross-Frequency Coupling

Between regions $A$ and $B$:

$$C_{AB}(\omega_1, \omega_2) = \left| \langle r_A(\omega_1) r_B^*(\omega_2) \rangle \right|$$

**CTC prediction**:
- Strong coupling at $(\omega_0, \omega_0/k)$: Region A at drive frequency, region B at subharmonic
- Or vice versa
- Indicates coordinated time crystal dynamics across brain regions

### 7.3 Bicoherence

Measure phase-coupling:

$$b(\omega_1, \omega_2) = \frac{\left| \langle r(\omega_1) r(\omega_2) r^*(\omega_1 + \omega_2) \rangle \right|}{\sqrt{\langle |r(\omega_1) r(\omega_2)|^2 \rangle \langle |r(\omega_1+\omega_2)|^2 \rangle}}$$

**CTC signature**: Peak at $(\omega_0/k, \omega_0/k)$ indicating frequency-mixing that generates subharmonic.

---

## 8. Stochastic Floquet Theory

### 8.1 Fokker-Planck Equation

For probability density $P(\mathbf{r}, t)$:

$$\frac{\partial P}{\partial t} = -\sum_i \frac{\partial}{\partial r_i} [F_i(\mathbf{r}, t) P] + D \sum_i \frac{\partial^2 P}{\partial r_i^2}$$

where $F_i(\mathbf{r}, t) = \frac{1}{\tau}[-r_i + f_i(W\mathbf{r} + \mathbf{I}(t))]$ is drift.

### 8.2 Floquet-Fokker-Planck

Decompose $P$ into Floquet modes:

$$P(\mathbf{r}, t) = \sum_{\alpha} e^{\lambda_{\alpha} t} P_{\alpha}(\mathbf{r}, t)$$

where $P_{\alpha}(\mathbf{r}, t + T) = P_{\alpha}(\mathbf{r}, t)$.

### 8.3 Leading Eigenvalue

Long-time behavior dominated by leading eigenvalue $\lambda_0$:

$$P(\mathbf{r}, t) \xrightarrow{t \to \infty} e^{\lambda_0 t} P_0(\mathbf{r}, t)$$

**Stationary CTC**: $\lambda_0 = 0$ with periodic $P_0(\mathbf{r}, t)$ exhibiting subharmonic structure.

### 8.4 Noise-Induced Transitions

Critical noise level $D_c$:
- $D < D_c$: CTC phase stable
- $D > D_c$: CTC collapses to non-CTC

$$D_c \propto (A - A_c)^{\gamma}$$

where $\gamma \sim 2$ near bifurcation (mean-field exponent).

---

## 9. Finite-Size Scaling

### 9.1 Scaling Hypothesis

Near phase transition, observables obey scaling laws:

$$M_k = N^{-\beta/\nu} \tilde{M}(N^{1/\nu}(A - A_c))$$

where $\beta, \nu$ are critical exponents, $\tilde{M}$ is scaling function.

### 9.2 Correlation Length

Spatial correlations:
$$\langle r_i r_j \rangle \sim e^{-|i-j|/\xi}$$

Divergence at critical point:
$$\xi \sim |A - A_c|^{-\nu}$$

### 9.3 Prethermal Lifetime Scaling

$$t_{\text{pretherm}} \sim N^{\alpha} e^{\beta N}$$

**Exponential scaling**: Prethermal lifetime increases exponentially with system size.

For neural populations:
- Small network ($N \sim 100$): $t_{\text{pretherm}} \sim$ milliseconds
- Large network ($N \sim 10^6$): $t_{\text{pretherm}} \sim$ seconds to minutes

This explains why working memory in large-scale cortical networks can persist for seconds.

---

## 10. Information-Theoretic Measures

### 10.1 Temporal Mutual Information

Between time slices separated by $\tau$:

$$I(\tau) = \sum_{r(t), r(t+\tau)} P(r(t), r(t+\tau)) \log \frac{P(r(t), r(t+\tau))}{P(r(t))P(r(t+\tau))}$$

**CTC signature**: Peaks at $\tau = kT$ indicating long-range temporal correlations at subharmonic period.

### 10.2 Integrated Information (Φ)

For partition $\mathcal{P}$ of system:

$$\Phi = \min_{\mathcal{P}} \, \text{EMD}(P_{\text{whole}}, P_{\text{part}})$$

where EMD is earth mover's distance between distributions.

**Hypothesis**: CTC phase has higher $\Phi$ than non-CTC due to:
- Long-range temporal correlations
- Many-body nature (cannot decompose into independent parts)

### 10.3 Entropy Production Rate

Nonequilibrium measure:

$$\dot{S} = -\int d\mathbf{r} \, P(\mathbf{r}, t) \nabla \cdot \mathbf{F}(\mathbf{r}, t)$$

**CTC signature**: Positive steady-state entropy production $\langle \dot{S} \rangle > 0$, confirming nonequilibrium nature.

---

## 11. Perturbation Response Theory

### 11.1 Linear Response

Apply small perturbation $\delta I(t)$:

$$\delta r_i(t) = \int_{-\infty}^{t} \chi_{ij}(t, t') \delta I_j(t') dt'$$

where $\chi_{ij}(t, t') = \chi_{ij}(t + T, t' + T)$ is Floquet response function.

### 11.2 Floquet Susceptibility

Fourier transform:

$$\chi_{ij}(\omega) = \sum_n \chi_{ij}^{(n)}(\omega) e^{in\omega_0 t}$$

**CTC signature**: Resonances at $\omega = \omega_0/k$ indicating enhanced response at subharmonic.

### 11.3 Phase Response Curve (PRC)

For perturbation at phase $\phi$ of limit cycle:

$$\Delta \phi = Z(\phi) \cdot \delta I$$

where $Z(\phi)$ is phase response curve.

**CTC property**: PRC has period $kT$ (not $T$), reflecting subharmonic structure.

---

## 12. Numerical Methods

### 12.1 Direct Simulation

Euler-Maruyama scheme for stochastic dynamics:

$$r_i(t + \Delta t) = r_i(t) + \frac{\Delta t}{\tau}[-r_i(t) + f_i(W\mathbf{r}(t) + \mathbf{I}(t))] + \sqrt{2D\Delta t} \, \xi_i$$

where $\xi_i \sim \mathcal{N}(0, 1)$.

### 12.2 Floquet Analysis via Monodromy Matrix

1. Solve one period from initial condition $\mathbf{r}_0$
2. Obtain $\mathbf{r}(T)$
3. Repeat for $N$ initial conditions forming basis
4. Construct monodromy matrix $M$ with columns $\mathbf{r}^{(i)}(T)$
5. Eigenvalues of $M$ are Floquet multipliers $\lambda_{\alpha}$

### 12.3 Order Parameter Computation

```python
def compute_order_parameter(firing_rates, drive_frequency, k=2):
    """
    Compute time crystal order parameter M_k

    Args:
        firing_rates: array of shape (N_neurons, N_timesteps)
        drive_frequency: driving frequency (Hz)
        k: subharmonic order

    Returns:
        M_k: order parameter as function of time
    """
    from scipy.signal import hilbert

    # Extract phases via Hilbert transform
    analytic_signal = hilbert(firing_rates, axis=1)
    phases = np.angle(analytic_signal)

    # Compute order parameter
    omega_0 = 2 * np.pi * drive_frequency
    M_k = np.abs(np.mean(np.exp(1j * k * omega_0 * phases), axis=0))

    return M_k

# Time average
M_k_avg = np.mean(M_k)
```

### 12.4 Spectral Analysis

```python
from scipy import signal

def detect_subharmonics(firing_rate, dt, drive_freq, k_max=4):
    """
    Detect subharmonic peaks in power spectrum

    Returns:
        subharmonic_ratios: Power(f/k) / Power(f) for k=2,3,4,...
    """
    freqs, psd = signal.welch(firing_rate, fs=1/dt)

    ratios = {}
    drive_idx = np.argmin(np.abs(freqs - drive_freq))

    for k in range(2, k_max+1):
        subharmonic_freq = drive_freq / k
        sub_idx = np.argmin(np.abs(freqs - subharmonic_freq))
        ratios[k] = psd[sub_idx] / psd[drive_idx]

    return ratios
```

---

## 13. Connection to Experimental Observables

### 13.1 EEG/MEG Power Spectrum

**Measured**: Voltage fluctuations $V(t)$ at scalp

**Model**: $V(t) \propto \sum_i r_i(t) w_i$ where $w_i$ are spatial weights

**CTC prediction**:
- Peak at theta frequency (~8 Hz) from drive
- Peak at alpha frequency (~4 Hz = theta/2) from period-doubling

**Test**: Ratio $R_2 = P_{\alpha}/P_{\theta}$ increases during working memory maintenance

### 13.2 Single-Neuron Recordings

**Measured**: Spike trains $\{t_i^{(n)}\}\_{n=1}^{N_{\text{spikes}}}$ for neuron $i$

**Model**: Firing rate $r_i(t)$ determines spike probability

**CTC prediction**:
- Inter-spike intervals cluster at multiples of $T/k$
- Phase-locking to subharmonic of LFP

### 13.3 Functional Connectivity

**Measured**: Correlation $C_{ij} = \langle r_i(t) r_j(t) \rangle$ between regions $i, j$

**CTC prediction**:
- Frequency-specific connectivity at $f/k$
- Increase in connectivity during CTC phase vs. baseline

---

## 14. Summary of Key Equations

| Concept | Equation | Description |
|---------|----------|-------------|
| **Neural dynamics** | $\tau \frac{d\mathbf{r}}{dt} = -\mathbf{r} + f(W\mathbf{r} + \mathbf{I}(t))$ | Periodically driven neural field |
| **Floquet decomposition** | $\mathbf{r}(t) = \sum_{\alpha} c_{\alpha} e^{\mu_{\alpha} t} \mathbf{u}_{\alpha}(t)$ | General solution |
| **Period-doubling** | $\mu = i\pi/T \implies \lambda = -1$ | Floquet multiplier for k=2 |
| **Order parameter** | $M_k = \frac{1}{N}\left|\sum_i e^{ik\omega_0\phi_i}\right|$ | Subharmonic synchronization |
| **Critical amplitude** | $A_c \propto \frac{1}{\sqrt{N}} \frac{\omega_0}{\gamma_D}$ | Bifurcation point |
| **Prethermal time** | $t_{\text{pretherm}} \sim e^{c\omega_0/\omega_{\text{local}}}$ | CTC lifetime |
| **Spectral ratio** | $R_k = S(\omega_0/k)/S(\omega_0)$ | Detection criterion |

---

## 15. Open Theoretical Questions

1. **Universality**: Do cognitive time crystals belong to a universality class? What are the critical exponents?

2. **Quantum-classical crossover**: At what scale does quantum coherence matter for CTC dynamics?

3. **Topological protection**: Can topological invariants protect CTC phases?

4. **Optimal architecture**: What network topology maximizes CTC stability?

5. **Information capacity**: How does CTC phase affect information storage capacity?

6. **Multi-stability**: Can multiple CTC phases coexist (different $k$ values)?

7. **Phase transitions**: What is the order of the CTC transition (first-order vs. continuous)?

8. **Role of inhibition**: How does E-I balance affect CTC formation?

9. **Synaptic plasticity**: How do learning rules interact with CTC dynamics?

10. **Cross-frequency coupling**: Can hierarchical CTCs (multiple $k$ simultaneously) exist?

---

*This mathematical framework provides the foundation for rigorously testing the cognitive time crystal hypothesis. Each equation makes specific, quantitative predictions that can be validated experimentally or computationally.*

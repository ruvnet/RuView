# Computing Edge Weights for RF Sensing Graphs from CSI Measurements

**Research Document 02** | RuView Project | March 2026

## Abstract

In a multistatic WiFi sensing mesh, each transmitter-receiver (TX-RX) pair defines
an edge in a spatial graph. The weight assigned to each edge encodes the coherence
and stability of the wireless channel between those two nodes. This document
presents methods for computing, filtering, and normalizing edge weights from
Channel State Information (CSI) measurements in real time. The target deployment
is a 16-node ESP32 mesh producing 120 bidirectional TX-RX edges, with edge weight
updates at 20 Hz. We cover CSI feature extraction, coherence metrics between link
pairs, multipath stability scoring via subspace methods, temporal windowing for
online estimation, noise robustness under real hardware constraints, and
normalization strategies for heterogeneous link geometries.

---

## 1. CSI Feature Extraction

### 1.1 CSI Measurement Model

An ESP32 node operating on an HT20 (20 MHz) channel reports CSI as a vector of
complex-valued subcarrier gains. For 802.11n HT20, the CSI vector has up to 56
usable subcarriers (indices -28 to +28, excluding nulls and the DC subcarrier).
Each CSI snapshot at time $t$ for link $(i,j)$ is:

$$
\mathbf{h}_{ij}(t) = [H_{ij}(f_1, t), H_{ij}(f_2, t), \ldots, H_{ij}(f_K, t)]^T \in \mathbb{C}^K
$$

where $K \leq 56$ and $f_k$ is the center frequency of the $k$-th subcarrier
spaced at $\Delta f = 312.5$ kHz.

### 1.2 Amplitude Features

The amplitude response $|H_{ij}(f_k, t)|$ captures the combined effect of
path loss, multipath fading, and any obstruction or reflection changes caused by
human presence. Key amplitude-derived features:

**Subcarrier Amplitude Variance (SAV).** Across a short window of $W$ packets:

$$
\text{SAV}_{ij}(k) = \frac{1}{W-1} \sum_{w=1}^{W} \left(|H_{ij}(f_k, t_w)| - \overline{|H_{ij}(f_k)|}\right)^2
$$

A high SAV on subcarrier $k$ indicates that the channel at that frequency is
being perturbed -- typically by motion in a Fresnel zone that subcarrier is
sensitive to.

**Amplitude Stability Index (ASI).** The reciprocal of the coefficient of
variation averaged across subcarriers:

$$
\text{ASI}_{ij} = \frac{1}{K} \sum_{k=1}^{K} \frac{\overline{|H_{ij}(f_k)|}}{\sigma_{|H_{ij}(f_k)|} + \epsilon}
$$

where $\epsilon$ is a small constant preventing division by zero. Higher ASI
means a more stable link. This forms a direct candidate for an edge weight.

**Principal Component Energy Ratio.** Applying PCA to the $K \times W$ amplitude
matrix and computing the fraction of variance explained by the first principal
component. A static channel concentrates energy in PC1; a dynamic channel
spreads energy across multiple components.

### 1.3 Phase Features

Raw CSI phase from ESP32 hardware is corrupted by:
- Sampling frequency offset (SFO): linear phase slope across subcarriers
- Carrier frequency offset (CFO): constant phase offset across all subcarriers
- Packet detection delay (PDD): random phase jump per packet
- Local oscillator (LO) phase noise: slow random walk

**Phase Sanitization.** Before extracting features, apply linear regression
to remove the SFO and CFO components:

$$
\hat{\phi}_{ij}(f_k, t) = \angle H_{ij}(f_k, t) - \left(\hat{a}(t) \cdot k + \hat{b}(t)\right)
$$

where $\hat{a}(t)$ and $\hat{b}(t)$ are the slope and intercept of the
least-squares fit to the unwrapped phase across subcarriers at time $t$.

**Phase Difference Stability.** Rather than using absolute phase (which drifts),
compute the phase difference between adjacent subcarriers:

$$
\Delta\phi_{ij}(k, t) = \angle H_{ij}(f_{k+1}, t) - \angle H_{ij}(f_k, t)
$$

The temporal variance of $\Delta\phi_{ij}(k, t)$ over a window is robust to
CFO and SFO since those affect all subcarriers similarly. This is the basis
for the conjugate multiplication approach used in SpotFi and subsequent work.

**Circular Phase Variance.** Because phase wraps modulo $2\pi$, use circular
statistics. The circular variance of a set of angles $\{\theta_1, \ldots, \theta_W\}$:

$$
V_{\text{circ}} = 1 - \left|\frac{1}{W} \sum_{w=1}^{W} e^{j\theta_w}\right|
$$

$V_{\text{circ}} = 0$ for perfectly stable phase; $V_{\text{circ}} = 1$ for
uniform (maximally unstable) phase.

### 1.4 Multipath Profile Features

The channel impulse response (CIR) is obtained via IFFT of the CSI vector:

$$
h_{ij}(\tau, t) = \text{IFFT}\{H_{ij}(f_k, t)\}
$$

The delay resolution is $1/B \approx 50$ ns for a 20 MHz bandwidth, corresponding
to a path length resolution of approximately 15 meters. Key CIR features:

- **RMS Delay Spread**: $\tau_{\text{rms}} = \sqrt{\overline{\tau^2} - \bar{\tau}^2}$
  weighted by tap power. Stability of delay spread indicates a static scattering
  environment.
- **Tap Count**: Number of CIR taps exceeding a noise threshold. Sudden changes
  indicate new reflectors or obstructions.
- **Dominant Tap Ratio**: Power in the strongest tap divided by total power.
  A high ratio means a dominant line-of-sight or specular path.

### 1.5 Packet Timing Features

At 20 Hz packet rate, inter-packet timing is nominally 50 ms. Deviations in
packet arrival time can indicate:
- Network congestion or contention (CSMA/CA backoff)
- Node reboot or firmware fault
- Deliberate TDM schedule slip

The packet jitter $J_{ij}(t)$ provides a link health indicator. Consistently
high jitter degrades the temporal resolution of edge weight estimation and
should reduce confidence (and thus weight) assigned to that edge.

---

## 2. Coherence Metrics

### 2.1 Cross-Correlation Coefficient

The Pearson correlation between CSI amplitude time series on two different
links $(i,j)$ and $(k,l)$ measures whether those links respond similarly to
environmental changes:

$$
\rho_{(ij),(kl)} = \frac{\text{Cov}(|\mathbf{h}_{ij}|, |\mathbf{h}_{kl}|)}{\sigma_{|\mathbf{h}_{ij}|} \cdot \sigma_{|\mathbf{h}_{kl}|}}
$$

For edge weight computation on a single link, the self-coherence (temporal
autocorrelation at lag $\tau$) is more relevant:

$$
R_{ij}(\tau) = \frac{1}{W} \sum_{t=1}^{W-\tau} \frac{(|\mathbf{h}_{ij}(t)| - \bar{h})(|\mathbf{h}_{ij}(t+\tau)| - \bar{h})}{\sigma^2}
$$

A rapidly decaying autocorrelation function indicates an unstable channel. The
decorrelation time $\tau_d$ (lag at which $R_{ij}(\tau)$ drops below $1/e$)
directly characterizes edge stability.

### 2.2 Mutual Information

For two CSI feature vectors $\mathbf{x}$ and $\mathbf{y}$ (possibly from
different subcarrier groups or different time windows), the mutual information:

$$
I(\mathbf{x}; \mathbf{y}) = H(\mathbf{x}) + H(\mathbf{y}) - H(\mathbf{x}, \mathbf{y})
$$

can be estimated using the Kraskov-Stoegbauer-Grassberger (KSG) estimator,
which uses $k$-nearest-neighbor distances in the joint space. This captures
nonlinear dependencies missed by correlation.

For real-time operation at 20 Hz on an ESP32 aggregator, the KSG estimator is
too expensive. Instead, use a binned estimator with $B = 8$-16 bins on quantized
amplitude values. The computational cost is $O(W \cdot B^2)$ per edge per update,
which is tractable for $W = 20$ and $B = 8$.

### 2.3 Spectral Coherence

The magnitude-squared coherence (MSC) between CSI time series at subcarrier $k$
across two links measures their frequency-domain correlation:

$$
C_{(ij),(kl)}(f) = \frac{|P_{(ij),(kl)}(f)|^2}{P_{(ij),(ij)}(f) \cdot P_{(kl),(kl)}(f)}
$$

where $P$ denotes the cross-spectral density estimated via Welch's method.

For a single link's edge weight, spectral coherence between the CSI at time $t$
and a reference (static) CSI captures how much the channel has deviated from
its baseline:

$$
C_{ij}^{\text{ref}}(f) = \frac{|P_{ij,\text{ref}}(f)|^2}{P_{ij}(f) \cdot P_{\text{ref}}(f)}
$$

The mean spectral coherence across all subcarrier frequencies is a scalar edge
weight: $w_{ij} = \frac{1}{K}\sum_k C_{ij}^{\text{ref}}(f_k)$.

### 2.4 Phase Phasor Coherence

This is the core metric used in the RuView coherence gate. For a window of $W$
phase measurements at subcarrier $k$:

$$
\gamma_{ij}(k) = \left|\frac{1}{W} \sum_{w=1}^{W} e^{j\hat{\phi}_{ij}(f_k, t_w)}\right|
$$

This is the magnitude of the mean phasor. Properties:
- $\gamma = 1$: all phase samples identical (perfectly coherent)
- $\gamma = 0$: phase uniformly distributed on the circle (no coherence)
- Robust to phase wrapping by construction (operates on the unit circle)
- Does not require phase unwrapping or sanitization beyond CFO removal

**Broadband Phasor Coherence.** Average across subcarriers:

$$
\Gamma_{ij} = \frac{1}{K} \sum_{k=1}^{K} \gamma_{ij}(k)
$$

This is the primary edge weight candidate. It ranges in $[0, 1]$, is
dimensionless, and degrades gracefully under motion.

**Differential Phasor Coherence.** To remove common-mode phase drift, compute
phasor coherence on the phase difference between subcarrier pairs $(k, k+1)$:

$$
\gamma_{ij}^{\Delta}(k) = \left|\frac{1}{W} \sum_{w=1}^{W} e^{j\Delta\phi_{ij}(k, t_w)}\right|
$$

This is strictly more robust to LO drift than the direct phasor coherence and
is the variant used in the RuView coherence gate.

### 2.5 Composite Coherence Score

Combine amplitude stability and phase coherence into a single edge weight:

$$
w_{ij} = \alpha \cdot \Gamma_{ij}^{\Delta} + (1 - \alpha) \cdot \text{ASI}_{ij}^{\text{norm}}
$$

where $\alpha \in [0.5, 0.8]$ typically favors phase coherence (more sensitive
to small motions) and $\text{ASI}^{\text{norm}}$ is the amplitude stability index
normalized to $[0, 1]$.

The optimal $\alpha$ depends on the SNR regime. At low SNR (long links, NLOS),
amplitude features are more reliable because phase noise dominates. At high SNR
(short links, LOS), phase coherence provides superior motion sensitivity.

---

## 3. Multipath Stability Scoring

### 3.1 Motivation

The CSI vector captures the superposition of all multipath components. A stable
CSI does not necessarily mean a stable environment -- it could mean that the
dominant path is stable while secondary paths fluctuate. Decomposing the channel
into individual multipath components and tracking their stability provides richer
information for edge weighting.

### 3.2 MUSIC Algorithm for Multipath Decomposition

The MUltiple SIgnal Classification (MUSIC) algorithm estimates the angles of
arrival (AoA) and/or time of arrival (ToA) of individual multipath components
from the CSI.

**Spatial Smoothing.** With a single antenna (as on the ESP32), spatial smoothing
constructs a pseudo-array from the frequency-domain CSI. Partition the $K$
subcarriers into overlapping subarrays of size $L$:

$$
\mathbf{R} = \frac{1}{K-L+1} \sum_{i=0}^{K-L} \mathbf{h}_i \mathbf{h}_i^H
$$

where $\mathbf{h}_i = [H(f_i), H(f_{i+1}), \ldots, H(f_{i+L-1})]^T$.

**Eigendecomposition.** Decompose $\mathbf{R} = \mathbf{U}\boldsymbol{\Lambda}\mathbf{U}^H$.
The eigenvectors corresponding to the $P$ largest eigenvalues span the signal
subspace; the remaining $L-P$ eigenvectors span the noise subspace
$\mathbf{U}_n$.

**MUSIC Pseudospectrum.** For delay $\tau$:

$$
P_{\text{MUSIC}}(\tau) = \frac{1}{\mathbf{a}^H(\tau)\mathbf{U}_n\mathbf{U}_n^H\mathbf{a}(\tau)}
$$

where $\mathbf{a}(\tau) = [1, e^{-j2\pi\Delta f\tau}, \ldots, e^{-j2\pi(L-1)\Delta f\tau}]^T$
is the steering vector.

**ESP32 Constraints.** With $K = 56$ subcarriers and $L = 20$, we can resolve
up to $P = 5$ multipath components with delay resolution finer than the FFT
limit. The eigendecomposition of a $20 \times 20$ Hermitian matrix requires
approximately 15,000 floating-point operations -- feasible on the aggregator
node at 20 Hz for 120 edges if batched efficiently, but not on each ESP32
independently.

### 3.3 ESPRIT for Multipath Delay Estimation

The Estimation of Signal Parameters via Rotational Invariance Techniques
(ESPRIT) algorithm provides direct delay estimates without pseudospectrum search.

Given the signal subspace $\mathbf{U}_s$ (the $P$ dominant eigenvectors), form
two submatrices by selecting the first $L-1$ and last $L-1$ rows:

$$
\mathbf{U}_1 = \mathbf{U}_s(1:L-1, :), \quad \mathbf{U}_2 = \mathbf{U}_s(2:L, :)
$$

The rotation matrix $\boldsymbol{\Phi} = \mathbf{U}_1^{\dagger}\mathbf{U}_2$
has eigenvalues $e^{-j2\pi\Delta f\tau_p}$, from which the delays $\tau_p$ are
extracted directly.

ESPRIT is computationally cheaper than MUSIC (no grid search) and provides
closed-form delay estimates. For real-time operation, ESPRIT is preferred.

### 3.4 Compressive Sensing for Sparse Multipath

When the multipath channel is sparse (few dominant paths in a large delay
spread), compressive sensing provides an alternative decomposition. Model:

$$
\mathbf{h}_{ij} = \mathbf{A}\mathbf{x} + \mathbf{n}
$$

where $\mathbf{A}$ is the $K \times G$ dictionary matrix with $G \gg K$ delay
grid points, $\mathbf{x}$ is a sparse vector of path gains, and $\mathbf{n}$
is noise. Solve via ISTA (Iterative Shrinkage-Thresholding Algorithm):

$$
\mathbf{x}^{(n+1)} = \mathcal{S}_{\lambda}\left(\mathbf{x}^{(n)} + \mu\mathbf{A}^H(\mathbf{h} - \mathbf{A}\mathbf{x}^{(n)})\right)
$$

where $\mathcal{S}_{\lambda}$ is the soft-thresholding operator with threshold
$\lambda$ and $\mu$ is the step size. ISTA converges in 20-50 iterations for
typical CSI sparsity levels.

The RuView tomography module uses ISTA with an $\ell_1$ penalty for voxel-space
reconstruction. The same solver can be repurposed for per-link multipath
decomposition by operating on the delay domain rather than the spatial domain.

### 3.5 Multipath Stability Score

Given the decomposed multipath parameters $\{(\tau_p, \alpha_p)\}_{p=1}^{P}$
(delays and complex amplitudes) at each time step, compute stability as:

**Path Persistence.** Track multipath components across time using a Hungarian
algorithm assignment (minimum-cost matching on delay differences). A path that
persists across $N$ consecutive windows contributes a persistence score of
$N/N_{\max}$.

**Path Amplitude Stability.** For each tracked path $p$, compute:

$$
S_p = \frac{\bar{|\alpha_p|}}{\sigma_{|\alpha_p|} + \epsilon}
$$

This is the inverse coefficient of variation of the path amplitude.

**Composite Multipath Stability Score (MSS).**

$$
\text{MSS}_{ij} = \sum_{p=1}^{P} \frac{|\alpha_p|^2}{\sum_q |\alpha_q|^2} \cdot S_p \cdot \frac{N_p}{N_{\max}}
$$

This power-weighted average of per-path stability scores gives higher weight
to stronger paths and penalizes paths that appear and disappear (low persistence).

### 3.6 Subspace Tracking for Real-Time Updates

Full eigendecomposition at every time step is expensive. Instead, use rank-one
subspace tracking algorithms:

**PAST (Projection Approximation Subspace Tracking).** Updates the signal
subspace incrementally as each new CSI vector arrives. Computational cost is
$O(LP)$ per update rather than $O(L^3)$ for full eigendecomposition.

**GROUSE (Grassmannian Rank-One Update Subspace Estimation).** Operates on the
Grassmann manifold, providing guaranteed convergence with $O(LP)$ complexity.

For the 20 Hz update rate with $L = 20$ and $P = 5$, subspace tracking costs
approximately 200 multiply-accumulate operations per edge per update -- trivially
cheap even on the aggregator.

---

## 4. Temporal Windowing

### 4.1 Requirements

Edge weights must balance two competing goals:
1. **Responsiveness**: Detect motion onset within 100-200 ms (2-4 packets at 20 Hz)
2. **Stability**: Avoid spurious weight fluctuations from thermal noise or
   transient interference

### 4.2 Exponential Moving Average (EMA)

The simplest temporal filter. For edge weight $w_{ij}(t)$ computed from the
current CSI packet:

$$
\hat{w}_{ij}(t) = \beta \cdot \hat{w}_{ij}(t-1) + (1-\beta) \cdot w_{ij}(t)
$$

The effective memory length is $1/(1-\beta)$ packets. For 20 Hz rate:
- $\beta = 0.9$: 10-packet memory (500 ms), good responsiveness
- $\beta = 0.95$: 20-packet memory (1 s), smoother but slower
- $\beta = 0.8$: 5-packet memory (250 ms), fastest response, noisiest

The EMA requires only one multiply-add per edge per update and stores a single
floating-point value per edge. For 120 edges, total memory is 480 bytes.

### 4.3 Welford Online Statistics

For computing running mean and variance without storing the full window, the
Welford algorithm provides numerically stable one-pass updates:

```
n += 1
delta = x - mean
mean += delta / n
delta2 = x - mean
M2 += delta * delta2
variance = M2 / (n - 1)
```

For edge weight computation, Welford statistics on the raw coherence values
provide both the smoothed weight (running mean) and a confidence bound (running
variance). The RuView longitudinal module uses Welford statistics for
biomechanics drift detection; the same infrastructure applies here.

**Windowed Welford.** Standard Welford accumulates over all time. For a sliding
window, maintain a circular buffer of the last $W$ values and use the removal
formula:

```
delta_old = x_old - mean
mean -= delta_old / n
delta2_old = x_old - mean
M2 -= delta_old * delta2_old
```

This gives exact windowed statistics with $O(1)$ per update and $O(W)$ memory.

### 4.4 Kalman Filtering of Edge Weights

Model the true edge weight as a random walk with Gaussian noise:

**State equation:**
$$
w_{ij}(t) = w_{ij}(t-1) + q(t), \quad q(t) \sim \mathcal{N}(0, Q)
$$

**Observation equation:**
$$
z_{ij}(t) = w_{ij}(t) + r(t), \quad r(t) \sim \mathcal{N}(0, R)
$$

where $z_{ij}(t)$ is the measured coherence/stability metric and $Q$, $R$ are
the process and measurement noise variances.

The Kalman filter equations for this scalar case:

```
# Predict
w_pred = w_est_prev
P_pred = P_prev + Q

# Update
K = P_pred / (P_pred + R)
w_est = w_pred + K * (z - w_pred)
P = (1 - K) * P_pred
```

**Advantages over EMA:**
- Automatically adapts the effective smoothing based on the noise level
- Provides a posterior variance $P$ that serves as a confidence metric
- The Kalman gain $K$ decreases as the estimate stabilizes, increasing
  inertia against spurious perturbations

**Tuning $Q$ and $R$.**
- $R$ is estimated from the measurement noise floor (thermal noise variance
  of the coherence metric). Typically $R \in [0.001, 0.05]$ depending on SNR.
- $Q$ controls how quickly the filter tracks changes. Higher $Q$ makes the
  filter more responsive. Typical range: $Q \in [0.0001, 0.01]$.
- The ratio $Q/R$ determines the steady-state Kalman gain. For motion detection
  applications, $Q/R \approx 0.1$ provides a good balance.

**Adaptive Q.** When a motion event is detected (e.g., coherence drops sharply),
temporarily increase $Q$ by a factor of 10-100 to allow the filter to track the
rapid change, then decay back to the baseline $Q$ over 1-2 seconds.

### 4.5 Multi-Rate Estimation

Maintain edge weights at multiple time scales simultaneously:

| Time Scale | Window | Use Case |
|------------|--------|----------|
| Fast (100 ms) | 2 packets | Motion onset detection |
| Medium (500 ms) | 10 packets | Activity classification |
| Slow (5 s) | 100 packets | Occupancy/presence |
| Baseline (60 s) | 1200 packets | Static environment model |

The fast estimate provides immediate reactivity; the slow estimate provides
the reference for "normal" channel behavior. The edge weight for sensing is
typically the ratio of fast to slow:

$$
w_{ij}^{\text{sensing}} = \frac{\Gamma_{ij}^{\text{fast}}}{\Gamma_{ij}^{\text{slow}} + \epsilon}
$$

A value near 1.0 means no change from baseline; values significantly below 1.0
indicate active perturbation. This ratio-based approach automatically adapts to
per-link baseline variations.

### 4.6 Computational Budget

At 20 Hz with 120 edges, the temporal windowing must process 2,400 edge updates
per second. Budget per update:

| Method | Operations | Memory/Edge | Total Memory (120 edges) |
|--------|-----------|-------------|--------------------------|
| EMA | 2 FLOP | 4 bytes | 480 bytes |
| Welford (windowed, W=20) | 8 FLOP | 84 bytes | ~10 KB |
| Kalman (scalar) | 10 FLOP | 8 bytes | 960 bytes |
| Multi-rate (4 EMAs) | 8 FLOP | 16 bytes | 1.9 KB |

All methods are trivially within the computational budget of the ESP32-S3
aggregator (240 MHz dual-core, 512 KB SRAM).

---

## 5. Noise Robustness

### 5.1 Sources of Noise in ESP32 CSI

**Phase Noise.** The ESP32's crystal oscillator has a phase noise floor of
approximately -90 dBc/Hz at 1 kHz offset. At 2.4 GHz carrier frequency, this
translates to a phase standard deviation of roughly 5-10 degrees per packet.
This is the dominant noise source for phase-based coherence metrics.

**Automatic Gain Control (AGC).** The ESP32 receiver adjusts its gain
automatically based on received signal strength. AGC changes manifest as
step changes in CSI amplitude across all subcarriers simultaneously. AGC
events occur when the received power changes by more than approximately 3 dB.

**Clock Drift.** The ESP32's 40 MHz crystal has a typical drift of 10-20 ppm.
Over a 1-second measurement window, this causes a phase ramp of up to
$2\pi \times 2.4 \times 10^9 \times 20 \times 10^{-6} \times 1 \approx 300$ radians
-- far larger than any sensing signal. This must be removed before phase-based
feature extraction.

**Quantization Noise.** The ESP32's ADC resolution for CSI is approximately
8-10 bits per I/Q component. Quantization noise power is $\Delta^2/12$ where
$\Delta$ is the quantization step. This is typically 20-30 dB below the thermal
noise floor and can be ignored.

**Co-Channel Interference.** In the 2.4 GHz ISM band, interfering traffic from
other WiFi networks, Bluetooth devices, and microwave ovens creates bursty
interference that can corrupt individual CSI measurements.

### 5.2 AGC Compensation

AGC changes affect all subcarriers equally (multiplicative scaling). Detection
and compensation:

1. **Detection.** Compute the ratio of total CSI power between consecutive packets:
   $$r(t) = \frac{\sum_k |H(f_k, t)|^2}{\sum_k |H(f_k, t-1)|^2}$$
   If $|r(t) - 1| > \theta_{\text{AGC}}$ (typically $\theta_{\text{AGC}} = 0.5$,
   corresponding to approximately 1.75 dB), flag an AGC event.

2. **Compensation.** Normalize each CSI vector by its total power:
   $$\tilde{H}(f_k, t) = \frac{H(f_k, t)}{\sqrt{\sum_k |H(f_k, t)|^2}}$$
   This removes any multiplicative gain change. The normalized CSI preserves
   the spectral shape (relative subcarrier amplitudes and phases) while
   discarding absolute power information.

3. **Weight impact.** During AGC transitions, amplitude-based edge weights will
   show a transient artifact. Apply a brief hold (1-2 packets) on the edge
   weight update after an AGC event to prevent false motion detection.

### 5.3 Clock Drift Removal

Two approaches, in order of increasing robustness:

**Linear Regression per Packet.** Fit a line to the unwrapped phase across
subcarriers and subtract. This removes SFO (slope) and CFO (intercept) at
each packet independently. Limitations: fails when the unwrapped phase has
ambiguities due to large multipath spread.

**Conjugate Multiplication.** Compute the product:
$$
H_{\text{conj}}(f_k, t) = H(f_k, t) \cdot H^*(f_k, t-1)
$$

The phase of $H_{\text{conj}}$ equals the phase change between packets, which
cancels any static phase offset. The clock drift contribution to $H_{\text{conj}}$
is a constant phase rotation across all subcarriers (since drift is linear in
frequency and constant over one packet interval). This constant can be estimated
and removed by the circular mean:

$$
\psi_{\text{drift}}(t) = \angle\left(\frac{1}{K}\sum_k H_{\text{conj}}(f_k, t)\right)
$$

$$
\tilde{H}_{\text{conj}}(f_k, t) = H_{\text{conj}}(f_k, t) \cdot e^{-j\psi_{\text{drift}}(t)}
$$

### 5.4 Robust Statistics for Outlier Rejection

Individual CSI packets may be corrupted by interference or hardware glitches.
Rather than discarding packets (which reduces the effective sample rate),
use robust estimators:

**Median Absolute Deviation (MAD).** For a window of coherence values
$\{c_1, \ldots, c_W\}$:

$$
\text{MAD} = \text{median}(|c_i - \text{median}(c)|)
$$

The robust standard deviation estimate is $\hat{\sigma} = 1.4826 \cdot \text{MAD}$.
Values beyond $3\hat{\sigma}$ from the median are flagged as outliers.

**Trimmed Mean.** Discard the top and bottom 10% of coherence values in each
window before computing the mean. This removes the influence of extreme
outliers while retaining most of the data.

**Huber M-estimator.** For the edge weight as a location estimator, the Huber
loss function provides optimal bias-variance tradeoff:

$$
\rho(x) = \begin{cases} \frac{1}{2}x^2 & |x| \leq k \\ k|x| - \frac{1}{2}k^2 & |x| > k \end{cases}
$$

with $k = 1.345$ for 95% efficiency at the Gaussian model. The iteratively
reweighted least squares (IRLS) solution converges in 3-5 iterations.

### 5.5 Z-Score Anomaly Detection

The RuView coherence module uses Z-score-based gating to classify link quality:

$$
z_{ij}(t) = \frac{\Gamma_{ij}(t) - \mu_{ij}}{\sigma_{ij}}
$$

where $\mu_{ij}$ and $\sigma_{ij}$ are the running mean and standard deviation
from Welford statistics. The gate decisions:

| Z-Score Range | Gate Decision | Action |
|---------------|---------------|--------|
| $|z| < 2$ | Accept | Use edge weight directly |
| $2 \leq |z| < 3$ | PredictOnly | Use Kalman prediction, skip measurement update |
| $3 \leq |z| < 5$ | Reject | Hold previous edge weight |
| $|z| \geq 5$ | Recalibrate | Reset running statistics, start fresh baseline |

This gating mechanism prevents single corrupted packets from destabilizing the
edge weight while allowing legitimate large changes (actual motion events) to
be captured through the recalibration path.

### 5.6 Interference Detection and Mitigation

Co-channel interference from non-mesh transmitters appears as:
- Elevated noise floor on specific subcarriers
- Burst errors in CSI magnitude
- Phase incoherence unrelated to motion

**Subcarrier-Level SNR Estimation.** Estimate the per-subcarrier SNR using the
ratio of signal power (from the slow baseline) to residual power (deviation
from baseline):

$$
\text{SNR}(f_k) = \frac{|\bar{H}(f_k)|^2}{\text{Var}(|H(f_k)|)}
$$

Subcarriers with $\text{SNR}(f_k)$ below a threshold (e.g., 5 dB) are excluded
from the coherence calculation. This adaptive subcarrier selection improves
edge weight quality at the cost of reduced frequency diversity.

The RuVector subcarrier selection module (`subcarrier_selection.rs`) implements
mincut-based selection that identifies the optimal subset of subcarriers
maximizing signal-to-interference ratio. This can be applied per-edge to
customize the subcarrier set to each link's interference environment.

---

## 6. Edge Weight Normalization

### 6.1 The Heterogeneity Problem

In a 16-node mesh, the 120 TX-RX edges span a wide range of conditions:

- **Distance**: Links range from 1 m (adjacent nodes) to 15+ m (diagonal)
- **Orientation**: Some links are LOS, others traverse walls (NLOS)
- **Antenna Pattern**: ESP32 PCB antenna has a roughly omnidirectional
  pattern but with 3-5 dB variation depending on orientation
- **Frequency Response**: Different links have different multipath profiles,
  leading to different baseline coherence levels

Without normalization, a short LOS link will always have a higher raw coherence
than a long NLOS link, regardless of whether motion is occurring. The edge
weights must be normalized so that each edge's weight reflects motion-induced
perturbation relative to its own baseline.

### 6.2 Per-Edge Baseline Normalization

The simplest approach: normalize each edge weight by its own baseline (static
environment) statistics:

$$
w_{ij}^{\text{norm}}(t) = \frac{\Gamma_{ij}(t) - \mu_{ij}^{\text{base}}}{\sigma_{ij}^{\text{base}}}
$$

or equivalently, the Z-score relative to baseline. This produces a standardized
edge weight where 0 means "at baseline" and negative values mean "coherence has
dropped" (motion detected).

**Baseline Estimation.** Compute $\mu_{ij}^{\text{base}}$ and
$\sigma_{ij}^{\text{base}}$ during a calibration period (e.g., 30 seconds with
no motion) or adaptively using the slow EMA from the multi-rate estimation.

**Limitation.** Per-edge normalization makes each edge independently calibrated
but does not account for the fact that some edges are inherently more sensitive
to motion than others (due to Fresnel zone geometry).

### 6.3 Fresnel Zone Sensitivity Weighting

The sensitivity of a TX-RX link to motion at a point $\mathbf{p}$ depends on
whether $\mathbf{p}$ lies within the first Fresnel zone of that link. The first
Fresnel zone radius at the midpoint of a link of length $d$ at wavelength
$\lambda$:

$$
r_F = \sqrt{\frac{\lambda d}{4}} \approx \sqrt{\frac{0.125 \times d}{4}} \text{ meters (at 2.4 GHz)}
$$

For a 5 m link, $r_F \approx 0.40$ m. For a 15 m link, $r_F \approx 0.69$ m.

Longer links have wider Fresnel zones and thus are sensitive to motion over a
larger area, but with less per-unit-area sensitivity. The effective sensitivity
of a link to a point perturbation scales as:

$$
S_{ij}(\mathbf{p}) \propto \frac{1}{d_{ij}} \cdot \exp\left(-\frac{\rho^2(\mathbf{p})}{r_F^2}\right)
$$

where $\rho(\mathbf{p})$ is the perpendicular distance from $\mathbf{p}$ to the
line segment connecting TX $i$ and RX $j$.

**Application to normalization.** Weight the edge contribution to the sensing
graph by $S_{ij}$, effectively upweighting short links (higher sensitivity)
and links whose Fresnel zone passes through the region of interest.

### 6.4 Distance-Dependent Normalization

Path loss causes the received SNR to decrease with distance, which in turn
increases the noise floor of the coherence estimate. A simple distance-based
correction:

$$
w_{ij}^{\text{dist}}(t) = w_{ij}^{\text{norm}}(t) \cdot \left(\frac{d_{ij}}{d_{\text{ref}}}\right)^{\eta/2}
$$

where $d_{\text{ref}}$ is a reference distance (e.g., 1 m) and $\eta$ is the
path loss exponent ($\eta \approx 2$ for free space, $\eta \approx 3$-$4$ for
indoor environments). The exponent $\eta/2$ is used because coherence noise
scales with the square root of the SNR (voltage domain).

Alternatively, estimate the distance correction empirically by measuring the
baseline coherence variance $\sigma_{ij}^{\text{base}}$ for each link and using
$\sigma_{ij}^{\text{base}}$ as the normalization factor. This automatically
captures distance, NLOS effects, and antenna pattern variations without
requiring explicit distance measurements.

### 6.5 Antenna Pattern Compensation

The ESP32 PCB antenna has an irregular pattern that depends on:
- Board orientation and mounting
- Nearby metallic objects (enclosure, mounting hardware)
- Polarization alignment between TX and RX

For precise normalization, characterize the antenna gain pattern during
deployment by measuring the average received power on each link and computing
the link budget discrepancy from a simple path loss model. The residual
(measured - predicted) captures the combined antenna pattern effect.

In practice, per-edge baseline normalization (Section 6.2) implicitly absorbs
antenna pattern effects, making explicit antenna compensation unnecessary
for most deployments.

### 6.6 Cross-Link Normalization for Graph Algorithms

When edge weights are consumed by graph algorithms (e.g., for tomographic
reconstruction or graph neural networks), they must be on a consistent scale.
Two standard approaches:

**Min-Max Normalization.**

$$
\tilde{w}_{ij}(t) = \frac{w_{ij}(t) - w_{\min}(t)}{w_{\max}(t) - w_{\min}(t)}
$$

where $w_{\min}$ and $w_{\max}$ are taken across all edges at time $t$.
Produces weights in $[0, 1]$ but is sensitive to outliers.

**Softmax Normalization.**

$$
\tilde{w}_{ij}(t) = \frac{e^{w_{ij}(t) / T}}{\sum_{(k,l)} e^{w_{kl}(t) / T}}
$$

where $T$ is a temperature parameter. This produces a probability distribution
over edges, useful for attention-weighted fusion. Higher $T$ produces more
uniform weights; lower $T$ concentrates weight on the most coherent links.

**Rank-Based Normalization.** Replace each weight with its rank among all 120
edges, then divide by 120. This is maximally robust to outliers and produces
a uniform marginal distribution, but discards magnitude information.

### 6.7 Temporal Normalization

Edge weights should also be normalized in the temporal domain to prevent
long-term drift from affecting graph computations:

**Detrending.** Subtract a slow-moving average (e.g., 60-second EMA) from
the edge weight to remove environmental drift (temperature changes, furniture
movement, seasonal daylight effects on materials):

$$
w_{ij}^{\text{detrend}}(t) = w_{ij}(t) - \text{EMA}_{60s}(w_{ij})(t)
$$

**Whitening.** Divide by the running standard deviation to produce unit-variance
edge weight fluctuations:

$$
w_{ij}^{\text{white}}(t) = \frac{w_{ij}^{\text{detrend}}(t)}{\sigma_{ij}^{\text{running}}(t)}
$$

This whitened signal is the input to detection algorithms (e.g., CFAR
detectors for motion onset).

---

## 7. Implementation Architecture

### 7.1 Pipeline Overview

The edge weight computation pipeline for the 16-node ESP32 mesh operates in
three stages:

```
Stage 1: Per-Node (ESP32)          Stage 2: Aggregator            Stage 3: Sensing Server
+-----------------------+     +---------------------------+     +---------------------+
| CSI extraction        |     | Collect 120 CSI vectors   |     | Graph construction  |
| AGC detection         | --> | Phase sanitization        | --> | Edge weight matrix  |
| Packet timestamping   |     | Coherence computation     |     | Tomographic recon   |
| TDM slot compliance   |     | Multipath decomposition   |     | Activity inference  |
+-----------------------+     | Temporal filtering        |     +---------------------+
                              | Normalization             |
                              | Z-score gating            |
                              +---------------------------+
```

**Stage 1** runs on each ESP32 node. Minimal processing: extract the CSI vector,
detect AGC events, and timestamp the packet using the TDM schedule.

**Stage 2** runs on the aggregator node (ESP32-S3 with 512 KB SRAM or an
external Raspberry Pi). This is where all 120 edge weights are computed and
filtered. The computational budget at 20 Hz for 120 edges:
- Phase sanitization: 120 x 200 FLOP = 24,000 FLOP
- Phasor coherence: 120 x 56 x 4 FLOP = 26,880 FLOP
- Kalman filter: 120 x 10 FLOP = 1,200 FLOP
- Normalization: 120 x 20 FLOP = 2,400 FLOP
- Total: ~55,000 FLOP per cycle = 1.1 MFLOP/s

This is well within the 240 MHz ESP32-S3's capability (approximately 100 MFLOP/s
in single precision).

**Stage 3** runs on the sensing server (Rust binary) which receives the 120
edge weights and constructs the spatial graph for higher-level processing.

### 7.2 Data Flow

Each edge weight update cycle:

1. TDM frame completes (all 16 nodes have transmitted in their slots)
2. Aggregator collects 120 CSI vectors (one per TX-RX pair, where RX nodes
   report CSI for each TX they receive)
3. Sanitize phase on all 120 vectors
4. Compute $\Gamma_{ij}^{\Delta}$ (differential phasor coherence) for all edges
5. Apply Kalman filter to produce smoothed edge weights
6. Apply Z-score gating to flag anomalous measurements
7. Apply per-edge baseline normalization
8. Broadcast the 120-element edge weight vector to the sensing server

The edge weight vector is a compact 120 x 4 = 480 byte payload (float32 per
edge), easily fitting in a single UDP packet.

### 7.3 Memory Layout

For 120 edges, the complete state for edge weight computation:

| Component | Per-Edge | Total |
|-----------|----------|-------|
| Kalman state ($\hat{w}$, $P$) | 8 B | 960 B |
| Welford stats ($n$, $\mu$, $M_2$) | 12 B | 1.4 KB |
| Multi-rate EMAs (4 scales) | 16 B | 1.9 KB |
| Baseline stats ($\mu_b$, $\sigma_b$) | 8 B | 960 B |
| Phase buffer (last packet) | 224 B | 26.2 KB |
| AGC state (last power) | 4 B | 480 B |
| **Total** | **272 B** | **~32 KB** |

Fits comfortably in ESP32-S3 SRAM with substantial headroom for the multipath
decomposition buffers if ESPRIT/MUSIC is run on the aggregator.

### 7.4 Rust Implementation Mapping

The edge weight computation maps to existing RuView crate structure:

| Component | Crate | Module |
|-----------|-------|--------|
| Phasor coherence | `wifi-densepose-signal` | `ruvsense/coherence.rs` |
| Coherence gating | `wifi-densepose-signal` | `ruvsense/coherence_gate.rs` |
| Phase alignment | `wifi-densepose-signal` | `ruvsense/phase_align.rs` |
| Multipath decomposition | `wifi-densepose-signal` | `ruvsense/field_model.rs` |
| Welford statistics | `wifi-densepose-signal` | `ruvsense/longitudinal.rs` |
| Subcarrier selection | `wifi-densepose-ruvector` | via `ruvector-mincut` |
| Kalman filtering | `wifi-densepose-signal` | `ruvsense/pose_tracker.rs` |
| Tomographic reconstruction | `wifi-densepose-signal` | `ruvsense/tomography.rs` |
| TDM protocol | `wifi-densepose-hardware` | `esp32/tdm.rs` |

---

## 8. Validation and Benchmarking

### 8.1 Ground Truth Generation

Edge weight quality can be validated against controlled experiments:

1. **Static baseline.** With no motion in the environment, all edge weights
   should remain at their baseline values with variance bounded by thermal noise.
   Measure the false alarm rate (fraction of time edge weights deviate beyond
   a threshold when no motion is present).

2. **Single-path perturbation.** Have a person walk along a known trajectory
   that crosses specific TX-RX links. Edge weights on crossed links should
   drop; non-crossed links should remain stable. Measure the detection
   probability and spatial selectivity.

3. **Multi-target separation.** Two people moving simultaneously. Edge weights
   should reflect the independent perturbation from each person. Use the
   temporal correlation between edge weight drops on different links to
   verify spatial discrimination.

### 8.2 Performance Metrics

| Metric | Definition | Target |
|--------|-----------|--------|
| Detection latency | Time from motion onset to edge weight drop > threshold | < 200 ms |
| False alarm rate | Fraction of static windows with edge weight deviations | < 1% |
| Spatial selectivity | Ratio of on-path to off-path edge weight change | > 10 dB |
| Update rate | Edge weight refresh frequency | 20 Hz |
| Computational load | CPU utilization on aggregator | < 20% |

### 8.3 Comparison of Edge Weight Methods

| Method | Motion Sensitivity | Noise Robustness | Computational Cost | Recommended Use |
|--------|-------------------|-------------------|--------------------|--------------------|
| Amplitude stability (ASI) | Medium | High | Very low | Low-SNR, NLOS links |
| Phase phasor coherence | High | Medium | Low | LOS links, fine motion |
| Differential phasor coherence | High | High | Low | General purpose (default) |
| Spectral coherence | Medium-High | Medium | Medium | Frequency-selective fading |
| Multipath stability (ESPRIT) | Very High | Low | High | High-value links |
| Composite (phase + amplitude) | High | High | Low | Recommended default |

---

## 9. Open Research Questions

### 9.1 Optimal Subcarrier Grouping

Should edge weights be computed from all 56 subcarriers, or should subcarriers
be grouped into frequency bands that respond differently to motion? Preliminary
results suggest that grouping subcarriers into 4 bands of 14 and computing
independent coherence values per band provides better spatial resolution
(different bands are sensitive to different path lengths) at the cost of
higher variance per estimate.

### 9.2 Cross-Band Coherence as Edge Feature

The coherence between CSI in different frequency bands on the same link may
carry additional information about the number and geometry of multipath
components. This cross-band feature has not been explored for edge weighting.

### 9.3 Asymmetric Edge Weights

In the current model, $w_{ij} = w_{ji}$ (channel reciprocity). In practice,
reciprocity holds for the physical channel but not for the measured CSI (due
to independent hardware impairments at TX and RX). Using directed edges with
potentially asymmetric weights may improve sensitivity at the cost of doubling
the edge count to 240.

### 9.4 Learned Edge Weights

A graph neural network could learn optimal edge weight functions from labeled
data (motion events with known locations). The learned function would subsume
all the hand-crafted features described in this document. The challenge is
obtaining sufficient labeled training data from realistic deployments.

### 9.5 Information-Theoretic Optimal Weighting

Given $K$ subcarriers and $W$ packets in a window, what is the
information-theoretically optimal edge weight that maximizes the mutual
information between the weight and the presence/absence of motion in the
link's Fresnel zone? This remains an open question and likely depends on
the specific multipath geometry of each link.

---

## References

1. Halperin, D., Hu, W., Sheth, A., & Wetherall, D. (2011). Tool release:
   Gathering 802.11n traces with channel state information. ACM SIGCOMM CCR.

2. Kotaru, M., Joshi, K., Bharadia, D., & Katti, S. (2015). SpotFi: Decimeter
   level localization using WiFi. ACM SIGCOMM.

3. Wang, W., Liu, A. X., Shahzad, M., Ling, K., & Lu, S. (2015). Understanding
   and modeling of WiFi signal based human activity recognition. ACM MobiCom.

4. Li, X., Li, S., Zhang, D., Xiong, J., Wang, Y., & Mei, H. (2016). Dynamic-
   MUSIC: Accurate device-free indoor localization. ACM UbiComp.

5. Qian, K., Wu, C., Yang, Z., Liu, Y., & He, F. (2018). Enabling contactless
   detection of moving humans with dynamic speeds using CSI. ACM TOSN.

6. Jiang, W., et al. (2020). Towards 3D human pose construction using WiFi.
   ACM MobiCom.

7. Yang, Z., Zhou, Z., & Liu, Y. (2013). From RSSI to CSI: Indoor localization
   via channel response. ACM Computing Surveys.

8. Schmidt, R. O. (1986). Multiple emitter location and signal parameter
   estimation. IEEE Transactions on Antennas and Propagation.

9. Roy, R., & Kailath, T. (1989). ESPRIT -- estimation of signal parameters via
   rotational invariance techniques. IEEE Transactions on ASSP.

10. Welford, B. P. (1962). Note on a method for calculating corrected sums of
    squares and products. Technometrics.

---

*Document prepared for the RuView project. Last updated March 2026.*

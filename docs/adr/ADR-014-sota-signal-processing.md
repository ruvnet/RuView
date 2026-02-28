# ADR-014: SOTA Signal Processing Algorithms for WiFi Sensing

## Status
Accepted

## Context

The existing signal processing pipeline (ADR-002) provides foundational CSI processing:
phase unwrapping, FFT-based feature extraction, and variance-based motion detection.
However, the academic state-of-the-art in WiFi sensing (2020-2025) has advanced
significantly beyond these basics. To achieve research-grade accuracy, we need
algorithms grounded in the physics of WiFi signal propagation and human body interaction.

### Current Gaps vs SOTA

| Capability | Current | SOTA Reference |
|-----------|---------|----------------|
| Phase cleaning | Z-score outlier + unwrapping | Conjugate multiplication (SpotFi 2015, IndoTrack 2017) |
| Outlier detection | Z-score | Hampel filter (robust median-based) |
| Breathing detection | Zero-crossing frequency | Fresnel zone model (FarSense 2019, Wi-Sleep 2021) |
| Signal representation | Raw amplitude/phase | CSI spectrogram (time-frequency 2D matrix) |
| Subcarrier usage | All subcarriers equally | Sensitivity-based selection (variance ratio) |
| Motion profiling | Single motion score | Body Velocity Profile / BVP (Widar 3.0 2019) |

## Decision

Implement six SOTA algorithms in the `wifi-densepose-signal` crate as new modules,
each with deterministic tests and no mock data.

### 1. Conjugate Multiplication (CSI Ratio Model)

**What:** Multiply CSI from antenna pair (i,j) as `H_i * conj(H_j)` to cancel
carrier frequency offset (CFO), sampling frequency offset (SFO), and packet
detection delay — all of which corrupt raw phase measurements.

**Why:** Raw CSI phase from commodity hardware (ESP32, Intel 5300) includes
random offsets that change per packet. Conjugate multiplication preserves only
the phase difference caused by the environment (human motion), not the hardware.

**Math:** `CSI_ratio[k] = H_1[k] * conj(H_2[k])` where k is subcarrier index.
The resulting phase `angle(CSI_ratio[k])` reflects only path differences between
the two antenna elements.

**Reference:** SpotFi (SIGCOMM 2015), IndoTrack (MobiCom 2017)

### 2. Hampel Filter

**What:** Replace outliers using running median ± scaled MAD (Median Absolute
Deviation), which is robust to the outliers themselves (unlike mean/std Z-score).

**Why:** WiFi CSI has burst interference, multipath spikes, and hardware glitches
that create outliers. Z-score outlier detection uses mean/std, which are themselves
corrupted by the outliers (masking effect). Hampel filter uses median/MAD, which
resist up to 50% contamination.

**Math:** For window around sample i: `median = med(x[i-w..i+w])`,
`MAD = med(|x[j] - median|)`, `σ_est = 1.4826 * MAD`.
If `|x[i] - median| > t * σ_est`, replace x[i] with median.

**Reference:** Standard DSP technique, used in WiGest (2015), WiDance (2017)

### 3. Fresnel Zone Breathing Model

**What:** Model WiFi signal variation as a function of human chest displacement
crossing Fresnel zone boundaries. The chest moves ~5-10mm during breathing,
which at 5 GHz (λ=60mm) is a significant fraction of the Fresnel zone width.

**Why:** Zero-crossing counting works for strong signals but fails in multipath-rich
environments. The Fresnel model predicts *where* in the signal cycle a breathing
motion should appear based on the TX-RX-body geometry, enabling detection even
with weak signals.

**Math:** Fresnel zone radius at point P: `F_n = sqrt(n * λ * d1 * d2 / (d1 + d2))`.
Signal variation: `ΔΦ = 2π * 2Δd / λ` where Δd is chest displacement.
Expected breathing amplitude: `A = |sin(ΔΦ/2)|`.

**Reference:** FarSense (MobiCom 2019), Wi-Sleep (UbiComp 2021)

### 4. CSI Spectrogram

**What:** Construct a 2D time-frequency matrix by applying sliding-window FFT
(STFT) to the temporal CSI amplitude stream per subcarrier. This reveals how
the frequency content of body motion changes over time.

**Why:** Spectrograms are the standard input to CNN-based activity recognition.
A breathing person shows a ~0.2-0.4 Hz band, walking shows 1-2 Hz, and
stationary environment shows only noise. The 2D structure allows spatial
pattern recognition that 1D features miss.

**Math:** `S[t,f] = |Σ_n x[n] * w[n-t] * exp(-j2πfn)|²`

**Reference:** Used in virtually all CNN-based WiFi sensing papers since 2018

### 5. Subcarrier Sensitivity Selection

**What:** Rank subcarriers by their sensitivity to human motion (variance ratio
between motion and static periods) and select only the top-K for further processing.

**Why:** Not all subcarriers respond equally to body motion. Some are in
multipath nulls, some carry mainly noise. Using all subcarriers dilutes the signal.
Selecting the 10-20 most sensitive subcarriers improves SNR by 6-10 dB.

**Math:** `sensitivity[k] = var_motion(amp[k]) / (var_static(amp[k]) + ε)`.
Select top-K subcarriers by sensitivity score.

**Reference:** WiDance (MobiCom 2017), WiGest (SenSys 2015)

### 6. Body Velocity Profile (BVP)

**What:** Extract velocity distribution of body parts from Doppler shifts across
subcarriers. BVP is a 2D representation (velocity × time) that encodes how
different body parts move at different speeds.

**Why:** BVP is domain-independent — the same velocity profile appears regardless
of room layout, furniture, or AP placement. This makes it the basis for
cross-environment gesture and activity recognition.

**Math:** Apply DFT across time for each subcarrier, then aggregate across
subcarriers: `BVP[v,t] = Σ_k |STFT_k[v,t]|` where v maps to velocity via
`v = f_doppler * λ / 2`.

**Reference:** Widar 3.0 (MobiSys 2019), WiDar (MobiSys 2017)

## Implementation

All algorithms implemented in `wifi-densepose-signal/src/` as new modules:
- `csi_ratio.rs` — Conjugate multiplication
- `hampel.rs` — Hampel filter
- `fresnel.rs` — Fresnel zone breathing model
- `spectrogram.rs` — CSI spectrogram generation
- `subcarrier_selection.rs` — Sensitivity-based selection
- `bvp.rs` — Body Velocity Profile extraction

Each module has:
- Deterministic unit tests with known input/output
- No random data, no mocks
- Documentation with references to source papers
- Integration with existing `CsiData` types

## Consequences

### Positive
- Research-grade signal processing matching 2019-2023 publications
- Physics-grounded algorithms (Fresnel zones, Doppler) not just heuristics
- Cross-environment robustness via BVP and CSI ratio
- CNN-ready features via spectrograms
- Improved SNR via subcarrier selection

### Negative
- Increased computational cost (STFT, complex multiplication per frame)
- Fresnel model requires TX-RX distance estimate (geometry input)
- BVP requires sufficient temporal history (>1 second at 100+ Hz sampling)

## References
- SpotFi: Decimeter Level Localization Using WiFi (SIGCOMM 2015)
- IndoTrack: Device-Free Indoor Human Tracking (MobiCom 2017)
- FarSense: Pushing the Range Limit of WiFi-based Respiration Sensing (MobiCom 2019)
- Widar 3.0: Zero-Effort Cross-Domain Gesture Recognition (MobiSys 2019)
- Wi-Sleep: Contactless Sleep Staging (UbiComp 2021)
- DensePose from WiFi (arXiv 2022, CMU)

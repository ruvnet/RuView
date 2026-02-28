# Remote Vital Sign Sensing: RF, Radar, and Quantum Modalities

Beyond Wi-Fi DensePose-style sensing, there is active research and state-of-the-art (SOTA) work on remotely detecting people and physiological vital signs using RF/EM signals, radar, and quantum/quantum-inspired sensors. Below is a snapshot of current and emerging modalities, with research examples.

---

## RF-Based & Wireless Signal Approaches (Non-Optical)

### 1. RF & Wi-Fi Channel Sensing

Systems analyze perturbations in RF signals (e.g., changes in amplitude/phase) caused by human presence, motion, or micro-movement such as breathing or heartbeat:

- **Wi-Fi CSI (Channel State Information)** can capture micro-movements from chest motion due to respiration and heartbeats by tracking subtle phase shifts in reflected packets. Applied in real-time vital sign monitoring and indoor tracking.
- **RF signal variation** can encode gait, posture and motion biometric features for person identification and pose estimation without cameras or wearables.

These methods are fundamentally passive RF sensing, relying on signal decomposition and ML to extract physiological signatures from ambient communication signals.

---

### 2. Millimeter-Wave & Ultra-Wideband Radar

Active RF systems send high-frequency signals and analyze reflections:

- **Millimeter-wave & FMCW radars** can detect sub-millimeter chest movements due to breathing and heartbeats remotely with high precision.
- Researchers have extended this to **simultaneous multi-person vital sign estimation**, using phased-MIMO radar to isolate and track multiple subjects' breathing and heart rates.
- **Impulse-Radio Ultra-Wideband (IR-UWB)** airborne radar prototypes are being developed for search-and-rescue sensing, extracting respiratory and heartbeat signals amid clutter.

Radar-based approaches are among the most mature non-contact vital sign sensing technologies at range.

---

### 3. Through-Wall & Occluded Sensing

Some advanced radars and RF systems can sense humans behind obstacles by analyzing micro-Doppler signatures and reflectometry:

- Research surveys show **through-wall radar** and deep learning-based RF pose reconstruction for human activity and pose sensing without optical views.

These methods go beyond presence detection to enable coarse body pose and action reconstruction.

---

## Optical & Vision-Based Non-Contact Sensing

### 4. Remote Photoplethysmography (rPPG)

Instead of RF, rPPG uses cameras to infer vital signs by analyzing subtle skin color changes due to blood volume pulses:

- Cameras, including RGB and NIR sensor arrays, can estimate **heart rate, respiration rate, and even oxygenation** without contact.

This is already used in some wellness and telemedicine systems.

---

## Quantum / Quantum-Inspired Approaches

### 5. Quantum Radar and Quantum-Enhanced Remote Sensing

Quantum radar (based on entanglement/correlations or quantum illumination) is under research:

- **Quantum radar** aims to use quantum correlations to outperform classical radar in target detection at short ranges. Early designs have demonstrated proof of concept but remain limited to near-field/short distances — potential for biomedical scanning is discussed.
- **Quantum-inspired computational imaging** and quantum sensors promise enhanced sensitivity, including in foggy, low visibility or internal sensing contexts.

While full quantum remote vital sign sensing (like single-photon quantum radar scanning people's heartbeat) isn't yet operational, quantum sensors — especially atomic magnetometers and NV-centre devices — offer a path toward ultrasensitive biomedical field detection.

### 6. Quantum Biomedical Instrumentation

Parallel research on quantum imaging and quantum sensors aims to push biomedical detection limits:

- Projects are funded to apply **quantum sensing and imaging in smart health environments**, potentially enabling unobtrusive physiological monitoring.
- **Quantum enhancements in MRI** promise higher sensitivity for continuous physiological parameter imaging (temperature, heartbeat signatures) though mostly in controlled medical settings.

These are quantum-sensor-enabled biomedical detection advances rather than direct RF remote sensing; practical deployment for ubiquitous vital sign detection is still emerging.

---

## Modality Comparison

| Modality | Detects | Range | Privacy | Maturity |
|----------|---------|-------|---------|----------|
| Wi-Fi CSI Sensing | presence, respiration, coarse pose | indoor | high (non-visual) | early commercial |
| mmWave / UWB Radar | respiration, heartbeat | meters | medium | mature research, niche products |
| Through-wall RF | pose/activity thru occlusions | short-medium | high | research |
| rPPG (optical) | HR, RR, SpO2 | line-of-sight | low | commercial |
| Quantum Radar (lab) | target detection | very short | high | early research |
| Quantum Sensors (biomedical) | field, magnetic signals | body-proximal | medium | R&D |

---

## Key Insights & State-of-Research

- **RF and radar sensing** are the dominant SOTA methods for non-contact vital sign detection outside optical imaging. These use advanced signal processing and ML to extract micro-movement signatures.
- **Quantum sensors** are showing promise for enhanced biomedical detection at finer scales — especially magnetic and other field sensing — but practical remote vital sign sensing (people at distance) is still largely research.
- **Hybrid approaches** (RF + ML, quantum-inspired imaging) represent emerging research frontiers with potential breakthroughs in sensitivity and privacy.

---

## Relevance to WiFi-DensePose

This project's signal processing pipeline (ADR-014) implements several of the core algorithms used across these modalities:

| WiFi-DensePose Algorithm | Cross-Modality Application |
|--------------------------|---------------------------|
| Conjugate Multiplication (CSI ratio) | Phase sanitization for any multi-antenna RF system |
| Hampel Filter | Outlier rejection in radar and UWB returns |
| Fresnel Zone Model | Breathing detection applicable to mmWave and UWB |
| CSI Spectrogram (STFT) | Time-frequency analysis used in all radar modalities |
| Subcarrier Selection | Channel/frequency selection in OFDM and FMCW systems |
| Body Velocity Profile | Doppler-velocity mapping used in mmWave and through-wall radar |

The algorithmic foundations are shared across modalities — what differs is the carrier frequency, bandwidth, and hardware interface.

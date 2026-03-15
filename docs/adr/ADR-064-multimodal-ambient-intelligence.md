# ADR-064: Multimodal Ambient Intelligence — WiFi CSI + mmWave + Environmental Sensors

**Status:** Proposed
**Date:** 2026-03-15
**Deciders:** @ruvnet
**Related:** ADR-063 (mmWave fusion), ADR-039 (edge intelligence), ADR-042 (CHCI), ADR-029 (RuvSense multistatic), ADR-024 (AETHER contrastive embeddings)

## Context

With ADR-063 we demonstrated real-time fusion of WiFi CSI (ESP32-S3, COM7) and 60 GHz mmWave radar (Seeed MR60BHA2 on ESP32-C6, COM4). The live capture showed:

- **mmWave**: HR 75 bpm, BR 25/min, presence at 52 cm, 1.4 Hz update
- **WiFi CSI**: Channel 5, RSSI -41, 20+ Hz frame rate, through-wall coverage
- **BH1750**: Ambient light 0.0-0.7 lux (room darkness level)

This ADR explores the full spectrum of what becomes possible when these modalities are combined — from immediately practical applications to speculative research directions.

---

## Tier 1: Practical (Build Now)

### 1.1 Intelligent Fall Detection with Zero False Positives

**Current state:** CSI-only fall detection with 15.0 rad/s² threshold (v0.4.3.1).
**With fusion:** mmWave confirms fall via range-velocity signature (sudden height drop + impact deceleration). CSI provides the alert; mmWave provides the confirmation.

```
CSI phase acceleration > 15 rad/s² ─┐
                                     ├─► AND gate + temporal correlation
mmWave: height drop > 50cm in <1s ──┘   → CONFIRMED FALL (call 911)
```

**Impact:** Elderly care facilities spend $34B/year on fall injuries. A $24 sensor node with zero false positives replaces $200/month medical alert wearables that residents forget to wear.

### 1.2 Sleep Quality Monitoring

**Sensors used:** mmWave (BR/HR), CSI (bed occupancy, movement), BH1750 (light)

| Metric | Source | Method |
|--------|--------|--------|
| Sleep onset | CSI motion → still transition | Phase variance drops below threshold |
| Sleep stages | mmWave BR variability | BR 12-20 = light sleep, 6-12 = deep sleep |
| REM detection | mmWave HR variability | HR variability increases during REM |
| Restlessness | CSI motion energy | Counts of motion episodes per hour |
| Room darkness | BH1750 | Correlate light exposure with sleep latency |
| Wake events | CSI + mmWave | Motion + HR spike = awakening |

**Output:** Sleep score (0-100), time in each stage, disturbance log.
**No wearable required.** Works through a mattress.

### 1.3 Occupancy-Aware HVAC and Lighting

**Sensors:** CSI (room-level presence through walls), mmWave (precise count + distance), BH1750 (ambient light)

- CSI detects which rooms are occupied (through walls, whole-floor sensing)
- mmWave counts exact number of people in the sensor's room
- BH1750 measures if lights are on/needed
- System sends MQTT/UDP commands to smart home controllers

**Energy savings:** 20-40% HVAC reduction by not heating/cooling empty rooms.

### 1.4 Bathroom Safety for Elderly

**Sensor placement:** One CSI node outside bathroom (through-wall), one mmWave inside.

- CSI detects person entered bathroom (through-wall)
- mmWave monitors vitals while showering (waterproof enclosure)
- If no movement for > N minutes AND HR drops: alert
- Fall detection in shower (slippery surface = high risk)

### 1.5 Baby/Infant Breathing Monitor

**mmWave at crib-side:** Contactless breathing monitoring at 0.5-1m range.
- BR < 10 or BR = 0 for > 20s: alarm (apnea detection)
- CSI provides room context (parent present? other motion?)
- BH1750 tracks night feeding times (light on/off events)

---

## Tier 2: Advanced (Research Prototype)

### 2.1 Gait Analysis and Fall Risk Prediction

**Method:** CSI tracks walking pattern across the room; mmWave measures stride length and velocity.

| Feature | Source | Clinical Use |
|---------|--------|-------------|
| Gait velocity | mmWave Doppler | < 0.8 m/s = fall risk indicator |
| Stride variability | CSI phase patterns | High variability = cognitive decline marker |
| Turning stability | CSI + mmWave | Difficulty turning = Parkinson's indicator |
| Get-up time | mmWave (sit→stand) | Timed Up and Go (TUG) test, contactless |

**Clinical value:** Gait velocity is called the "sixth vital sign" — it predicts hospitalization, cognitive decline, and mortality. Currently requires a $10,000 GAITRite mat. A $24 sensor node replaces it.

### 2.2 Emotion and Stress Detection via Micro-Vitals

**mmWave at desk:** Continuous HR variability (HRV) monitoring during work.

- **HRV time-domain:** SDNN, RMSSD from beat-to-beat intervals
- **HRV frequency-domain:** LF/HF ratio (sympathetic/parasympathetic balance)
- Low HF power = stress; high HF = relaxation
- CSI detects fidgeting, posture shifts (correlated with stress)
- BH1750 correlates lighting with mood/productivity

**Application:** Smart office that adjusts lighting, temperature, and notification frequency based on detected stress level.

### 2.3 Gesture Recognition as Room Control

**CSI:** Already has DTW template matching gesture classifier (`ruvsense/gesture.rs`).
**mmWave:** Adds range-Doppler micro-gesture detection (hand wave, swipe, circle).

- CSI recognizes gross gestures (wave arm, walk pattern)
- mmWave recognizes fine hand gestures (swipe left/right, push/pull)
- Fused: spatial context (CSI knows where you are) + precise gesture (mmWave knows what your hand did)

**Use case:** Wave at the sensor to turn off lights. Swipe to change music. No voice assistant, no camera, no wearable.

### 2.4 Respiratory Disease Screening

**mmWave BR patterns over days/weeks:**

| Pattern | Indicator |
|---------|-----------|
| BR > 20 at rest, trending up | Possible pneumonia/COVID |
| Periodic breathing (Cheyne-Stokes) | Heart failure |
| Obstructive apnea pattern | Sleep apnea (> 5 events/hour) |
| BR variability decrease | COPD exacerbation |

**CSI adds:** Cough detection (sudden phase disturbance pattern), movement reduction (malaise indicator).

**Longitudinal tracking** via `ruvsense/longitudinal.rs` (Welford stats, biomechanics drift detection) — the system learns your normal breathing pattern and alerts on deviations.

### 2.5 Multi-Room Activity Recognition

**3-6 CSI nodes (through walls) + 1-2 mmWave (key rooms):**

```
Kitchen (CSI):     person detected, high motion → cooking
Living room (mmWave + CSI): 2 people, low motion, HR stable → watching TV
Bedroom (CSI):     person detected, minimal motion → sleeping
Bathroom (CSI):    person entered 3 min ago, still inside → OK
Front door (CSI):  motion pattern = leaving/arriving
```

**Output:** Activity timeline, daily routine deviation alerts, loneliness detection (no visitors in N days).

---

## Tier 3: Speculative (Research Frontier)

### 3.1 Cardiac Arrhythmia Detection

**mmWave at < 1m range:** Beat-to-beat interval extraction from chest wall displacement.

- Atrial fibrillation: irregular R-R intervals (coefficient of variation > 0.1)
- Bradycardia/tachycardia: sustained HR < 60 or > 100
- Premature ventricular contractions: occasional short-long-short patterns

**Challenge:** Requires sub-millimeter displacement resolution. The MR60BHA2 may lack the SNR for single-beat extraction, but clinical-grade 60 GHz modules (Infineon BGT60TR13C) can achieve this.

**CSI role:** Validates that the person is stationary (motion corrupts beat-to-beat analysis).

### 3.2 Blood Pressure Estimation (Contactless)

**Theory:** Pulse Transit Time (PTT) between two body points correlates with blood pressure. With two mmWave sensors at different body positions, PTT can be estimated from the phase difference of reflected chest/wrist signals.

**Feasibility:** Academic papers demonstrate ±10 mmHg accuracy in controlled settings. Far from clinical grade but useful for trending.

### 3.3 RF Tomography — 3D Occupancy Imaging

**Method:** Multiple CSI nodes form a tomographic array. Each TX-RX pair measures signal attenuation. Inverse problem (ISTA L1 solver, already in `ruvsense/tomography.rs`) reconstructs a 3D voxel grid of where absorbers (people) are.

**mmWave adds:** Range-gated targets as sparse priors for the tomographic reconstruction, dramatically reducing the ill-posedness of the inverse problem.

```
CSI tomography (coarse 3D grid, 50cm resolution) ─┐
                                                    ├─► Sparse fusion
mmWave targets (precise range, cm resolution) ─────┘   → 10cm 3D occupancy map
```

### 3.4 Sign Language Recognition

**CSI phase patterns (body/arm movement) + mmWave Doppler (hand micro-movements):**

- CSI captures the gross arm trajectory of each sign
- mmWave captures the finger configuration at the pause point
- AETHER contrastive embeddings (`ADR-024`) learn to map (CSI phase sequence, mmWave Doppler) → sign label
- No camera required — works in the dark, preserves privacy

**Training data:** Record CSI + mmWave while performing signs with a camera as ground truth, then deploy camera-free.

### 3.5 Cognitive Load Estimation

**Multimodal features:**

| Feature | Source | Cognitive Load Indicator |
|---------|--------|------------------------|
| HR increase | mmWave | Sympathetic activation |
| BR irregularity | mmWave | Cognitive interference |
| Posture stiffness | CSI motion variance | Reduced when concentrating |
| Fidgeting frequency | CSI high-freq motion | Increases with frustration |
| Micro-saccade proxy | mmWave head micro-movement | Correlated with attention |

**Application:** Adaptive learning systems that slow down when the student is overloaded. Smart meeting rooms that detect when participants are disengaged.

### 3.6 Drone/Robot Navigation via RF Sensing

**CSI mesh as indoor GPS:** A network of CSI nodes creates a spatial RF fingerprint map. A robot or drone with an ESP32 can localize itself by matching its observed CSI to the map.

**mmWave on the robot:** Obstacle avoidance + human detection (don't collide with people).

**CSI from the environment:** Tells the robot where people are in adjacent rooms (through walls) so it can plan routes that avoid occupied spaces.

### 3.7 Building Structural Health Monitoring

**CSI multipath signature over months/years:**

- The CSI channel response is a fingerprint of the room's geometry
- Subtle shifts in multipath (wall crack propagation, foundation settlement) change the CSI signature
- `ruvsense/cross_room.rs` (environment fingerprinting) tracks these long-term drifts
- mmWave detects surface vibrations (micro-displacement from traffic, wind, seismic)

**Application:** Early warning for structural degradation in bridges, tunnels, old buildings.

### 3.8 Swarm Sensing — Emergent Spatial Awareness

**50+ nodes across a building:**

Each node runs local edge intelligence (ADR-039). The `hive-mind` consensus system (ADR-062) aggregates across nodes. Emergent behaviors:

- **Flow detection:** Track how people move between rooms over time
- **Anomaly detection:** "This hallway usually has 5 people/hour but had 0 today"
- **Emergency routing:** During fire, track which exits are blocked (no movement) vs available
- **Crowd density:** Concert/stadium safety — detect dangerous compression zones through walls

---

## Tier 4: Exotic / Sci-Fi Adjacent

### 4.1 Emotion Contagion Mapping

If multiple people are in a room and the system can estimate individual HR/HRV (via multi-target mmWave + CSI subcarrier clustering), you can detect:

- Physiological synchrony (two people's HR converging = rapport/empathy)
- Stress propagation (one person's stress → others' HR rises)
- "Emotional temperature" of a room

### 4.2 Dream State Detection and Lucid Dream Induction

During REM sleep (detected via mmWave HR variability + CSI minimal body movement):

- Detect REM onset with high confidence
- Trigger a subtle environmental cue (gentle light via smart bulb, barely audible tone)
- The sleeper incorporates the cue into the dream, recognizing it as a dream trigger
- BH1750 confirms room is dark (not a natural awakening)

Based on published lucid dreaming induction research (e.g., LaBerge's MILD technique with external cues).

### 4.3 Plant Growth Monitoring

WiFi signals pass through plant tissue differently based on water content.

- CSI amplitude through a greenhouse changes as plants absorb/release water
- mmWave reflects off leaf surfaces — micro-displacement from growth
- Long-term CSI drift correlates with biomass increase

Academic proof-of-concept: "Sensing Plant Water Content Using WiFi Signals" (2023).

### 4.4 Pet Behavior Analysis

- CSI detects pet movement patterns (different phase signature than humans — lower, faster)
- mmWave detects breathing rate (pets have higher BR than humans)
- System learns pet's daily routine and alerts on deviations (lethargy, pacing, not eating)

### 4.5 Paranormal Investigation Tool

(For the entertainment/hobbyist market)

- CSI detects "unexplained" signal disturbances in empty rooms
- mmWave confirms no physical presence
- System logs "anomalous RF events" with timestamps
- Export as Ghost Hunting report

**Actual explanation:** Temperature changes, HVAC drafts, and EMI cause CSI fluctuations. But it would sell.

---

## Implementation Priority Matrix

| Application | Sensors Needed | Effort | Value | Priority |
|------------|---------------|--------|-------|----------|
| Fall detection (zero false positive) | CSI + mmWave | 1 week | Critical (healthcare) | **P0** |
| Sleep monitoring | mmWave + BH1750 | 2 weeks | High (wellness) | **P1** |
| Occupancy HVAC/lighting | CSI + mmWave | 1 week | High (energy) | **P1** |
| Baby breathing monitor | mmWave | 1 week | Critical (safety) | **P1** |
| Bathroom safety | CSI + mmWave | 1 week | Critical (elderly) | **P1** |
| Gait analysis | CSI + mmWave | 3 weeks | High (clinical) | **P2** |
| Gesture control | CSI + mmWave | 4 weeks | Medium (UX) | **P2** |
| Multi-room activity | CSI mesh + mmWave | 4 weeks | High (elder care) | **P2** |
| Respiratory screening | mmWave longitudinal | 6 weeks | High (health) | **P2** |
| Stress/emotion detection | mmWave HRV + CSI | 6 weeks | Medium (wellness) | **P3** |
| RF tomography | CSI mesh + mmWave | 8 weeks | Medium (research) | **P3** |
| Sign language | CSI + mmWave + ML | 12 weeks | Medium (accessibility) | **P3** |
| Cardiac arrhythmia | High-res mmWave | 12 weeks | High (clinical) | **P3** |
| Swarm sensing | 50+ nodes | 16 weeks | High (safety) | **P3** |

## Decision

Document these possibilities as the product roadmap for the RuView multimodal ambient intelligence platform. Prioritize P0-P1 items (fall detection, sleep, occupancy, baby monitor, bathroom safety) for immediate implementation using the existing hardware (ESP32-S3 + MR60BHA2 + BH1750).

## Consequences

### Positive
- Positions RuView as a platform, not just a WiFi sensing demo
- Each application can ship as a WASM edge module (ADR-040), deployable to existing hardware
- Healthcare applications have clear regulatory paths (fall detection is FDA Class I exempt)
- Most P0-P1 applications require no additional hardware beyond what's already deployed

### Negative
- Clinical applications (arrhythmia, blood pressure) require medical device validation
- Privacy concerns scale with capability — need clear data retention policies
- Some exotic applications may attract scrutiny (surveillance concerns)

### Risk Mitigation
- All processing happens on-device (edge) — no cloud, no recordings by default
- No cameras — signal-based sensing preserves visual privacy
- Open source — users can audit exactly what is sensed and transmitted

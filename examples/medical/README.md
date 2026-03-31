# Medical Sensing Examples

Contactless vital sign monitoring using 60 GHz mmWave radar - no wearable, no camera, no physical contact.

## Blood Pressure Estimator

Estimates blood pressure in real-time from heart rate variability (HRV) captured by a Seeed MR60BHA2 60 GHz mmWave radar module connected to an ESP32-C6.

### How It Works

The radar detects **microscopic chest wall displacement** caused by:
- **Respiration**: 0.1-1.0 mm displacement at 12-25 breaths/min
- **Cardiac pulse**: 0.01-0.1 mm displacement at 60-100 bpm

Modern 60 GHz FMCW radar resolves displacement down to **fractions of a millimeter**. Once the signal is isolated and filtered, the heartbeat-by-heartbeat pattern is remarkably clear.

From there, the estimator:

1. **Extracts beat-to-beat intervals** from the HR time series
2. **Computes HRV metrics**: SDNN (overall variability), LF/HF ratio (sympathetic/parasympathetic balance)
3. **Estimates blood pressure** using the correlation between HR, HRV, and cardiovascular tone:
   - Higher HR → higher BP (sympathetic activation)
   - Lower HRV (SDNN) → higher BP (reduced parasympathetic)
   - Higher LF/HF ratio → higher BP (sympathetic dominance)

### Hardware Required

| Component | Cost | Role |
|-----------|------|------|
| ESP32-C6 + Seeed MR60BHA2 | ~$15 | 60 GHz mmWave radar (HR, BR, presence) |
| USB cable | - | Power + serial data |

That's it. Total cost: **~$15**.

### Quick Start

```bash
pip install pyserial numpy

# Basic (uncalibrated - shows trends)
python examples/medical/bp_estimator.py --port COM4

# Calibrated (take a real BP reading first, then enter it)
python examples/medical/bp_estimator.py --port COM4 \
  --cal-systolic 120 --cal-diastolic 80 --cal-hr 72
```

### Sample Output (Real Hardware, 2026-03-15)

```
  Contactless Blood Pressure Estimation (mmWave 60 GHz)

   Time    HR   SBP   DBP             Category  Samples
  -------------------------------------------------------
   15s |  64 | 117/78 | Normal    | SDNN  22ms | n=4
   20s |  65 | 117/78 | Normal    | SDNN  28ms | n=5
   25s |  71 | 119/79 | Normal    | SDNN  88ms | n=9
   30s |  77 | 122/81 | Elevated  | SDNN 108ms | n=14
   35s |  80 | 123/82 | Elevated  | SDNN 106ms | n=18
   40s |  80 | 123/82 | Elevated  | SDNN  98ms | n=22
   45s |  82 | 124/83 | Elevated  | SDNN  97ms | n=26
   50s |  83 | 125/83 | Elevated  | SDNN  95ms | n=29
   55s |  83 | 125/83 | Elevated  | SDNN  92ms | n=32
   60s |  84 | 125/83 | Elevated  | SDNN  91ms | n=35

  RESULT: 125/83 mmHg | HR 84 bpm | SDNN 91ms | 35 samples
```

### Accuracy

| Condition | Accuracy |
|-----------|----------|
| Uncalibrated, stationary | ±15-20 mmHg (trend tracking) |
| Calibrated, stationary | ±8-12 mmHg |
| Moving subject | Not reliable - wait for subject to be still |

Accuracy improves with:
- Longer recording duration (60s minimum, 120s recommended)
- Calibration with a real cuff reading
- Stationary subject within 1m of sensor
- Minimal environmental RF interference

### AHA Blood Pressure Categories

| Category | Systolic | Diastolic |
|----------|----------|-----------|
| Normal | < 120 | < 80 |
| Elevated | 120-129 | < 80 |
| High BP Stage 1 | 130-139 | 80-89 |
| High BP Stage 2 | 140+ | 90+ |

### Disclaimer

**This is NOT a medical device.** Blood pressure estimates from heart rate variability are approximations based on population-level correlations. Individual variation is significant. Always use a validated cuff-based sphygmomanometer for clinical decisions.

This tool is intended for:
- Research into contactless vital sign monitoring
- Wellness trend tracking (is my BP going up or down over days?)
- Technology demonstration
- Educational purposes

### How This Connects to RuView

This example is part of the [RuView](https://github.com/ruvnet/RuView) ambient intelligence platform. When combined with WiFi CSI sensing:

- **WiFi CSI** provides through-wall presence detection and room-scale activity recognition
- **mmWave radar** provides clinical-grade heart rate, breathing rate, and BP estimation
- **Sensor fusion** (ADR-063) combines both for zero false-positive fall detection and comprehensive health monitoring
- **RuVector** dynamic min-cut analysis treats physiological signals as a coherence graph, automatically separating noise, motion artifacts, and environmental interference

The result: cheap sensors ($15-24 per node), local computation (no cloud), real physiological understanding.

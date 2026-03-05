# ADR-048: Adaptive CSI Activity Classifier

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-05 |
| Deciders | ruv |
| Depends on | ADR-024 (AETHER Embeddings), ADR-039 (Edge Processing), ADR-045 (AMOLED Display) |

## Context

WiFi-based activity classification using ESP32 Channel State Information (CSI) relies on hand-tuned thresholds to distinguish between activity states (absent, present_still, present_moving, active). These static thresholds are brittle — they don't account for:

- **Environment-specific signal patterns**: Room geometry, furniture, wall materials, and ESP32 placement all affect how CSI signals respond to human activity.
- **Temporal noise characteristics**: Real ESP32 CSI data at ~10 FPS has significant frame-to-frame jitter that causes classification to jump between states.
- **Vital signs estimation noise**: Heart rate and breathing rate estimates from Goertzel filter banks produce large swings (50+ BPM frame-to-frame) at low confidence levels.

The existing threshold-based approach produces noisy, unstable classifications that degrade the user experience in the Observatory visualization and the main dashboard.

## Decision

### 1. Three-Stage Signal Smoothing Pipeline

All CSI-derived metrics pass through a three-stage pipeline before reaching the UI:

#### Stage 1: Adaptive Baseline Subtraction
- EMA with α=0.003 (~30s time constant) tracks the "quiet room" noise floor
- Only updates during low-motion periods to avoid inflating baseline during activity
- 50-frame warm-up period for initial baseline learning
- Subtracts 70% of baseline from raw motion score to remove environmental drift

#### Stage 2: EMA + Median Filtering
- **Motion score**: Blended from 4 signals (temporal diff 40%, variance 20%, motion band power 25%, change points 15%), then EMA-smoothed with α=0.15
- **Vital signs**: 21-frame sliding window → trimmed mean (drop top/bottom 25%) → EMA with α=0.02 (~5s time constant)
- **Dead-band**: HR won't update unless trimmed mean differs by >2 BPM; BR needs >0.5 BPM
- **Outlier rejection**: HR jumps >8 BPM/frame and BR jumps >2 BPM/frame are discarded

#### Stage 3: Hysteresis Debounce
- Activity state transitions require 4 consecutive frames (~0.4s) of agreement before committing
- Prevents rapid flickering between states
- Independent candidate tracking resets on new direction changes

### 2. Adaptive Classifier Module (`adaptive_classifier.rs`)

A Rust-native environment-tuned classifier that learns from labeled JSONL recordings:

#### Feature Extraction (15 features)
| # | Feature | Source | Discriminative Power |
|---|---------|--------|---------------------|
| 0 | variance | Server | Medium — temporal CSI spread |
| 1 | motion_band_power | Server | Medium — high-frequency subcarrier energy |
| 2 | breathing_band_power | Server | Low — respiratory band energy |
| 3 | spectral_power | Server | Low — mean squared amplitude |
| 4 | dominant_freq_hz | Server | Low — peak subcarrier index |
| 5 | change_points | Server | Medium — threshold crossing count |
| 6 | mean_rssi | Server | Low — received signal strength |
| 7 | amp_mean | Subcarrier | Medium — mean amplitude across 56 subcarriers |
| 8 | amp_std | Subcarrier | **High** — amplitude spread (motion increases spread) |
| 9 | amp_skew | Subcarrier | Medium — asymmetry of amplitude distribution |
| 10 | amp_kurt | Subcarrier | **High** — peakedness (presence creates peaks) |
| 11 | amp_iqr | Subcarrier | Medium — inter-quartile range |
| 12 | amp_entropy | Subcarrier | **High** — spectral entropy (motion increases disorder) |
| 13 | amp_max | Subcarrier | Medium — peak amplitude value |
| 14 | amp_range | Subcarrier | Medium — amplitude dynamic range |

#### Training Algorithm
- **Multiclass logistic regression** with softmax output
- **Mini-batch SGD** (batch size 32, 200 epochs, linear learning rate decay)
- **Z-score normalisation** using global mean/stddev computed from all training data
- Per-class statistics (mean, stddev) stored for Mahalanobis distance fallback
- Deterministic shuffling (LCG PRNG, seed 42) for reproducible results

#### Training Data Pipeline
1. Record labeled CSI sessions via `POST /api/v1/recording/start {"id":"train_<label>"}`
2. Filename-based label assignment: `*empty*`→absent, `*still*`→present_still, `*walking*`→present_moving, `*active*`→active
3. Train via `POST /api/v1/adaptive/train`
4. Model saved to `data/adaptive_model.json`, auto-loaded on server restart

#### Inference Pipeline
1. Extract 15-feature vector from current CSI frame
2. Z-score normalise using stored global mean/stddev
3. Compute softmax probabilities across 4 classes
4. Blend adaptive model confidence (70%) with smoothed threshold confidence (30%)
5. Override classification only when adaptive model is loaded

### 3. API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/adaptive/train` | Train classifier from `train_*` recordings |
| GET | `/api/v1/adaptive/status` | Check model status, accuracy, class stats |
| POST | `/api/v1/adaptive/unload` | Revert to threshold-based classification |
| POST | `/api/v1/recording/start` | Start recording CSI frames (JSONL) |
| POST | `/api/v1/recording/stop` | Stop recording |
| GET | `/api/v1/recording/list` | List available recordings |

### 4. Vital Signs Smoothing

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Median window | 21 frames | ~2s of history, robust to transients |
| Aggregation | Trimmed mean (middle 50%) | More stable than pure median, less noisy than raw mean |
| EMA alpha | 0.02 | ~5s time constant — readings change very slowly |
| HR dead-band | ±2 BPM | Prevents display creep from micro-fluctuations |
| BR dead-band | ±0.5 BPM | Same for breathing rate |
| HR max jump | 8 BPM/frame | Outlier rejection threshold |
| BR max jump | 2 BPM/frame | Outlier rejection threshold |

## Consequences

### Benefits
- **Stable UI**: Vital signs readings hold steady for 5-10+ seconds instead of jumping every frame
- **Environment adaptation**: Classifier learns the specific room's signal characteristics
- **Graceful fallback**: If no adaptive model is loaded, threshold-based classification with smoothing still works
- **No external dependencies**: Pure Rust implementation, no Python/ML frameworks needed
- **Fast training**: 3,000+ frames train in <1 second on commodity hardware
- **Portable model**: JSON serialisation, loadable on any platform

### Limitations
- **Single-link**: With one ESP32, the feature space is limited. Multi-AP setups (ADR-029) would dramatically improve separability.
- **No temporal features**: Current frame-level classification doesn't use sequence models (LSTM/Transformer). Could be added later.
- **Label quality**: Training accuracy depends heavily on recording quality (distinct activities, actual room vacancy for "empty").
- **Linear classifier**: Logistic regression may underfit non-linear decision boundaries. Could upgrade to 2-layer MLP if needed.

### Future Work
- **Online learning**: Continuously update model weights from user corrections
- **Sequence models**: Use sliding window of N frames as input for temporal pattern recognition
- **Contrastive pretraining**: Leverage ADR-024 AETHER embeddings for self-supervised feature learning
- **Multi-AP fusion**: Use ADR-029 multistatic sensing for richer feature space
- **Edge deployment**: Export learned thresholds to ESP32 firmware (ADR-039 Tier 2) for on-device classification

## Files

| File | Purpose |
|------|---------|
| `crates/wifi-densepose-sensing-server/src/adaptive_classifier.rs` | Adaptive classifier module (feature extraction, training, inference) |
| `crates/wifi-densepose-sensing-server/src/main.rs` | Smoothing pipeline, API endpoints, integration |
| `ui/observatory/js/hud-controller.js` | UI-side lerp smoothing (4% per frame) |
| `data/adaptive_model.json` | Trained model (auto-created by training endpoint) |
| `data/recordings/train_*.jsonl` | Labeled training recordings |

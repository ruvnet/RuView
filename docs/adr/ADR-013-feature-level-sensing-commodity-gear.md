# ADR-013: Feature-Level Sensing on Commodity Gear (Option 3)

## Status
Proposed

## Date
2026-02-28

## Context

### Not Everyone Can Deploy Custom Hardware

ADR-012 specifies an ESP32 CSI mesh that provides real CSI data. However, it requires:
- Purchasing ESP32 boards
- Flashing custom firmware
- ESP-IDF toolchain installation
- Physical placement of nodes

For many users - especially those evaluating WiFi-DensePose or deploying in managed environments - modifying hardware is not an option. We need a sensing path that works with **existing, unmodified consumer WiFi gear**.

### What Commodity Hardware Exposes

Standard WiFi drivers and tools expose several metrics without custom firmware:

| Signal | Source | Availability | Sampling Rate |
|--------|--------|-------------|---------------|
| RSSI (Received Signal Strength) | `iwconfig`, `iw`, NetworkManager | Universal | 1-10 Hz |
| Noise floor | `iw dev wlan0 survey dump` | Most Linux drivers | ~1 Hz |
| Link quality | `/proc/net/wireless` | Linux | 1-10 Hz |
| MCS index / PHY rate | `iw dev wlan0 link` | Most drivers | Per-packet |
| TX/RX bytes | `/sys/class/net/wlan0/statistics/` | Universal | Continuous |
| Retry count | `iw dev wlan0 station dump` | Most drivers | ~1 Hz |
| Beacon interval timing | `iw dev wlan0 scan dump` | Universal | Per-scan |
| Channel utilization | `iw dev wlan0 survey dump` | Most drivers | ~1 Hz |

**RSSI is the primary signal**. It varies when humans move through the propagation path between any transmitter-receiver pair. Research confirms RSSI-based sensing for:
- Presence detection (single receiver, threshold on variance)
- Device-free motion detection (RSSI variance increases with movement)
- Coarse room-level localization (multi-receiver RSSI fingerprinting)
- Breathing detection (specialized setups, marginal quality)

### Research Support

- **RSSI-based presence**: Youssef et al. (2007) demonstrated device-free passive detection using RSSI from multiple receivers with >90% accuracy.
- **RSSI breathing**: Abdelnasser et al. (2015) showed respiration detection via RSSI variance in controlled settings with ~85% accuracy using 4+ receivers.
- **Device-free tracking**: Multiple receivers with RSSI fingerprinting achieve room-level (3-5m) accuracy.

## Decision

We will implement a Feature-Level Sensing module that extracts motion, presence, and coarse activity information from standard WiFi metrics available on any Linux machine without hardware modification.

### Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│              Feature-Level Sensing Pipeline                           │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  Data Sources (any Linux WiFi device):                               │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌──────────────┐              │
│  │ RSSI    │ │ Noise   │ │ Link    │ │ Packet Stats │              │
│  │ Stream  │ │ Floor   │ │ Quality │ │ (TX/RX/Retry)│              │
│  └────┬────┘ └────┬────┘ └────┬────┘ └──────┬───────┘              │
│       │           │           │              │                       │
│       └───────────┴───────────┴──────────────┘                       │
│                           │                                          │
│                           ▼                                          │
│  ┌────────────────────────────────────────────────┐                  │
│  │           Feature Extraction Engine             │                  │
│  │                                                 │                  │
│  │  1. Rolling statistics (mean, var, skew, kurt)  │                  │
│  │  2. Spectral features (FFT of RSSI time series) │                  │
│  │  3. Change-point detection (CUSUM, PELT)        │                  │
│  │  4. Cross-receiver correlation                   │                  │
│  │  5. Packet timing jitter analysis               │                  │
│  └────────────────────────┬───────────────────────┘                  │
│                           │                                          │
│                           ▼                                          │
│  ┌────────────────────────────────────────────────┐                  │
│  │          Classification / Decision              │                  │
│  │                                                 │                  │
│  │  • Presence: RSSI variance > threshold          │                  │
│  │  • Motion class: spectral peak frequency        │                  │
│  │  • Occupancy change: change-point event         │                  │
│  │  • Confidence: cross-receiver agreement         │                  │
│  └────────────────────────┬───────────────────────┘                  │
│                           │                                          │
│                           ▼                                          │
│  ┌────────────────────────────────────────────────┐                  │
│  │         Output: Presence/Motion Events          │                  │
│  │                                                 │                  │
│  │  { "timestamp": "...",                          │                  │
│  │    "presence": true,                            │                  │
│  │    "motion_level": "active",                    │                  │
│  │    "confidence": 0.87,                          │                  │
│  │    "receivers_agreeing": 3,                     │                  │
│  │    "rssi_variance": 4.2 }                       │                  │
│  └────────────────────────────────────────────────┘                  │
└──────────────────────────────────────────────────────────────────────┘
```

### Feature Extraction Specification

```python
class RssiFeatureExtractor:
    """Extract sensing features from RSSI and link statistics.

    No custom hardware required. Works with any WiFi interface
    that exposes standard Linux wireless statistics.
    """

    def __init__(self, config: FeatureSensingConfig):
        self.window_size = config.window_size  # 30 seconds
        self.sampling_rate = config.sampling_rate  # 10 Hz
        self.rssi_buffer = deque(maxlen=self.window_size * self.sampling_rate)
        self.noise_buffer = deque(maxlen=self.window_size * self.sampling_rate)

    def extract_features(self) -> FeatureVector:
        rssi_array = np.array(self.rssi_buffer)

        return FeatureVector(
            # Time-domain statistics
            rssi_mean=np.mean(rssi_array),
            rssi_variance=np.var(rssi_array),
            rssi_skewness=scipy.stats.skew(rssi_array),
            rssi_kurtosis=scipy.stats.kurtosis(rssi_array),
            rssi_range=np.ptp(rssi_array),
            rssi_iqr=np.subtract(*np.percentile(rssi_array, [75, 25])),

            # Spectral features (FFT of RSSI time series)
            spectral_energy=self._spectral_energy(rssi_array),
            dominant_frequency=self._dominant_freq(rssi_array),
            breathing_band_power=self._band_power(rssi_array, 0.1, 0.5),  # Hz
            motion_band_power=self._band_power(rssi_array, 0.5, 3.0),    # Hz

            # Change-point features
            num_change_points=self._cusum_changes(rssi_array),
            max_step_magnitude=self._max_step(rssi_array),

            # Noise floor features (environment stability)
            noise_mean=np.mean(np.array(self.noise_buffer)),
            snr_estimate=np.mean(rssi_array) - np.mean(np.array(self.noise_buffer)),
        )

    def _spectral_energy(self, rssi: np.ndarray) -> float:
        """Total spectral energy excluding DC component."""
        spectrum = np.abs(scipy.fft.rfft(rssi - np.mean(rssi)))
        return float(np.sum(spectrum[1:] ** 2))

    def _dominant_freq(self, rssi: np.ndarray) -> float:
        """Dominant frequency in RSSI time series."""
        spectrum = np.abs(scipy.fft.rfft(rssi - np.mean(rssi)))
        freqs = scipy.fft.rfftfreq(len(rssi), d=1.0/self.sampling_rate)
        return float(freqs[np.argmax(spectrum[1:]) + 1])

    def _band_power(self, rssi: np.ndarray, low_hz: float, high_hz: float) -> float:
        """Power in a specific frequency band."""
        spectrum = np.abs(scipy.fft.rfft(rssi - np.mean(rssi))) ** 2
        freqs = scipy.fft.rfftfreq(len(rssi), d=1.0/self.sampling_rate)
        mask = (freqs >= low_hz) & (freqs <= high_hz)
        return float(np.sum(spectrum[mask]))

    def _cusum_changes(self, rssi: np.ndarray) -> int:
        """Count change points using CUSUM algorithm."""
        mean = np.mean(rssi)
        cusum_pos = np.zeros_like(rssi)
        cusum_neg = np.zeros_like(rssi)
        threshold = 3.0 * np.std(rssi)
        changes = 0
        for i in range(1, len(rssi)):
            cusum_pos[i] = max(0, cusum_pos[i-1] + rssi[i] - mean - 0.5)
            cusum_neg[i] = max(0, cusum_neg[i-1] - rssi[i] + mean - 0.5)
            if cusum_pos[i] > threshold or cusum_neg[i] > threshold:
                changes += 1
                cusum_pos[i] = 0
                cusum_neg[i] = 0
        return changes
```

### Data Collection (No Root Required)

```python
class LinuxWifiCollector:
    """Collect WiFi statistics from standard Linux interfaces.

    No root required for most operations.
    No custom drivers or firmware.
    Works with NetworkManager, wpa_supplicant, or raw iw.
    """

    def __init__(self, interface: str = "wlan0"):
        self.interface = interface

    def get_rssi(self) -> float:
        """Get current RSSI from connected AP."""
        # Method 1: /proc/net/wireless (no root)
        with open("/proc/net/wireless") as f:
            for line in f:
                if self.interface in line:
                    parts = line.split()
                    return float(parts[3].rstrip('.'))

        # Method 2: iw (no root for own station)
        result = subprocess.run(
            ["iw", "dev", self.interface, "link"],
            capture_output=True, text=True
        )
        for line in result.stdout.split('\n'):
            if 'signal:' in line:
                return float(line.split(':')[1].strip().split()[0])

        raise SensingError(f"Cannot read RSSI from {self.interface}")

    def get_noise_floor(self) -> float:
        """Get noise floor estimate."""
        result = subprocess.run(
            ["iw", "dev", self.interface, "survey", "dump"],
            capture_output=True, text=True
        )
        for line in result.stdout.split('\n'):
            if 'noise:' in line:
                return float(line.split(':')[1].strip().split()[0])
        return -95.0  # Default noise floor estimate

    def get_link_stats(self) -> dict:
        """Get link quality statistics."""
        result = subprocess.run(
            ["iw", "dev", self.interface, "station", "dump"],
            capture_output=True, text=True
        )
        stats = {}
        for line in result.stdout.split('\n'):
            if 'tx bytes:' in line:
                stats['tx_bytes'] = int(line.split(':')[1].strip())
            elif 'rx bytes:' in line:
                stats['rx_bytes'] = int(line.split(':')[1].strip())
            elif 'tx retries:' in line:
                stats['tx_retries'] = int(line.split(':')[1].strip())
            elif 'signal:' in line:
                stats['signal'] = float(line.split(':')[1].strip().split()[0])
        return stats
```

### Classification Rules

```python
class PresenceClassifier:
    """Rule-based presence and motion classifier.

    Uses simple, interpretable rules rather than ML to ensure
    transparency and debuggability.
    """

    def __init__(self, config: ClassifierConfig):
        self.variance_threshold = config.variance_threshold  # 2.0 dBm²
        self.motion_threshold = config.motion_threshold      # 5.0 dBm²
        self.spectral_threshold = config.spectral_threshold  # 10.0
        self.confidence_min_receivers = config.min_receivers  # 2

    def classify(self, features: FeatureVector,
                 multi_receiver: list[FeatureVector] = None) -> SensingResult:

        # Presence: RSSI variance exceeds empty-room baseline
        presence = features.rssi_variance > self.variance_threshold

        # Motion level
        if features.rssi_variance > self.motion_threshold:
            motion = MotionLevel.ACTIVE
        elif features.rssi_variance > self.variance_threshold:
            motion = MotionLevel.PRESENT_STILL
        else:
            motion = MotionLevel.ABSENT

        # Confidence from spectral energy and receiver agreement
        spectral_conf = min(1.0, features.spectral_energy / self.spectral_threshold)
        if multi_receiver:
            agreeing = sum(1 for f in multi_receiver
                          if (f.rssi_variance > self.variance_threshold) == presence)
            receiver_conf = agreeing / len(multi_receiver)
        else:
            receiver_conf = 0.5  # Single receiver = lower confidence

        confidence = 0.6 * spectral_conf + 0.4 * receiver_conf

        return SensingResult(
            presence=presence,
            motion_level=motion,
            confidence=confidence,
            dominant_frequency=features.dominant_frequency,
            breathing_band_power=features.breathing_band_power,
        )
```

### Capability Matrix (Honest Assessment)

| Capability | Single Receiver | 3 Receivers | 6 Receivers | Accuracy |
|-----------|----------------|-------------|-------------|----------|
| Binary presence | Yes | Yes | Yes | 90-95% |
| Coarse motion (still/moving) | Yes | Yes | Yes | 85-90% |
| Room-level location | No | Marginal | Yes | 70-80% |
| Person count | No | Marginal | Marginal | 50-70% |
| Activity class (walk/sit/stand) | Marginal | Marginal | Yes | 60-75% |
| Respiration detection | No | Marginal | Marginal | 40-60% |
| Heartbeat | No | No | No | N/A |
| Body pose | No | No | No | N/A |

**Bottom line**: Feature-level sensing on commodity gear does presence and motion well. It does NOT do pose estimation, heartbeat, or reliable respiration. Any claim otherwise would be dishonest.

### Decision Matrix: Option 2 (ESP32) vs Option 3 (Commodity)

| Factor | ESP32 CSI (ADR-012) | Commodity (ADR-013) |
|--------|---------------------|---------------------|
| Headline capability | Respiration + motion | Presence + coarse motion |
| Hardware cost | $54 (3-node kit) | $0 (existing gear) |
| Setup time | 2-4 hours | 15 minutes |
| Technical barrier | Medium (firmware flash) | Low (pip install) |
| Data quality | Real CSI (amplitude + phase) | RSSI only |
| Multi-person | Marginal | Poor |
| Pose estimation | Marginal | No |
| Reproducibility | High (controlled hardware) | Medium (varies by hardware) |
| Public credibility | High (real CSI artifact) | Medium (RSSI is "obvious") |

### Proof Bundle for Commodity Sensing

```
v1/data/proof/commodity/
├── rssi_capture_30sec.json         # 30 seconds of RSSI from 3 receivers
├── rssi_capture_meta.json          # Hardware: Intel AX200, Router: TP-Link AX1800
├── scenario.txt                    # "Person walks through room at t=10s, sits at t=20s"
├── expected_features.json          # Feature extraction output
├── expected_classification.json    # Classification output
├── expected_features.sha256        # Verification hash
└── verify_commodity.py             # One-command verification
```

### Integration with WiFi-DensePose Pipeline

The commodity sensing module outputs the same `SensingResult` type as the CSI pipeline, allowing graceful degradation:

```python
class SensingBackend(Protocol):
    """Common interface for all sensing backends."""

    def get_features(self) -> FeatureVector: ...
    def get_capabilities(self) -> set[Capability]: ...

class CsiBackend(SensingBackend):
    """Full CSI pipeline (ESP32 or research NIC)."""
    def get_capabilities(self):
        return {Capability.PRESENCE, Capability.MOTION, Capability.RESPIRATION,
                Capability.LOCATION, Capability.POSE}

class CommodityBackend(SensingBackend):
    """RSSI-only commodity hardware."""
    def get_capabilities(self):
        return {Capability.PRESENCE, Capability.MOTION}
```

## Consequences

### Positive
- **Zero-cost entry**: Works with existing WiFi hardware
- **15-minute setup**: `pip install wifi-densepose && wdp sense --interface wlan0`
- **Broad adoption**: Any Linux laptop, Pi, or phone can participate
- **Honest capability reporting**: `get_capabilities()` tells users exactly what works
- **Complements ESP32**: Users start with commodity, upgrade to ESP32 for more capability
- **No mock data**: Real RSSI from real hardware, deterministic pipeline

### Negative
- **Limited capability**: No pose, no heartbeat, marginal respiration
- **Hardware variability**: RSSI calibration differs across chipsets
- **Environmental sensitivity**: Commodity RSSI is more affected by interference than CSI
- **Not a "pose estimation" demo**: This module honestly cannot do what the project name implies
- **Lower credibility ceiling**: RSSI sensing is well-known; less impressive than CSI

## References

- [Youssef et al. - Challenges in Device-Free Passive Localization](https://doi.org/10.1145/1287853.1287880)
- [Device-Free WiFi Sensing Survey](https://arxiv.org/abs/1901.09683)
- [RSSI-based Breathing Detection](https://ieeexplore.ieee.org/document/7127688)
- [Linux Wireless Tools](https://wireless.wiki.kernel.org/en/users/documentation/iw)
- ADR-011: Python Proof-of-Reality and Mock Elimination
- ADR-012: ESP32 CSI Sensor Mesh

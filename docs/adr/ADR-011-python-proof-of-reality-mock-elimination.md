# ADR-011: Python Proof-of-Reality and Mock Elimination

## Status
Proposed (URGENT)

## Date
2026-02-28

## Context

### The Credibility Problem

The WiFi-DensePose Python codebase contains real, mathematically sound signal processing (FFT, phase unwrapping, Doppler extraction, correlation features) alongside mock/placeholder code that fatally undermines credibility. External reviewers who encounter **any** mock path in the default execution flow conclude the entire system is synthetic. This is not a technical problem - it is a perception problem with technical root causes.

### Specific Mock/Placeholder Inventory

The following code paths produce fake data **in the default configuration** or are easily mistaken for indicating fake functionality:

#### Critical Severity (produces fake output on default path)

| File | Line | Issue | Impact |
|------|------|-------|--------|
| `v1/src/core/csi_processor.py` | 390 | `doppler_shift = np.random.rand(10)  # Placeholder` | **Real feature extractor returns random Doppler** - kills credibility of entire feature pipeline |
| `v1/src/hardware/csi_extractor.py` | 83-84 | `amplitude = np.random.rand(...)` in CSI extraction fallback | Random data silently substituted when parsing fails |
| `v1/src/hardware/csi_extractor.py` | 129-135 | `_parse_atheros()` returns `np.random.rand()` with comment "placeholder implementation" | Named as if it parses real data, actually random |
| `v1/src/hardware/router_interface.py` | 211-212 | `np.random.rand(3, 56)` in fallback path | Silent random fallback |
| `v1/src/services/pose_service.py` | 431 | `mock_csi = np.random.randn(64, 56, 3)  # Mock CSI data` | Mock CSI in production code path |
| `v1/src/services/pose_service.py` | 293-356 | `_generate_mock_poses()` with `random.randint` throughout | Entire mock pose generator in service layer |
| `v1/src/services/pose_service.py` | 489-607 | Multiple `random.randint` for occupancy, historical data | Fake statistics that look real in API responses |
| `v1/src/api/dependencies.py` | 82, 408 | "return a mock user for development" | Auth bypass in default path |

#### Moderate Severity (mock gated behind flags but confusing)

| File | Line | Issue |
|------|------|-------|
| `v1/src/config/settings.py` | 144-145 | `mock_hardware=False`, `mock_pose_data=False` defaults - correct, but mock infrastructure exists |
| `v1/src/core/router_interface.py` | 27-300 | 270+ lines of mock data generation infrastructure in production code |
| `v1/src/services/pose_service.py` | 84-88 | Silent conditional: `if not self.settings.mock_pose_data` with no logging of real-mode |
| `v1/src/services/hardware_service.py` | 72-375 | Interleaved mock/real paths throughout |

#### Low Severity (placeholders/TODOs)

| File | Line | Issue |
|------|------|-------|
| `v1/src/core/router_interface.py` | 198 | "Collect real CSI data from router (placeholder implementation)" |
| `v1/src/api/routers/health.py` | 170-171 | `uptime_seconds = 0.0  # TODO` |
| `v1/src/services/pose_service.py` | 739 | `"uptime_seconds": 0.0  # TODO` |

### Root Cause Analysis

1. **No separation between mock and real**: Mock generators live in the same modules as real processors. A reviewer reading `csi_processor.py` hits `np.random.rand(10)` at line 390 and stops trusting the 400 lines of real signal processing above it.

2. **Silent fallbacks**: When real hardware isn't available, the system silently falls back to random data instead of failing loudly. This means the default `docker compose up` produces plausible-looking but entirely fake results.

3. **No proof artifact**: There is no shipped CSI capture file, no expected output hash, no way for a reviewer to verify that the pipeline produces deterministic results from real input.

4. **Build environment fragility**: The `Dockerfile` references `requirements.txt` which doesn't exist as a standalone file. The `setup.py` hardcodes 87 dependencies. ONNX Runtime and BLAS are not in the container. A `docker build` may or may not succeed depending on the machine.

5. **No CI verification**: No GitHub Actions workflow runs the pipeline on a real or deterministic input and verifies the output.

## Decision

We will eliminate the credibility gap through five concrete changes:

### 1. Eliminate All Silent Mock Fallbacks (HARD FAIL)

**Every path that currently returns `np.random.rand()` will either be replaced with real computation or will raise an explicit error.**

```python
# BEFORE (csi_processor.py:390)
doppler_shift = np.random.rand(10)  # Placeholder

# AFTER
def _extract_doppler_features(self, csi_data: CSIData) -> tuple:
    """Extract Doppler and frequency domain features from CSI temporal history."""
    if len(self.csi_history) < 2:
        # Not enough history for temporal analysis - return zeros, not random
        doppler_shift = np.zeros(self.window_size)
        psd = np.abs(scipy.fft.fft(csi_data.amplitude.flatten(), n=128))**2
        return doppler_shift, psd

    # Real Doppler extraction from temporal CSI differences
    history_array = np.array([h.amplitude for h in self.get_recent_history(self.window_size)])
    # Compute phase differences over time (proportional to Doppler shift)
    temporal_phase_diff = np.diff(np.angle(history_array + 1j * np.zeros_like(history_array)), axis=0)
    # Average across antennas, FFT across time for Doppler spectrum
    doppler_spectrum = np.abs(scipy.fft.fft(temporal_phase_diff.mean(axis=1), axis=0))
    doppler_shift = doppler_spectrum.mean(axis=1)

    psd = np.abs(scipy.fft.fft(csi_data.amplitude.flatten(), n=128))**2
    return doppler_shift, psd
```

```python
# BEFORE (csi_extractor.py:129-135)
def _parse_atheros(self, raw_data):
    """Parse Atheros CSI format (placeholder implementation)."""
    # For now, return mock data for testing
    return CSIData(amplitude=np.random.rand(3, 56), ...)

# AFTER
def _parse_atheros(self, raw_data: bytes) -> CSIData:
    """Parse Atheros CSI Tool format.

    Format: https://dhalperi.github.io/linux-80211n-csitool/
    """
    if len(raw_data) < 25:  # Minimum Atheros CSI header
        raise CSIExtractionError(
            f"Atheros CSI data too short ({len(raw_data)} bytes). "
            "Expected real CSI capture from Atheros-based NIC. "
            "See docs/hardware-setup.md for capture instructions."
        )
    # Parse actual Atheros binary format
    # ... real parsing implementation ...
```

### 2. Isolate Mock Infrastructure Behind Explicit Flag with Banner

**All mock code moves to a dedicated module. Default execution NEVER touches mock paths.**

```
v1/src/
├── core/
│   ├── csi_processor.py        # Real processing only
│   └── router_interface.py     # Real hardware interface only
├── testing/                    # NEW: isolated mock module
│   ├── __init__.py
│   ├── mock_csi_generator.py   # Mock CSI generation (moved from router_interface)
│   ├── mock_pose_generator.py  # Mock poses (moved from pose_service)
│   └── fixtures/               # Test fixtures, not production paths
│       ├── sample_csi_capture.bin  # Real captured CSI data (tiny sample)
│       └── expected_output.json    # Expected pipeline output for sample
```

**Runtime enforcement:**
```python
import os
import sys

MOCK_MODE = os.environ.get("WIFI_DENSEPOSE_MOCK", "").lower() == "true"

if MOCK_MODE:
    # Print banner on EVERY log line
    _original_log = logging.Logger._log
    def _mock_banner_log(self, level, msg, args, **kwargs):
        _original_log(self, level, f"[MOCK MODE] {msg}", args, **kwargs)
    logging.Logger._log = _mock_banner_log

    print("=" * 72, file=sys.stderr)
    print("  WARNING: RUNNING IN MOCK MODE - ALL DATA IS SYNTHETIC", file=sys.stderr)
    print("  Set WIFI_DENSEPOSE_MOCK=false for real operation", file=sys.stderr)
    print("=" * 72, file=sys.stderr)
```

### 3. Ship a Reproducible Proof Bundle

A small real CSI capture file + one-command verification pipeline:

```
v1/data/proof/
├── README.md                      # How to verify
├── sample_csi_capture.bin         # Real CSI data (1 second, ~50 KB)
├── sample_csi_capture_meta.json   # Capture metadata (hardware, env)
├── expected_features.json         # Expected feature extraction output
├── expected_features.sha256       # SHA-256 hash of expected output
└── verify.py                      # One-command verification script
```

**verify.py**:
```python
#!/usr/bin/env python3
"""Verify WiFi-DensePose pipeline produces deterministic output from real CSI data.

Usage:
    python v1/data/proof/verify.py

Expected output:
    PASS: Pipeline output matches expected hash
    SHA256: <hash>

If this passes, the signal processing pipeline is producing real,
deterministic results from real captured CSI data.
"""
import hashlib
import json
import sys
import os

# Ensure reproducibility
os.environ["PYTHONHASHSEED"] = "42"
import numpy as np
np.random.seed(42)  # Only affects any remaining random elements

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../.."))

from src.core.csi_processor import CSIProcessor
from src.hardware.csi_extractor import CSIExtractor

def main():
    # Load real captured CSI data
    capture_path = os.path.join(os.path.dirname(__file__), "sample_csi_capture.bin")
    meta_path = os.path.join(os.path.dirname(__file__), "sample_csi_capture_meta.json")
    expected_hash_path = os.path.join(os.path.dirname(__file__), "expected_features.sha256")

    with open(meta_path) as f:
        meta = json.load(f)

    # Extract CSI from binary capture
    extractor = CSIExtractor(format=meta["format"])
    csi_data = extractor.extract_from_file(capture_path)

    # Process through feature pipeline
    config = {
        "sampling_rate": meta["sampling_rate"],
        "window_size": meta["window_size"],
        "overlap": meta["overlap"],
        "noise_threshold": meta["noise_threshold"],
    }
    processor = CSIProcessor(config)
    features = processor.extract_features(csi_data)

    # Serialize features deterministically
    output = {
        "amplitude_mean": features.amplitude_mean.tolist(),
        "amplitude_variance": features.amplitude_variance.tolist(),
        "phase_difference": features.phase_difference.tolist(),
        "doppler_shift": features.doppler_shift.tolist(),
        "psd_first_16": features.power_spectral_density[:16].tolist(),
    }
    output_json = json.dumps(output, sort_keys=True, separators=(",", ":"))
    output_hash = hashlib.sha256(output_json.encode()).hexdigest()

    # Verify against expected hash
    with open(expected_hash_path) as f:
        expected_hash = f.read().strip()

    if output_hash == expected_hash:
        print(f"PASS: Pipeline output matches expected hash")
        print(f"SHA256: {output_hash}")
        print(f"Features: {len(output['amplitude_mean'])} subcarriers processed")
        return 0
    else:
        print(f"FAIL: Hash mismatch")
        print(f"Expected: {expected_hash}")
        print(f"Got:      {output_hash}")
        return 1

if __name__ == "__main__":
    sys.exit(main())
```

### 4. Pin the Build Environment

**Option A (recommended): Deterministic Dockerfile that works on fresh machine**

```dockerfile
FROM python:3.11-slim

# System deps that actually matter
RUN apt-get update && apt-get install -y --no-install-recommends \
    libopenblas-dev \
    libfftw3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Pinned requirements (not a reference to missing file)
COPY v1/requirements-lock.txt ./requirements.txt
RUN pip install --no-cache-dir -r requirements.txt

COPY v1/ ./v1/

# Proof of reality: verify pipeline on build
RUN cd v1 && python data/proof/verify.py

EXPOSE 8000
# Default: REAL mode (mock requires explicit opt-in)
ENV WIFI_DENSEPOSE_MOCK=false
CMD ["uvicorn", "v1.src.api.main:app", "--host", "0.0.0.0", "--port", "8000"]
```

**Key change**: `RUN python data/proof/verify.py` **during build** means the Docker image cannot be created unless the pipeline produces correct output from real CSI data.

**Requirements lockfile** (`v1/requirements-lock.txt`):
```
# Core (required)
fastapi==0.115.6
uvicorn[standard]==0.34.0
pydantic==2.10.4
pydantic-settings==2.7.1
numpy==1.26.4
scipy==1.14.1

# Signal processing (required)
# No ONNX required for basic pipeline verification

# Optional (install separately for full features)
# torch>=2.1.0
# onnxruntime>=1.17.0
```

### 5. CI Pipeline That Proves Reality

```yaml
# .github/workflows/verify-pipeline.yml
name: Verify Signal Pipeline

on:
  push:
    paths: ['v1/src/**', 'v1/data/proof/**']
  pull_request:
    paths: ['v1/src/**']

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: '3.11'
      - name: Install minimal deps
        run: pip install numpy scipy pydantic pydantic-settings
      - name: Verify pipeline determinism
        run: python v1/data/proof/verify.py
      - name: Verify no random in production paths
        run: |
          # Fail if np.random appears in production code (not in testing/)
          ! grep -r "np\.random\.\(rand\|randn\|randint\)" v1/src/ \
            --include="*.py" \
            --exclude-dir=testing \
            || (echo "FAIL: np.random found in production code" && exit 1)
```

### Concrete File Changes Required

| File | Action | Description |
|------|--------|-------------|
| `v1/src/core/csi_processor.py:390` | **Replace** | Real Doppler extraction from temporal CSI history |
| `v1/src/hardware/csi_extractor.py:83-84` | **Replace** | Hard error with descriptive message when parsing fails |
| `v1/src/hardware/csi_extractor.py:129-135` | **Replace** | Real Atheros CSI parser or hard error with hardware instructions |
| `v1/src/hardware/router_interface.py:198-212` | **Replace** | Hard error for unimplemented hardware, or real `iwconfig` + CSI tool integration |
| `v1/src/services/pose_service.py:293-356` | **Move** | Move `_generate_mock_poses()` to `v1/src/testing/mock_pose_generator.py` |
| `v1/src/services/pose_service.py:430-431` | **Remove** | Remove mock CSI generation from production path |
| `v1/src/services/pose_service.py:489-607` | **Replace** | Real statistics from database, or explicit "no data" response |
| `v1/src/core/router_interface.py:60-300` | **Move** | Move mock generator to `v1/src/testing/mock_csi_generator.py` |
| `v1/src/api/dependencies.py:82,408` | **Replace** | Real auth check or explicit dev-mode bypass with logging |
| `v1/data/proof/` | **Create** | Proof bundle (sample capture + expected hash + verify script) |
| `v1/requirements-lock.txt` | **Create** | Pinned minimal dependencies |
| `.github/workflows/verify-pipeline.yml` | **Create** | CI verification |

### Hardware Documentation

```
v1/docs/hardware-setup.md (to be created)

# Supported Hardware Matrix

| Chipset | Tool | OS | Capture Command |
|---------|------|----|-----------------|
| Intel 5300 | Linux 802.11n CSI Tool | Ubuntu 18.04 | `sudo ./log_to_file csi.dat` |
| Atheros AR9580 | Atheros CSI Tool | Ubuntu 14.04 | `sudo ./recv_csi csi.dat` |
| Broadcom BCM4339 | Nexmon CSI | Android/Nexus 5 | `nexutil -m1 -k1 ...` |
| ESP32 | ESP32-CSI | ESP-IDF | `csi_recv --format binary` |

# Calibration
1. Place router and receiver 2m apart, line of sight
2. Capture 10 seconds of empty-room baseline
3. Have one person walk through at normal pace
4. Capture 10 seconds during walk-through
5. Run calibration: `python v1/scripts/calibrate.py --baseline empty.dat --activity walk.dat`
```

## Consequences

### Positive
- **"Clone, build, verify" in one command**: `docker build . && docker run --rm wifi-densepose python v1/data/proof/verify.py` produces a deterministic PASS
- **No silent fakes**: Random data never appears in production output
- **CI enforcement**: PRs that introduce `np.random` in production paths fail automatically
- **Credibility anchor**: SHA-256 verified output from real CSI capture is unchallengeable proof
- **Clear mock boundary**: Mock code exists only in `v1/src/testing/`, never imported by production modules

### Negative
- **Requires real CSI capture**: Someone must capture and commit a real CSI sample (one-time effort)
- **Build may fail without hardware**: Without mock fallback, systems without WiFi hardware cannot demo - must use proof bundle instead
- **Migration effort**: Moving mock code to separate module requires updating imports in test files
- **Stricter development workflow**: Developers must explicitly opt in to mock mode

### Acceptance Criteria

A stranger can:
1. `git clone` the repository
2. Run ONE command (`docker build .` or `python v1/data/proof/verify.py`)
3. See `PASS: Pipeline output matches expected hash` with a specific SHA-256
4. Confirm no `np.random` in any non-test file via CI badge

If this works 100% over 5 runs on a clean machine, the "fake" narrative dies.

### Answering the Two Key Questions

**Q1: Docker or Nix first?**
Recommendation: **Docker first**. The Dockerfile already exists, just needs fixing. Nix is higher quality but smaller audience. Docker gives the widest "clone and verify" coverage.

**Q2: Are external crates public and versioned?**
The Python dependencies are all public PyPI packages. The Rust `ruvector-core` and `ruvector-data-framework` crates are currently commented out in `Cargo.toml` (lines 83-84: `# ruvector-core = "0.1"`) and are not yet published to crates.io. They are internal to ruvnet. This is a blocker for the Rust path but does not affect the Python proof-of-reality work in this ADR.

## References

- [Linux 802.11n CSI Tool](https://dhalperi.github.io/linux-80211n-csitool/)
- [Atheros CSI Tool](https://wands.sg/research/wifi/AthesCSI/)
- [Nexmon CSI](https://github.com/seemoo-lab/nexmon_csi)
- [ESP32 CSI](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/wifi.html#wi-fi-channel-state-information)
- [Reproducible Builds](https://reproducible-builds.org/)
- ADR-002: RuVector RVF Integration Strategy

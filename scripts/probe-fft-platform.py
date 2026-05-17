#!/usr/bin/env python3
"""Platform probe: reproduce verify.py's hash-relevant FFT steps in isolation.

Runs the same scipy.fft.fft / scipy.signal calls that verify.py hashes
(csi_processor.py:426, :438, :349) on a deterministic synthetic input,
without dragging in src.app / pydantic Settings. Used to empirically
locate the source of platform divergence in issue #560.

Usage:  python3 scripts/probe-fft-platform.py
Output: single JSON object on stdout. Run on each platform and diff.

If two machines print the same `first8_doppler_bytes_hex` and the same
`first4_psd_floats` but different `sha256`, the divergence is in later
FFT bins (SIMD reordering). If even the first values differ, it's a
true ULP-level divergence at every bin (Apple Silicon NEON vs x86_64
AVX, or different scipy pocketfft builds).
"""
import hashlib
import json
import platform
import struct
import sys

import numpy as np
import scipy.fft
import scipy.signal

# Deterministic synthetic input -- no IO, no .env, no Settings
rng = np.random.RandomState(42)
N_FRAMES = 100
N_SUBC = 100
amp = rng.randn(N_FRAMES, N_SUBC).astype(np.float64)

# Mirror the three scipy calls verify.py's hash depends on:
#   archive/v1/src/core/csi_processor.py:349 -> scipy.signal.windows.hamming
#   archive/v1/src/core/csi_processor.py:426 -> scipy.fft.fft(mean_phase_diff, n=64)
#   archive/v1/src/core/csi_processor.py:438 -> scipy.fft.fft(amp.flatten(), n=128)
mean_phase_diff = amp.mean(axis=1)
doppler = np.abs(scipy.fft.fft(mean_phase_diff, n=64)) ** 2
psd = np.abs(scipy.fft.fft(amp.flatten(), n=128)) ** 2
window = scipy.signal.windows.hamming(56)

# Pack the same way verify.py:features_to_bytes does (little-endian f64)
parts = []
for arr in (doppler, psd, window):
    flat = np.asarray(arr, dtype=np.float64).ravel()
    parts.append(struct.pack(f"<{len(flat)}d", *flat))
blob = b"".join(parts)

try:
    blas_info = np.show_config(mode="dicts")
except Exception:
    blas_info = {"error": "show_config(mode=dicts) unavailable"}

print(json.dumps({
    "uname": platform.uname()._asdict(),
    "python": sys.version.split()[0],
    "numpy": np.__version__,
    "scipy": __import__("scipy").__version__,
    "blob_len": len(blob),
    "sha256": hashlib.sha256(blob).hexdigest(),
    "first8_doppler_bytes_hex": doppler[:8].tobytes().hex(),
    "first4_psd_floats": psd[:4].tolist(),
    "blas_backend": blas_info if isinstance(blas_info, dict) else str(blas_info),
}, indent=2, default=str))

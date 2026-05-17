#!/usr/bin/env python3
"""Platform probe: reproduce verify.py's hash-relevant FFT steps in isolation.

Runs the same scipy.fft.fft / scipy.signal calls that verify.py hashes
(csi_processor.py:426, :438, :349) on a deterministic synthetic input,
without dragging in src.app / pydantic Settings. Used to empirically
locate the source of platform divergence in issue #560 — and now also to
verify the quantize-before-hash fix shipped in archive/v1/data/proof/verify.py.

Usage:  python3 scripts/probe-fft-platform.py
Output: single JSON object on stdout. Run on each platform and diff.

The output now contains TWO hashes:
- `sha256_raw`       — hash of unrounded little-endian f64 bytes (legacy)
- `sha256_quantized` — hash after np.round(.., 9) (matches verify.py
                       behaviour after the issue-#560 fix; should be
                       IDENTICAL across Intel AVX, ARM NEON, and any
                       scipy pocketfft build)

If `sha256_raw` differs across machines but `sha256_quantized` matches,
the quantize-before-hash fix is doing its job.
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

# Quantization decimals — kept in sync with
# archive/v1/data/proof/verify.py:HASH_QUANTIZATION_DECIMALS so this probe
# verifies the production hash, not just the FFT outputs.
HASH_QUANTIZATION_DECIMALS = 6


def pack_floats(arrays, quantize):
    """Pack arrays as little-endian f64, optionally rounding first."""
    parts = []
    for arr in arrays:
        flat = np.asarray(arr, dtype=np.float64).ravel()
        if quantize:
            flat = np.round(flat, HASH_QUANTIZATION_DECIMALS)
        parts.append(struct.pack(f"<{len(flat)}d", *flat))
    return b"".join(parts)


arrays = (doppler, psd, window)
blob_raw = pack_floats(arrays, quantize=False)
blob_quantized = pack_floats(arrays, quantize=True)

try:
    blas_info = np.show_config(mode="dicts")
except Exception:
    blas_info = {"error": "show_config(mode=dicts) unavailable"}

print(json.dumps({
    "uname": platform.uname()._asdict(),
    "python": sys.version.split()[0],
    "numpy": np.__version__,
    "scipy": __import__("scipy").__version__,
    "blob_len": len(blob_raw),
    "sha256_raw": hashlib.sha256(blob_raw).hexdigest(),
    "sha256_quantized": hashlib.sha256(blob_quantized).hexdigest(),
    "quantization_decimals": HASH_QUANTIZATION_DECIMALS,
    "first8_doppler_bytes_hex": doppler[:8].tobytes().hex(),
    "first4_psd_floats": psd[:4].tolist(),
    "blas_backend": blas_info if isinstance(blas_info, dict) else str(blas_info),
}, indent=2, default=str))

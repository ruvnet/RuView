"""WiFi-DensePose — passive human sensing from WiFi CSI.

ADR-117 — v2.0 is a PyO3-bound replacement for the legacy pure-Python
``wifi-densepose==1.1.0`` (released 2025-06-07). The compiled core is
the same Rust workspace published in `v2/crates/` of the
`ruvnet/RuView <https://github.com/ruvnet/RuView>`_ repository.

Quick start::

    import wifi_densepose
    print(wifi_densepose.__version__)
    print(wifi_densepose.__rust_version__)
    print(wifi_densepose.hello())   # → "ok"

P1 (this release): scaffold. Core types land in P2; vital signs +
signal DSP in P3; WebSocket/MQTT client in P4. See the
`ADR-117 modernization plan
<https://github.com/ruvnet/RuView/blob/main/docs/adr/ADR-117-pip-wifi-densepose-modernization.md>`_
for the full phase ledger.

Migrating from v1.x: the v1 line was pure-Python and had a different
API surface. v2 is a hard break (semver-justified). See the
``v1.99.0`` tombstone wheel for the migration URL.
"""

from __future__ import annotations

# Public Python version follows the wheel version, NOT the Rust core
# version. The Rust core version is surfaced separately as
# `__rust_version__` for diagnostics.
__version__ = "2.0.0"

# Re-export the compiled module's surface. The leading underscore on
# `_native` is intentional — it marks the binding module as internal.
# Users always import from `wifi_densepose` directly.
from wifi_densepose import _native

# ─── P2 — Core type re-exports ───────────────────────────────────────
# Bound types land in `wifi_densepose._native` and are re-exported here
# under their stable public names. Users always `from wifi_densepose
# import Keypoint, KeypointType` — never reach into `_native`.
Keypoint = _native.Keypoint
KeypointType = _native.KeypointType
BoundingBox = _native.BoundingBox
PersonPose = _native.PersonPose
PoseEstimate = _native.PoseEstimate

# ─── P3 — Vital sign extraction ──────────────────────────────────────
VitalStatus = _native.VitalStatus
VitalEstimate = _native.VitalEstimate
VitalReading = _native.VitalReading
CsiVitalPreprocessor = _native.CsiVitalPreprocessor
BreathingExtractor = _native.BreathingExtractor
HeartRateExtractor = _native.HeartRateExtractor

# ─── P3.5 — BFLD (Beamforming Feedback Loop Data) ─────────────────────
BfldKind = _native.BfldKind
BfldFrame = _native.BfldFrame
BfldReport = _native.BfldReport


__rust_version__: str = _native.__rust_version__
"""Version of the bound Rust core. Useful for bug reports."""

__rust_build_tag__: str = _native.__rust_build_tag__
"""Build tag of the Rust core (P5 will swap this for the git SHA)."""

__build_features__: list[str] = list(_native.__build_features__)
"""Feature flags the wheel was compiled with."""


def verify_witness(data_path: str, expected: str) -> bool:
    """ADR-117 §11.3 — re-run the v2 witness and compare against ``expected``.

    Feeds ``data_path`` (``archive/v1/data/proof/sample_csi_data.json``)
    through the Rust vitals pipeline and returns True if the SHA-256 of the
    output matches ``expected`` (a hex digest or a path to
    ``expected_features_v2.sha256``). See :mod:`wifi_densepose.witness`.
    """
    from wifi_densepose.witness import verify_witness as _verify

    return _verify(data_path, expected)


def hello() -> str:
    """Smoke test — confirms the compiled module loads and is callable.

    Returns:
        Always ``"ok"`` if the wheel built and loaded correctly.

    Used by ``python/tests/test_smoke.py`` to assert the PyO3 round-trip
    works end-to-end on every cibuildwheel target.
    """
    return _native.hello()


__all__ = [
    "__version__",
    "__rust_version__",
    "__rust_build_tag__",
    "__build_features__",
    "hello",
    # P2 — core types
    "Keypoint",
    "KeypointType",
    "BoundingBox",
    "PersonPose",
    "PoseEstimate",
    # P3 — vital sign extraction
    "VitalStatus",
    "VitalEstimate",
    "VitalReading",
    "CsiVitalPreprocessor",
    "BreathingExtractor",
    "HeartRateExtractor",
    # P3.5 — BFLD (forward-compat surface for the future Rust crate)
    "BfldKind",
    "BfldFrame",
    "BfldReport",
    # ADR-117 §11.3 — v2 witness
    "verify_witness",
]

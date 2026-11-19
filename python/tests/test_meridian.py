"""ADR-185 P2 — MERIDIAN binding tests, incl. the §4.1 bit-for-bit parity gate.

The parity test packs the binding's concatenated outputs (2× canonical
frame, geometry vector, rapid-adapt LoRA weights) to little-endian f32
bytes and asserts SHA-256 equality with the golden produced by the
native-Rust reference (`tests/meridian_parity.rs`). A mismatch is a
release blocker.
"""

from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path

import pytest

from wifi_densepose import meridian as mer

GOLDEN = Path(__file__).parent / "golden"


def fixture() -> dict:
    return json.loads((GOLDEN / "meridian_input.json").read_text())


# ─── HardwareType / HardwareNormalizer / CanonicalCsiFrame ───────────

def test_hardware_type_detect() -> None:
    assert mer.HardwareType.detect(64) == mer.HardwareType.Esp32S3
    assert mer.HardwareType.detect(30) == mer.HardwareType.Intel5300
    assert mer.HardwareType.detect(56) == mer.HardwareType.Atheros
    assert mer.HardwareType.detect(128) == mer.HardwareType.Generic


def test_hardware_type_properties() -> None:
    assert mer.HardwareType.Esp32S3.subcarrier_count == 64
    assert mer.HardwareType.Esp32S3.mimo_streams == 1
    assert mer.HardwareType.Intel5300.mimo_streams == 3


def test_normalize_shapes_and_hardware() -> None:
    fx = fixture()
    norm = mer.HardwareNormalizer()
    assert norm.canonical_subcarriers == 56
    frame = norm.normalize(fx["esp32_amplitude"], fx["esp32_phase"], mer.HardwareType.Esp32S3)
    assert len(frame.amplitude) == 56
    assert len(frame.phase) == 56
    assert frame.hardware_type == mer.HardwareType.Esp32S3


def test_normalize_rejects_mismatched_lengths() -> None:
    norm = mer.HardwareNormalizer()
    with pytest.raises(ValueError):
        norm.normalize([1.0, 2.0], [1.0], mer.HardwareType.Generic)


# ─── GeometryEncoder ─────────────────────────────────────────────────

def test_geometry_encode_dim_and_permutation_invariance() -> None:
    enc = mer.GeometryEncoder(mer.MeridianGeometryConfig())
    aps = [[0.25, 0.5, 0.75], [1.0, 1.25, 1.5], [2.0, 0.0, -0.5]]
    v = enc.encode(aps)
    assert len(v) == 64
    # DeepSets mean-pool is permutation-invariant.
    v_perm = enc.encode([aps[2], aps[0], aps[1]])
    assert max(abs(a - b) for a, b in zip(v, v_perm, strict=True)) < 1e-5


def test_geometry_encode_rejects_empty_and_bad_shape() -> None:
    enc = mer.GeometryEncoder()
    with pytest.raises(ValueError):
        enc.encode([])
    with pytest.raises(ValueError):
        enc.encode([[0.0, 1.0]])  # not 3 coords


# ─── RapidAdaptation ─────────────────────────────────────────────────

def test_rapid_adaptation_adapt() -> None:
    fx = fixture()
    ra = mer.RapidAdaptation(
        min_calibration_frames=10, lora_rank=4, loss_kind="combined",
        epochs=5, lr=0.001, lambda_ent=0.5,
    )
    for frame in fx["rapid_frames"]:
        ra.push_frame(frame)
    assert ra.is_ready()
    assert ra.buffer_len == 12
    res = ra.adapt()
    assert res.frames_used == 12
    assert res.adaptation_epochs == 5
    assert len(res.lora_weights) == 2 * 16 * 4  # 2 * fdim * rank


def test_rapid_adaptation_rejects_bad_loss_kind() -> None:
    with pytest.raises(ValueError):
        mer.RapidAdaptation(10, 4, loss_kind="nonsense")


def test_rapid_adaptation_empty_buffer_raises() -> None:
    ra = mer.RapidAdaptation(1, 4)
    with pytest.raises(ValueError):
        ra.adapt()


# ─── CrossDomainEvaluator ────────────────────────────────────────────

def test_cross_domain_evaluator_gap_ratio() -> None:
    ev = mer.CrossDomainEvaluator(1)
    preds = [
        ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),  # domain 0, err 1
        ([0.0, 0.0, 0.0], [2.0, 0.0, 0.0]),  # domain 1, err 2
    ]
    m = ev.evaluate(preds, [0, 1])
    assert abs(m["in_domain_mpjpe"] - 1.0) < 1e-6
    assert abs(m["cross_domain_mpjpe"] - 2.0) < 1e-6
    assert abs(m["domain_gap_ratio"] - 2.0) < 1e-6


def test_mpjpe_module_fn() -> None:
    assert abs(mer.mpjpe([0.0, 0.0, 0.0], [3.0, 4.0, 0.0], 1) - 5.0) < 1e-6


# ─── §4.1 bit-for-bit parity gate (release-blocking) ─────────────────

def test_bit_for_bit_parity_with_native_rust() -> None:
    fx = fixture()
    out: list[float] = []

    norm = mer.HardwareNormalizer()
    esp = norm.normalize(fx["esp32_amplitude"], fx["esp32_phase"], mer.HardwareType.Esp32S3)
    out += list(esp.amplitude) + list(esp.phase)
    intel = norm.normalize(fx["intel_amplitude"], fx["intel_phase"], mer.HardwareType.Intel5300)
    out += list(intel.amplitude) + list(intel.phase)

    enc = mer.GeometryEncoder(mer.MeridianGeometryConfig())
    out += list(enc.encode(fx["ap_positions"]))

    ra = mer.RapidAdaptation(
        min_calibration_frames=10, lora_rank=4, loss_kind="combined",
        epochs=5, lr=0.001, lambda_ent=0.5,
    )
    for frame in fx["rapid_frames"]:
        ra.push_frame(frame)
    out += list(ra.adapt().lora_weights)

    packed = b"".join(struct.pack("<f", x) for x in out)
    got = hashlib.sha256(packed).hexdigest()
    expected = (GOLDEN / "meridian_output.sha256").read_text().strip()
    assert got == expected, (
        f"Python binding MERIDIAN output diverged from native-Rust golden "
        f"({got} != {expected})"
    )


def test_base_wheel_import_error_message() -> None:
    src = Path(mer.__file__).read_text()
    assert "--features meridian" in src
    assert "pip install wifi-densepose[meridian]" not in src

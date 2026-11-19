"""ADR-185 P1 — AETHER binding tests, incl. the §4.1 bit-for-bit parity gate.

The parity test compares the binding's embedding to a committed golden VECTOR
produced by the native-Rust reference (`tests/aether_parity.rs`), within a
numerical tolerance. It is NOT a byte-hash: the embedding is f32 with
transcendental ops, so exact bytes are not reproducible across the CPU
architectures this project ships wheels for. A mismatch beyond tolerance is a
release blocker, not a warning.
"""

from __future__ import annotations

import json
import math
import struct
import tempfile
from pathlib import Path

import pytest

from wifi_densepose import aether

GOLDEN = Path(__file__).parent / "golden"

# Cross-architecture f32 parity tolerance. The AETHER embedding is pure f32 with
# transcendental ops that differ in the last bits across CPUs/libm, so exact
# byte equality is not portable across the wheels this project builds. 1e-4 is
# ~100x the observed cross-arch drift on unit-normed values and ~100x smaller
# than any real algorithm change. Combined atol+rtol so both small and larger
# components are bounded. (ADR-185 §4.1.)
PARITY_ATOL = 1e-4
PARITY_RTOL = 1e-4


def load_input() -> list[list[float]]:
    return json.loads((GOLDEN / "aether_input.json").read_text())


def assert_embedding_matches_golden(embedding: list[float], golden_name: str) -> None:
    """Assert `embedding` matches the committed golden vector.

    Two independent checks, because a per-element tolerance alone is not enough:
    - **Per element**, within atol+rtol — catches a single component drifting.
    - **Whole-vector cosine** ≥ 1 - 1e-6 — catches a *coherent* shift that stays
      inside the per-element bound on every component yet moves the vector as a
      whole (the failure mode a loose element tolerance would hide).

    NaN/inf are rejected explicitly: `abs(nan - b) > tol` is False, so a bare
    tolerance check would silently PASS an all-NaN embedding. Every value must be
    finite first.
    """
    golden = json.loads((GOLDEN / golden_name).read_text())
    assert len(embedding) == len(golden), (
        f"{golden_name}: length {len(embedding)} != golden {len(golden)}"
    )
    for i, x in enumerate(embedding):
        assert math.isfinite(x), f"{golden_name}: element {i} is not finite ({x})"

    for i, (a, b) in enumerate(zip(embedding, golden, strict=True)):
        tol = PARITY_ATOL + PARITY_RTOL * abs(b)
        assert abs(a - b) <= tol, (
            f"{golden_name}: element {i} diverged from native golden beyond "
            f"tolerance (got {a}, golden {b}, |Δ|={abs(a - b):.3e}) — "
            "a real regression, not cross-arch f32 drift."
        )

    dot = sum(a * b for a, b in zip(embedding, golden, strict=True))
    na = math.sqrt(sum(a * a for a in embedding))
    nb = math.sqrt(sum(b * b for b in golden))
    cosine = dot / (na * nb) if na > 0 and nb > 0 else 0.0
    assert cosine >= 1.0 - 1e-6, (
        f"{golden_name}: whole-vector cosine similarity to the golden is "
        f"{cosine:.9f} (< 1 - 1e-6) — a coherent shift the per-element "
        "tolerance did not catch."
    )


def build_extractor() -> aether.EmbeddingExtractor:
    # Must match the native-Rust reference construction exactly.
    cfg = aether.AetherConfig(d_model=64, d_proj=128, temperature=0.07, normalize=True)
    return aether.EmbeddingExtractor(n_subcarriers=56, config=cfg)


def _formula_weights(n: int) -> list[float]:
    # Byte-identical to aether_weights_parity.rs (k/65536 is exact in f32+f64).
    return [((i * 1103515245 + 12345) % 65536) / 65536.0 - 0.5 for i in range(n)]


def _write_weight_file(path: Path, weights: list[float]) -> None:
    # AETHER weight format: b"AETHERW1" + u32 count + LE f32 payload.
    with open(path, "wb") as f:
        f.write(b"AETHERW1")
        f.write(struct.pack("<I", len(weights)))
        f.write(b"".join(struct.pack("<f", w) for w in weights))


def test_config_roundtrips_fields() -> None:
    cfg = aether.AetherConfig(d_model=64, d_proj=128, temperature=0.07, normalize=True)
    assert cfg.d_model == 64
    assert cfg.d_proj == 128
    assert abs(cfg.temperature - 0.07) < 1e-6
    assert cfg.normalize is True


# ─── Constructor input validation (raise ValueError, never panic) ─────────
# Bad dimensions used to reach Rust and either panic (surfacing as an opaque
# PanicException) or allocate multi-gigabyte matrices that abort the interpreter.

@pytest.mark.parametrize("kwargs", [
    {"d_model": 0, "d_proj": 128},
    {"d_model": 64, "d_proj": 0},
    {"d_model": 100_000, "d_proj": 128},   # unbounded allocation guard
    {"d_model": 64, "d_proj": 100_000},
])
def test_aether_config_rejects_bad_dims(kwargs: dict) -> None:
    with pytest.raises(ValueError):
        aether.AetherConfig(**kwargs)


def test_extractor_rejects_zero_heads_instead_of_panicking() -> None:
    # THE crash codex flagged: n_heads=0 -> `d_model % n_heads` -> panic.
    cfg = aether.AetherConfig(d_model=64, d_proj=128)
    with pytest.raises(ValueError):
        aether.EmbeddingExtractor(n_subcarriers=56, config=cfg, n_heads=0)


def test_extractor_rejects_indivisible_head_count() -> None:
    # 64 % 5 != 0 trips the native assert; must be a clean ValueError.
    cfg = aether.AetherConfig(d_model=64, d_proj=128)
    with pytest.raises(ValueError):
        aether.EmbeddingExtractor(n_subcarriers=56, config=cfg, n_heads=5)


@pytest.mark.parametrize("kwargs", [
    {"n_subcarriers": 0},
    {"n_subcarriers": 100_000},
    {"n_keypoints": 0},
])
def test_extractor_rejects_bad_shape(kwargs: dict) -> None:
    cfg = aether.AetherConfig(d_model=64, d_proj=128)
    base = {"n_subcarriers": 56, "config": cfg}
    base.update(kwargs)
    with pytest.raises(ValueError):
        aether.EmbeddingExtractor(**base)


def test_valid_extractor_still_constructs() -> None:
    # The negatives above must not pass by making the constructor reject
    # everything: a valid config still builds and embeds.
    cfg = aether.AetherConfig(d_model=64, d_proj=128)
    ext = aether.EmbeddingExtractor(n_subcarriers=56, config=cfg, n_heads=4)
    assert len(ext.embed(load_input())) == 128


def test_embedding_shape_and_unit_norm() -> None:
    emb = build_extractor().embed(load_input())
    assert len(emb) == 128
    norm = math.sqrt(sum(x * x for x in emb))
    assert abs(norm - 1.0) < 1e-4, f"expected unit-norm embedding, got {norm}"


def test_binding_matches_native_golden_within_tolerance() -> None:
    """The release-blocking §4.1 gate: binding output == native Rust reference.

    Compares to a committed golden VECTOR within a numerical tolerance, not a
    SHA-256 of the raw f32 bytes. The embedding is pure f32 and uses
    transcendental ops (ln/sqrt/cos in the Gaussian init), which are NOT
    bit-reproducible across CPU architectures or libm implementations. A
    byte-hash therefore only ever matched the one arch that generated it, and
    failed on every other wheel this project builds (aarch64, macOS-arm). The
    tolerance below (1e-4) is orders of magnitude larger than cross-arch f32
    drift yet far tighter than any real algorithm change, which moves
    unit-normed elements by ~1e-2 or more. See ADR-185 §4.1.
    """
    emb = build_extractor().embed(load_input())
    assert_embedding_matches_golden(emb, "aether_embedding.json")


def test_embedding_is_deterministic() -> None:
    ext = build_extractor()
    inp = load_input()
    assert ext.embed(inp) == ext.embed(inp)


def test_cosine_similarity_self_is_one() -> None:
    v = [0.1 * i - 0.5 for i in range(32)]
    assert abs(aether.cosine_similarity(v, v) - 1.0) < 1e-5


def test_cosine_similarity_orthogonal_is_zero() -> None:
    a = [1.0, 0.0, 0.0, 0.0]
    b = [0.0, 1.0, 0.0, 0.0]
    assert abs(aether.cosine_similarity(a, b)) < 1e-6


def test_info_nce_loss_identical_batch_is_log_n() -> None:
    # Identical embeddings → all similarities equal → loss == ln(N).
    emb = [[1.0, 0.0, 0.0]] * 4
    loss = aether.info_nce_loss(emb, emb, 0.07)
    assert abs(loss - math.log(4)) < 0.1


def test_augment_pair_preserves_shape_and_differs() -> None:
    window = load_input()
    view_a, view_b = aether.CsiAugmenter().augment_pair(window, seed=42)
    assert len(view_a) == len(window)
    assert len(view_b) == len(window)
    assert len(view_a[0]) == len(window[0])
    differs = any(
        abs(x - y) > 1e-6
        for ra, rb in zip(view_a, view_b, strict=True)
        for x, y in zip(ra, rb, strict=True)
    )
    assert differs, "augment_pair should return two distinct views"


def test_missing_feature_message_names_the_real_fix() -> None:
    # The guard fires only on a from-source build without the feature. Its
    # message must name the real fix — rebuild with the feature — and must NOT
    # tell users to `pip install [aether]`, which is an empty extra that cannot
    # add compiled code to a built wheel.
    src = (Path(aether.__file__)).read_text()
    assert "--features aether" in src
    assert "pip install wifi-densepose[aether]" not in src


# ─── Weight loading (ADR-185 §13.a) ──────────────────────────────────

def test_load_weights_is_used_and_matches_native_golden() -> None:
    ext = build_extractor()
    baseline = ext.embed(load_input())  # random Xavier init

    weights = _formula_weights(ext.param_count)
    with tempfile.TemporaryDirectory() as d:
        wpath = Path(d) / "weights.bin"
        _write_weight_file(wpath, weights)
        ext.load_weights(str(wpath))

    loaded = ext.embed(load_input())

    # (1) The loaded weights actually take effect (not a silent no-op).
    assert any(abs(a - b) > 1e-6 for a, b in zip(baseline, loaded, strict=True)), (
        "load_weights had no effect — embedding still equals the random-init baseline"
    )
    # (2) Matches the native-Rust reference that loaded the same weights, within
    #     tolerance. See test_binding_matches_native_golden_within_tolerance for
    #     why this is a tolerance compare and not a byte-hash.
    assert_embedding_matches_golden(loaded, "aether_loaded_embedding.json")


def test_save_then_load_weights_round_trips() -> None:
    ext = build_extractor()
    inp = load_input()
    with tempfile.TemporaryDirectory() as d:
        wpath = Path(d) / "roundtrip.bin"
        ext.save_weights(str(wpath))          # serialize current (random) weights
        emb_before = ext.embed(inp)
        ext2 = build_extractor()
        ext2.load_weights(str(wpath))         # load them into a fresh extractor
    assert ext2.embed(inp) == emb_before


def test_load_weights_rejects_bad_magic() -> None:
    ext = build_extractor()
    with tempfile.TemporaryDirectory() as d:
        wpath = Path(d) / "bad.bin"
        wpath.write_bytes(b"NOTAETHER" + b"\x00" * 8)
        with pytest.raises(ValueError):
            ext.load_weights(str(wpath))


def test_load_weights_rejects_wrong_param_count() -> None:
    ext = build_extractor()
    with tempfile.TemporaryDirectory() as d:
        wpath = Path(d) / "short.bin"
        _write_weight_file(wpath, [0.1, 0.2, 0.3])  # far too few params
        with pytest.raises(ValueError):
            ext.load_weights(str(wpath))

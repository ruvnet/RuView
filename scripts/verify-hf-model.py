#!/usr/bin/env python3
"""Verify the published ruvnet/wifi-densepose-pretrained model bundle.

Inspects every file in the downloaded model directory:
  - model.safetensors     -> tensor names + shapes + dtypes
  - model.rvf.jsonl       -> line count, first three lines, distinct top-level keys
  - presence-head.json    -> shallow dump (depth <= 3)
  - config.json           -> full dump
  - training-metrics.json -> final loss / quantization / lora numbers

If torch is importable, builds a synthetic input matching the inferred encoder
input dim and runs encoder.w1 (the first linear layer) to confirm the weights
yield finite outputs (no NaN / Inf).

Exits 0 on success, non-zero with a clear error on any failure.

Usage:
    python scripts/verify-hf-model.py
    python scripts/verify-hf-model.py --local-dir models/wifi-densepose-pretrained/
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path
from typing import Any

DEFAULT_LOCAL_DIR = Path("models/wifi-densepose-pretrained/")

# safetensors -> torch dtype lookup. Subset is enough for this bundle.
_SAFETENSORS_DTYPE_NAMES = {
    "F64", "F32", "F16", "BF16",
    "I64", "I32", "I16", "I8", "U8", "BOOL",
}


# --------------------------------------------------------------------------- #
# safetensors loading
# --------------------------------------------------------------------------- #
def _load_safetensors(path: Path):
    """Load a .safetensors file as a dict[name -> torch.Tensor].

    Tries the upstream `safetensors.torch.load_file` first. The published HF
    bundle has a non-fatal header bug (declared header length includes 3
    trailing NUL bytes after the JSON object), which the strict Rust parser
    rejects with `trailing characters at line 1 column 1462`. When that
    happens we fall back to a small pure-Python loader that strips the
    padding and rebuilds tensors from the body.
    """
    try:
        from safetensors.torch import load_file  # type: ignore

        return load_file(str(path)), "safetensors.torch.load_file"
    except Exception as exc:  # noqa: BLE001 - we want any failure here
        msg = str(exc)
        if "trailing characters" not in msg and "invalid JSON" not in msg:
            raise
        # Fall through to the manual loader below.
        first_err = f"{type(exc).__name__}: {exc}"

    import torch  # local import so the fallback message is precise

    dtype_map = {
        "F64": torch.float64,
        "F32": torch.float32,
        "F16": torch.float16,
        "BF16": torch.bfloat16,
        "I64": torch.int64,
        "I32": torch.int32,
        "I16": torch.int16,
        "I8": torch.int8,
        "U8": torch.uint8,
        "BOOL": torch.bool,
    }
    raw = path.read_bytes()
    if len(raw) < 8:
        raise ValueError(f"{path}: file too short to be a safetensors blob")
    header_len = struct.unpack("<Q", raw[:8])[0]
    if header_len <= 0 or 8 + header_len > len(raw):
        raise ValueError(
            f"{path}: header length {header_len} inconsistent with file size {len(raw)}"
        )
    header_bytes = raw[8 : 8 + header_len]
    # Strip the published-bundle trailing padding (NULs / whitespace) before parsing.
    header_text = header_bytes.rstrip(b"\x00 \t\r\n").decode("utf-8")
    header = json.loads(header_text)
    body = raw[8 + header_len :]

    state: dict[str, Any] = {}
    for name, info in header.items():
        if name == "__metadata__":
            continue
        dtype_name = info["dtype"]
        if dtype_name not in dtype_map:
            raise ValueError(f"{name}: unsupported safetensors dtype {dtype_name!r}")
        shape = list(info["shape"])
        start, end = info["data_offsets"]
        if start < 0 or end > len(body) or start > end:
            raise ValueError(
                f"{name}: bad offsets [{start}, {end}] for body of size {len(body)}"
            )
        tensor = torch.frombuffer(
            bytearray(body[start:end]), dtype=dtype_map[dtype_name]
        ).reshape(shape)
        state[name] = tensor.clone()  # detach from the bytearray buffer
    return state, (
        "manual fallback (published bundle has trailing NULs in header; "
        f"first error was: {first_err})"
    )


# --------------------------------------------------------------------------- #
# JSONL helpers
# --------------------------------------------------------------------------- #
def _truncate(s: str, n: int) -> str:
    return s if len(s) <= n else s[:n] + "..."


def _inspect_jsonl(path: Path) -> tuple[int, list[str], list[str]]:
    """Return (line_count, first_three_truncated, sorted_distinct_top_keys)."""
    lines: list[str] = []
    keys: set[str] = set()
    total = 0
    with path.open("r", encoding="utf-8") as fh:
        for idx, raw_line in enumerate(fh):
            line = raw_line.rstrip("\n")
            if not line.strip():
                continue
            total += 1
            if idx < 3:
                lines.append(_truncate(line, 200))
            try:
                obj = json.loads(line)
            except json.JSONDecodeError as exc:
                raise ValueError(f"{path}: line {idx + 1} is not valid JSON: {exc}") from exc
            if isinstance(obj, dict):
                keys.update(obj.keys())
    return total, lines, sorted(keys)


# --------------------------------------------------------------------------- #
# Pretty printers
# --------------------------------------------------------------------------- #
def _dump_shallow(obj: Any, depth: int = 0, max_depth: int = 3) -> str:
    """Render `obj` as JSON, but collapse anything below max_depth to its type."""
    if depth >= max_depth:
        if isinstance(obj, dict):
            return f"<dict with {len(obj)} keys>"
        if isinstance(obj, list):
            return f"<list len={len(obj)}>"
        return repr(obj)
    if isinstance(obj, dict):
        body = ", ".join(
            f'"{k}": {_dump_shallow(v, depth + 1, max_depth)}' for k, v in obj.items()
        )
        return "{" + body + "}"
    if isinstance(obj, list):
        if len(obj) > 8:
            sample = ", ".join(_dump_shallow(v, depth + 1, max_depth) for v in obj[:8])
            return f"[{sample}, ... (+{len(obj) - 8} more)]"
        return "[" + ", ".join(_dump_shallow(v, depth + 1, max_depth) for v in obj) + "]"
    return json.dumps(obj)


def _section(title: str) -> None:
    print()
    print("=" * 78)
    print(title)
    print("=" * 78)


# --------------------------------------------------------------------------- #
# Verification steps
# --------------------------------------------------------------------------- #
def _verify_safetensors(path: Path) -> tuple[dict, str, dict[str, tuple]]:
    state, loader_note = _load_safetensors(path)
    info: dict[str, tuple] = {}
    print(f"loader: {loader_note}")
    print(f"tensor count: {len(state)}")
    print(f"{'name':<30} {'shape':<22} dtype")
    print("-" * 78)
    for name, tensor in state.items():
        shape = tuple(tensor.shape)
        dtype = str(tensor.dtype)
        info[name] = (shape, dtype)
        print(f"{name:<30} {str(shape):<22} {dtype}")
    return state, loader_note, info


def _verify_jsonl(path: Path) -> None:
    total, sample, keys = _inspect_jsonl(path)
    print(f"line count: {total}")
    print(f"distinct top-level keys observed: {keys}")
    print("first 3 lines (truncated to 200 chars):")
    for idx, line in enumerate(sample, start=1):
        print(f"  [{idx}] {line}")


def _verify_presence_head(path: Path) -> None:
    obj = json.loads(path.read_text(encoding="utf-8"))
    print(_dump_shallow(obj, max_depth=3))


def _verify_config(path: Path) -> dict:
    obj = json.loads(path.read_text(encoding="utf-8"))
    print(json.dumps(obj, indent=2, sort_keys=True))
    return obj


def _verify_training_metrics(path: Path) -> None:
    obj = json.loads(path.read_text(encoding="utf-8"))
    # Final metrics live in a few specific places.
    contrastive = obj.get("contrastive", {})
    task_heads = obj.get("taskHeads", {})
    lora = obj.get("lora", {})
    quant = obj.get("quantization", {})
    print(f"timestamp:           {obj.get('timestamp')}")
    print(f"total duration (ms): {obj.get('totalDurationMs')}")
    print(f"contrastive triplets / final loss: "
          f"{contrastive.get('triplets')} / {contrastive.get('finalLoss')}")
    print(f"task heads samples / final loss:   "
          f"{task_heads.get('samples')} / {task_heads.get('finalLoss')}")
    print(f"lora adapters / total params: "
          f"{lora.get('adapters')} / {lora.get('totalParameters')}")
    if quant:
        print("quantization:")
        for variant, stats in quant.items():
            print(f"  {variant}: {stats}")


def _verify_first_linear(
    state: dict, config: dict, tensor_info: dict[str, tuple]
) -> None:
    try:
        import torch  # noqa: F401  (used below)
    except ImportError:
        print("torch not importable - skipping forward-pass smoke test")
        return

    import torch

    custom = (config or {}).get("custom", {})
    input_dim = int(custom.get("inputDim", 8))
    hidden_dim = int(custom.get("hiddenDim", 64))

    w1 = state.get("encoder.w1")
    b1 = state.get("encoder.b1")
    if w1 is None or b1 is None:
        raise RuntimeError("encoder.w1 / encoder.b1 missing from safetensors")

    # The published encoder stores the first linear weight flat (input_dim * hidden_dim).
    if w1.numel() != input_dim * hidden_dim:
        raise RuntimeError(
            f"encoder.w1 numel={w1.numel()} does not match "
            f"inputDim*hiddenDim={input_dim * hidden_dim}"
        )
    if b1.numel() != hidden_dim:
        raise RuntimeError(
            f"encoder.b1 numel={b1.numel()} does not match hiddenDim={hidden_dim}"
        )

    weight = w1.reshape(input_dim, hidden_dim).to(torch.float32)
    bias = b1.to(torch.float32)

    torch.manual_seed(42)
    batch = 4
    x = torch.randn(batch, input_dim, dtype=torch.float32)
    y = x @ weight + bias

    if not torch.isfinite(y).all():
        bad = (~torch.isfinite(y)).sum().item()
        raise RuntimeError(f"first linear layer produced {bad} non-finite values")
    print(
        f"first linear OK: input={tuple(x.shape)} weight={tuple(weight.shape)} "
        f"bias={tuple(bias.shape)} output={tuple(y.shape)} "
        f"mean={y.mean().item():+.4f} std={y.std().item():.4f}"
    )


# --------------------------------------------------------------------------- #
# Entry point
# --------------------------------------------------------------------------- #
def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--local-dir",
        type=Path,
        default=DEFAULT_LOCAL_DIR,
        help="Directory containing the downloaded HF model bundle "
             f"(default: {DEFAULT_LOCAL_DIR})",
    )
    args = parser.parse_args(argv)

    root: Path = args.local_dir
    if not root.is_dir():
        print(f"ERROR: --local-dir does not exist or is not a directory: {root}",
              file=sys.stderr)
        return 2

    safetensors_path = root / "model.safetensors"
    rvf_path = root / "model.rvf.jsonl"
    presence_path = root / "presence-head.json"
    config_path = root / "config.json"
    metrics_path = root / "training-metrics.json"

    for p in (safetensors_path, rvf_path, presence_path, config_path, metrics_path):
        if not p.is_file():
            print(f"ERROR: required file missing: {p}", file=sys.stderr)
            return 2

    print(f"Verifying HF bundle at: {root}")

    try:
        _section("model.safetensors")
        state, _loader_note, tensor_info = _verify_safetensors(safetensors_path)

        _section("model.rvf.jsonl")
        _verify_jsonl(rvf_path)

        _section("presence-head.json (depth <= 3)")
        _verify_presence_head(presence_path)

        _section("config.json")
        config = _verify_config(config_path)

        _section("training-metrics.json (final metrics)")
        _verify_training_metrics(metrics_path)

        _section("encoder.w1 forward-pass smoke test")
        _verify_first_linear(state, config, tensor_info)
    except Exception as exc:  # noqa: BLE001 - surface anything as a clear failure
        print(f"\nFAIL: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1

    _section("OK - all checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

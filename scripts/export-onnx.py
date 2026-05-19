#!/usr/bin/env python3
"""Export pose_v1.safetensors -> pose_v1.onnx.

Builds the same architecture as v2/crates/cog-pose-estimation/src/inference.rs
in PyTorch, loads the trained weights from safetensors, and runs a torch.onnx
export with a fixed [1, 56, 20] input. Then verifies the ONNX loads and
matches the torch output to within 1e-5.
"""

import json
import struct
import sys
from pathlib import Path

import numpy as np
import torch
import torch.nn as nn


N_SUB = 56
N_FRAMES = 20
N_KP = 17


class PoseNet(nn.Module):
    """Mirrors inference.rs::PoseNet exactly."""

    def __init__(self) -> None:
        super().__init__()
        self.c1 = nn.Conv1d(N_SUB, 64, kernel_size=3, padding=1, dilation=1)
        self.c2 = nn.Conv1d(64, 128, kernel_size=3, padding=2, dilation=2)
        self.c3 = nn.Conv1d(128, 128, kernel_size=3, padding=4, dilation=4)
        self.fc1 = nn.Linear(128, 256)
        self.fc2 = nn.Linear(256, N_KP * 2)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # x: [B, 56, 20]
        h = torch.relu(self.c1(x))
        h = torch.relu(self.c2(h))
        h = torch.relu(self.c3(h))
        h = h.mean(dim=2)            # [B, 128]
        h = torch.relu(self.fc1(h))
        h = torch.sigmoid(self.fc2(h))
        return h


def load_safetensors(path: Path) -> dict[str, torch.Tensor]:
    """Pure-python safetensors reader. Avoids the safetensors pip dep."""
    with path.open("rb") as f:
        header_len = struct.unpack("<Q", f.read(8))[0]
        header = json.loads(f.read(header_len).decode("utf-8"))
        out: dict[str, torch.Tensor] = {}
        for name, meta in header.items():
            if name == "__metadata__":
                continue
            start, end = meta["data_offsets"]
            shape = meta["shape"]
            dtype = meta["dtype"]
            assert dtype == "F32", f"unsupported dtype {dtype} for {name}"
            f.seek(8 + header_len + start)
            buf = f.read(end - start)
            arr = np.frombuffer(buf, dtype=np.float32).copy().reshape(shape)
            out[name] = torch.from_numpy(arr)
    return out


def main() -> None:
    weights_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("pose_v1.safetensors")
    out_path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("pose_v1.onnx")

    if not weights_path.exists():
        raise SystemExit(f"weights file not found: {weights_path}")

    print(f"reading {weights_path}")
    tensors = load_safetensors(weights_path)
    print(f"  found {len(tensors)} tensors: {sorted(tensors.keys())}")

    model = PoseNet()
    # Map safetensors names (enc.c1.weight, head.fc1.weight, ...) to module params
    mapping = {
        "enc.c1.weight": "c1.weight",
        "enc.c1.bias": "c1.bias",
        "enc.c2.weight": "c2.weight",
        "enc.c2.bias": "c2.bias",
        "enc.c3.weight": "c3.weight",
        "enc.c3.bias": "c3.bias",
        "head.fc1.weight": "fc1.weight",
        "head.fc1.bias": "fc1.bias",
        "head.fc2.weight": "fc2.weight",
        "head.fc2.bias": "fc2.bias",
    }
    state = {dst: tensors[src] for src, dst in mapping.items()}
    model.load_state_dict(state)
    model.eval()
    print("  weights loaded into PyTorch model")

    # Sanity check forward
    x = torch.zeros(1, N_SUB, N_FRAMES)
    with torch.no_grad():
        y = model(x)
    print(f"  zero-input forward: shape={tuple(y.shape)} sample={y[0, :4].tolist()}")

    # Export to ONNX
    torch.onnx.export(
        model,
        x,
        out_path,
        export_params=True,
        opset_version=18,
        do_constant_folding=True,
        input_names=["csi_window"],
        output_names=["keypoints"],
        dynamic_axes={"csi_window": {0: "batch"}, "keypoints": {0: "batch"}},
    )
    print(f"  wrote {out_path} ({out_path.stat().st_size} bytes)")

    # Verify the ONNX file loads + matches torch output
    try:
        import onnx
        import onnxruntime as ort

        onnx_model = onnx.load(str(out_path))
        onnx.checker.check_model(onnx_model)
        print("  ONNX model checker: ok")

        sess = ort.InferenceSession(str(out_path), providers=["CPUExecutionProvider"])
        rng = np.random.default_rng(42)
        x_np = rng.standard_normal((1, N_SUB, N_FRAMES), dtype=np.float32)
        with torch.no_grad():
            y_torch = model(torch.from_numpy(x_np)).numpy()
        y_onnx = sess.run(["keypoints"], {"csi_window": x_np})[0]
        max_abs = float(np.max(np.abs(y_torch - y_onnx)))
        print(f"  parity vs torch: max |torch - onnx| = {max_abs:.2e}")
        assert max_abs < 1e-5, "ONNX output diverges from torch output"
        print("  parity ok (<1e-5)")
    except ImportError as e:
        print(f"  WARN: onnx/onnxruntime not installed, skipping verification: {e}")

    print("\nDone.")


if __name__ == "__main__":
    main()

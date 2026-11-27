"""Self-contained regression test for the RuView calibration service.

Exercises the committed CLI end-to-end on synthetic data (CPU, no GPU, no real checkpoint):
  build a base -> calibrate.py fits an adapter -> infer.py runs base+adapter -> assert the
  adapter is small, inference is shape-correct and finite, and the adapter actually changes output.

Run:  python test_calibration.py    (or via pytest)
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import torch

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
from model import PoseNet, standardize  # noqa: E402


def _make_base(path: Path):
    torch.manual_seed(0)
    net = PoseNet()
    # Save without the deterministic gr.A buffer (mirrors the published checkpoint;
    # calibrate.py/infer.py load with strict=False).
    sd = {k: v for k, v in net.state_dict().items() if k != "gr.A"}
    torch.save(sd, path)


def _make_data(path: Path, n: int, seed: int):
    rng = np.random.default_rng(seed)
    X = rng.standard_normal((n, 3, 114, 10)).astype(np.float32)
    Y = rng.random((n, 17, 2)).astype(np.float32)  # keypoints in [0,1]
    np.savez(path, X=X, Y=Y)


def _run(*args):
    r = subprocess.run(
        [sys.executable, str(HERE / args[0]), *map(str, args[1:])],
        capture_output=True, text=True,
    )
    assert r.returncode == 0, f"{args[0]} failed:\n{r.stdout}\n{r.stderr}"
    return r.stdout


def test_calibration_end_to_end():
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        base = d / "base.pt"
        calib = d / "calib.npz"
        frames = d / "frames.npz"
        adapter = d / "room.adapter.npz"
        kp = d / "kp.npy"

        _make_base(base)
        _make_data(calib, n=40, seed=1)     # ≥20 → no underfit warning
        _make_data(frames, n=16, seed=2)

        # 1) calibrate -> adapter
        out = _run("calibrate.py", "--base", base, "--data", calib, "--out", adapter,
                   "--iters", "50", "--device", "cpu")
        assert adapter.exists(), "adapter not written"
        assert "saved" in out.lower()
        sz = adapter.stat().st_size
        assert sz < 200_000, f"adapter unexpectedly large ({sz} bytes)"

        # adapter contains the expected LoRA tensors (materialize + close so the
        # Windows tempdir can be cleaned up — np.load keeps a lazy file handle).
        with np.load(adapter) as z:
            keys = [k for k in z.files if k.endswith(".A") or k.endswith(".B")]
            assert keys, f"adapter has no LoRA tensors: {z.files}"
            lora = {k: z[k].astype(np.float32) for k in keys}

        # 2) infer with adapter -> keypoints
        _run("infer.py", "--base", base, "--adapter", adapter, "--data", frames,
             "--out", kp, "--device", "cpu")
        out_kp = np.load(kp)
        assert out_kp.shape == (16, 17, 2), f"bad keypoint shape {out_kp.shape}"
        assert np.isfinite(out_kp).all(), "non-finite keypoints"
        assert (out_kp >= 0).all() and (out_kp <= 1).all(), "keypoints out of [0,1]"

        # 3) adapter must actually change the output vs the zero-shot base
        with np.load(frames) as fz:
            frames_x = fz["X"][:]
        net = PoseNet()
        net.load_state_dict(torch.load(base, map_location="cpu", weights_only=True), strict=False)
        net.eval()
        x = standardize(torch.tensor(frames_x))
        with torch.no_grad():
            base_kp = net(x).reshape(16, 17, 2).numpy()
        net.add_lora()
        net.load_lora(lora)
        net.eval()
        with torch.no_grad():
            cal_kp = net(x).reshape(16, 17, 2).numpy()
        assert np.abs(base_kp - cal_kp).sum() > 1e-4, "adapter did not change output"


if __name__ == "__main__":
    test_calibration_end_to_end()
    print("PASS: calibration service end-to-end (calibrate -> adapter -> infer)")

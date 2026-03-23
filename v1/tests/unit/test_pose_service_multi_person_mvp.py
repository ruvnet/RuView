import importlib.util
from pathlib import Path

import numpy as np


MODULE_PATH = Path(__file__).resolve().parents[2] / "src" / "services" / "multi_person_mvp.py"
_spec = importlib.util.spec_from_file_location("multi_person_mvp", MODULE_PATH)
_module = importlib.util.module_from_spec(_spec)
assert _spec is not None and _spec.loader is not None
_spec.loader.exec_module(_module)
apply_multi_person_mvp = _module.apply_multi_person_mvp


def _single_pose() -> dict:
    return {
        "person_id": "0",
        "confidence": 0.92,
        "bounding_box": {"x": 0.20, "y": 0.25, "width": 0.30, "height": 0.45},
        "keypoints": [
            {"name": "nose", "x": 0.30, "y": 0.20, "confidence": 0.9},
            {"name": "left_shoulder", "x": 0.28, "y": 0.33, "confidence": 0.88},
        ],
        "activity": "walking",
        "timestamp": "2026-03-03T00:00:00",
    }


def test_mvp_promotes_to_two_persons_when_motion_energy_high():
    poses = [_single_pose()]

    # High motion energy: alternating values => large first-order deltas.
    high_motion_csi = np.array([0.0, 1.0, 0.0, 1.0, 0.0, 1.0], dtype=float)

    promoted = apply_multi_person_mvp(
        poses=poses,
        csi_data=high_motion_csi,
        max_persons=3,
        energy_threshold=0.35,
    )

    assert len(promoted) == 2
    assert promoted[0]["person_id"] == "0"
    assert promoted[1]["person_id"].endswith("_mvp2")
    assert promoted[1]["confidence"] < promoted[0]["confidence"]
    assert promoted[1]["bounding_box"]["x"] > promoted[0]["bounding_box"]["x"]


def test_mvp_keeps_single_person_when_motion_energy_low():
    poses = [_single_pose()]

    low_motion_csi = np.array([0.10, 0.11, 0.10, 0.11, 0.10], dtype=float)

    result = apply_multi_person_mvp(
        poses=poses,
        csi_data=low_motion_csi,
        max_persons=3,
        energy_threshold=0.35,
    )

    assert len(result) == 1
    assert result[0]["person_id"] == "0"

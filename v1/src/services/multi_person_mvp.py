"""Lightweight multi-person MVP helpers for single-stream CSI pipelines."""

from __future__ import annotations

import copy
from datetime import datetime
from typing import Any, Dict, List

import numpy as np


def estimate_motion_energy(csi_data: np.ndarray) -> float:
    """Estimate motion energy from CSI amplitude deltas."""
    array = np.asarray(csi_data, dtype=float)
    if array.size < 2:
        return 0.0
    flattened = array.reshape(-1)
    deltas = np.diff(flattened)
    return float(np.mean(np.abs(deltas)))


def synthesize_secondary_person(pose: Dict[str, Any]) -> Dict[str, Any]:
    """Generate a synthetic second person with a small spatial offset."""
    synthetic = copy.deepcopy(pose)
    synthetic["person_id"] = f"{pose.get('person_id', 0)}_mvp2"
    synthetic["confidence"] = max(0.05, float(pose.get("confidence", 0.0)) * 0.85)

    bbox = synthetic.get("bounding_box") or {"x": 0.0, "y": 0.0, "width": 0.0, "height": 0.0}
    width = float(bbox.get("width", 0.0) or 0.0)
    x = float(bbox.get("x", 0.0) or 0.0)
    bbox["x"] = min(1.0, max(0.0, x + max(0.06, width * 0.35)))
    synthetic["bounding_box"] = bbox

    keypoints = synthetic.get("keypoints")
    if isinstance(keypoints, list):
        shifted = []
        for kp in keypoints:
            if not isinstance(kp, dict):
                shifted.append(kp)
                continue
            shifted_kp = dict(kp)
            if isinstance(shifted_kp.get("x"), (int, float)):
                shifted_kp["x"] = min(1.0, max(0.0, float(shifted_kp["x"]) + 0.08))
            shifted.append(shifted_kp)
        synthetic["keypoints"] = shifted

    synthetic["activity"] = pose.get("activity", "standing")
    synthetic["timestamp"] = datetime.now().isoformat()
    return synthetic


def apply_multi_person_mvp(
    poses: List[Dict[str, Any]],
    csi_data: np.ndarray,
    *,
    max_persons: int,
    energy_threshold: float,
) -> List[Dict[str, Any]]:
    """Promote single-person result to two persons when motion energy is high."""
    if len(poses) != 1 or max_persons < 2:
        return poses

    energy = estimate_motion_energy(csi_data)
    if energy <= float(energy_threshold):
        return poses

    return [copy.deepcopy(poses[0]), synthesize_secondary_person(poses[0])]

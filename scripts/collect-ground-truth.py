#!/usr/bin/env python3
"""Camera ground-truth collection for WiFi pose estimation training (ADR-079).

Captures webcam keypoints via MediaPipe PoseLandmarker (Tasks API) and
synchronizes with ESP32 CSI recording from the sensing server.

Output: JSONL file in data/ground-truth/ with per-frame 17-keypoint COCO poses.

With --calibration <bundle.json> (produced by scripts/calibrate-camera-room.py,
ADR-152 S2.1.3), every record is additionally stamped with room-frame bearing
rays for each keypoint, the calibration_id, and the transceiver geometry --
the PerceptAlign-style defense against coordinate overfitting. Raw image
coordinates are always kept; without depth the room-frame representation is
a projective alignment (rays, not 3D points) -- see scripts/calibration_lib.py.
Without --calibration the output is byte-identical to the original ADR-079
format.

Usage:
    python scripts/collect-ground-truth.py --preview --duration 60
    python scripts/collect-ground-truth.py --server http://192.168.1.10:3000
    python scripts/collect-ground-truth.py --calibration data/calibration/camera-room.json
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import sys
import time
import urllib.request
import urllib.error
from pathlib import Path
from datetime import datetime

import cv2
import numpy as np

import mediapipe as mp
from mediapipe.tasks.python import BaseOptions
from mediapipe.tasks.python.vision import (
    PoseLandmarker,
    PoseLandmarkerOptions,
    RunningMode,
)

# ---------------------------------------------------------------------------
# MediaPipe 33 landmarks -> 17 COCO keypoints
# ---------------------------------------------------------------------------
# COCO idx : MP idx : joint name
#   0       :   0   : nose
#   1       :   2   : left_eye
#   2       :   5   : right_eye
#   3       :   7   : left_ear
#   4       :   8   : right_ear
#   5       :  11   : left_shoulder
#   6       :  12   : right_shoulder
#   7       :  13   : left_elbow
#   8       :  14   : right_elbow
#   9       :  15   : left_wrist
#  10       :  16   : right_wrist
#  11       :  23   : left_hip
#  12       :  24   : right_hip
#  13       :  25   : left_knee
#  14       :  26   : right_knee
#  15       :  27   : left_ankle
#  16       :  28   : right_ankle

MP_TO_COCO = [0, 2, 5, 7, 8, 11, 12, 13, 14, 15, 16, 23, 24, 25, 26, 27, 28]

COCO_BONES = [
    (5, 7), (7, 9), (6, 8), (8, 10),   # arms
    (5, 6),                              # shoulders
    (11, 13), (13, 15), (12, 14), (14, 16),  # legs
    (11, 12),                            # hips
    (5, 11), (6, 12),                    # torso
    (0, 1), (0, 2), (1, 3), (2, 4),     # face
]

MODEL_URL = (
    "https://storage.googleapis.com/mediapipe-models/"
    "pose_landmarker/pose_landmarker_lite/float16/latest/"
    "pose_landmarker_lite.task"
)
MODEL_FILENAME = "pose_landmarker_lite.task"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def ensure_model(cache_dir: Path) -> Path:
    """Download the PoseLandmarker model if not already cached."""
    model_path = cache_dir / MODEL_FILENAME
    if model_path.exists():
        return model_path

    cache_dir.mkdir(parents=True, exist_ok=True)
    print(f"Downloading {MODEL_FILENAME} ...")
    try:
        urllib.request.urlretrieve(MODEL_URL, str(model_path))
        print(f"  saved to {model_path}")
    except Exception as exc:
        print(f"ERROR: Failed to download model: {exc}", file=sys.stderr)
        print(
            "Download manually from:\n"
            f"  {MODEL_URL}\n"
            f"and place at {model_path}",
            file=sys.stderr,
        )
        sys.exit(1)
    return model_path


def load_api_token(token_file: str | None) -> str | None:
    """Bearer token for the sensing server: --token-file, else RUVIEW_API_TOKEN."""
    if token_file:
        try:
            return Path(token_file).read_text(encoding="utf-8").strip() or None
        except (OSError, UnicodeDecodeError) as exc:
            print(
                f"ERROR: cannot read --token-file {token_file!r}: {exc}",
                file=sys.stderr,
            )
            sys.exit(2)
    return os.environ.get("RUVIEW_API_TOKEN", "").strip() or None


def post_json(
    url: str,
    payload: dict | None = None,
    token: str | None = None,
    timeout: float = 5.0,
) -> bool:
    """POST JSON to a sensing-server endpoint.

    Returns True only when the server accepted the request. The recording
    endpoints answer HTTP 200 with ``{"success": false, "error": ...}`` for
    refusals such as "recording already in progress", so the body is checked.
    """
    data = json.dumps(payload or {}).encode("utf-8")
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read() or b"{}"
    except Exception as exc:
        print(f"WARNING: POST {url} failed: {exc}", file=sys.stderr)
        return False
    try:
        body = json.loads(raw)
    except ValueError as exc:
        print(
            f"WARNING: POST {url} returned a non-JSON body: {exc}",
            file=sys.stderr,
        )
        return False
    if not isinstance(body, dict):
        print(
            f"WARNING: POST {url} returned unexpected JSON "
            f"({type(body).__name__}), expected an object",
            file=sys.stderr,
        )
        return False
    if body.get("success") is False:
        print(f"WARNING: POST {url} refused: {body.get('error')}", file=sys.stderr)
        return False
    return True


def draw_skeleton(frame: np.ndarray, keypoints: list[list[float]], w: int, h: int):
    """Draw COCO skeleton overlay on a BGR frame."""
    pts = []
    for x, y in keypoints:
        px, py = int(x * w), int(y * h)
        pts.append((px, py))
        cv2.circle(frame, (px, py), 4, (0, 255, 0), -1)

    for i, j in COCO_BONES:
        if i < len(pts) and j < len(pts):
            cv2.line(frame, pts[i], pts[j], (0, 200, 255), 2)


# ---------------------------------------------------------------------------
# Main collection loop
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Collect camera ground-truth keypoints for WiFi pose training (ADR-079)."
    )
    parser.add_argument(
        "--server",
        default="http://localhost:3000",
        help="Sensing server URL (default: http://localhost:3000)",
    )
    parser.add_argument(
        "--preview",
        action="store_true",
        help="Show live skeleton overlay window",
    )
    parser.add_argument(
        "--duration",
        type=int,
        default=300,
        help="Recording duration in seconds (default: 300)",
    )
    parser.add_argument(
        "--camera",
        type=int,
        default=0,
        help="Camera device index (default: 0)",
    )
    parser.add_argument(
        "--output",
        default="data/ground-truth",
        help="Output directory (default: data/ground-truth)",
    )
    parser.add_argument(
        "--calibration",
        default=None,
        help="Camera-room calibration bundle JSON from scripts/calibrate-camera-room.py "
        "(ADR-152 S2.1.3); adds room-frame keypoint rays + transceiver geometry "
        "to every record",
    )
    parser.add_argument(
        "--token-file",
        default=None,
        help="File holding the sensing-server API bearer token "
        "(default: the RUVIEW_API_TOKEN environment variable)",
    )
    parser.add_argument(
        "--allow-no-csi",
        action="store_true",
        help="Keep capturing camera keypoints even if the CSI recording cannot "
        "be started (the output is then unpaired and useless for training)",
    )
    args = parser.parse_args()
    api_token = load_api_token(args.token_file)

    if not args.calibration:
        print(
            "WARNING: no --calibration bundle; labels stay in raw camera coordinates "
            "and are layout-brittle (coordinate overfitting, ADR-152 S2.1.3) -- run "
            "scripts/calibrate-camera-room.py first.",
            file=sys.stderr,
        )

    # --- Resolve paths relative to repo root ---
    repo_root = Path(__file__).resolve().parent.parent
    output_dir = repo_root / args.output
    output_dir.mkdir(parents=True, exist_ok=True)
    cache_dir = repo_root / "data" / ".cache"

    # --- Download / locate model ---
    model_path = ensure_model(cache_dir)

    # --- Open camera ---
    cap = cv2.VideoCapture(args.camera)
    if not cap.isOpened():
        print(
            f"ERROR: Cannot open camera index {args.camera}. "
            "Check that a webcam is connected and not in use by another app.",
            file=sys.stderr,
        )
        sys.exit(1)

    frame_w = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
    frame_h = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
    print(f"Camera opened: {frame_w}x{frame_h}")

    # --- Load calibration bundle (ADR-152 S2.1.3) ---
    calib_ctx = None
    if args.calibration:
        # Lazy import keeps the no-calibration path identical to the original.
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import calibration_lib

        try:
            calib_ctx = calibration_lib.load_calibration_context(
                Path(args.calibration), frame_w, frame_h
            )
        except (OSError, ValueError, json.JSONDecodeError) as exc:
            print(f"ERROR: Cannot load calibration bundle {args.calibration}: {exc}",
                  file=sys.stderr)
            sys.exit(1)
        n_nodes = len(calib_ctx.transceiver_geometry.get("nodes", []))
        print(f"Calibration: {calib_ctx.calibration_id[:23]}... "
              f"({n_nodes} transceiver node(s)); emitting room-frame keypoint rays")

    # --- Create PoseLandmarker ---
    options = PoseLandmarkerOptions(
        base_options=BaseOptions(model_asset_path=str(model_path)),
        running_mode=RunningMode.IMAGE,
        num_poses=1,
        min_pose_detection_confidence=0.5,
        min_pose_presence_confidence=0.5,
        min_tracking_confidence=0.5,
    )
    landmarker = PoseLandmarker.create_from_options(options)

    # --- Start CSI recording (named after the keypoints file so they pair) ---
    timestamp_str = datetime.now().strftime("%Y%m%d_%H%M%S")
    recording_id = f"gt_{timestamp_str}"
    recording_url_start = f"{args.server}/api/v1/recording/start"
    recording_url_stop = f"{args.server}/api/v1/recording/stop"
    csi_started = post_json(recording_url_start, {"id": recording_id}, api_token)
    if csi_started:
        print(f"CSI recording '{recording_id}' started on sensing server.")
    elif args.allow_no_csi:
        print(
            "WARNING: Could not start CSI recording; capturing unpaired "
            "camera keypoints (--allow-no-csi).",
            file=sys.stderr,
        )
    else:
        print(
            "ERROR: Could not start CSI recording, so keypoints would have no "
            "CSI to pair with. Check the server URL and RUVIEW_API_TOKEN / "
            "--token-file, or pass --allow-no-csi.",
            file=sys.stderr,
        )
        cap.release()
        landmarker.close()
        sys.exit(1)

    # --- Output file ---
    out_path = output_dir / f"keypoints_{timestamp_str}.jsonl"
    out_file = open(out_path, "w", encoding="utf-8")
    print(f"Output: {out_path}")

    # --- Graceful shutdown ---
    shutdown_requested = False

    def _handle_signal(signum, frame):
        nonlocal shutdown_requested
        shutdown_requested = True

    signal.signal(signal.SIGINT, _handle_signal)
    signal.signal(signal.SIGTERM, _handle_signal)

    # --- Collection loop ---
    start_time = time.monotonic()
    frame_count = 0
    total_confidence = 0.0
    total_visible = 0

    print(f"Collecting for {args.duration}s ... (press 'q' in preview to stop)")

    try:
        while not shutdown_requested:
            elapsed = time.monotonic() - start_time
            if elapsed >= args.duration:
                break

            ret, frame = cap.read()
            if not ret:
                print("WARNING: Failed to read frame, retrying ...", file=sys.stderr)
                time.sleep(0.01)
                continue

            ts_ns = time.time_ns()

            # Convert BGR -> RGB for MediaPipe
            rgb = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
            mp_image = mp.Image(image_format=mp.ImageFormat.SRGB, data=rgb)

            result = landmarker.detect(mp_image)

            n_persons = len(result.pose_landmarks)

            if n_persons > 0:
                landmarks = result.pose_landmarks[0]
                keypoints = []
                visibilities = []
                for coco_idx in range(17):
                    mp_idx = MP_TO_COCO[coco_idx]
                    lm = landmarks[mp_idx]
                    keypoints.append([round(lm.x, 5), round(lm.y, 5)])
                    visibilities.append(lm.visibility if lm.visibility else 0.0)

                confidence = float(np.mean(visibilities))
                n_visible = int(sum(1 for v in visibilities if v > 0.5))
            else:
                keypoints = []
                confidence = 0.0
                n_visible = 0

            record = {
                "ts_ns": ts_ns,
                "keypoints": keypoints,
                "confidence": round(confidence, 4),
                "n_visible": n_visible,
                "n_persons": n_persons,
            }
            if calib_ctx is not None:
                # Adds keypoints_room (bearing rays), camera_origin_room,
                # calibration_id, transceiver_geometry (ADR-152 S2.1.3).
                record = calibration_lib.augment_record(record, calib_ctx)
            out_file.write(json.dumps(record) + "\n")
            frame_count += 1
            total_confidence += confidence
            total_visible += n_visible

            # Preview overlay
            if args.preview and keypoints:
                draw_skeleton(frame, keypoints, frame_w, frame_h)

            if args.preview:
                remaining = max(0, int(args.duration - elapsed))
                cv2.putText(
                    frame,
                    f"Frames: {frame_count}  Visible: {n_visible}/17  Time: {remaining}s",
                    (10, 30),
                    cv2.FONT_HERSHEY_SIMPLEX,
                    0.7,
                    (255, 255, 255),
                    2,
                )
                cv2.imshow("Ground Truth Collection (ADR-079)", frame)
                if cv2.waitKey(1) & 0xFF == ord("q"):
                    break

    finally:
        # --- Cleanup ---
        out_file.close()
        cap.release()
        if args.preview:
            cv2.destroyAllWindows()
        landmarker.close()

        # Stop CSI recording
        if csi_started:
            if post_json(recording_url_stop, token=api_token):
                print("CSI recording stopped.")
            else:
                print("WARNING: Failed to stop CSI recording.", file=sys.stderr)

        # --- Summary ---
        avg_conf = total_confidence / frame_count if frame_count > 0 else 0.0
        avg_vis = total_visible / frame_count if frame_count > 0 else 0.0
        print()
        print("=== Collection Summary ===")
        print(f"  Total frames:      {frame_count}")
        print(f"  Avg confidence:    {avg_conf:.3f}")
        print(f"  Avg visible joints: {avg_vis:.1f} / 17")
        print(f"  Output:            {out_path}")


if __name__ == "__main__":
    main()

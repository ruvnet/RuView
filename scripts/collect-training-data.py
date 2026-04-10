#!/usr/bin/env python3
"""
WiFi-DensePose Training Data Collector

Listens on UDP for CSI data from ESP32 nodes and records to .csi.jsonl
files compatible with the Rust training pipeline (MmFiDataset / CsiDataset).

Supports two packet formats:
  - ADR-069 feature vectors (magic 0xC5110003, 48 bytes) — 8-dim pre-extracted
  - ADR-018 raw CSI frames (V1/V2, variable) — full subcarrier data

Usage:
    # Interactive — prompts for scenario labels
    python scripts/collect-training-data.py --port 5006

    # Scripted — fixed label, 60s per recording
    python scripts/collect-training-data.py --port 5006 --label walking --duration 60

    # Multiple scenarios in sequence
    python scripts/collect-training-data.py --port 5006 --scenarios walking,standing,sitting --duration 30

    # Dual-node collection (two ESP32s on different ports)
    python scripts/collect-training-data.py --port 5005 --port2 5006 --label walking

    # Generate manifest only from existing recordings
    python scripts/collect-training-data.py --manifest-only --output-dir data/recordings

Prerequisites:
    - ESP32 nodes streaming CSI on UDP (see firmware/esp32-csi-node)
    - Python 3.9+
"""

from __future__ import annotations

import argparse
import json
import logging
import os
import socket
import struct
import sys
import time
import signal
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%H:%M:%S",
)
log = logging.getLogger("collect-data")

# ── Packet formats (must match firmware) ─────────────────────────────────────

# ADR-018 raw CSI frame header
MAGIC_CSI_RAW_V1 = 0xC5110001
MAGIC_CSI_RAW_V2 = 0xC5110006
# ADR-069 feature vector packet
MAGIC_FEATURES = 0xC5110003
FEATURE_PKT_FMT = "<IBBHq8f"
FEATURE_PKT_SIZE = struct.calcsize(FEATURE_PKT_FMT)  # 48 bytes

RAW_CSI_HEADER_SIZE_V1 = 20
RAW_CSI_HEADER_SIZE_V2 = 26
RAW_CSI_MAX_ANTENNAS = 4
RAW_CSI_MAX_SUBCARRIERS = 256


# ── Packet parsing ───────────────────────────────────────────────────────────

def parse_packet(data: bytes) -> Optional[dict]:
    """Parse a UDP packet into a frame dict, or None if unrecognized."""
    if len(data) < 4:
        return None

    raw_packet = _parse_raw_csi_packet(data)
    if raw_packet is not None:
        return raw_packet

    magic = struct.unpack_from("<I", data)[0]
    if magic == MAGIC_FEATURES and len(data) >= FEATURE_PKT_SIZE:
        return _parse_feature_packet(data)
    return None


def _parse_feature_packet(data: bytes) -> Optional[dict]:
    """Parse ADR-069 feature vector packet (48 bytes)."""
    try:
        magic, node_id, _, seq, ts_us, *features = struct.unpack_from(FEATURE_PKT_FMT, data)
    except struct.error:
        return None

    if magic != MAGIC_FEATURES:
        return None

    # Reject NaN/inf
    import math
    if any(math.isnan(f) or math.isinf(f) for f in features):
        return None

    return {
        "type": "features",
        "node_id": node_id,
        "seq": seq,
        "timestamp_us": ts_us,
        "timestamp": ts_us / 1_000_000.0,
        "features": features,
        "subcarriers": features,  # Use features as subcarrier proxy for training
        "rssi": 0.0,
        "noise_floor": 0.0,
    }


def _parse_raw_csi_packet(data: bytes) -> Optional[dict]:
    """Parse ADR-018 raw CSI frames (V1/V2) with full subcarrier data."""
    if len(data) < RAW_CSI_HEADER_SIZE_V1:
        return None

    magic = struct.unpack_from("<I", data)[0]
    if magic == MAGIC_CSI_RAW_V1:
        header_size = RAW_CSI_HEADER_SIZE_V1
        source_mac = None
    elif magic == MAGIC_CSI_RAW_V2:
        if len(data) < RAW_CSI_HEADER_SIZE_V2:
            return None
        header_size = RAW_CSI_HEADER_SIZE_V2
        source_mac = ":".join(f"{b:02x}" for b in data[20:26])
    else:
        return None

    node_id = data[4]
    n_antennas = data[5]
    n_sub = struct.unpack_from("<H", data, 6)[0]
    if (
        n_antennas == 0 or n_antennas > RAW_CSI_MAX_ANTENNAS or
        n_sub == 0 or n_sub > RAW_CSI_MAX_SUBCARRIERS
    ):
        return None

    freq_mhz = struct.unpack_from("<I", data, 8)[0]
    seq = struct.unpack_from("<I", data, 12)[0]
    rssi = struct.unpack_from("<b", data, 16)[0]
    noise = struct.unpack_from("<b", data, 17)[0]

    expected_bytes = n_sub * n_antennas * 2
    if len(data) < header_size + expected_bytes:
        return None

    def channel_from_freq_mhz(freq: int) -> int:
        if 2412 <= freq <= 2472:
            return round((freq - 2412) / 5) + 1
        if freq == 2484:
            return 14
        if freq >= 5005:
            return round((freq - 5000) / 5)
        return 0

    iq_data = data[header_size:header_size + expected_bytes]
    subcarriers = []
    for i in range(0, n_sub * 2, 2):
        real = struct.unpack_from("<b", iq_data, i)[0]
        imag = struct.unpack_from("<b", iq_data, i + 1)[0]
        amplitude = (real ** 2 + imag ** 2) ** 0.5
        subcarriers.append(amplitude)

    frame = {
        "type": "raw_csi",
        "node_id": node_id,
        "antenna_config": n_antennas,
        "n_subcarriers": n_sub,
        "channel": channel_from_freq_mhz(freq_mhz),
        "freq_mhz": freq_mhz,
        "sequence": seq,
        "timestamp": time.time(),
        "subcarriers": subcarriers,
        "rssi": float(rssi),
        "noise_floor": float(noise),
    }
    if source_mac is not None:
        frame["source_mac"] = source_mac
    return frame


# ── JSONL recording ──────────────────────────────────────────────────────────

class CsiRecorder:
    """Records CSI frames to .csi.jsonl files compatible with the Rust pipeline."""

    def __init__(self, output_dir: str, session_name: str, label: Optional[str] = None):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

        ts = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
        safe_name = session_name.replace(" ", "_").replace("/", "_")
        self.session_id = f"{safe_name}-{ts}"
        self.label = label
        self.file_path = self.output_dir / f"{self.session_id}.csi.jsonl"
        self.meta_path = self.output_dir / f"{self.session_id}.csi.meta.json"
        self.frame_count = 0
        self.start_time = time.time()
        self.started_at = datetime.now(timezone.utc).isoformat()
        self._file = None

    def open(self):
        self._file = open(self.file_path, "a", encoding="utf-8")
        log.info(f"Recording to: {self.file_path}")

    def write_frame(self, frame: dict):
        """Write a single frame as a JSONL line."""
        if self._file is None:
            return

        record = {
            "timestamp": frame.get("timestamp", time.time()),
            "subcarriers": frame.get("subcarriers", []),
            "rssi": frame.get("rssi", 0.0),
            "noise_floor": frame.get("noise_floor", 0.0),
            "features": {
                k: v for k, v in frame.items()
                if k not in ("timestamp", "subcarriers", "rssi", "noise_floor", "type")
            },
        }

        line = json.dumps(record, separators=(",", ":"))
        self._file.write(line + "\n")
        self.frame_count += 1

        if self.frame_count % 500 == 0:
            self._file.flush()

    def close(self) -> dict:
        """Close the recording and write metadata. Returns session info."""
        if self._file:
            self._file.flush()
            self._file.close()
            self._file = None

        ended_at = datetime.now(timezone.utc).isoformat()
        elapsed = time.time() - self.start_time
        file_size = self.file_path.stat().st_size if self.file_path.exists() else 0

        meta = {
            "id": self.session_id,
            "name": self.session_id,
            "label": self.label,
            "started_at": self.started_at,
            "ended_at": ended_at,
            "duration_secs": round(elapsed, 2),
            "frame_count": self.frame_count,
            "file_size_bytes": file_size,
            "file_path": str(self.file_path),
            "fps": round(self.frame_count / elapsed, 1) if elapsed > 0 else 0,
        }

        with open(self.meta_path, "w", encoding="utf-8") as f:
            json.dump(meta, f, indent=2)

        log.info(
            f"Recording stopped: {self.frame_count} frames in {elapsed:.1f}s "
            f"({meta['fps']} fps, {file_size / 1024:.1f} KB)"
        )
        return meta


# ── Manifest generation ──────────────────────────────────────────────────────

def generate_manifest(output_dir: str) -> dict:
    """Scan recordings directory and generate a dataset manifest JSON."""
    rec_dir = Path(output_dir)
    sessions = []

    for meta_file in sorted(rec_dir.glob("*.csi.meta.json")):
        try:
            with open(meta_file, "r") as f:
                meta = json.load(f)
            sessions.append(meta)
        except (json.JSONDecodeError, OSError) as e:
            log.warning(f"Skipping {meta_file}: {e}")

    # Aggregate stats
    total_frames = sum(s.get("frame_count", 0) for s in sessions)
    total_bytes = sum(s.get("file_size_bytes", 0) for s in sessions)
    labels = sorted(set(s.get("label", "unlabeled") or "unlabeled" for s in sessions))

    manifest = {
        "dataset": "wifi-densepose-csi",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "directory": str(rec_dir),
        "num_sessions": len(sessions),
        "total_frames": total_frames,
        "total_size_bytes": total_bytes,
        "total_size_mb": round(total_bytes / (1024 * 1024), 2),
        "labels": labels,
        "sessions": sessions,
    }

    manifest_path = rec_dir / "manifest.json"
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)

    log.info(
        f"Manifest: {len(sessions)} sessions, {total_frames} frames, "
        f"{manifest['total_size_mb']} MB, labels={labels}"
    )
    log.info(f"Written to: {manifest_path}")
    return manifest


# ── UDP listener ─────────────────────────────────────────────────────────────

def collect_session(
    port: int,
    port2: Optional[int],
    output_dir: str,
    label: str,
    duration: float,
    session_name: Optional[str] = None,
) -> dict:
    """Run a single collection session. Returns session metadata."""
    name = session_name or label or "session"
    recorder = CsiRecorder(output_dir, name, label)
    recorder.open()

    # Bind primary socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.bind(("0.0.0.0", port))
    sock.settimeout(1.0)
    sockets = [sock]

    # Bind secondary socket if specified
    if port2:
        sock2 = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock2.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock2.bind(("0.0.0.0", port2))
        sock2.settimeout(0.1)
        sockets.append(sock2)

    log.info(
        f"Collecting '{label}' for {duration}s on port(s) "
        f"{port}{f', {port2}' if port2 else ''}"
    )

    start = time.time()
    dropped = 0

    try:
        while time.time() - start < duration:
            for s in sockets:
                try:
                    data, addr = s.recvfrom(4096)
                except socket.timeout:
                    continue

                frame = parse_packet(data)
                if frame:
                    recorder.write_frame(frame)
                else:
                    dropped += 1

            # Progress update every 5s
            elapsed = time.time() - start
            if recorder.frame_count > 0 and int(elapsed) % 5 == 0 and int(elapsed) > 0:
                remaining = duration - elapsed
                if remaining > 0 and int(elapsed * 10) % 50 == 0:
                    log.info(
                        f"  {recorder.frame_count} frames collected, "
                        f"{remaining:.0f}s remaining..."
                    )
    except KeyboardInterrupt:
        log.info("Interrupted by user.")
    finally:
        for s in sockets:
            s.close()

    if dropped > 0:
        log.warning(f"  {dropped} unrecognized packets dropped")

    return recorder.close()


# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Collect CSI training data from ESP32 nodes via UDP",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Interactive label input
  python scripts/collect-training-data.py --port 5006

  # Fixed label, 60 seconds
  python scripts/collect-training-data.py --port 5006 --label walking --duration 60

  # Multiple scenarios
  python scripts/collect-training-data.py --port 5006 --scenarios walking,standing,sitting --duration 30

  # Dual ESP32 nodes
  python scripts/collect-training-data.py --port 5005 --port2 5006 --label test

  # Generate manifest from existing recordings
  python scripts/collect-training-data.py --manifest-only
""",
    )

    parser.add_argument("--port", type=int, default=5006, help="Primary UDP port (default: 5006)")
    parser.add_argument("--port2", type=int, default=None, help="Secondary UDP port for dual-node")
    parser.add_argument("--output-dir", default="data/recordings", help="Output directory (default: data/recordings)")
    parser.add_argument("--label", default=None, help="Activity label for the recording")
    parser.add_argument("--duration", type=float, default=30.0, help="Recording duration in seconds (default: 30)")
    parser.add_argument("--scenarios", default=None, help="Comma-separated list of scenarios to record sequentially")
    parser.add_argument("--pause", type=float, default=5.0, help="Pause between scenarios in seconds (default: 5)")
    parser.add_argument("--manifest-only", action="store_true", help="Only generate manifest from existing recordings")
    parser.add_argument("--repeats", type=int, default=1, help="Number of repeats per scenario (default: 1)")

    args = parser.parse_args()

    # Manifest-only mode
    if args.manifest_only:
        generate_manifest(args.output_dir)
        return

    # Collect scenarios
    all_sessions = []

    if args.scenarios:
        # Multi-scenario sequential collection
        scenarios = [s.strip() for s in args.scenarios.split(",") if s.strip()]
        total = len(scenarios) * args.repeats
        idx = 0

        for repeat in range(args.repeats):
            for scenario in scenarios:
                idx += 1
                print(f"\n{'='*60}")
                print(f"  Scenario {idx}/{total}: '{scenario}' (repeat {repeat+1}/{args.repeats})")
                print(f"  Duration: {args.duration}s")
                print(f"{'='*60}")

                if idx > 1:
                    print(f"  Starting in {args.pause}s... (get into position)")
                    time.sleep(args.pause)

                meta = collect_session(
                    port=args.port,
                    port2=args.port2,
                    output_dir=args.output_dir,
                    label=scenario,
                    duration=args.duration,
                    session_name=f"{scenario}_r{repeat+1:02d}",
                )
                all_sessions.append(meta)

    elif args.label:
        # Single labeled recording
        meta = collect_session(
            port=args.port,
            port2=args.port2,
            output_dir=args.output_dir,
            label=args.label,
            duration=args.duration,
        )
        all_sessions.append(meta)

    else:
        # Interactive mode — prompt for labels
        print("\nInteractive data collection mode.")
        print("Type a label for each recording, or 'q' to quit.\n")

        while True:
            label = input("Label (or 'q' to quit): ").strip()
            if label.lower() in ("q", "quit", "exit"):
                break
            if not label:
                print("  Empty label. Try again.")
                continue

            duration = args.duration
            try:
                dur_input = input(f"Duration in seconds [{duration}]: ").strip()
                if dur_input:
                    duration = float(dur_input)
            except ValueError:
                pass

            print(f"  Recording '{label}' for {duration}s — starting now...")
            meta = collect_session(
                port=args.port,
                port2=args.port2,
                output_dir=args.output_dir,
                label=label,
                duration=duration,
            )
            all_sessions.append(meta)
            print()

    # Generate manifest
    if all_sessions:
        print(f"\nCollected {len(all_sessions)} session(s).")
        manifest = generate_manifest(args.output_dir)

        total_frames = sum(s.get("frame_count", 0) for s in all_sessions)
        print(f"\nSummary:")
        print(f"  Sessions: {len(all_sessions)}")
        print(f"  Total frames: {total_frames}")
        print(f"  Output: {args.output_dir}/")
        print(f"  Manifest: {args.output_dir}/manifest.json")
    else:
        print("No sessions recorded.")


if __name__ == "__main__":
    main()

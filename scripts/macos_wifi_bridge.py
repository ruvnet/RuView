#!/usr/bin/env python3
"""Forward the canonical macOS CoreWLAN helper stream to the explicit bridge UDP source."""

from __future__ import annotations

import argparse
import json
import os
import socket
import subprocess
import sys
from pathlib import Path

BRIDGE_KIND = "connected_rssi"
DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 5006
DEFAULT_INTERVAL_MS = 100
HELPER_ENV_VAR = "RUVIEW_MAC_WIFI_HELPER"
REPO_HELPER_REL = Path("rust-port/wifi-densepose-rs/target/tools/macos-wifi-scan/macos-wifi-scan")

REQUIRED_FIELDS = {
    "timestamp",
    "interface",
    "ssid",
    "bssid",
    "bssid_synthetic",
    "rssi",
    "noise",
    "channel",
    "band",
    "tx_rate_mbps",
    "is_connected",
}


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be a positive integer")
    return parsed


def resolve_helper(explicit: str | None) -> str:
    if explicit:
        return explicit

    env_override = os.environ.get(HELPER_ENV_VAR)
    if env_override:
        return env_override

    repo_helper = repo_root() / REPO_HELPER_REL
    if repo_helper.is_file():
        return str(repo_helper)

    return "macos-wifi-scan"


def validate_record(record: object) -> dict[str, object]:
    if not isinstance(record, dict):
        raise ValueError("helper output must be a JSON object")

    missing = sorted(REQUIRED_FIELDS.difference(record))
    if missing:
        raise ValueError(f"helper output missing required fields: {', '.join(missing)}")

    if not record.get("is_connected", False):
        raise ValueError("helper stream record is not marked as connected")

    bridged = dict(record)
    bridged["bridge_kind"] = BRIDGE_KIND
    return bridged


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Forward macOS CoreWLAN helper records to the explicit RuView macOS bridge source."
    )
    parser.add_argument("--helper", help="Path to the macOS Wi-Fi helper binary")
    parser.add_argument("--host", default=DEFAULT_HOST, help="Bridge receiver host (default: 127.0.0.1)")
    parser.add_argument(
        "--port",
        type=positive_int,
        default=DEFAULT_PORT,
        help="Bridge receiver UDP port (default: 5006)",
    )
    parser.add_argument(
        "--interval-ms",
        type=positive_int,
        default=DEFAULT_INTERVAL_MS,
        help="Polling interval passed to the helper stream mode (default: 100)",
    )
    args = parser.parse_args()

    helper = resolve_helper(args.helper)
    command = [helper, "--stream", "--interval-ms", str(args.interval_ms)]

    try:
        process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=sys.stderr,
            text=True,
            bufsize=1,
        )
    except OSError as exc:
        print(
            f"failed to start macOS Wi-Fi helper '{helper}': {exc}. "
            f"Build it with scripts/build-mac-wifi.sh or set {HELPER_ENV_VAR}.",
            file=sys.stderr,
        )
        return 1

    destination = (args.host, args.port)
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

    try:
        assert process.stdout is not None
        for line in process.stdout:
            line = line.strip()
            if not line:
                continue

            try:
                record = validate_record(json.loads(line))
            except (json.JSONDecodeError, ValueError) as exc:
                print(f"skipping helper record: {exc}", file=sys.stderr)
                continue

            payload = json.dumps(record, separators=(",", ":")).encode("utf-8")
            sock.sendto(payload, destination)
    except KeyboardInterrupt:
        print("stopping macOS Wi-Fi bridge", file=sys.stderr)
    finally:
        sock.close()
        process.terminate()
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()

    return process.returncode or 0


if __name__ == "__main__":
    raise SystemExit(main())

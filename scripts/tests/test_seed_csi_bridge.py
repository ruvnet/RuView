#!/usr/bin/env python3
"""--raw-csi ingest policy in scripts/seed_csi_bridge.py.

On edge tier 2 a node sends ADR-069 features (~1 Hz), ADR-039 vitals (~1 Hz)
and raw ADR-018 CSI (~30-40 pps) to one UDP target. Raw frames only yield a
placeholder vector, so should_ingest() decides which packets reach the Seed.

The unit tests drive should_ingest() directly. The integration test runs the
real bridge process against a local fake Seed HTTP server and counts the
vectors it ingests. All data is SYNTHETIC; no hardware or Seed is needed.

Run:  python -m pytest scripts/tests/test_seed_csi_bridge.py -q --no-cov
"""

from __future__ import annotations

import json
import socket
import struct
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import seed_csi_bridge as bridge  # noqa: E402

STALE = bridge.REAL_FEATURE_STALE_S


def feature_packet(node: int, seq: int) -> bytes:
    return struct.pack(bridge.FEATURE_PKT_FMT, bridge.MAGIC_FEATURES, node, 0, seq,
                       int(time.time() * 1e6), 0.8, 0.2, 0.5, 0.6, 0.1, 0.25, 0.0, 0.4)


def raw_packet(node: int, rssi: int = -55) -> bytes:
    return struct.pack("<IBb", bridge.MAGIC_CSI_RAW, node, rssi) + bytes(120)


def pkt(kind: str, node: int) -> dict:
    return {"kind": kind, "node_id": node, "features": [0.0] * 8}


# ── parse_packet kind tagging ────────────────────────────────────────────────

def test_parse_packet_tags_raw_frames():
    parsed = bridge.parse_packet(raw_packet(3, rssi=-60))
    assert parsed["kind"] == "raw"
    assert parsed["node_id"] == 3


def test_parse_packet_tags_feature_packets():
    parsed = bridge.parse_packet(feature_packet(7, 1))
    assert parsed["kind"] == "features"
    assert parsed["node_id"] == 7


def test_parse_packet_ignores_unknown_magic():
    assert bridge.parse_packet(struct.pack("<I", 0xDEADBEEF) + bytes(44)) is None


# ── should_ingest policies ───────────────────────────────────────────────────

def test_ingest_keeps_everything():
    seen: dict[int, float] = {}
    assert bridge.should_ingest(pkt("raw", 1), seen, 0.0, "ingest")
    assert bridge.should_ingest(pkt("features", 1), seen, 0.0, "ingest")


@pytest.mark.parametrize("policy", ["drop", "fallback"])
def test_real_packets_always_pass_and_refresh(policy):
    seen: dict[int, float] = {}
    assert bridge.should_ingest(pkt("features", 1), seen, 10.0, policy)
    assert bridge.should_ingest(pkt("vitals", 1), seen, 10.5, policy)
    assert seen == {1: 10.5}


def test_drop_never_ingests_raw():
    seen: dict[int, float] = {}
    assert not bridge.should_ingest(pkt("raw", 1), seen, 0.0, "drop")  # never-seen node
    bridge.should_ingest(pkt("features", 1), seen, 1.0, "drop")
    assert not bridge.should_ingest(pkt("raw", 1), seen, 1.0 + STALE + 1, "drop")  # stale node


def test_fallback_admits_raw_from_never_seen_node():
    assert bridge.should_ingest(pkt("raw", 1), {}, 0.0, "fallback")


def test_fallback_blocks_raw_while_features_are_fresh():
    seen = {1: 1.0}
    assert not bridge.should_ingest(pkt("raw", 1), seen, 1.0 + STALE - 0.01, "fallback")


def test_fallback_admits_raw_once_features_go_stale():
    seen = {1: 1.0}
    assert bridge.should_ingest(pkt("raw", 1), seen, 1.0 + STALE + 0.01, "fallback")


def test_fallback_tracks_nodes_independently():
    seen = {1: 1.0}
    assert not bridge.should_ingest(pkt("raw", 1), seen, 2.0, "fallback")
    assert bridge.should_ingest(pkt("raw", 2), seen, 2.0, "fallback")


def test_raw_frames_never_refresh_freshness():
    seen: dict[int, float] = {}
    for t in range(10):
        bridge.should_ingest(pkt("raw", 2), seen, float(t), "fallback")
    assert 2 not in seen


# ── end to end: real bridge process + fake Seed ──────────────────────────────

class _FakeSeed(BaseHTTPRequestHandler):
    vectors: list = []
    status_calls = 0

    def log_message(self, *args):
        pass

    def _reply(self, obj: dict) -> None:
        body = json.dumps(obj).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        type(self).status_calls += 1
        self._reply({"device_id": "fake", "total_vectors": len(self.vectors),
                     "epoch": 1, "dimension": 8, "uptime_secs": 1})

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        data = json.loads(self.rfile.read(length) or b"{}")
        if self.path == "/api/v1/store/ingest":
            self.vectors.extend(data.get("vectors", []))
        self._reply({"ingested": len(data.get("vectors", []))})


def _free_udp_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _wait_until(predicate, timeout: float) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return True
        time.sleep(0.05)
    return False


def _run_bridge(policy: str) -> int:
    """Send node 1: 2 features + 60 raw; node 2: 60 raw. Return vectors ingested."""
    handler = type("Seed", (_FakeSeed,), {"vectors": [], "status_calls": 0})
    server = HTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    udp_port = _free_udp_port()
    proc = subprocess.Popen(
        [sys.executable, str(SCRIPTS_DIR / "seed_csi_bridge.py"),
         "--seed-url", f"http://127.0.0.1:{server.server_port}", "--token", "test",
         "--udp-port", str(udp_port), "--bind-addr", "127.0.0.1",
         "--allowed-sources", "127.0.0.1", "--raw-csi", policy,
         "--batch-size", "5", "--flush-interval", "1"],
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    try:
        assert _wait_until(lambda: handler.status_calls > 0, 10), "bridge never contacted Seed"
        time.sleep(0.3)  # status check happens just before the UDP bind
        dst = ("127.0.0.1", udp_port)
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.sendto(feature_packet(1, 1), dst)
            for i in range(60):
                s.sendto(raw_packet(1), dst)
                s.sendto(raw_packet(2), dst)
                if i == 30:
                    s.sendto(feature_packet(1, 2), dst)
                time.sleep(0.002)
        # Wait for the count to settle (time-based flush runs every ~1 s).
        last = -1
        while _wait_until(lambda: len(handler.vectors) != last, 2.5):
            last = len(handler.vectors)
        assert proc.poll() is None, proc.stderr.read()
        return len(handler.vectors)
    finally:
        proc.terminate()
        proc.wait(timeout=5)
        server.shutdown()


@pytest.mark.integration
@pytest.mark.parametrize("policy,expected", [
    ("ingest", 122),    # 2 features + 120 raw placeholders
    ("drop", 2),        # features only
    ("fallback", 62),   # features + node 2's raw (node 1 sent features)
])
def test_bridge_end_to_end(policy, expected):
    assert _run_bridge(policy) == expected

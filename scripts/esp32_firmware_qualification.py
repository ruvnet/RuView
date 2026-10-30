#!/usr/bin/env python3
"""ESP32 firmware release qualification contract (RuView#2092).

This module holds the logic behind ``scripts/qualify-esp32-firmware.py``. It
is importable so the contract can be unit tested without hardware.

The procedure is the one written down in
``docs/validation/2026-08-31-esp32-s3-rate-aware-transport.md``: after any
S3 timing, WiFi, transport, or task scheduling change, observe the node for
five minutes and pass only when the raw callback yield stays at or above
20 pps, the node frame coverage stays at or above 99 percent, and the device
and the server report zero send failures, offline frames, parser errors,
WebSocket errors, watchdogs, panics, and unexpected resets.

Two properties matter more than the mechanics:

* **Cumulative counters are differenced, never read absolutely.** The node's
  send-failure counter has been counting since boot, so a burn that observes
  a non-zero value has not necessarily failed. Only the change across the
  window is a failure.
* **The environment must be declared.** The acceptance thresholds were
  measured on one board, one access point, and one room. A verdict without
  the environment it was measured in is not evidence, so the harness refuses
  to run without an environment note.

Only aggregate counters are read. Nothing here touches raw CSI, amplitudes,
or per-person data (ADR-299).
"""

from __future__ import annotations

import base64
import json
import os
import re
import socket
import ssl
import struct
import time
from typing import Any, Dict, List, Optional

# The acceptance thresholds, as written in the 2026-08-31 qualification
# record. They are the default, but they are configurable so a run can use a
# threshold established for its own environment.
DEFAULT_MIN_YIELD_PPS = 20.0
DEFAULT_MIN_COVERAGE_PERCENT = 99.0
DEFAULT_SECONDS = 300

# -- Device console signatures ------------------------------------------------
# The 1 Hz controller tick (adaptive_controller.c, CONFIG_ADAPTIVE_MEDIUM_LOOP_MS
# = 1000). The yield is the raw CSI callback packets per second.
TICK_RE = re.compile(
    r"medium tick: state=(\d+)\s+yield=(\d+)pps\s+dsp=([0-9.]+)Hz"
    r"(?:\s+motion=(-?[0-9.]+)\s+presence=(-?[0-9.]+)\s+rssi=(-?\d+))?"
)
# The mesh sync counter line (c6_sync_espnow.c). Both counters are cumulative
# since boot, so only their change across the window means anything.
MESH_COUNTER_RE = re.compile(r"tx#(\d+)\s+\(fail=(\d+)\)")
# csi_collector.c increments a counter and logs the first five failures.
SEND_FAIL_RE = re.compile(r"sendto failed \(fail #(\d+)\)")
# The same module's back-off message when lwIP runs out of packet buffers.
ENOMEM_RE = re.compile(r"sendto ENOMEM")
# main.c trips the uplink watchdog when no send succeeds for too long.
UPLINK_WATCHDOG_RE = re.compile(r"uplink watchdog: no successful send for (\d+) s")
# main.c prints the reset reason at boot.
RESET_REASON_RE = re.compile(r"reset reason: (.+?) \((\d+)\)")
# Every ESP-IDF log line carries the milliseconds since boot. A decrease in
# that counter is a restart, whatever the boot reason was.
UPTIME_RE = re.compile(r"[IWE]\s+\((\d+)\)")

# The controller states (adaptive_controller.h). DEGRADED means the node
# decided its own radio is not delivering.
ADAPT_STATE_DEGRADED = 8


class QualificationError(Exception):
    """A usage or environment problem that prevents the run."""


# -- Minimal WebSocket client (RFC 6455) --------------------------------------
# The sensing server publishes the aggregated frame on /ws/sensing. The
# acceptance criteria need the frame STREAM (frame count, offline frames,
# gap between frames), not a polled snapshot, so a polling client cannot
# reproduce them.
class WebSocketClient:
    """A tiny text-frame WebSocket client built on the standard library."""

    def __init__(self, url: str, timeout: float = 10.0) -> None:
        self.url = url
        self.timeout = timeout
        self.sock: Optional[socket.socket] = None
        self._buffer = b""

    def connect(self) -> None:
        if not self.url.startswith(("ws://", "wss://")):
            raise QualificationError(f"not a WebSocket URL: {self.url}")
        secure = self.url.startswith("wss://")
        rest = self.url.split("://", 1)[1]
        hostport, _, path = rest.partition("/")
        path = "/" + path
        host, _, port_text = hostport.partition(":")
        port = int(port_text) if port_text else (443 if secure else 80)

        self.sock = socket.create_connection((host, port), timeout=self.timeout)
        if secure:
            context = ssl.create_default_context()
            self.sock = context.wrap_socket(self.sock, server_hostname=host)

        key = base64.b64encode(os.urandom(16)).decode("ascii")
        request = (
            f"GET {path} HTTP/1.1\r\n"
            f"Host: {hostport}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n"
            "\r\n"
        )
        self.sock.sendall(request.encode("ascii"))

        header = b""
        while b"\r\n\r\n" not in header:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise QualificationError("the server closed the connection during the handshake")
            header += chunk
        status_line = header.split(b"\r\n", 1)[0].decode("latin-1")
        if "101" not in status_line:
            raise QualificationError(f"the server refused the WebSocket upgrade: {status_line}")
        self._buffer = header.split(b"\r\n\r\n", 1)[1]

    def close(self) -> None:
        if self.sock is not None:
            try:
                self.sock.close()
            finally:
                self.sock = None

    def _read_exactly(self, count: int) -> bytes:
        while len(self._buffer) < count:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise ConnectionError("the WebSocket closed")
            self._buffer += chunk
        data, self._buffer = self._buffer[:count], self._buffer[count:]
        return data

    def _send_frame(self, opcode: int, payload: bytes) -> None:
        # Client frames must be masked.
        mask = os.urandom(4)
        masked = bytes(byte ^ mask[i % 4] for i, byte in enumerate(payload))
        header = bytearray([0x80 | opcode])
        length = len(payload)
        if length < 126:
            header.append(0x80 | length)
        elif length < (1 << 16):
            header.append(0x80 | 126)
            header += struct.pack(">H", length)
        else:
            header.append(0x80 | 127)
            header += struct.pack(">Q", length)
        header += mask
        self.sock.sendall(bytes(header) + masked)

    def recv_text(self, timeout: float) -> Optional[str]:
        """Return the next text message, or None on timeout or close."""
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return None
            self.sock.settimeout(remaining)
            try:
                first, second = self._read_exactly(2)
            except socket.timeout:
                return None
            fin = bool(first & 0x80)
            opcode = first & 0x0F
            masked = bool(second & 0x80)
            length = second & 0x7F
            if length == 126:
                length = struct.unpack(">H", self._read_exactly(2))[0]
            elif length == 127:
                length = struct.unpack(">Q", self._read_exactly(8))[0]
            mask = self._read_exactly(4) if masked else b""
            payload = self._read_exactly(length) if length else b""
            if masked:
                payload = bytes(byte ^ mask[i % 4] for i, byte in enumerate(payload))

            if opcode == 0x8:  # close
                return None
            if opcode == 0x9:  # ping -> answer with a pong
                self._send_frame(0xA, payload)
                continue
            if opcode == 0xA:  # pong
                continue
            if not fin:
                # A fragmented text message: keep reading until the final frame.
                chunks = [payload]
                while True:
                    first, second = self._read_exactly(2)
                    fin = bool(first & 0x80)
                    length = second & 0x7F
                    if length == 126:
                        length = struct.unpack(">H", self._read_exactly(2))[0]
                    elif length == 127:
                        length = struct.unpack(">Q", self._read_exactly(8))[0]
                    chunks.append(self._read_exactly(length) if length else b"")
                    if fin:
                        break
                payload = b"".join(chunks)
            if opcode in (0x1, 0x2):
                return payload.decode("utf-8", errors="replace")


# -- Frame inspection ----------------------------------------------------------
def find_node_ids(value: Any, found: Optional[set] = None) -> set:
    """Collect every node id that appears in a published frame.

    The published shape has changed over time, so the harness does not
    hard-code one path. It walks the frame and collects the values of any
    ``node_id`` field it finds, wherever they sit.
    """
    if found is None:
        found = set()
    if isinstance(value, dict):
        for key, item in value.items():
            if key in ("node_id", "nodeId") and isinstance(item, int):
                found.add(item)
            elif key in ("node_id", "nodeId") and isinstance(item, str) and item.isdigit():
                found.add(int(item))
            else:
                find_node_ids(item, found)
    elif isinstance(value, list):
        for item in value:
            find_node_ids(item, found)
    return found


# -- Device console parsing ----------------------------------------------------
def parse_console(text: str) -> Dict[str, Any]:
    """Turn a node console capture into the device-side acceptance counters."""
    yields: List[int] = []
    degraded_ticks = 0
    dsp_rates: List[float] = []
    send_fail_warnings = 0
    enomem = 0
    watchdogs = 0
    reset_reasons: List[str] = []
    mesh_fail_samples: List[int] = []
    last_uptime: Optional[int] = None
    uptime_regressions = 0

    for line in text.splitlines():
        uptime = UPTIME_RE.search(line)
        if uptime:
            current = int(uptime.group(1))
            if last_uptime is not None and current < last_uptime:
                # The boot counter went backwards: the node restarted.
                uptime_regressions += 1
            last_uptime = current

        tick = TICK_RE.search(line)
        if tick:
            if int(tick.group(1)) == ADAPT_STATE_DEGRADED:
                degraded_ticks += 1
            yields.append(int(tick.group(2)))
            dsp_rates.append(float(tick.group(3)))
            continue
        mesh = MESH_COUNTER_RE.search(line)
        if mesh:
            mesh_fail_samples.append(int(mesh.group(2)))
            continue
        if SEND_FAIL_RE.search(line):
            send_fail_warnings += 1
            continue
        if ENOMEM_RE.search(line):
            enomem += 1
            continue
        if UPLINK_WATCHDOG_RE.search(line):
            watchdogs += 1
            continue
        reset = RESET_REASON_RE.search(line)
        if reset:
            reset_reasons.append(reset.group(1).strip())

    # The mesh counter is cumulative since boot, so only its change across
    # the capture is a failure. A capture that starts after boot sees the
    # same value twice and observes no failure.
    mesh_fail_delta = 0
    if len(mesh_fail_samples) >= 2:
        mesh_fail_delta = max(0, mesh_fail_samples[-1] - mesh_fail_samples[0])

    yield_mean = (sum(yields) / len(yields)) if yields else 0.0
    return {
        "tick_samples": len(yields),
        "yield_pps_mean": round(yield_mean, 2),
        "yield_pps_min": min(yields) if yields else 0,
        "yield_pps_max": max(yields) if yields else 0,
        "degraded_ticks": degraded_ticks,
        "dsp_hz_mean": round(sum(dsp_rates) / len(dsp_rates), 2) if dsp_rates else 0.0,
        "mesh_send_failures": mesh_fail_delta,
        "send_fail_warnings": send_fail_warnings,
        "enomem_backoffs": enomem,
        "uplink_watchdogs": watchdogs,
        "restarts": uptime_regressions,
        "reset_reasons": reset_reasons,
    }


# -- Server-side observation ---------------------------------------------------
def observe_server(url: str, node_id: int, seconds: int) -> Dict[str, Any]:
    """Watch the sensing WebSocket for `seconds` and count the frames."""
    client = WebSocketClient(url)
    try:
        client.connect()
    except (OSError, QualificationError) as exc:
        raise QualificationError(f"cannot open {url}: {exc}") from exc

    frames = 0
    with_node = 0
    offline = 0
    parse_errors = 0
    ws_errors = 0
    early_closes = 0
    gaps: List[float] = []
    previous: Optional[float] = None
    started = time.monotonic()
    deadline = started + seconds

    try:
        while time.monotonic() < deadline:
            remaining = deadline - time.monotonic()
            message = client.recv_text(timeout=min(2.0, max(0.05, remaining)))
            now = time.monotonic()
            if message is None:
                if time.monotonic() >= deadline:
                    break
                # A closed stream before the window ended is an early close.
                early_closes += 1
                break
            frames += 1
            if previous is not None:
                gaps.append((now - previous) * 1000.0)
            previous = now
            try:
                payload = json.loads(message)
            except json.JSONDecodeError:
                parse_errors += 1
                continue
            if node_id in find_node_ids(payload):
                with_node += 1
            else:
                offline += 1
    except (OSError, ConnectionError):
        ws_errors += 1
    finally:
        client.close()

    elapsed = time.monotonic() - started
    coverage = (with_node / frames * 100.0) if frames else 0.0
    return {
        "seconds_observed": round(elapsed, 2),
        "frames": frames,
        "frames_with_node": with_node,
        "source_offline_frames": offline,
        "json_parse_errors": parse_errors,
        "websocket_errors": ws_errors,
        "early_closes": early_closes,
        "node_frame_coverage_percent": round(coverage, 3),
        "max_frame_gap_ms": round(max(gaps), 1) if gaps else 0.0,
    }


# -- Verdict -------------------------------------------------------------------
def evaluate(server: Dict[str, Any], device: Optional[Dict[str, Any]],
             min_yield_pps: float = DEFAULT_MIN_YIELD_PPS,
             min_coverage_percent: float = DEFAULT_MIN_COVERAGE_PERCENT) -> Dict[str, Any]:
    """Apply the documented acceptance criteria and return the findings."""
    findings: List[Dict[str, Any]] = []

    def check(name: str, ok: bool, measured: Any, required: Any, detail: str) -> None:
        findings.append({
            "criterion": name,
            "pass": bool(ok),
            "measured": measured,
            "required": required,
            "detail": detail,
        })

    if device is None:
        check("device counters", False, "not measured",
              "a console capture of the qualified node",
              "the device side of the acceptance needs a console capture")
    else:
        check("raw CSI yield",
              device["tick_samples"] > 0 and device["yield_pps_mean"] >= min_yield_pps,
              f"{device['yield_pps_mean']} pps over {device['tick_samples']} samples",
              f"at least {min_yield_pps} pps",
              "the 1 Hz controller tick reports the raw callback yield")
        check("device send failures", device["mesh_send_failures"] == 0,
              device["mesh_send_failures"], 0,
              "the change in the node's cumulative send-failure counter across the window")
        check("device ENOMEM back-offs", device["enomem_backoffs"] == 0,
              device["enomem_backoffs"], 0, "lwIP buffer exhaustion reported by the node")
        check("device watchdogs", device["uplink_watchdogs"] == 0,
              device["uplink_watchdogs"], 0, "uplink watchdog trips reported by the node")
        check("unexpected restarts", device["restarts"] == 0,
              device["restarts"], 0, "the node's boot counter went backwards during the window")

    check("node frame coverage",
          server["node_frame_coverage_percent"] >= min_coverage_percent,
          f"{server['node_frame_coverage_percent']} percent",
          f"at least {min_coverage_percent} percent",
          "the share of sensing frames that carry the qualified node")
    check("source offline frames", server["source_offline_frames"] == 0,
          server["source_offline_frames"], 0, "frames published without the node")
    check("JSON parse errors", server["json_parse_errors"] == 0,
          server["json_parse_errors"], 0, "frames that were not valid JSON")
    check("WebSocket errors", server["websocket_errors"] == 0,
          server["websocket_errors"], 0, "transport errors on the sensing stream")
    check("early closes", server["early_closes"] == 0,
          server["early_closes"], 0, "the stream closed before the window ended")
    check("observation window", server["frames"] > 0,
          f"{server['frames']} frames in {server['seconds_observed']} s",
          "a non-empty observation", "the burn has to observe something")

    failed = [f for f in findings if not f["pass"]]
    return {"pass": not failed, "findings": findings, "failed": len(failed)}


QUALIFICATION_MARKER = "ruview:esp32-firmware-qualification"


def qualification_marker(firmware_version: str, node_id: int, verdict: str) -> str:
    """The machine-readable line that binds a record to one firmware version.

    A record is only a qualification for the version it names, so the
    readiness gate reads this line rather than guessing from prose.
    """
    return (f"<!-- {QUALIFICATION_MARKER} version={firmware_version} "
            f"node={node_id} verdict={verdict} -->")


def render_markdown(result: Dict[str, Any]) -> str:
    """Render the qualification result as a Markdown report."""
    server = result["server"]
    device = result.get("device")
    lines: List[str] = []
    lines.append(qualification_marker(
        result.get("firmware_version", "unknown"),
        result.get("node", 0),
        "PASS" if result["verdict"]["pass"] else "FAIL"))
    lines.append("")
    lines.append("| Criterion | Measured | Required | Result |")
    lines.append("|---|---:|---:|---|")
    for finding in result["verdict"]["findings"]:
        measured = str(finding["measured"]).replace("|", "\\|")
        required = str(finding["required"]).replace("|", "\\|")
        lines.append(
            f"| {finding['criterion']} | {measured} | {required} | "
            f"{'PASS' if finding['pass'] else 'FAIL'} |"
        )
    lines.append("")
    lines.append(
        f"Server observation: {server['frames']} frames in {server['seconds_observed']} s, "
        f"maximum frame gap {server['max_frame_gap_ms']} ms."
    )
    if device:
        lines.append(
            f"Device observation: {device['tick_samples']} controller ticks, "
            f"raw yield {device['yield_pps_mean']} pps "
            f"(range {device['yield_pps_min']}-{device['yield_pps_max']}), "
            f"dsp {device['dsp_hz_mean']} Hz, "
            f"{device['degraded_ticks']} ticks in the controller's degraded state."
        )
    if result.get("environment_note"):
        lines.append("")
        lines.append(f"Environment: {result['environment_note']}")
    lines.append("")
    lines.append("Result: **PASS**" if result["verdict"]["pass"] else "Result: **FAIL**")
    return "\n".join(lines)


def build_result(node_id: int, environment_note: str, server: Dict[str, Any],
                 device: Optional[Dict[str, Any]], min_yield_pps: float,
                 min_coverage_percent: float,
                 firmware_version: str = "unknown") -> Dict[str, Any]:
    """Assemble the machine-readable qualification result."""
    verdict = evaluate(server, device, min_yield_pps, min_coverage_percent)
    return {
        "schema": "ruview/esp32-firmware-qualification/v1",
        "issue": "RuView#2092",
        "node": node_id,
        "firmware_version": firmware_version,
        "environment_note": environment_note,
        "server": server,
        "device": device,
        "thresholds": {
            "min_yield_pps": min_yield_pps,
            "min_coverage_percent": min_coverage_percent,
        },
        "verdict": verdict,
    }

#!/usr/bin/env python3
"""ruvultra → browser CSI bridge.

Reads adaptive_ctrl tick lines from the ESP32-S3 RuView firmware on
/dev/ttyACM0 and forwards normalized per-node metrics over a WebSocket
that the helpers-skinned-realtime demo can subscribe to via Tailscale.

Sample serial line (1 Hz cadence from firmware):
    I (22890561) adaptive_ctrl: medium tick: state=6 yield=15pps motion=1.00 presence=5.35 rssi=-33

Output JSON (per tick):
    {
      "ts": 1716830400.123,
      "node": 0,                # always 0 (single node), client expands to 4
      "motion": 1.00,           # raw firmware metric
      "presence": 5.35,
      "rssi": -33,
      "yield_pps": 15,
      "amp": 0.78               # synthesized CSI amplitude in [0..1] for the bar
    }

Run on ruvultra:
    python3 -u ruvultra-csi-bridge.py
"""
import asyncio
import builtins
import json
import re
import sys
import time
from contextlib import suppress

# Force every print to flush — we're often piped to a log file
_orig_print = builtins.print
def _print(*a, **kw):
    kw.setdefault("flush", True)
    return _orig_print(*a, **kw)
builtins.print = _print

import serial
import websockets

PORT = "/dev/ttyACM0"
BAUD = 115200
WS_HOST = "0.0.0.0"
WS_PORT = 8766

TICK_RE = re.compile(
    r"adaptive_ctrl:\s*\w+\s+tick:\s*"
    r"state=(?P<state>\d+)\s+"
    r"yield=(?P<yield>\d+)pps\s+"
    r"motion=(?P<motion>[\d.]+)\s+"
    r"presence=(?P<presence>[\d.]+)\s+"
    r"rssi=(?P<rssi>-?\d+)"
)

clients = set()
last_payload = None


def amp_from_metrics(motion, presence, rssi):
    """Map firmware metrics to a [0..1] CSI-style amplitude."""
    rssi_norm = max(0.0, min(1.0, (rssi + 80) / 50))      # -80..-30 → 0..1
    presence_norm = max(0.0, min(1.0, presence / 8.0))    # cap at 8
    motion_norm = max(0.0, min(1.0, motion))              # already 0..1ish
    return 0.40 * rssi_norm + 0.35 * presence_norm + 0.25 * motion_norm


async def serial_reader_loop():
    global last_payload
    print(f"[bridge] opening {PORT} @ {BAUD}…")
    while True:
        try:
            ser = serial.Serial(PORT, BAUD, timeout=1)
        except (serial.SerialException, OSError) as e:
            print(f"[bridge] serial open failed ({e}); retry in 3s")
            await asyncio.sleep(3)
            continue

        print(f"[bridge] connected to {PORT}")
        loop = asyncio.get_event_loop()
        try:
            while True:
                line = await loop.run_in_executor(None, ser.readline)
                if not line:
                    continue
                try:
                    text = line.decode(errors="replace").strip()
                except Exception:
                    continue
                m = TICK_RE.search(text)
                if not m:
                    continue
                motion = float(m["motion"])
                presence = float(m["presence"])
                rssi = int(m["rssi"])
                payload = {
                    "ts": time.time(),
                    "node": 0,
                    "state": int(m["state"]),
                    "yield_pps": int(m["yield"]),
                    "motion": motion,
                    "presence": presence,
                    "rssi": rssi,
                    "amp": amp_from_metrics(motion, presence, rssi),
                }
                last_payload = payload
                msg = json.dumps(payload)
                if clients:
                    dead = []
                    for ws in list(clients):
                        try:
                            await ws.send(msg)
                        except websockets.ConnectionClosed:
                            dead.append(ws)
                    for d in dead:
                        clients.discard(d)
                print(
                    f"[tick] motion={motion:.2f} presence={presence:5.2f} "
                    f"rssi={rssi:+d} yield={int(m['yield']):3d}pps "
                    f"amp={payload['amp']:.2f} clients={len(clients)}"
                )
        except (serial.SerialException, OSError) as e:
            print(f"[bridge] serial error ({e}); reopen in 1s")
            with suppress(Exception):
                ser.close()
            await asyncio.sleep(1)


async def ws_handler(ws):
    addr = ws.remote_address
    clients.add(ws)
    print(f"[ws] client connected: {addr}  total={len(clients)}")
    try:
        if last_payload is not None:
            await ws.send(json.dumps(last_payload))
        await ws.wait_closed()
    finally:
        clients.discard(ws)
        print(f"[ws] client gone: {addr}  total={len(clients)}")


async def main():
    print(f"[bridge] websocket on ws://{WS_HOST}:{WS_PORT}")
    async with websockets.serve(ws_handler, WS_HOST, WS_PORT):
        await serial_reader_loop()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass

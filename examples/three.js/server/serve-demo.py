"""Tiny threaded HTTP server for the three.js demos, plus an optional
UDP → WebSocket tracking gateway (ADR-none / Phase 8).

Why a sibling helper script instead of `python -m http.server`?
The stdlib SimpleHTTPServer is single-threaded; Chrome opens many parallel
connections (HTML + 9 script tags + FBX), the first eats the worker, the
rest time out with net::ERR_EMPTY_RESPONSE. ThreadingHTTPServer fixes it.

Usage (static demos only — default, unchanged):
    python examples/three.js/server/serve-demo.py
    open http://localhost:8765/examples/three.js/demos/06-cyber-hud.html

Usage (with the live multi-target gateway):
    python examples/three.js/server/serve-demo.py --gateway
    # then stream ESP32 RSSI frames as UDP JSON to 0.0.0.0:5555:
    #   {"node_id": "ESP32_01", "mac": "AA:BB:CC:DD:EE:FF", "rssi": -58}
    # the gateway aggregates per-MAC, broadcasts a multi-target /pose frame on
    # ws://<host>:8770/pose; 06-cyber-hud.html solves + renders each target.

Design note (honest): the server forwards raw `rssi_dbm` per node — it does NOT
convert RSSI→range here, because the log-distance coefficients (N, P0) are the
browser's live UI calibration and the server has no channel to them. The HUD
converts with its own sliders. The WebSocket is a from-scratch stdlib
implementation (no `websockets` dependency) — RFC 6455 §1.3 handshake, unmasked
server text frames.
"""
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
import argparse
import base64
import hashlib
import json
import os
import socket
import struct
import sys
import threading
import time

# Always serve from the repo root regardless of where the script is launched.
# This file lives at examples/three.js/server/serve-demo.py — three levels deep.
REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))

DEMOS = [
    "01-helpers.html",
    "02-cinematic.html",
    "03-skinned.html",
    "04-skinned-fbx.html",
    "05-skinned-realtime.html",
    "06-cyber-hud.html",
]

# ── Static HTTP handler ──────────────────────────────────────────────────────

class NoCacheHandler(SimpleHTTPRequestHandler):
    def end_headers(self):
        # Aggressive no-cache so the browser ALWAYS fetches the latest .html
        # after an edit; otherwise stale code sticks around even on hard refresh.
        self.send_header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()

    def log_message(self, *a):  # keep the console clean; gateway prints its own
        pass


# ── Pure gateway helpers (unit-tested in tests/gateway_selftest.py) ──────────

WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"

def ws_accept_key(client_key: str) -> str:
    """RFC 6455 §1.3 Sec-WebSocket-Accept from the client's Sec-WebSocket-Key."""
    digest = hashlib.sha1((client_key + WS_GUID).encode("ascii")).digest()
    return base64.b64encode(digest).decode("ascii")


def ws_text_frame(payload: bytes) -> bytes:
    """Encode a single unmasked server→client text frame (FIN=1, opcode=0x1)."""
    n = len(payload)
    header = bytearray([0x81])
    if n < 126:
        header.append(n)
    elif n < 65536:
        header.append(126)
        header += struct.pack(">H", n)
    else:
        header.append(127)
        header += struct.pack(">Q", n)
    return bytes(header) + payload


def parse_udp_frame(raw: bytes):
    """Parse one UDP tracking datagram. Returns (node_id, mac, rssi) or None."""
    try:
        d = json.loads(raw.decode("utf-8", "replace"))
    except (ValueError, TypeError):
        return None
    node_id = d.get("node_id")
    mac = d.get("mac")
    rssi = d.get("rssi", d.get("rssi_dbm"))
    if not isinstance(node_id, str) or not isinstance(mac, str):
        return None
    if not isinstance(rssi, (int, float)):
        return None
    return node_id, mac, float(rssi)


def build_targets(store: dict, now: float, node_ttl: float):
    """Aggregate the per-MAC RSSI store into a multi-target broadcast payload.

    store: { mac: { node_id: (rssi, ts) } }. Node readings older than
    `node_ttl` are dropped; a MAC with no fresh readings is omitted.
    Returns [{ "mac", "ranges":[{ "node_id", "rssi_dbm" }] }].
    """
    targets = []
    for mac, nodes in store.items():
        ranges = [
            {"node_id": nid, "rssi_dbm": rssi}
            for nid, (rssi, ts) in nodes.items()
            if now - ts <= node_ttl
        ]
        if ranges:
            targets.append({"mac": mac, "ranges": ranges})
    return targets


# ── UDP → WebSocket gateway ──────────────────────────────────────────────────

class Gateway:
    NODE_TTL = 5.0        # seconds a node's RSSI reading for a MAC stays fresh
    MAC_TTL = 12.0        # seconds a MAC with no fresh readings is retained
    BROADCAST_HZ = 20.0

    def __init__(self, udp_port: int, ws_port: int):
        self.udp_port = udp_port
        self.ws_port = ws_port
        self.store = {}                     # mac -> { node_id: (rssi, ts) }
        self.store_lock = threading.Lock()
        self.clients = set()                # connected ws client sockets
        self.clients_lock = threading.Lock()

    # -- UDP ingestion --------------------------------------------------------
    def udp_loop(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.bind(("0.0.0.0", self.udp_port))
        print(f"[gateway] UDP listening on 0.0.0.0:{self.udp_port}")
        while True:
            try:
                raw, _addr = s.recvfrom(4096)
            except OSError:
                break
            parsed = parse_udp_frame(raw)
            if parsed is None:
                continue
            node_id, mac, rssi = parsed
            now = time.time()
            with self.store_lock:
                self.store.setdefault(mac, {})[node_id] = (rssi, now)

    # -- WebSocket accept + per-client reader ---------------------------------
    def ws_accept_loop(self):
        srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        srv.bind(("0.0.0.0", self.ws_port))
        srv.listen(16)
        print(f"[gateway] WebSocket /pose on ws://0.0.0.0:{self.ws_port}")
        while True:
            try:
                conn, _ = srv.accept()
            except OSError:
                break
            threading.Thread(target=self._handshake_and_hold, args=(conn,), daemon=True).start()

    def _handshake_and_hold(self, conn: socket.socket):
        try:
            conn.settimeout(5.0)
            data = b""
            while b"\r\n\r\n" not in data and len(data) < 8192:
                chunk = conn.recv(1024)
                if not chunk:
                    conn.close(); return
                data += chunk
            key = None
            for line in data.decode("latin1").split("\r\n"):
                if line.lower().startswith("sec-websocket-key:"):
                    key = line.split(":", 1)[1].strip()
            if not key:
                conn.close(); return
            resp = (
                "HTTP/1.1 101 Switching Protocols\r\n"
                "Upgrade: websocket\r\nConnection: Upgrade\r\n"
                f"Sec-WebSocket-Accept: {ws_accept_key(key)}\r\n\r\n"
            )
            conn.sendall(resp.encode("ascii"))
            conn.settimeout(None)
        except OSError:
            conn.close(); return
        with self.clients_lock:
            self.clients.add(conn)
        # Drain client frames only to detect disconnect (we don't parse them).
        try:
            while conn.recv(1024):
                pass
        except OSError:
            pass
        finally:
            with self.clients_lock:
                self.clients.discard(conn)
            try:
                conn.close()
            except OSError:
                pass

    # -- Broadcast loop -------------------------------------------------------
    def broadcast_loop(self):
        period = 1.0 / self.BROADCAST_HZ
        while True:
            time.sleep(period)
            now = time.time()
            with self.store_lock:
                # GC stale MACs.
                for mac in list(self.store.keys()):
                    nodes = self.store[mac]
                    for nid in list(nodes.keys()):
                        if now - nodes[nid][1] > self.MAC_TTL:
                            del nodes[nid]
                    if not nodes:
                        del self.store[mac]
                targets = build_targets(self.store, now, self.NODE_TTL)
            if not targets:
                continue
            msg = ws_text_frame(json.dumps(
                {"type": "pose", "src": "udp", "targets": targets}).encode("utf-8"))
            with self.clients_lock:
                dead = []
                for c in self.clients:
                    try:
                        c.sendall(msg)
                    except OSError:
                        dead.append(c)
                for c in dead:
                    self.clients.discard(c)

    def start(self):
        for fn in (self.udp_loop, self.ws_accept_loop, self.broadcast_loop):
            threading.Thread(target=fn, daemon=True).start()


# ── Entrypoint ───────────────────────────────────────────────────────────────

def main():
    ap = argparse.ArgumentParser(description="RuView demo server + optional tracking gateway")
    ap.add_argument("--http-port", type=int, default=int(os.environ.get("PORT", 8765)))
    ap.add_argument("--gateway", action="store_true",
                    default=os.environ.get("RUVIEW_UDP_GATEWAY") in ("1", "true", "True"),
                    help="also run the UDP:5555 → WebSocket:8770 multi-target gateway")
    ap.add_argument("--udp-port", type=int, default=5555)
    ap.add_argument("--ws-port", type=int, default=8770)
    args = ap.parse_args()

    os.chdir(REPO_ROOT)

    if args.gateway:
        Gateway(args.udp_port, args.ws_port).start()

    with ThreadingHTTPServer(("127.0.0.1", args.http_port), NoCacheHandler) as srv:
        print(f"serving {os.getcwd()} on http://127.0.0.1:{args.http_port}/")
        print("demos:")
        for d in DEMOS:
            print(f"  http://127.0.0.1:{args.http_port}/examples/three.js/demos/{d}")
        if args.gateway:
            print(f"[gateway] ACTIVE — stream UDP JSON to :{args.udp_port}, "
                  f"open 06-cyber-hud.html (it auto-connects ws://…:{args.ws_port}/pose)")
        try:
            srv.serve_forever()
        except KeyboardInterrupt:
            sys.exit(0)


if __name__ == "__main__":
    main()

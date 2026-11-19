#!/usr/bin/env python3
"""Debug listener: prints frame headers for each ESP32-format CSI frame
the bridge emits. Reproduces only the fields needed for sanity checks.

Usage: python3 listen.py [port]   (default port: 5005)
"""
import socket
import struct
import sys
import time


def main() -> int:
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 5005
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", port))
    print(f"[listen] udp://127.0.0.1:{port} — Ctrl-C to stop", flush=True)
    n, t0 = 0, time.time()
    try:
        while True:
            data, _ = sock.recvfrom(4096)
            if len(data) < 20:
                continue
            magic, node_id, n_ant, n_sub = struct.unpack_from("<IBBB", data, 0)
            seq = struct.unpack_from("<I", data, 10)[0]
            rssi, noise = struct.unpack_from("bb", data, 14)
            n += 1
            print(
                f"#{n:>4} t+{time.time() - t0:5.1f}s "
                f"magic=0x{magic:08x} seq={seq:>4} node={node_id} "
                f"ant={n_ant} sub={n_sub} rssi={rssi}dBm noise={noise}dBm",
                flush=True,
            )
    except KeyboardInterrupt:
        print(f"[listen] {n} frames in {time.time() - t0:.1f}s")
        return 0


if __name__ == "__main__":
    raise SystemExit(main())

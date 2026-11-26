#!/usr/bin/env python3
"""Capture ADR-018 datagrams on a UDP port for c5l8-udp-check.

usage: capture-udp.py <out.txt> [--port 5005] [--seconds 60]

Writes one line per datagram: "<unix-time> len=<n> magic=<u32 LE as hex> hex=<payload>".
The sender address is not recorded. Feed the file to:
    cargo run --offline --bin c5l8-udp-check < out.txt
"""
import argparse
import collections
import socket
import time

ap = argparse.ArgumentParser()
ap.add_argument("out")
ap.add_argument("--port", type=int, default=5005)
ap.add_argument("--seconds", type=float, default=60.0)
a = ap.parse_args()

sizes = collections.Counter()
end = time.time() + a.seconds
u = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
u.bind(("0.0.0.0", a.port))
u.settimeout(0.5)
with open(a.out, "w") as f:
    while time.time() < end:
        try:
            d, _ = u.recvfrom(4096)
        except socket.timeout:
            continue
        sizes[len(d)] += 1
        f.write(f"{time.time():.3f} len={len(d)} magic={d[:4][::-1].hex()} hex={d.hex()}\n")
print("datagrams by size:", dict(sorted(sizes.items())))

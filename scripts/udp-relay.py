#!/usr/bin/env python3
"""
UDP relay for Docker Desktop on Windows (issue #374, #386).

Docker Desktop on Windows multiplexes inbound UDP from multiple source IPs to
a single source IP inside the container, which causes packets from all but one
ESP32 node to be silently dropped at the WSL/Hyper-V boundary.

This relay listens on the host, then re-emits each datagram from its own
single socket back to a localhost port that Docker forwards into the
container. Because every forwarded datagram now has the same source IP/port
(the relay's loopback socket), Docker passes them all through.

Usage:
    # Default: listen on host:5005, forward to 127.0.0.1:5006
    # Container should be started with -p 5006:5005/udp.
    python scripts/udp-relay.py

    # Custom ports
    python scripts/udp-relay.py --listen-port 5005 --forward-port 5006

    # Verbose (one line per packet)
    python scripts/udp-relay.py --verbose
"""

import argparse
import socket
import sys
import time


def run_relay(listen_host: str, listen_port: int, forward_host: str,
              forward_port: int, stats_interval: float, verbose: bool) -> int:
    rx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    rx.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        rx.bind((listen_host, listen_port))
    except OSError as e:
        print(f"udp-relay: failed to bind {listen_host}:{listen_port}: {e}",
              file=sys.stderr)
        return 1

    tx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    forward_addr = (forward_host, forward_port)

    print(f"udp-relay: listening on {listen_host}:{listen_port} "
          f"-> forwarding to {forward_host}:{forward_port}")
    print("udp-relay: collapses multi-source UDP to a single loopback source "
          "so Docker Desktop on Windows forwards every packet (issue #374).")

    sources: dict[tuple[str, int], int] = {}
    total = 0
    last_stats = time.monotonic()

    try:
        while True:
            data, src = rx.recvfrom(65535)
            tx.sendto(data, forward_addr)
            total += 1
            sources[src] = sources.get(src, 0) + 1

            if verbose:
                print(f"udp-relay: {src[0]}:{src[1]} -> "
                      f"{forward_host}:{forward_port} ({len(data)}B)")

            now = time.monotonic()
            if now - last_stats >= stats_interval:
                print(f"udp-relay: forwarded {total} pkts from "
                      f"{len(sources)} sources in last {stats_interval:.0f}s")
                sources.clear()
                total = 0
                last_stats = now
    except KeyboardInterrupt:
        print("udp-relay: stopping")
        return 0
    finally:
        rx.close()
        tx.close()


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--listen-host", default="0.0.0.0",
                   help="Host interface to bind (default: 0.0.0.0)")
    p.add_argument("--listen-port", type=int, default=5005,
                   help="Port the ESP32 nodes send to (default: 5005)")
    p.add_argument("--forward-host", default="127.0.0.1",
                   help="Where to forward packets (default: 127.0.0.1)")
    p.add_argument("--forward-port", type=int, default=5006,
                   help="Port Docker maps into the container (default: 5006)")
    p.add_argument("--stats-interval", type=float, default=10.0,
                   help="Seconds between stats lines (default: 10)")
    p.add_argument("--verbose", action="store_true",
                   help="Log every forwarded packet")
    args = p.parse_args()

    return run_relay(args.listen_host, args.listen_port, args.forward_host,
                     args.forward_port, args.stats_interval, args.verbose)


if __name__ == "__main__":
    sys.exit(main())

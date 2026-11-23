#!/usr/bin/env python3
"""Firewall-free CSI UDP relay for local Windows ESP32 testing.

On Windows, a freshly-built binary (e.g. `wifi-densepose calibrate-serve`) is
blocked from receiving inbound LAN UDP by Windows Defender Firewall unless an
admin adds an allow rule. `python.exe` is typically already allowed. This relay
binds the public CSI port, receives the ESP32's frames, and forwards each
datagram verbatim to a loopback port where the calibration server listens
(loopback is exempt from the inbound firewall). No admin required.

Usage:
    python scripts/csi-udp-relay.py --listen 5005 --forward 5006

Then run the calibration server on the loopback port:
    wifi-densepose calibrate-serve --udp-bind 127.0.0.1 --udp-port 5006

Frames are passed through byte-for-byte; the relay never parses or mutates them.
"""
import argparse
import socket
import time


def main() -> None:
    ap = argparse.ArgumentParser(description="Forward ESP32 CSI UDP to a loopback port (no admin).")
    ap.add_argument("--listen", type=int, default=5005, help="public UDP port the ESP32 streams to")
    ap.add_argument("--listen-host", default="0.0.0.0", help="bind address for the public port")
    ap.add_argument("--forward", type=int, default=5006, help="loopback port the calibration server listens on")
    ap.add_argument("--forward-host", default="127.0.0.1", help="loopback host to forward to")
    ap.add_argument("--quiet", action="store_true", help="suppress the periodic stats line")
    args = ap.parse_args()

    rx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    rx.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    rx.bind((args.listen_host, args.listen))
    rx.settimeout(1.0)
    tx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    dst = (args.forward_host, args.forward)

    print(f"[relay] {args.listen_host}:{args.listen}  ->  {dst[0]}:{dst[1]}  (Ctrl-C to stop)")
    count = 0
    last_report = time.time()
    last_src = None
    try:
        while True:
            try:
                data, src = rx.recvfrom(65535)
            except socket.timeout:
                data = None
            if data is not None:
                tx.sendto(data, dst)
                count += 1
                last_src = src
            now = time.time()
            if not args.quiet and now - last_report >= 5.0:
                print(f"[relay] forwarded {count} frames (last src={last_src})")
                last_report = now
    except KeyboardInterrupt:
        print(f"\n[relay] stopped after {count} frames")
    finally:
        rx.close()
        tx.close()


if __name__ == "__main__":
    main()

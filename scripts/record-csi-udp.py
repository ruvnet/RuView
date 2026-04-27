#!/usr/bin/env python3
"""
Lightweight ESP32 CSI UDP recorder (ADR-079).

Captures raw CSI packets from ESP32 nodes over UDP and writes to JSONL.
Runs alongside collect-ground-truth.py for synchronized capture.

Usage:
    python scripts/record-csi-udp.py --duration 300 --output data/recordings
"""

import argparse
import json
import os
import socket
import struct
import time


def parse_csi_packet(data):
    """Parse ADR-018 binary CSI packet into dict."""
    if len(data) < 8:
        return None

    # ADR-018 header: [magic(2), len(2), node_id(1), seq(1), rssi(1), channel(1), iq_data...]
    # Simplified: extract what we can from the raw packet
    node_id = data[4] if len(data) > 4 else 0
    rssi = struct.unpack('b', bytes([data[6]]))[0] if len(data) > 6 else 0
    channel = data[7] if len(data) > 7 else 0

    # IQ data starts at offset 8
    iq_data = data[8:] if len(data) > 8 else b''
    n_subcarriers = len(iq_data) // 2  # I,Q pairs

    # Compute amplitudes
    amplitudes = []
    for i in range(0, len(iq_data) - 1, 2):
        I = struct.unpack('b', bytes([iq_data[i]]))[0]
        Q = struct.unpack('b', bytes([iq_data[i + 1]]))[0]
        amplitudes.append(round((I * I + Q * Q) ** 0.5, 2))

    return {
        "type": "raw_csi",
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S.") + f"{int(time.time() * 1000) % 1000:03d}Z",
        "ts_ns": time.time_ns(),
        "node_id": node_id,
        "rssi": rssi,
        "channel": channel,
        "subcarriers": n_subcarriers,
        "amplitudes": amplitudes,
        "iq_hex": iq_data.hex(),
    }


def main():
    parser = argparse.ArgumentParser(description="Record ESP32 CSI over UDP")
    parser.add_argument("--port", type=int, default=5005, help="UDP port (default: 5005)")
    parser.add_argument("--duration", type=int, default=300, help="Duration in seconds (default: 300)")
    parser.add_argument("--output", default="data/recordings", help="Output directory")
    args = parser.parse_args()

    os.makedirs(args.output, exist_ok=True)
    filename = f"csi-{int(time.time())}.csi.jsonl"
    filepath = os.path.join(args.output, filename)

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.bind(("0.0.0.0", args.port))
    sock.settimeout(1)

    print(f"Recording CSI on UDP :{args.port} for {args.duration}s")
    print(f"Output: {filepath}")

    count = 0
    start = time.time()
    nodes_seen = set()

    with open(filepath, "w") as f:
        try:
            while time.time() - start < args.duration:
                try:
                    data, addr = sock.recvfrom(4096)
                    frame = parse_csi_packet(data)
                    if frame:
                        f.write(json.dumps(frame) + "\n")
                        count += 1
                        nodes_seen.add(frame["node_id"])

                        if count % 500 == 0:
                            elapsed = time.time() - start
                            rate = count / elapsed
                            print(f"  {count} frames | {rate:.0f} fps | "
                                  f"nodes: {sorted(nodes_seen)} | "
                                  f"{elapsed:.0f}s / {args.duration}s")
                except socket.timeout:
                    continue
        except KeyboardInterrupt:
            print("\nStopped by user")

    sock.close()
    elapsed = time.time() - start
    print(f"\n=== CSI Recording Complete ===")
    print(f"  Frames: {count}")
    print(f"  Duration: {elapsed:.0f}s")
    print(f"  Rate: {count / max(elapsed, 1):.0f} fps")
    print(f"  Nodes: {sorted(nodes_seen)}")
    print(f"  Output: {filepath}")


if __name__ == "__main__":
    main()

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
from datetime import datetime, timezone


# ADR-018 header, as written by csi_collector.c and read by esp32_parser.rs:
# magic u32 @0, node_id @4, n_antennas @5, n_subcarriers u16 @6, freq_mhz u32 @8,
# sequence u32 @12, rssi i8 @16, noise_floor i8 @17, ppdu_type @18, flags @19,
# then n_antennas * n_subcarriers int8 I/Q pairs from @20.
CSI_MAGIC = 0xC5110001
CSI_HDR_FMT = "<IBBHIIbbBB"
CSI_HDR_SIZE = struct.calcsize(CSI_HDR_FMT)  # 20 bytes


def freq_to_channel(freq_mhz):
    """802.11 channel number for a 2.4/5 GHz centre frequency (0 if unknown)."""
    if freq_mhz == 2484:
        return 14
    if 2412 <= freq_mhz <= 2472:
        return (freq_mhz - 2407) // 5
    if 5000 <= freq_mhz <= 5900:
        return (freq_mhz - 5000) // 5
    return 0


def parse_csi_packet(data):
    """Parse ADR-018 binary CSI packet into dict, or None if it is not one."""
    if len(data) < CSI_HDR_SIZE:
        return None

    (magic, node_id, n_antennas, n_sub, freq_mhz, seq,
     rssi, noise_floor, ppdu_type, flags) = struct.unpack_from(CSI_HDR_FMT, data)
    # Same antenna/subcarrier limits as esp32_parser.rs
    if magic != CSI_MAGIC or not 1 <= n_antennas <= 4 or n_sub > 256:
        return None

    n_pairs = n_antennas * n_sub
    iq_data = data[CSI_HDR_SIZE:CSI_HDR_SIZE + n_pairs * 2]
    if len(iq_data) < n_pairs * 2:
        return None

    # Compute amplitudes
    amplitudes = []
    for i in range(0, len(iq_data), 2):
        I = struct.unpack('b', bytes([iq_data[i]]))[0]
        Q = struct.unpack('b', bytes([iq_data[i + 1]]))[0]
        amplitudes.append(round((I * I + Q * Q) ** 0.5, 2))

    return {
        "type": "raw_csi",
        # true UTC, not local-time-labeled-Z (#1007 Bug 1) — e.g. "2026-06-17T01:23:45.678Z"
        "timestamp": datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z"),
        "ts_ns": time.time_ns(),
        "node_id": node_id,
        "seq": seq,
        "rssi": rssi,
        "noise_floor": noise_floor,
        "freq_mhz": freq_mhz,
        "channel": freq_to_channel(freq_mhz),
        "ppdu_type": ppdu_type,
        "flags": flags,
        "n_antennas": n_antennas,
        "n_subcarriers": n_sub,
        "subcarriers": n_pairs,  # I/Q pairs in iq_hex (n_antennas * n_subcarriers)
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

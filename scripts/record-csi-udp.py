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
    """Parse ADR-018 V1/V2 binary CSI packet into dict.

    V1 (magic 0xC5110001): 20-byte header, no source MAC.
    V2 (magic 0xC5110006): 26-byte header, 6-byte source MAC at offset 20.

    Header layout:
        [0..3]   Magic (u32 LE)
        [4]      Node ID (u8)
        [5]      N antennas (u8)
        [6..7]   N subcarriers (u16 LE)
        [8..11]  Frequency MHz (u32 LE)
        [12..15] Sequence (u32 LE)
        [16]     RSSI (i8)
        [17]     Noise floor (i8)
        [18..19] Reserved
        [20..25] Source MAC (V2 only)
        [20/26+] I/Q data (i8, i8 pairs)
    """
    if len(data) < 20:
        return None

    magic = struct.unpack('<I', data[0:4])[0]

    if magic == 0xC5110001:
        header_size = 20
        source_mac = None
    elif magic == 0xC5110006:
        if len(data) < 26:
            return None
        header_size = 26
        source_mac = ':'.join(f'{b:02x}' for b in data[20:26])
    else:
        return None

    node_id = data[4]
    n_antennas = data[5]
    n_subcarriers = struct.unpack('<H', data[6:8])[0]
    freq_mhz = struct.unpack('<I', data[8:12])[0]
    sequence = struct.unpack('<I', data[12:16])[0]
    rssi = struct.unpack('b', bytes([data[16]]))[0]
    noise_floor = struct.unpack('b', bytes([data[17]]))[0]

    iq_data = data[header_size:]
    n_pairs = n_antennas * n_subcarriers
    expected_iq = n_pairs * 2
    if len(iq_data) < expected_iq:
        return None

    amplitudes = []
    for i in range(0, expected_iq, 2):
        I = struct.unpack('b', bytes([iq_data[i]]))[0]
        Q = struct.unpack('b', bytes([iq_data[i + 1]]))[0]
        amplitudes.append(round((I * I + Q * Q) ** 0.5, 2))

    result = {
        "type": "raw_csi",
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S.") + f"{int(time.time() * 1000) % 1000:03d}Z",
        "ts_ns": time.time_ns(),
        "node_id": node_id,
        "n_antennas": n_antennas,
        "n_subcarriers": n_subcarriers,
        "freq_mhz": freq_mhz,
        "sequence": sequence,
        "rssi": rssi,
        "noise_floor": noise_floor,
        "subcarriers": len(amplitudes),
        "amplitudes": amplitudes,
        "iq_hex": iq_data[:expected_iq].hex(),
    }
    if source_mac is not None:
        result["source_mac"] = source_mac

    return result


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

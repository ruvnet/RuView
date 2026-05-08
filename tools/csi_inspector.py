#!/usr/bin/env python3
"""
CSI Packet Inspector - Diagnostic tool for RuView ESP32 CSI data
Analyzes UDP packets on port 5005 and validates CSI frame integrity.
"""

import socket
import struct
import time
import argparse
from collections import defaultdict
from datetime import datetime

# Expected magic number for CSI frames
MAGIC_CSI = 0xC5110001
MAGIC_VITALS = 0xC5110002
MAGIC_WASM = 0xC5110004

class DeviceStats:
    def __init__(self):
        self.packet_count = 0
        self.last_seq = None
        self.seq_gaps = 0
        self.rssi_values = []
        self.noise_values = []
        self.subcarrier_counts = set()
        self.node_ids = set()
        self.last_seen = None
        self.errors = []
        self.amplitudes_sum = 0
        self.amplitude_samples = 0

def parse_csi_frame(data):
    """Parse a CSI frame and return structured data."""
    if len(data) < 20:
        return None, "Packet too short"

    magic = struct.unpack("<I", data[0:4])[0]

    if magic == MAGIC_VITALS:
        return {"type": "vitals", "magic": magic}, None
    elif magic == MAGIC_WASM:
        return {"type": "wasm", "magic": magic}, None
    elif magic != MAGIC_CSI:
        return None, f"Invalid magic: 0x{magic:08X}"

    node_id = data[4]
    n_antennas = data[5]
    # ADR-018 20-byte header format
    n_subcarriers = struct.unpack("<H", data[6:8])[0]  # u16 at bytes [6..7]
    freq_mhz = struct.unpack("<I", data[8:12])[0]      # u32 at bytes [8..11]
    sequence = struct.unpack("<I", data[12:16])[0]     # u32 at bytes [12..15]
    rssi_raw = data[16]                                 # i8 at byte 16
    noise_raw = data[17]                                # i8 at byte 17

    # Convert to signed
    rssi = rssi_raw if rssi_raw < 128 else rssi_raw - 256
    noise = noise_raw if noise_raw < 128 else noise_raw - 256

    # Parse I/Q data
    iq_start = 20
    n_pairs = n_subcarriers * n_antennas
    expected_len = iq_start + n_pairs * 2

    amplitudes = []
    if len(data) >= expected_len:
        for k in range(min(n_pairs, (len(data) - iq_start) // 2)):
            i_val = data[iq_start + k * 2]
            q_val = data[iq_start + k * 2 + 1]
            # Convert to signed
            i_val = i_val if i_val < 128 else i_val - 256
            q_val = q_val if q_val < 128 else q_val - 256
            amp = (i_val * i_val + q_val * q_val) ** 0.5
            amplitudes.append(amp)

    return {
        "type": "csi",
        "magic": magic,
        "node_id": node_id,
        "n_antennas": n_antennas,
        "n_subcarriers": n_subcarriers,
        "freq_mhz": freq_mhz,
        "sequence": sequence,
        "rssi": rssi,
        "rssi_raw": rssi_raw,
        "noise": noise,
        "noise_raw": noise_raw,
        "amplitudes": amplitudes,
        "payload_len": len(data)
    }, None

def validate_frame(frame, device_stats):
    """Validate a CSI frame and return list of issues."""
    issues = []

    if frame["type"] != "csi":
        return issues

    # Check RSSI (should be negative, typically -30 to -90)
    if frame["rssi"] >= 0:
        issues.append(f"RSSI={frame['rssi']} (raw={frame['rssi_raw']}) - should be negative")
    elif frame["rssi"] > -20:
        issues.append(f"RSSI={frame['rssi']} - unusually strong")
    elif frame["rssi"] < -95:
        issues.append(f"RSSI={frame['rssi']} - very weak signal")

    # Check noise floor (should be around -90 to -100)
    if frame["noise"] == 0:
        issues.append(f"Noise=0 - likely uninitialized")
    elif frame["noise"] > -70:
        issues.append(f"Noise={frame['noise']} - unusually high")

    # Check subcarrier count
    valid_subs = [52, 56, 64, 114, 128, 234, 242, 256]
    if frame["n_subcarriers"] not in valid_subs:
        issues.append(f"Subcarriers={frame['n_subcarriers']} - unusual count")

    # Check frequency
    if frame["freq_mhz"] < 2400 or frame["freq_mhz"] > 5900:
        if frame["freq_mhz"] != 0:  # 0 might mean not set
            issues.append(f"Freq={frame['freq_mhz']}MHz - invalid")

    # Check sequence gaps
    if device_stats.last_seq is not None:
        expected = (device_stats.last_seq + 1) & 0xFFFFFFFF
        if frame["sequence"] != expected and frame["sequence"] != 0:
            # Allow for wrapping and small gaps
            gap = (frame["sequence"] - device_stats.last_seq) & 0xFFFFFFFF
            if gap > 1 and gap < 1000:
                issues.append(f"Seq gap: {gap} packets missed")

    # Check amplitude data
    if len(frame["amplitudes"]) > 0:
        avg_amp = sum(frame["amplitudes"]) / len(frame["amplitudes"])
        if avg_amp < 1:
            issues.append(f"Avg amplitude={avg_amp:.2f} - very low")
        elif avg_amp > 100:
            issues.append(f"Avg amplitude={avg_amp:.2f} - very high")

    return issues

def print_header():
    print("\n" + "=" * 80)
    print("  RuView CSI Packet Inspector")
    print("  Monitoring UDP port 5005 for ESP32 CSI frames")
    print("=" * 80)

def print_stats(devices, duration):
    print("\n" + "-" * 80)
    print(f"  Statistics after {duration:.1f} seconds")
    print("-" * 80)

    # Check for duplicate node IDs
    node_id_map = defaultdict(list)
    for ip, stats in devices.items():
        for nid in stats.node_ids:
            node_id_map[nid].append(ip)

    duplicates = {nid: ips for nid, ips in node_id_map.items() if len(ips) > 1}

    if duplicates:
        print("\n  ⚠️  DUPLICATE NODE IDs DETECTED:")
        for nid, ips in duplicates.items():
            print(f"      Node {nid}: {', '.join(ips)}")

    print("\n  Device Summary:")
    print("  " + "-" * 76)
    print(f"  {'IP Address':<16} {'NodeID':<8} {'Pkts':<8} {'RSSI':<12} {'Subs':<10} {'Status'}")
    print("  " + "-" * 76)

    for ip in sorted(devices.keys()):
        stats = devices[ip]
        node_str = ",".join(str(n) for n in sorted(stats.node_ids))

        if stats.rssi_values:
            avg_rssi = sum(stats.rssi_values) / len(stats.rssi_values)
            rssi_str = f"{avg_rssi:.0f} dBm"
        else:
            rssi_str = "N/A"

        subs_str = ",".join(str(s) for s in sorted(stats.subcarrier_counts))

        # Determine status
        has_errors = len(stats.errors) > 0
        status = "⚠️  Issues" if has_errors else "✅ OK"

        print(f"  {ip:<16} {node_str:<8} {stats.packet_count:<8} {rssi_str:<12} {subs_str:<10} {status}")

    print("  " + "-" * 76)

    # Print detailed errors
    print("\n  Detailed Issues:")
    for ip in sorted(devices.keys()):
        stats = devices[ip]
        if stats.errors:
            print(f"\n  {ip}:")
            # Group similar errors
            error_counts = defaultdict(int)
            for err in stats.errors:
                error_counts[err] += 1
            for err, count in sorted(error_counts.items(), key=lambda x: -x[1])[:5]:
                print(f"    - {err} (x{count})")

def main():
    parser = argparse.ArgumentParser(description="CSI Packet Inspector for RuView")
    parser.add_argument("-d", "--duration", type=int, default=10,
                        help="Duration to capture in seconds (default: 10)")
    parser.add_argument("-p", "--port", type=int, default=5005,
                        help="UDP port to monitor (default: 5005)")
    parser.add_argument("-v", "--verbose", action="store_true",
                        help="Print each packet")
    parser.add_argument("--continuous", action="store_true",
                        help="Run continuously, print stats every 10 seconds")
    args = parser.parse_args()

    print_header()

    # Create UDP socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)

    try:
        sock.bind(("0.0.0.0", args.port))
    except OSError as e:
        print(f"\n  ERROR: Cannot bind to port {args.port}: {e}")
        print("  The sensing server may be using this port.")
        print("  Stop the server first: sudo systemctl stop ruview")
        return 1

    sock.settimeout(1.0)

    devices = defaultdict(DeviceStats)
    start_time = time.time()
    last_stats_time = start_time
    packet_count = 0

    print(f"\n  Capturing for {args.duration} seconds..." if not args.continuous else "\n  Running continuously (Ctrl+C to stop)...")
    print()

    try:
        while True:
            elapsed = time.time() - start_time

            if not args.continuous and elapsed >= args.duration:
                break

            # Print periodic stats in continuous mode
            if args.continuous and time.time() - last_stats_time >= 10:
                print_stats(devices, time.time() - start_time)
                last_stats_time = time.time()

            try:
                data, addr = sock.recvfrom(2048)
                ip = addr[0]

                frame, error = parse_csi_frame(data)

                if error:
                    devices[ip].errors.append(error)
                    if args.verbose:
                        print(f"  [{ip}] ERROR: {error}")
                    continue

                if frame["type"] != "csi":
                    if args.verbose:
                        print(f"  [{ip}] {frame['type'].upper()} packet")
                    continue

                stats = devices[ip]
                stats.packet_count += 1
                stats.node_ids.add(frame["node_id"])
                stats.rssi_values.append(frame["rssi"])
                stats.noise_values.append(frame["noise"])
                stats.subcarrier_counts.add(frame["n_subcarriers"])
                stats.last_seen = time.time()

                if frame["amplitudes"]:
                    stats.amplitudes_sum += sum(frame["amplitudes"])
                    stats.amplitude_samples += len(frame["amplitudes"])

                issues = validate_frame(frame, stats)
                stats.errors.extend(issues)
                stats.last_seq = frame["sequence"]

                packet_count += 1

                if args.verbose:
                    status = "⚠️ " + issues[0] if issues else "✅"
                    print(f"  [{ip}] Node={frame['node_id']} RSSI={frame['rssi']:4d} "
                          f"Subs={frame['n_subcarriers']:3d} Seq={frame['sequence']:10d} {status}")
                else:
                    # Progress indicator
                    if packet_count % 20 == 0:
                        print(f"\r  Captured {packet_count} packets from {len(devices)} devices...", end="", flush=True)

            except socket.timeout:
                continue

    except KeyboardInterrupt:
        print("\n\n  Interrupted by user.")

    finally:
        sock.close()

    # Print final stats
    print_stats(devices, time.time() - start_time)

    # Summary
    print("\n" + "=" * 80)
    total_issues = sum(len(s.errors) for s in devices.values())
    if total_issues == 0:
        print("  ✅ All packets valid - no issues detected")
    else:
        print(f"  ⚠️  Found {total_issues} issues across {len(devices)} devices")
        print("\n  Recommendations:")

        # Check specific issues
        has_rssi_issue = any("RSSI" in e for s in devices.values() for e in s.errors)
        has_dup_nodes = len({nid for s in devices.values() for nid in s.node_ids}) < len(devices)
        has_noise_issue = any("Noise" in e for s in devices.values() for e in s.errors)

        if has_rssi_issue:
            print("    1. RSSI values incorrect - check ESP32 firmware byte encoding")
        if has_dup_nodes:
            print("    2. Duplicate Node IDs - reflash each ESP32 with unique ID (1,2,3,4)")
        if has_noise_issue:
            print("    3. Noise floor not set - update ESP32 firmware to include noise value")

    print("=" * 80 + "\n")
    return 0

if __name__ == "__main__":
    exit(main())

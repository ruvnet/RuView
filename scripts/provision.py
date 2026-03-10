#!/usr/bin/env python3
"""
ESP32-S3 CSI Node Provisioning Script

Writes WiFi credentials and aggregator target to the ESP32's NVS partition
so users can configure a pre-built firmware binary without recompiling.

Usage:
    python provision.py --port COM7 --ssid "MyWiFi" --password "secret" --target-ip 192.168.1.20

Requirements:
    pip install esptool nvs-partition-gen
    (or use the nvs_partition_gen.py bundled with ESP-IDF)
"""

import argparse
import csv
import io
import os
import struct
import subprocess
import sys
import tempfile


# NVS partition table offset — default for ESP-IDF 4MB flash with standard
# partition scheme.  The "nvs" partition starts at 0x9000 (36864) and is
# 0x6000 (24576) bytes.
NVS_PARTITION_OFFSET = 0x9000
NVS_PARTITION_SIZE = 0x6000  # 24 KiB


def build_nvs_csv(ssid, password, target_ip, target_port, node_id,
                   edge_tier=None, pres_thresh=None, fall_thresh=None,
                   vital_window=None, vital_interval_ms=None, subk_count=None,
                   wasm_verify=None, wasm_pubkey=None):
    """Build an NVS CSV string for the csi_cfg namespace."""
    buf = io.StringIO()
    writer = csv.writer(buf)
    writer.writerow(["key", "type", "encoding", "value"])
    writer.writerow(["csi_cfg", "namespace", "", ""])
    if ssid:
        writer.writerow(["ssid", "data", "string", ssid])
    if password is not None:
        writer.writerow(["password", "data", "string", password])
    if target_ip:
        writer.writerow(["target_ip", "data", "string", target_ip])
    if target_port is not None:
        writer.writerow(["target_port", "data", "u16", str(target_port)])
    if node_id is not None:
        writer.writerow(["node_id", "data", "u8", str(node_id)])
    # ADR-039: Edge intelligence configuration.
    if edge_tier is not None:
        writer.writerow(["edge_tier", "data", "u8", str(edge_tier)])
    if pres_thresh is not None:
        writer.writerow(["pres_thresh", "data", "u16", str(int(pres_thresh * 1000))])
    if fall_thresh is not None:
        writer.writerow(["fall_thresh", "data", "u16", str(int(fall_thresh * 1000))])
    if vital_window is not None:
        writer.writerow(["vital_win", "data", "u16", str(vital_window)])
    if vital_interval_ms is not None:
        writer.writerow(["vital_int", "data", "u16", str(vital_interval_ms)])
    if subk_count is not None:
        writer.writerow(["subk_count", "data", "u8", str(subk_count)])
    # ADR-040: WASM signature verification.
    if wasm_verify is not None:
        writer.writerow(["wasm_verify", "data", "u8", str(1 if wasm_verify else 0)])
    if wasm_pubkey is not None:
        # Store 32-byte Ed25519 public key as hex-encoded blob.
        writer.writerow(["wasm_pubkey", "data", "hex2bin", wasm_pubkey])
    return buf.getvalue()


def generate_nvs_binary(csv_content, size):
    """Generate an NVS partition binary from CSV using nvs_partition_gen.py."""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".csv", delete=False) as f_csv:
        f_csv.write(csv_content)
        csv_path = f_csv.name

    bin_path = csv_path.replace(".csv", ".bin")

    try:
        # Try the pip-installed version first (esp_idf_nvs_partition_gen package)
        try:
            from esp_idf_nvs_partition_gen import nvs_partition_gen
            nvs_partition_gen.generate(csv_path, bin_path, size)
            with open(bin_path, "rb") as f:
                return f.read()
        except ImportError:
            pass

        # Try legacy import name (older versions)
        try:
            import nvs_partition_gen
            nvs_partition_gen.generate(csv_path, bin_path, size)
            with open(bin_path, "rb") as f:
                return f.read()
        except ImportError:
            pass

        # Fall back to calling the ESP-IDF script directly
        idf_path = os.environ.get("IDF_PATH", "")
        gen_script = os.path.join(idf_path, "components", "nvs_flash",
                                  "nvs_partition_generator", "nvs_partition_gen.py")
        if os.path.isfile(gen_script):
            subprocess.check_call([
                sys.executable, gen_script, "generate",
                csv_path, bin_path, hex(size)
            ])
            with open(bin_path, "rb") as f:
                return f.read()

        # Last resort: try as a module
        subprocess.check_call([
            sys.executable, "-m", "nvs_partition_gen", "generate",
            csv_path, bin_path, hex(size)
        ])
        with open(bin_path, "rb") as f:
            return f.read()

    finally:
        for p in (csv_path, bin_path):
            if os.path.isfile(p):
                os.unlink(p)


def flash_nvs(port, baud, nvs_bin):
    """Flash the NVS partition binary to the ESP32."""
    with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as f:
        f.write(nvs_bin)
        bin_path = f.name

    try:
        cmd = [
            sys.executable, "-m", "esptool",
            "--chip", "esp32s3",
            "--port", port,
            "--baud", str(baud),
            "write_flash",
            hex(NVS_PARTITION_OFFSET), bin_path,
        ]
        print(f"Flashing NVS partition ({len(nvs_bin)} bytes) to {port}...")
        subprocess.check_call(cmd)
        print("NVS provisioning complete!")
    finally:
        os.unlink(bin_path)


def main():
    parser = argparse.ArgumentParser(
        description="Provision ESP32-S3 CSI Node with WiFi and aggregator settings",
        epilog="Example: python provision.py --port COM7 --ssid MyWiFi --password secret --target-ip 192.168.1.20",
    )
    parser.add_argument("--port", required=True, help="Serial port (e.g. COM7, /dev/ttyUSB0)")
    parser.add_argument("--baud", type=int, default=460800, help="Flash baud rate (default: 460800)")
    parser.add_argument("--ssid", help="WiFi SSID")
    parser.add_argument("--password", help="WiFi password")
    parser.add_argument("--target-ip", help="Aggregator host IP (e.g. 192.168.1.20)")
    parser.add_argument("--target-port", type=int, help="Aggregator UDP port (default: 5005)")
    parser.add_argument("--node-id", type=int, help="Node ID 0-255 (default: 1)")
    # ADR-039: Edge intelligence configuration.
    parser.add_argument("--edge-tier", type=int, choices=[0, 1, 2],
                        help="Edge processing tier: 0=raw, 1=basic, 2=full")
    parser.add_argument("--pres-thresh", type=float,
                        help="Presence detection threshold (0=auto-calibrate)")
    parser.add_argument("--fall-thresh", type=float,
                        help="Fall detection threshold in rad/s^2 (default: 2.0)")
    parser.add_argument("--vital-window", type=int,
                        help="Phase history window for BPM estimation (32-256)")
    parser.add_argument("--vital-interval", type=int,
                        help="Vitals packet send interval in ms (100-10000)")
    parser.add_argument("--subk-count", type=int,
                        help="Number of top-K subcarriers to track (1-32)")
    wasm_verify_group = parser.add_mutually_exclusive_group()
    wasm_verify_group.add_argument("--wasm-verify", action="store_true", default=None,
                                   help="Enable Ed25519 signature verification for WASM uploads (ADR-040)")
    wasm_verify_group.add_argument("--no-wasm-verify", action="store_true", default=None,
                                   help="Disable WASM signature verification (lab/dev use only)")
    parser.add_argument("--wasm-pubkey", type=str,
                        help="Ed25519 public key for WASM signature verification (64 hex chars)")
    parser.add_argument("--dry-run", action="store_true", help="Generate NVS binary but don't flash")

    args = parser.parse_args()

    # Resolve wasm_verify: --wasm-verify → True, --no-wasm-verify → False, neither → None
    wasm_verify_val = None
    if args.wasm_verify:
        wasm_verify_val = True
    elif args.no_wasm_verify:
        wasm_verify_val = False

    # Validate wasm_pubkey format.
    wasm_pubkey_val = None
    if args.wasm_pubkey:
        pk = args.wasm_pubkey.strip()
        if len(pk) != 64 or not all(c in '0123456789abcdefABCDEF' for c in pk):
            parser.error("--wasm-pubkey must be exactly 64 hex characters (32 bytes)")
        wasm_pubkey_val = pk.lower()

    if not any([args.ssid, args.password is not None, args.target_ip,
                args.target_port, args.node_id is not None,
                args.edge_tier is not None, args.pres_thresh is not None,
                args.fall_thresh is not None, args.vital_window is not None,
                args.vital_interval is not None, args.subk_count is not None,
                wasm_verify_val is not None, wasm_pubkey_val is not None]):
        parser.error("At least one config value must be specified "
                     "(--ssid, --password, --target-ip, --target-port, --node-id, "
                     "--edge-tier, --pres-thresh, --fall-thresh, --vital-window, "
                     "--vital-interval, --subk-count, --wasm-verify/--no-wasm-verify, "
                     "--wasm-pubkey)")

    print("Building NVS configuration:")
    if args.ssid:
        print(f"  WiFi SSID:     {args.ssid}")
    if args.password is not None:
        print(f"  WiFi Password: {'*' * len(args.password)}")
    if args.target_ip:
        print(f"  Target IP:     {args.target_ip}")
    if args.target_port:
        print(f"  Target Port:   {args.target_port}")
    if args.node_id is not None:
        print(f"  Node ID:       {args.node_id}")
    if args.edge_tier is not None:
        print(f"  Edge Tier:     {args.edge_tier}")
    if args.pres_thresh is not None:
        print(f"  Pres Thresh:   {args.pres_thresh}")
    if args.fall_thresh is not None:
        print(f"  Fall Thresh:   {args.fall_thresh}")
    if args.vital_window is not None:
        print(f"  Vital Window:  {args.vital_window}")
    if args.vital_interval is not None:
        print(f"  Vital Int(ms): {args.vital_interval}")
    if args.subk_count is not None:
        print(f"  Top-K Subs:    {args.subk_count}")
    if wasm_verify_val is not None:
        print(f"  WASM Verify:   {'enabled' if wasm_verify_val else 'disabled'}")
    if wasm_pubkey_val is not None:
        print(f"  WASM Pubkey:   {wasm_pubkey_val[:8]}...{wasm_pubkey_val[-8:]}")

    csv_content = build_nvs_csv(
        args.ssid, args.password, args.target_ip, args.target_port, args.node_id,
        edge_tier=args.edge_tier, pres_thresh=args.pres_thresh,
        fall_thresh=args.fall_thresh, vital_window=args.vital_window,
        vital_interval_ms=args.vital_interval, subk_count=args.subk_count,
        wasm_verify=wasm_verify_val, wasm_pubkey=wasm_pubkey_val,
    )

    try:
        nvs_bin = generate_nvs_binary(csv_content, NVS_PARTITION_SIZE)
    except Exception as e:
        print(f"\nError generating NVS binary: {e}", file=sys.stderr)
        print("\nFallback: save CSV and flash manually with ESP-IDF tools.", file=sys.stderr)
        fallback_path = "nvs_config.csv"
        with open(fallback_path, "w") as f:
            f.write(csv_content)
        print(f"Saved NVS CSV to {fallback_path}", file=sys.stderr)
        print(f"Flash with: python $IDF_PATH/components/nvs_flash/"
              f"nvs_partition_generator/nvs_partition_gen.py generate "
              f"{fallback_path} nvs.bin 0x6000", file=sys.stderr)
        sys.exit(1)

    if args.dry_run:
        out = "nvs_provision.bin"
        with open(out, "wb") as f:
            f.write(nvs_bin)
        print(f"NVS binary saved to {out} ({len(nvs_bin)} bytes)")
        print(f"Flash manually: python -m esptool --chip esp32s3 --port {args.port} "
              f"write_flash 0x9000 {out}")
        return

    flash_nvs(args.port, args.baud, nvs_bin)


if __name__ == "__main__":
    main()

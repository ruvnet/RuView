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


def build_nvs_csv(ssid, password, target_ip, target_port, node_id):
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
    return buf.getvalue()


def generate_nvs_binary(csv_content, size):
    """Generate an NVS partition binary from CSV using nvs_partition_gen.py."""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".csv", delete=False) as f_csv:
        f_csv.write(csv_content)
        csv_path = f_csv.name

    bin_path = csv_path.replace(".csv", ".bin")

    try:
        # Try the pip-installed version first
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
    parser.add_argument("--dry-run", action="store_true", help="Generate NVS binary but don't flash")

    args = parser.parse_args()

    if not any([args.ssid, args.password is not None, args.target_ip,
                args.target_port, args.node_id is not None]):
        parser.error("At least one config value must be specified "
                     "(--ssid, --password, --target-ip, --target-port, --node-id)")

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

    csv_content = build_nvs_csv(args.ssid, args.password, args.target_ip,
                                args.target_port, args.node_id)

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

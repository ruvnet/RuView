#!/usr/bin/env python3
"""
ESP32-S3 CSI Node Provisioning Script

Writes WiFi credentials and aggregator target to the ESP32's NVS partition
so users can configure a pre-built firmware binary without recompiling.

Usage:
    python provision.py --port COM7 --ssid "MyWiFi" --password "secret" --target-ip 192.168.1.20

Requirements:
    pip install 'esptool>=5.0' nvs-partition-gen
    (or use the nvs_partition_gen.py bundled with ESP-IDF)

WARNING -- FULL-REPLACE SEMANTICS (issue #391):
    Every invocation REPLACES the entire `csi_cfg` NVS namespace on the device.
    Any key you don't pass on the CLI is erased. Always include WiFi credentials
    (--ssid, --password, --target-ip) unless you pass --force-partial.
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


def build_nvs_csv(args):
    """Build an NVS CSV string for the csi_cfg namespace."""
    buf = io.StringIO()
    writer = csv.writer(buf)
    writer.writerow(["key", "type", "encoding", "value"])
    writer.writerow(["csi_cfg", "namespace", "", ""])
    if args.ssid:
        writer.writerow(["ssid", "data", "string", args.ssid])
    if args.password is not None:
        writer.writerow(["password", "data", "string", args.password])
    if args.target_ip:
        writer.writerow(["target_ip", "data", "string", args.target_ip])
    if args.target_port is not None:
        writer.writerow(["target_port", "data", "u16", str(args.target_port)])
    if args.node_id is not None:
        writer.writerow(["node_id", "data", "u8", str(args.node_id)])
    # TDM mesh settings
    if args.tdm_slot is not None:
        writer.writerow(["tdm_slot", "data", "u8", str(args.tdm_slot)])
    if args.tdm_total is not None:
        writer.writerow(["tdm_nodes", "data", "u8", str(args.tdm_total)])
    # Edge intelligence settings (ADR-039)
    if args.edge_tier is not None:
        writer.writerow(["edge_tier", "data", "u8", str(args.edge_tier)])
    if args.pres_thresh is not None:
        writer.writerow(["pres_thresh", "data", "u16", str(args.pres_thresh)])
    if args.fall_thresh is not None:
        writer.writerow(["fall_thresh", "data", "u16", str(args.fall_thresh)])
    if args.vital_win is not None:
        writer.writerow(["vital_win", "data", "u16", str(args.vital_win)])
    if args.vital_int is not None:
        writer.writerow(["vital_int", "data", "u16", str(args.vital_int)])
    if args.subk_count is not None:
        writer.writerow(["subk_count", "data", "u8", str(args.subk_count)])
    # ADR-060: Channel override and MAC filter
    if args.channel is not None:
        writer.writerow(["csi_channel", "data", "u8", str(args.channel)])
    if args.filter_mac is not None:
        mac_bytes = bytes(int(b, 16) for b in args.filter_mac.split(":"))
        # NVS blob: write as hex-encoded string for CSV compatibility
        writer.writerow(["filter_mac", "data", "hex2bin", mac_bytes.hex()])
    # ADR-073: Multi-frequency channel hopping
    if args.hop_channels is not None:
        channels = [int(c.strip()) for c in args.hop_channels.split(",")]
        writer.writerow(["hop_count", "data", "u8", str(len(channels))])
        # Store as NVS blob (firmware reads "chan_list" as uint8 blob)
        chan_bytes = bytes(channels)
        writer.writerow(["chan_list", "data", "hex2bin", chan_bytes.hex()])
        writer.writerow(["dwell_ms", "data", "u32", str(args.hop_dwell)])
    # ADR-066: Swarm bridge configuration
    if args.seed_url is not None:
        writer.writerow(["seed_url", "data", "string", args.seed_url])
    if args.seed_token is not None:
        writer.writerow(["seed_token", "data", "string", args.seed_token])
    if args.zone is not None:
        writer.writerow(["zone_name", "data", "string", args.zone])
    if args.swarm_hb is not None:
        writer.writerow(["swarm_hb", "data", "u16", str(args.swarm_hb)])
    if args.swarm_ingest is not None:
        writer.writerow(["swarm_ingest", "data", "u16", str(args.swarm_ingest)])
    return buf.getvalue()


def generate_nvs_binary(csv_content, size):
    """Generate an NVS partition binary from CSV using nvs_partition_gen.py."""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".csv", delete=False) as f_csv:
        f_csv.write(csv_content)
        csv_path = f_csv.name

    bin_path = csv_path.replace(".csv", ".bin")

    try:
        # Method 1: subprocess invocation (most reliable across package versions)
        for module_name in ["esp_idf_nvs_partition_gen", "nvs_partition_gen"]:
            try:
                subprocess.check_call(
                    [sys.executable, "-m", module_name, "generate",
                     csv_path, bin_path, hex(size)],
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                )
                with open(bin_path, "rb") as f:
                    return f.read()
            except (subprocess.CalledProcessError, FileNotFoundError):
                continue

        # Method 2: ESP-IDF bundled script
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

        raise RuntimeError(
            "NVS partition generator not available. "
            "Install: pip install esp-idf-nvs-partition-gen"
        )

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
            # Keep underscore form — ESP-IDF v5.4 bundles esptool 4.10.0 which only
            # accepts "write_flash". pip's esptool >=5.x accepts both (hyphenated
            # form preferred) but keeps underscore working. Do not "correct" this.
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
    # TDM mesh settings
    parser.add_argument("--tdm-slot", type=int, help="TDM slot index for this node (0-based)")
    parser.add_argument("--tdm-total", type=int, help="Total number of TDM nodes in mesh")
    # Edge intelligence settings (ADR-039)
    parser.add_argument("--edge-tier", type=int, choices=[0, 1, 2],
                        help="Edge processing tier: 0=off, 1=stats, 2=vitals")
    parser.add_argument("--pres-thresh", type=int, help="Presence detection threshold (default: 50)")
    parser.add_argument("--fall-thresh", type=int, help="Fall detection threshold in milli-units "
                        "(value/1000 = rad/s²). Default: 15000 → 15.0 rad/s². "
                        "Raise to reduce false positives in high-traffic areas.")
    parser.add_argument("--vital-win", type=int, help="Phase history window in frames (default: 300)")
    parser.add_argument("--vital-int", type=int, help="Vitals packet interval in ms (default: 1000)")
    parser.add_argument("--subk-count", type=int, help="Top-K subcarrier count (default: 32)")
    # ADR-060: Channel override and MAC filter
    parser.add_argument("--channel", type=int, help="CSI channel (1-14 for 2.4GHz, 36-177 for 5GHz). "
                        "Overrides auto-detection from connected AP.")
    parser.add_argument("--filter-mac", type=str, help="MAC address to filter CSI frames (AA:BB:CC:DD:EE:FF)")
    # ADR-073: Multi-frequency channel hopping
    parser.add_argument("--hop-channels", type=str, help="Comma-separated channel list for hopping (e.g. '1,6,11')")
    parser.add_argument("--hop-dwell", type=int, default=200, help="Dwell time per channel in ms (default: 200)")
    # ADR-066: Swarm bridge
    parser.add_argument("--seed-url", type=str, help="Cognitum Seed base URL (e.g. http://10.1.10.236)")
    parser.add_argument("--seed-token", type=str, help="Seed Bearer token (from pairing)")
    parser.add_argument("--zone", type=str, help="Zone name for this node (e.g. lobby, hallway)")
    parser.add_argument("--swarm-hb", type=int, help="Swarm heartbeat interval in seconds (default 30)")
    parser.add_argument("--swarm-ingest", type=int, help="Swarm vector ingest interval in seconds (default 5)")
    parser.add_argument("--dry-run", action="store_true", help="Generate NVS binary but don't flash")
    parser.add_argument("--force-partial", action="store_true",
                        help="Allow partial config without WiFi credentials. "
                        "WARNING: flashing REPLACES the entire csi_cfg NVS namespace - "
                        "any key not passed on the CLI will be erased (issue #391).")

    args = parser.parse_args()

    has_value = any([
        args.ssid, args.password is not None, args.target_ip,
        args.target_port, args.node_id is not None,
        args.tdm_slot is not None, args.tdm_total is not None,
        args.edge_tier is not None, args.pres_thresh is not None,
        args.fall_thresh is not None, args.vital_win is not None,
        args.vital_int is not None, args.subk_count is not None,
        args.channel is not None, args.filter_mac is not None,
        args.seed_url is not None, args.zone is not None,
    ])
    if not has_value:
        parser.error("At least one config value must be specified")

    # Bug 2 (#391): Prevent silent wipe of WiFi credentials on partial invocations.
    # Flashing the generated NVS binary to offset 0x9000 REPLACES the entire
    # csi_cfg namespace — there is no merge with existing NVS. Require the full
    # WiFi trio unless the user explicitly opts in with --force-partial.
    wifi_trio_missing = [
        name for name, val in [
            ("--ssid", args.ssid),
            ("--password", args.password),
            ("--target-ip", args.target_ip),
        ] if val is None or val == ""
    ]
    if wifi_trio_missing and not args.force_partial:
        parser.error(
            f"Missing required WiFi credentials: {', '.join(wifi_trio_missing)}.\n"
            f"\n"
            f"  provision.py REPLACES the entire csi_cfg NVS namespace on each run.\n"
            f"  Any key not passed on the CLI will be erased -- including WiFi creds.\n"
            f"\n"
            f"  Either pass all of --ssid, --password, --target-ip,\n"
            f"  or add --force-partial to acknowledge that other NVS keys will be wiped."
        )
    if args.force_partial and wifi_trio_missing:
        print("WARNING: --force-partial is set. The following NVS keys will be WIPED "
              "(not present in this invocation):", file=sys.stderr)
        for k in wifi_trio_missing:
            print(f"  - {k.lstrip('-')}", file=sys.stderr)
        print("  Plus any other csi_cfg keys not passed on the CLI.\n", file=sys.stderr)

    # Validate TDM: if one is given, both should be
    if (args.tdm_slot is not None) != (args.tdm_total is not None):
        parser.error("--tdm-slot and --tdm-total must be specified together")
    if args.tdm_slot is not None and args.tdm_slot >= args.tdm_total:
        parser.error(f"--tdm-slot ({args.tdm_slot}) must be less than --tdm-total ({args.tdm_total})")

    # ADR-060: Validate channel and MAC filter
    if args.channel is not None:
        if not ((1 <= args.channel <= 14) or (36 <= args.channel <= 177)):
            parser.error(f"--channel must be 1-14 (2.4GHz) or 36-177 (5GHz), got {args.channel}")
    if args.filter_mac is not None:
        parts = args.filter_mac.split(":")
        if len(parts) != 6:
            parser.error(f"--filter-mac must be in AA:BB:CC:DD:EE:FF format, got '{args.filter_mac}'")
        try:
            for p in parts:
                val = int(p, 16)
                if val < 0 or val > 255:
                    raise ValueError
        except ValueError:
            parser.error(f"--filter-mac contains invalid hex bytes: '{args.filter_mac}'")

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
    if args.tdm_slot is not None:
        print(f"  TDM Slot:      {args.tdm_slot} of {args.tdm_total}")
    if args.edge_tier is not None:
        tier_desc = {0: "off (raw CSI)", 1: "stats", 2: "vitals"}
        print(f"  Edge Tier:     {args.edge_tier} ({tier_desc.get(args.edge_tier, '?')})")
    if args.pres_thresh is not None:
        print(f"  Pres Thresh:   {args.pres_thresh}")
    if args.fall_thresh is not None:
        print(f"  Fall Thresh:   {args.fall_thresh}")
    if args.vital_win is not None:
        print(f"  Vital Window:  {args.vital_win} frames")
    if args.vital_int is not None:
        print(f"  Vital Interval:{args.vital_int} ms")
    if args.subk_count is not None:
        print(f"  Top-K Subcarr: {args.subk_count}")
    if args.channel is not None:
        print(f"  CSI Channel:   {args.channel}")
    if args.filter_mac is not None:
        print(f"  Filter MAC:    {args.filter_mac}")
    if args.seed_url is not None:
        print(f"  Seed URL:      {args.seed_url}")
    if args.zone is not None:
        print(f"  Zone:          {args.zone}")
    if args.swarm_hb is not None:
        print(f"  Swarm HB:      {args.swarm_hb}s")
    if args.swarm_ingest is not None:
        print(f"  Swarm Ingest:  {args.swarm_ingest}s")

    csv_content = build_nvs_csv(args)

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
              f"write-flash 0x9000 {out}")
        return

    flash_nvs(args.port, args.baud, nvs_bin)


if __name__ == "__main__":
    main()

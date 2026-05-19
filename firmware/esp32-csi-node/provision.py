#!/usr/bin/env python3
"""
ESP32 CSI node provisioning (ESP32-S3, ESP32-C6, other targets).

Writes WiFi credentials and aggregator target to the ESP32's NVS partition
so users can configure a pre-built firmware binary without recompiling.

Usage:
    python provision.py --port COM7 --ssid "MyWiFi" --password "secret" --target-ip 192.168.1.20
    python provision.py --port /dev/ttyUSB0 --chip esp32c6 --ssid "..." \\
        --password "..." --target-ip 192.168.1.20

Requirements:
    pip install 'esptool>=5.0' nvs-partition-gen
    (or use the nvs_partition_gen.py bundled with ESP-IDF)

ADDITIVE-BY-DEFAULT (issue #391, #574 phase 1):
    Earlier versions of this script REPLACED the entire `csi_cfg` NVS namespace
    on the device every invocation, wiping any key you didn't pass on the CLI.
    That cost customers hours of unnecessary friction.

    The script now MERGES new CLI flags with the per-port state previously
    written from this machine (stored under your user config dir; see
    `--state-dir` to override or `--state` to inspect). On every invocation:

        1. Read the prior per-port state file (or treat as empty if absent).
        2. Overlay the new CLI flags on top.
        3. Generate + flash NVS from the merged state.
        4. Write the merged state back to the state file.

    Net effect: partial reconfigure works the way users expect. Pass `--reset`
    to wipe both the state file AND the device NVS for first-time provisioning
    of a recycled board.

    Caveat: state lives on the controlling machine. Provisioning the same
    device from a second machine starts from an empty state — pass the keys
    you want to keep on that invocation, or pre-seed the state file. A future
    follow-up will add USB-CDC NVS dump for true device-authoritative merging
    (tracked in #574).
"""

import argparse
import csv
import io
import json
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


CONFIG_VALUE_CHECKS = [
    ("ssid", bool),
    ("password", lambda value: value is not None),
    ("target_ip", bool),
    ("target_port", lambda value: value is not None),
    ("node_id", lambda value: value is not None),
    ("tdm_slot", lambda value: value is not None),
    ("tdm_total", lambda value: value is not None),
    ("edge_tier", lambda value: value is not None),
    ("pres_thresh", lambda value: value is not None),
    ("fall_thresh", lambda value: value is not None),
    ("vital_win", lambda value: value is not None),
    ("vital_int", lambda value: value is not None),
    ("subk_count", lambda value: value is not None),
    ("channel", lambda value: value is not None),
    ("filter_mac", lambda value: value is not None),
    ("hop_channels", lambda value: value is not None),
    ("seed_url", lambda value: value is not None),
    ("seed_token", lambda value: value is not None),
    ("zone", lambda value: value is not None),
    ("swarm_hb", lambda value: value is not None),
    ("swarm_ingest", lambda value: value is not None),
]


def has_config_value(args):
    """Return True when args include at least one NVS-writing config value."""
    return any(
        check(getattr(args, name, None))
        for name, check in CONFIG_VALUE_CHECKS
    )


# ---------------------------------------------------------------------------
# Per-port state file (additive-by-default merging, #391 / #574)
# ---------------------------------------------------------------------------
#
# The state file is JSON keyed by `args` attribute name. It captures every
# config value previously written to a given serial port from this machine.
# On the next invocation, missing CLI flags fall back to the stored value.

# argparse attribute names that participate in the merge. Order doesn't
# matter; this is just the surface area to round-trip.
MERGEABLE_ATTRS = [
    "ssid", "password", "target_ip", "target_port", "node_id",
    "tdm_slot", "tdm_total",
    "edge_tier", "pres_thresh", "fall_thresh",
    "vital_win", "vital_int", "subk_count",
    "channel", "filter_mac",
    "hop_channels", "hop_dwell",
    "seed_url", "seed_token", "zone", "swarm_hb", "swarm_ingest",
]


def _default_state_dir() -> str:
    """Per-user config dir for provision-state JSON files."""
    env = os.environ
    if sys.platform == "win32":
        base = env.get("APPDATA") or os.path.expanduser("~")
    else:
        base = env.get("XDG_CONFIG_HOME") or os.path.join(
            os.path.expanduser("~"), ".config"
        )
    return os.path.join(base, "wifi-densepose", "esp32-provision-state")


def _state_path_for(port: str, state_dir: str) -> str:
    """File path for a given serial port. Sanitize the port for filesystem use."""
    safe = port.replace("/", "_").replace(":", "_").replace("\\", "_")
    return os.path.join(state_dir, f"{safe}.json")


def load_state(port: str, state_dir: str) -> dict:
    """Return the merged-state dict for `port`, or `{}` if absent / unreadable."""
    path = _state_path_for(port, state_dir)
    if not os.path.isfile(path):
        return {}
    try:
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
        if isinstance(data, dict):
            return data
    except (OSError, json.JSONDecodeError) as exc:
        print(f"WARNING: could not read state file {path}: {exc}", file=sys.stderr)
    return {}


def save_state(port: str, state_dir: str, state: dict) -> str:
    """Write `state` to the per-port file, creating dirs as needed. Returns path."""
    os.makedirs(state_dir, exist_ok=True)
    path = _state_path_for(port, state_dir)
    # Sort keys for deterministic on-disk content (easier to diff).
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(state, f, indent=2, sort_keys=True)
        f.write("\n")
    os.replace(tmp, path)
    return path


def merge_state_into_args(args, prior: dict) -> dict:
    """Overlay `args` onto `prior` for every MERGEABLE_ATTRS attribute.

    CLI values win whenever they were explicitly set (i.e. not `None`).
    Returns the merged dict (for state persistence) and mutates `args`
    in place so downstream `build_nvs_csv` sees the merged values.
    """
    merged = dict(prior)
    for name in MERGEABLE_ATTRS:
        cli_val = getattr(args, name, None)
        if cli_val is not None:
            merged[name] = cli_val
        elif name in merged:
            setattr(args, name, merged[name])
    return merged


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


def flash_nvs(port, baud, nvs_bin, chip):
    """Flash the NVS partition binary to the ESP32."""
    with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as f:
        f.write(nvs_bin)
        bin_path = f.name

    try:
        cmd = [
            sys.executable, "-m", "esptool",
            "--chip", chip,
            "--port", port,
            "--baud", str(baud),
            "write-flash",
            hex(NVS_PARTITION_OFFSET), bin_path,
        ]
        print(f"Flashing NVS partition ({len(nvs_bin)} bytes) to {port} (chip={chip})...")
        subprocess.check_call(cmd)
        print("NVS provisioning complete!")
    finally:
        os.unlink(bin_path)


def main():
    parser = argparse.ArgumentParser(
        description="Provision CSI node NVS (WiFi + aggregator); works on S3, C6, etc.",
        epilog=(
            "Example: python provision.py --port COM7 --ssid MyWiFi --password secret "
            "--target-ip 192.168.1.20\n"
            "ESP32-C6: same, or pass --chip esp32c6 if auto-detect fails "
            "(default chip is auto for esptool v5+)."
        ),
    )
    parser.add_argument("--port", required=True, help="Serial port (e.g. COM7, /dev/ttyUSB0)")
    parser.add_argument(
        "--chip",
        default="auto",
        help="esptool target: auto (default), esp32s3, esp32c6, ... (must match connected chip)",
    )
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
                        help="[deprecated since #391/#574] Suppress the missing-WiFi-trio "
                        "error when no prior state file exists. The script now merges "
                        "with prior state by default, so this flag is rarely needed.")
    parser.add_argument("--reset", action="store_true",
                        help="Wipe this machine's per-port state file before merging. "
                        "Use for first-time provisioning of a recycled board where "
                        "previously-staged keys should NOT be re-applied.")
    parser.add_argument("--state-dir", default=_default_state_dir(),
                        help="Override the per-user state directory (default: per-OS user config dir).")
    parser.add_argument("--state", action="store_true",
                        help="Print the merged state that WOULD be flashed for this port and exit. "
                        "Useful for debugging which keys are about to land on the device.")

    args = parser.parse_args()

    # --- Per-port state load + merge (additive-by-default, #391 / #574) ---
    if args.reset:
        path = _state_path_for(args.port, args.state_dir)
        if os.path.isfile(path):
            os.unlink(path)
            print(f"--reset: removed state file {path}", file=sys.stderr)
        prior = {}
    else:
        prior = load_state(args.port, args.state_dir)
    merged = merge_state_into_args(args, prior)

    if args.state:
        print(json.dumps(merged, indent=2, sort_keys=True))
        return

    if not has_config_value(args):
        parser.error(
            "At least one config value must be specified (after merging prior state). "
            "If you intended to start fresh, pass --reset and the keys you want."
        )

    # WiFi-trio sanity check. After the merge, the trio should be present
    # unless the user is intentionally provisioning a brand-new board with
    # partial state. Keep --force-partial as the escape hatch for that case.
    wifi_trio_missing = [
        name for name, val in [
            ("--ssid", args.ssid),
            ("--password", args.password),
            ("--target-ip", args.target_ip),
        ] if val is None or val == ""
    ]
    if wifi_trio_missing and not args.force_partial:
        parser.error(
            f"Missing required WiFi credentials after merging prior state: "
            f"{', '.join(wifi_trio_missing)}.\n"
            f"\n"
            f"  No per-port state file at {_state_path_for(args.port, args.state_dir)}\n"
            f"  and the CLI didn't include them. Either pass --ssid + --password + --target-ip\n"
            f"  on this run, or add --force-partial to flash without WiFi.\n"
        )
    if args.force_partial and wifi_trio_missing:
        print(
            "WARNING: --force-partial is set and WiFi credentials are missing. "
            "The device will not connect to WiFi after flashing.",
            file=sys.stderr,
        )

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
        print(f"  WiFi Password: {'(set)' if args.password else '(empty)'}")
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
        print(f"Flash manually: python -m esptool --chip {args.chip} --port {args.port} "
              f"write-flash 0x9000 {out}")
        # Persist merged state even on dry-run so a subsequent real flash from
        # this machine sees the same staged config.
        path = save_state(args.port, args.state_dir, merged)
        print(f"State persisted to {path}")
        return

    flash_nvs(args.port, args.baud, nvs_bin, args.chip)
    # Persist merged state after a successful flash so future partial
    # invocations from this machine merge on top of what's actually on the
    # device. This is the heart of the additive-by-default fix (#391/#574).
    path = save_state(args.port, args.state_dir, merged)
    print(f"State persisted to {path}")


if __name__ == "__main__":
    main()

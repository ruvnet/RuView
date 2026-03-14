#!/usr/bin/env python3
"""
NVS Test Matrix Generator (ADR-061)

Generates NVS partition binaries for 14 test configurations using the
provision.py script's CSV builder and NVS binary generator. Each binary
can be injected into a QEMU flash image at offset 0x9000 for automated
firmware testing under different NVS configurations.

Usage:
    python3 generate_nvs_matrix.py --output-dir build/nvs_matrix

    # Generate only specific configs:
    python3 generate_nvs_matrix.py --output-dir build/nvs_matrix --only default,full-adr060

Requirements:
    - esp_idf_nvs_partition_gen (pip install) or ESP-IDF nvs_partition_gen.py
    - Python 3.8+
"""

import argparse
import csv
import io
import os
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Tuple


# NVS partition size must match partitions_display.csv: 0x6000 = 24576 bytes
NVS_PARTITION_SIZE = 0x6000


@dataclass
class NvsEntry:
    """A single NVS key-value entry."""
    key: str
    type: str       # "data" or "namespace"
    encoding: str   # "string", "u8", "u16", "u32", "hex2bin", ""
    value: str


@dataclass
class NvsConfig:
    """A named NVS configuration with a list of entries."""
    name: str
    description: str
    entries: List[NvsEntry] = field(default_factory=list)

    def to_csv(self) -> str:
        """Generate NVS CSV content."""
        buf = io.StringIO()
        writer = csv.writer(buf)
        writer.writerow(["key", "type", "encoding", "value"])
        writer.writerow(["csi_cfg", "namespace", "", ""])
        for entry in self.entries:
            writer.writerow([entry.key, entry.type, entry.encoding, entry.value])
        return buf.getvalue()


def define_configs() -> List[NvsConfig]:
    """Define all 14 NVS test configurations."""
    configs = []

    # 1. default - no NVS entries (firmware uses Kconfig defaults)
    configs.append(NvsConfig(
        name="default",
        description="No NVS entries; firmware uses Kconfig defaults",
        entries=[],
    ))

    # 2. wifi-only - just WiFi credentials
    configs.append(NvsConfig(
        name="wifi-only",
        description="WiFi SSID and password only",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
        ],
    ))

    # 3. full-adr060 - channel override + MAC filter
    configs.append(NvsConfig(
        name="full-adr060",
        description="ADR-060: channel override + MAC filter + full config",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("target_port", "data", "u16", "5005"),
            NvsEntry("node_id", "data", "u8", "1"),
            NvsEntry("csi_channel", "data", "u8", "6"),
            NvsEntry("filter_mac", "data", "hex2bin", "aabbccddeeff"),
        ],
    ))

    # 4. edge-tier0 - raw passthrough (no DSP)
    configs.append(NvsConfig(
        name="edge-tier0",
        description="Edge tier 0: raw CSI passthrough, no on-device DSP",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("edge_tier", "data", "u8", "0"),
        ],
    ))

    # 5. edge-tier1 - basic presence/motion detection
    configs.append(NvsConfig(
        name="edge-tier1",
        description="Edge tier 1: basic presence and motion detection",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("edge_tier", "data", "u8", "1"),
            NvsEntry("pres_thresh", "data", "u16", "50"),
        ],
    ))

    # 6. edge-tier2-custom - full pipeline with custom thresholds
    configs.append(NvsConfig(
        name="edge-tier2-custom",
        description="Edge tier 2: full pipeline with custom thresholds",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("edge_tier", "data", "u8", "2"),
            NvsEntry("pres_thresh", "data", "u16", "100"),
            NvsEntry("fall_thresh", "data", "u16", "3000"),
            NvsEntry("vital_win", "data", "u16", "256"),
            NvsEntry("vital_int", "data", "u16", "500"),
            NvsEntry("subk_count", "data", "u8", "16"),
        ],
    ))

    # 7. tdm-3node - TDM mesh with 3 nodes (slot 0)
    configs.append(NvsConfig(
        name="tdm-3node",
        description="TDM mesh: 3-node schedule, this node is slot 0",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("node_id", "data", "u8", "0"),
            NvsEntry("tdm_slot", "data", "u8", "0"),
            NvsEntry("tdm_nodes", "data", "u8", "3"),
        ],
    ))

    # 8. wasm-signed - WASM runtime with signature verification
    configs.append(NvsConfig(
        name="wasm-signed",
        description="WASM runtime enabled with Ed25519 signature verification",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("edge_tier", "data", "u8", "2"),
            # wasm_verify=1 + a 32-byte dummy Ed25519 pubkey
            NvsEntry("wasm_verify", "data", "u8", "1"),
            NvsEntry("wasm_pubkey", "data", "hex2bin",
                     "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
        ],
    ))

    # 9. wasm-unsigned - WASM runtime without signature verification
    configs.append(NvsConfig(
        name="wasm-unsigned",
        description="WASM runtime with signature verification disabled",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("edge_tier", "data", "u8", "2"),
            NvsEntry("wasm_verify", "data", "u8", "0"),
            NvsEntry("wasm_max", "data", "u8", "2"),
        ],
    ))

    # 10. 5ghz-channel - 5 GHz channel override
    configs.append(NvsConfig(
        name="5ghz-channel",
        description="ADR-060: 5 GHz channel 36 override",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork5G"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("csi_channel", "data", "u8", "36"),
        ],
    ))

    # 11. boundary-max - maximum VALID values for all numeric fields
    # Uses firmware-validated max ranges (not raw u8/u16 max):
    #   vital_win: 32-256, top_k: 1-32, power_duty: 10-100
    configs.append(NvsConfig(
        name="boundary-max",
        description="Boundary test: maximum valid values per firmware validation ranges",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("target_port", "data", "u16", "65535"),
            NvsEntry("node_id", "data", "u8", "255"),
            NvsEntry("edge_tier", "data", "u8", "2"),
            NvsEntry("pres_thresh", "data", "u16", "65535"),
            NvsEntry("fall_thresh", "data", "u16", "65535"),
            NvsEntry("vital_win", "data", "u16", "256"),     # max validated
            NvsEntry("vital_int", "data", "u16", "10000"),
            NvsEntry("subk_count", "data", "u8", "32"),
            NvsEntry("power_duty", "data", "u8", "100"),
        ],
    ))

    # 12. boundary-min - minimum VALID values for all numeric fields
    configs.append(NvsConfig(
        name="boundary-min",
        description="Boundary test: minimum valid values per firmware validation ranges",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("target_port", "data", "u16", "1024"),
            NvsEntry("node_id", "data", "u8", "0"),
            NvsEntry("edge_tier", "data", "u8", "0"),
            NvsEntry("pres_thresh", "data", "u16", "1"),
            NvsEntry("fall_thresh", "data", "u16", "100"),    # min valid (0.1 rad/s²)
            NvsEntry("vital_win", "data", "u16", "32"),       # min validated
            NvsEntry("vital_int", "data", "u16", "100"),
            NvsEntry("subk_count", "data", "u8", "1"),
            NvsEntry("power_duty", "data", "u8", "10"),
        ],
    ))

    # 13. power-save - low power duty cycle configuration
    configs.append(NvsConfig(
        name="power-save",
        description="Power-save mode: 10% duty cycle for battery-powered nodes",
        entries=[
            NvsEntry("ssid", "data", "string", "TestNetwork"),
            NvsEntry("password", "data", "string", "testpass123"),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
            NvsEntry("edge_tier", "data", "u8", "1"),
            NvsEntry("power_duty", "data", "u8", "10"),
        ],
    ))

    # 14. empty-strings - empty SSID/password to test fallback to Kconfig
    configs.append(NvsConfig(
        name="empty-strings",
        description="Empty SSID and password to verify Kconfig fallback",
        entries=[
            NvsEntry("ssid", "data", "string", ""),
            NvsEntry("password", "data", "string", ""),
            NvsEntry("target_ip", "data", "string", "10.0.2.2"),
        ],
    ))

    return configs


def generate_nvs_binary(csv_content: str, size: int) -> bytes:
    """Generate an NVS partition binary from CSV content.

    Tries multiple methods to find nvs_partition_gen:
    1. esp_idf_nvs_partition_gen pip package
    2. Legacy nvs_partition_gen pip package
    3. ESP-IDF bundled script (via IDF_PATH)
    4. Module invocation
    """
    import subprocess
    import tempfile

    with tempfile.NamedTemporaryFile(mode="w", suffix=".csv", delete=False) as f_csv:
        f_csv.write(csv_content)
        csv_path = f_csv.name

    bin_path = csv_path.replace(".csv", ".bin")

    try:
        # Try pip-installed version first
        try:
            from esp_idf_nvs_partition_gen import nvs_partition_gen
            nvs_partition_gen.generate(csv_path, bin_path, size)
            with open(bin_path, "rb") as f:
                return f.read()
        except ImportError:
            pass

        # Try legacy import
        try:
            import nvs_partition_gen
            nvs_partition_gen.generate(csv_path, bin_path, size)
            with open(bin_path, "rb") as f:
                return f.read()
        except ImportError:
            pass

        # Try ESP-IDF bundled script
        idf_path = os.environ.get("IDF_PATH", "")
        gen_script = os.path.join(
            idf_path, "components", "nvs_flash",
            "nvs_partition_generator", "nvs_partition_gen.py"
        )
        if os.path.isfile(gen_script):
            subprocess.check_call([
                sys.executable, gen_script, "generate",
                csv_path, bin_path, hex(size)
            ])
            with open(bin_path, "rb") as f:
                return f.read()

        # Last resort: try as a module
        try:
            subprocess.check_call([
                sys.executable, "-m", "nvs_partition_gen", "generate",
                csv_path, bin_path, hex(size)
            ])
            with open(bin_path, "rb") as f:
                return f.read()
        except (subprocess.CalledProcessError, FileNotFoundError):
            print("ERROR: NVS partition generator tool not found.", file=sys.stderr)
            print("Install: pip install esp-idf-nvs-partition-gen", file=sys.stderr)
            print("Or set IDF_PATH to your ESP-IDF installation", file=sys.stderr)
            raise RuntimeError(
                "NVS partition generator not available. "
                "Install: pip install esp-idf-nvs-partition-gen"
            )

    finally:
        for p in (csv_path, bin_path):
            if os.path.isfile(p):
                os.unlink(p)


def main():
    parser = argparse.ArgumentParser(
        description="Generate NVS partition binaries for QEMU firmware test matrix (ADR-061)",
    )
    parser.add_argument(
        "--output-dir", required=True,
        help="Directory to write NVS binary files",
    )
    parser.add_argument(
        "--only", type=str, default=None,
        help="Comma-separated list of config names to generate (default: all)",
    )
    parser.add_argument(
        "--csv-only", action="store_true",
        help="Only generate CSV files, skip binary generation",
    )
    parser.add_argument(
        "--list", action="store_true", dest="list_configs",
        help="List all available configurations and exit",
    )

    args = parser.parse_args()

    all_configs = define_configs()

    if args.list_configs:
        print(f"{'Name':<20} {'Description'}")
        print("-" * 70)
        for cfg in all_configs:
            print(f"{cfg.name:<20} {cfg.description}")
        sys.exit(0)

    # Filter configs if --only specified
    if args.only:
        selected = set(args.only.split(","))
        configs = [c for c in all_configs if c.name in selected]
        missing = selected - {c.name for c in configs}
        if missing:
            print(f"WARNING: Unknown config names: {', '.join(sorted(missing))}",
                  file=sys.stderr)
    else:
        configs = all_configs

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"Generating {len(configs)} NVS configurations in {output_dir}/")
    print()

    success = 0
    errors = 0

    for cfg in configs:
        csv_content = cfg.to_csv()

        # Always write the CSV for reference
        csv_path = output_dir / f"nvs_{cfg.name}.csv"
        csv_path.write_text(csv_content)

        if cfg.name == "default" and not cfg.entries:
            # "default" means no NVS — just produce an empty marker
            print(f"  [{cfg.name}] No NVS entries (uses Kconfig defaults)")
            # Write a zero-filled NVS partition (erased state = 0xFF)
            bin_path = output_dir / f"nvs_{cfg.name}.bin"
            bin_path.write_bytes(b"\xff" * NVS_PARTITION_SIZE)
            success += 1
            continue

        if args.csv_only:
            print(f"  [{cfg.name}] CSV only: {csv_path}")
            success += 1
            continue

        try:
            nvs_bin = generate_nvs_binary(csv_content, NVS_PARTITION_SIZE)
            bin_path = output_dir / f"nvs_{cfg.name}.bin"
            bin_path.write_bytes(nvs_bin)
            print(f"  [{cfg.name}] {len(nvs_bin)} bytes -> {bin_path}")
            success += 1
        except Exception as e:
            print(f"  [{cfg.name}] ERROR: {e}", file=sys.stderr)
            errors += 1

    print()
    print(f"Done: {success} succeeded, {errors} failed")

    if errors > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()

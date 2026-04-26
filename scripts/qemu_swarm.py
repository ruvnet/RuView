#!/usr/bin/env python3
"""
QEMU ESP32-S3 Swarm Configurator (ADR-062)

Orchestrates multiple QEMU ESP32-S3 instances from a YAML configuration.
Supports star/mesh/line/ring topologies, role-based nodes (sensor/coordinator/
gateway), per-node NVS provisioning, and swarm-level health assertions.

Usage:
    python3 qemu_swarm.py --config swarm_presets/standard.yaml
    python3 qemu_swarm.py --preset smoke
    python3 qemu_swarm.py --preset standard --timeout 90
    python3 qemu_swarm.py --list-presets
    python3 qemu_swarm.py --config custom.yaml --dry-run
"""

import argparse
import atexit
import json
import os
import platform
import re
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# ---------------------------------------------------------------------------
# Optional YAML import with helpful error
# ---------------------------------------------------------------------------
try:
    import yaml
except ImportError:
    print("ERROR: PyYAML is required but not installed.")
    print("  Install: pip install pyyaml")
    print("  Or:      pip3 install pyyaml")
    sys.exit(3)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent
FIRMWARE_DIR = PROJECT_ROOT / "firmware" / "esp32-csi-node"
RUST_DIR = PROJECT_ROOT / "v2" / "wifi-densepose-rs"
PROVISION_SCRIPT = FIRMWARE_DIR / "provision.py"
PRESETS_DIR = SCRIPT_DIR / "swarm_presets"

VALID_TOPOLOGIES = ("star", "mesh", "line", "ring")
VALID_ROLES = ("sensor", "coordinator", "gateway")
EXIT_PASS = 0
EXIT_WARN = 1
EXIT_FAIL = 2
EXIT_FATAL = 3

NVS_OFFSET = 0x9000  # NVS partition offset in flash image

IS_LINUX = platform.system() == "Linux"

# ---------------------------------------------------------------------------
# Logging helpers
# ---------------------------------------------------------------------------
USE_COLOR = sys.stdout.isatty()


def _c(text: str, code: str) -> str:
    return f"\033[{code}m{text}\033[0m" if USE_COLOR else text


def info(msg: str) -> None:
    print(f"[INFO]  {msg}")


def warn(msg: str) -> None:
    print(f"[{_c('WARN', '33')}]  {msg}")


def error(msg: str) -> None:
    print(f"[{_c('ERROR', '1;31')}] {msg}", file=sys.stderr)


def fatal(msg: str) -> None:
    print(f"[{_c('FATAL', '1;31')}] {msg}", file=sys.stderr)


# ---------------------------------------------------------------------------
# Schema validation
# ---------------------------------------------------------------------------
@dataclass
class NodeConfig:
    role: str
    node_id: int
    scenario: int = 0
    channel: int = 6
    tdm_slot: Optional[int] = None
    edge_tier: int = 0
    is_gateway: bool = False
    filter_mac: Optional[str] = None


@dataclass
class SwarmConfig:
    name: str
    duration_s: int
    topology: str
    aggregator_port: int
    nodes: List[NodeConfig]
    assertions: List[Any]

    def coordinator_nodes(self) -> List[NodeConfig]:
        return [n for n in self.nodes if n.role in ("coordinator", "gateway")]

    def sensor_nodes(self) -> List[NodeConfig]:
        return [n for n in self.nodes if n.role == "sensor"]


def validate_config(raw: dict) -> SwarmConfig:
    """Parse and validate YAML config into a SwarmConfig."""
    errors: List[str] = []

    swarm = raw.get("swarm", {})
    name = swarm.get("name", "unnamed-swarm")
    duration_s = int(swarm.get("duration_s", 60))
    topology = swarm.get("topology", "mesh")
    aggregator_port = int(swarm.get("aggregator_port", 5005))

    if topology not in VALID_TOPOLOGIES:
        errors.append(f"Invalid topology '{topology}'; must be one of {VALID_TOPOLOGIES}")

    if duration_s < 5:
        errors.append(f"duration_s={duration_s} too short; minimum is 5")

    raw_nodes = raw.get("nodes", [])
    if not raw_nodes:
        errors.append("No nodes defined")

    nodes: List[NodeConfig] = []
    seen_ids: set = set()
    for idx, rn in enumerate(raw_nodes):
        if not isinstance(rn, dict):
            errors.append(f"nodes[{idx}]: expected dict, got {type(rn).__name__}")
            continue

        role = rn.get("role", "sensor")
        if role not in VALID_ROLES:
            errors.append(f"nodes[{idx}]: invalid role '{role}'; must be one of {VALID_ROLES}")

        node_id = rn.get("node_id", idx)
        if node_id in seen_ids:
            errors.append(f"nodes[{idx}]: duplicate node_id={node_id}")
        seen_ids.add(node_id)

        nodes.append(NodeConfig(
            role=role,
            node_id=int(node_id),
            scenario=int(rn.get("scenario", 0)),
            channel=int(rn.get("channel", 6)),
            tdm_slot=rn.get("tdm_slot"),
            edge_tier=int(rn.get("edge_tier", 0)),
            is_gateway=bool(rn.get("is_gateway", False)),
            filter_mac=rn.get("filter_mac"),
        ))

    # Auto-assign TDM slots if not set
    for i, n in enumerate(nodes):
        if n.tdm_slot is None:
            n.tdm_slot = i

    assertions = raw.get("assertions", [])

    if errors:
        for e in errors:
            error(e)
        fatal(f"{len(errors)} config validation error(s)")
        sys.exit(EXIT_FATAL)

    return SwarmConfig(
        name=name,
        duration_s=duration_s,
        topology=topology,
        aggregator_port=aggregator_port,
        nodes=nodes,
        assertions=assertions,
    )


# ---------------------------------------------------------------------------
# Preset loading
# ---------------------------------------------------------------------------
def list_presets() -> List[Tuple[str, str]]:
    """Return list of (name, description) for available presets."""
    presets = []
    if not PRESETS_DIR.is_dir():
        return presets
    for f in sorted(PRESETS_DIR.glob("*.yaml")):
        name = f.stem
        # Read first comment line as description
        desc = ""
        try:
            text = f.read_text(encoding="utf-8")
            for line in text.splitlines():
                if line.startswith("#"):
                    desc = line.lstrip("#").strip()
                    break
        except OSError:
            pass
        presets.append((name, desc))
    return presets


def load_preset(name: str) -> dict:
    """Load a preset YAML file by name."""
    path = PRESETS_DIR / f"{name}.yaml"
    if not path.exists():
        # Try with underscores/hyphens swapped
        alt = PRESETS_DIR / f"{name.replace('-', '_')}.yaml"
        if alt.exists():
            path = alt
        else:
            fatal(f"Preset '{name}' not found at {path}")
            available = list_presets()
            if available:
                print("Available presets:")
                for pname, pdesc in available:
                    print(f"  {pname:20s} {pdesc}")
            sys.exit(EXIT_FATAL)
    return yaml.safe_load(path.read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# Node provisioning
# ---------------------------------------------------------------------------
def provision_node(
    node: NodeConfig,
    build_dir: Path,
    n_total: int,
    aggregator_ip: str,
    aggregator_port: int,
) -> Path:
    """Generate NVS binary and per-node flash image. Returns flash image path."""

    nvs_bin = build_dir / f"nvs_node{node.node_id}.bin"
    flash_image = build_dir / f"qemu_flash_node{node.node_id}.bin"
    base_image = build_dir / "qemu_flash_base.bin"
    if not base_image.exists():
        base_image = build_dir / "qemu_flash.bin"

    if not base_image.exists():
        fatal(f"Base flash image not found: {build_dir / 'qemu_flash_base.bin'} or {build_dir / 'qemu_flash.bin'}")
        fatal("Build the firmware first, or run without --skip-build.")
        sys.exit(EXIT_FATAL)

    # Remove stale nvs_provision.bin to prevent race with prior node
    stale = build_dir / "nvs_provision.bin"
    if stale.exists():
        stale.unlink()

    # Build provision.py arguments
    args = [
        sys.executable, str(PROVISION_SCRIPT),
        "--port", "/dev/null",
        "--dry-run",
        "--node-id", str(node.node_id),
        "--tdm-slot", str(node.tdm_slot),
        "--tdm-total", str(n_total),
        "--target-ip", aggregator_ip,
        "--target-port", str(aggregator_port),
    ]

    if node.channel is not None:
        args.extend(["--channel", str(node.channel)])

    if node.edge_tier:
        args.extend(["--edge-tier", str(node.edge_tier)])

    if node.filter_mac:
        args.extend(["--filter-mac", node.filter_mac])

    info(f"  Provisioning node {node.node_id} ({node.role}, scenario={node.scenario}, "
         f"tdm={node.tdm_slot}/{n_total}, ch={node.channel})")

    result = subprocess.run(
        args,
        capture_output=True, text=True,
        cwd=str(build_dir),
        timeout=30,
    )

    if result.returncode != 0:
        error(f"  provision.py failed for node {node.node_id}:")
        error(f"  stdout: {result.stdout.strip()}")
        error(f"  stderr: {result.stderr.strip()}")
        sys.exit(EXIT_FATAL)

    # provision.py --dry-run writes nvs_provision.bin in cwd
    nvs_src = build_dir / "nvs_provision.bin"
    if not nvs_src.exists():
        fatal(f"  provision.py did not produce nvs_provision.bin for node {node.node_id}")
        sys.exit(EXIT_FATAL)

    nvs_src.rename(nvs_bin)

    # Copy base image and inject NVS at 0x9000
    shutil.copy2(str(base_image), str(flash_image))

    with open(flash_image, "r+b") as f:
        f.seek(NVS_OFFSET)
        f.write(nvs_bin.read_bytes())

    return flash_image


# ---------------------------------------------------------------------------
# Network topology setup (Linux TAP/bridge)
# ---------------------------------------------------------------------------
@dataclass
class NetworkState:
    """Tracks created bridges and TAPs for cleanup."""
    bridges: List[str] = field(default_factory=list)
    taps: List[str] = field(default_factory=list)
    use_slirp: bool = False


def _run_ip(args: List[str], check: bool = False) -> subprocess.CompletedProcess:
    try:
        return subprocess.run(["ip"] + args, capture_output=True, text=True, check=check)
    except FileNotFoundError:
        # 'ip' command not installed (e.g. minimal container image)
        return subprocess.CompletedProcess(args=["ip"] + args, returncode=127,
                                           stdout="", stderr="ip: command not found")


def setup_network(cfg: SwarmConfig, net: NetworkState) -> Dict[int, List[str]]:
    """
    Create network topology. Returns dict mapping node_id -> QEMU network args.

    Falls back to SLIRP user-mode networking if not root or not Linux.
    """
    node_net_args: Dict[int, List[str]] = {}
    n = len(cfg.nodes)

    # Check if we can use TAP/bridge (requires root on Linux + ip command)
    import shutil
    can_tap = (IS_LINUX and hasattr(os, 'geteuid') and os.geteuid() == 0
               and shutil.which("ip") is not None)

    if not can_tap:
        if IS_LINUX:
            warn("Not running as root; falling back to SLIRP user-mode networking.")
            warn("Nodes can reach the aggregator but cannot see each other.")
        else:
            info("Non-Linux platform; using SLIRP user-mode networking.")

        net.use_slirp = True
        for node in cfg.nodes:
            node_net_args[node.node_id] = [
                "-nic", f"user,id=net{node.node_id},"
                        f"hostfwd=udp::{cfg.aggregator_port + 100 + node.node_id}"
                        f"-:{cfg.aggregator_port}",
            ]
        return node_net_args

    # --- TAP/bridge topology ---
    info(f"Setting up {cfg.topology} topology with TAP/bridge...")

    if cfg.topology == "mesh":
        # Single bridge, all nodes attached
        br = "qemu-sw0"
        _run_ip(["link", "add", "name", br, "type", "bridge"])
        _run_ip(["addr", "add", "10.0.0.1/24", "dev", br])
        _run_ip(["link", "set", br, "up"])
        net.bridges.append(br)

        for node in cfg.nodes:
            tap = f"tap{node.node_id}"
            mac = f"52:54:00:00:00:{node.node_id:02x}"
            _run_ip(["tuntap", "add", "dev", tap, "mode", "tap"])
            _run_ip(["link", "set", tap, "master", br])
            _run_ip(["link", "set", tap, "up"])
            net.taps.append(tap)

            node_net_args[node.node_id] = [
                "-nic", f"tap,ifname={tap},script=no,downscript=no,mac={mac}",
            ]

    elif cfg.topology == "star":
        # One bridge per sensor; coordinator has a TAP on each bridge
        coord_ids = {n.node_id for n in cfg.coordinator_nodes()}
        for idx, sensor in enumerate(cfg.sensor_nodes()):
            br = f"qemu-br{idx}"
            _run_ip(["link", "add", "name", br, "type", "bridge"])
            _run_ip(["addr", "add", f"10.0.{idx + 1}.1/24", "dev", br])
            _run_ip(["link", "set", br, "up"])
            net.bridges.append(br)

            # Sensor TAP
            s_tap = f"tap-s{sensor.node_id}"
            s_mac = f"52:54:00:01:{idx:02x}:{sensor.node_id:02x}"
            _run_ip(["tuntap", "add", "dev", s_tap, "mode", "tap"])
            _run_ip(["link", "set", s_tap, "master", br])
            _run_ip(["link", "set", s_tap, "up"])
            net.taps.append(s_tap)
            node_net_args.setdefault(sensor.node_id, []).extend([
                "-nic", f"tap,ifname={s_tap},script=no,downscript=no,mac={s_mac}",
            ])

            # Coordinator TAP on this bridge
            for cnode in cfg.coordinator_nodes():
                c_tap = f"tap-c{cnode.node_id}-b{idx}"
                c_mac = f"52:54:00:02:{idx:02x}:{cnode.node_id:02x}"
                _run_ip(["tuntap", "add", "dev", c_tap, "mode", "tap"])
                _run_ip(["link", "set", c_tap, "master", br])
                _run_ip(["link", "set", c_tap, "up"])
                net.taps.append(c_tap)
                node_net_args.setdefault(cnode.node_id, []).extend([
                    "-nic", f"tap,ifname={c_tap},script=no,downscript=no,mac={c_mac}",
                ])

    elif cfg.topology in ("line", "ring"):
        # Chain of bridges: br_i connects node_i <-> node_(i+1)
        pairs = list(range(n - 1))
        if cfg.topology == "ring" and n > 2:
            pairs.append(n - 1)  # extra bridge: last <-> first

        for pair_idx in range(len(pairs)):
            left_idx = pairs[pair_idx]
            right_idx = (pairs[pair_idx] + 1) % n

            left_node = cfg.nodes[left_idx]
            right_node = cfg.nodes[right_idx]

            br = f"qemu-br{pair_idx}"
            _run_ip(["link", "add", "name", br, "type", "bridge"])
            _run_ip(["addr", "add", f"10.0.{pair_idx + 1}.1/24", "dev", br])
            _run_ip(["link", "set", br, "up"])
            net.bridges.append(br)

            for side, nd in [("l", left_node), ("r", right_node)]:
                tap = f"tap-{side}{nd.node_id}-b{pair_idx}"
                mac = f"52:54:00:03:{pair_idx:02x}:{nd.node_id:02x}"
                _run_ip(["tuntap", "add", "dev", tap, "mode", "tap"])
                _run_ip(["link", "set", tap, "master", br])
                _run_ip(["link", "set", tap, "up"])
                net.taps.append(tap)
                node_net_args.setdefault(nd.node_id, []).extend([
                    "-nic", f"tap,ifname={tap},script=no,downscript=no,mac={mac}",
                ])

    return node_net_args


def teardown_network(net: NetworkState) -> None:
    """Remove all created TAP interfaces and bridges."""
    if not IS_LINUX or net.use_slirp:
        return

    for tap in net.taps:
        _run_ip(["link", "set", tap, "down"])
        _run_ip(["link", "delete", tap])

    for br in net.bridges:
        _run_ip(["link", "set", br, "down"])
        _run_ip(["link", "delete", br, "type", "bridge"])


# ---------------------------------------------------------------------------
# QEMU instance launch
# ---------------------------------------------------------------------------
def launch_node(
    node: NodeConfig,
    flash_image: Path,
    log_file: Path,
    net_args: List[str],
    qemu_bin: str,
) -> subprocess.Popen:
    """Launch a single QEMU ESP32-S3 instance. Returns the Popen handle."""
    args = [
        qemu_bin,
        "-machine", "esp32s3",
        "-nographic",
        "-drive", f"file={flash_image},if=mtd,format=raw",
        "-serial", f"file:{log_file}",
        "-no-reboot",
    ]
    args.extend(net_args)

    return subprocess.Popen(
        args,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


# ---------------------------------------------------------------------------
# Aggregator
# ---------------------------------------------------------------------------
def start_aggregator(
    port: int, n_nodes: int, output_file: Path, log_file: Path
) -> Optional[subprocess.Popen]:
    """Start the Rust aggregator binary. Returns Popen or None on failure."""
    import shutil
    cargo_toml = RUST_DIR / "Cargo.toml"
    if not cargo_toml.exists():
        warn(f"Rust workspace not found at {RUST_DIR}; skipping aggregator.")
        return None
    if shutil.which("cargo") is None:
        warn("cargo not found; skipping aggregator (Rust not installed).")
        return None

    args = [
        "cargo", "run",
        "--manifest-path", str(cargo_toml),
        "-p", "wifi-densepose-hardware",
        "--bin", "aggregator", "--",
        "--listen", f"0.0.0.0:{port}",
        "--expect-nodes", str(n_nodes),
        "--output", str(output_file),
    ]

    with open(log_file, "w") as lf:
        proc = subprocess.Popen(args, stdout=lf, stderr=subprocess.STDOUT)

    # Give it a moment to bind
    time.sleep(1)
    if proc.poll() is not None:
        error(f"Aggregator failed to start. Check {log_file}")
        return None

    return proc


# ---------------------------------------------------------------------------
# Swarm-level health assertions
# ---------------------------------------------------------------------------
def run_assertions(
    cfg: SwarmConfig,
    build_dir: Path,
    results_file: Path,
) -> int:
    """
    Run swarm-level assertions via validate_mesh_test.py (for basic checks)
    and inline checks for swarm-specific assertions.

    Returns exit code: 0=PASS, 1=WARN, 2=FAIL, 3=FATAL.

    NOTE: These inline assertions duplicate swarm_health.py. A future refactor
    should delegate to swarm_health.run_assertions() to avoid divergence.
    See ADR-062 architecture diagram.
    """
    n_nodes = len(cfg.nodes)
    worst = EXIT_PASS

    # Collect node logs
    logs: Dict[int, str] = {}
    for node in cfg.nodes:
        log_path = build_dir / f"qemu_node{node.node_id}.log"
        if log_path.exists():
            logs[node.node_id] = log_path.read_text(encoding="utf-8", errors="replace")
        else:
            logs[node.node_id] = ""

    def _check(name: str, passed: bool, msg_pass: str, msg_fail: str, level: int = EXIT_FAIL):
        nonlocal worst
        if passed:
            print(f"  [{_c('PASS', '32')}] {name}: {msg_pass}")
        else:
            sev_str = {EXIT_WARN: "WARN", EXIT_FAIL: "FAIL", EXIT_FATAL: "FATAL"}.get(level, "FAIL")
            col = "33" if level == EXIT_WARN else "1;31"
            print(f"  [{_c(sev_str, col)}] {name}: {msg_fail}")
            worst = max(worst, level)

    print()
    print("=" * 60)
    print(f"  Swarm Validation: {cfg.name}")
    print("=" * 60)
    print()

    for assertion in cfg.assertions:
        # Handle parameterized assertions like {frame_rate_above: 15}
        if isinstance(assertion, dict):
            assert_name = list(assertion.keys())[0]
            assert_param = assertion[assert_name]
        else:
            assert_name = str(assertion)
            assert_param = None

        if assert_name == "all_nodes_boot":
            booted = [
                nid for nid, log in logs.items()
                if any(kw in log for kw in ["app_main", "main_task", "ESP32-S3 CSI Node"])
            ]
            _check("all_nodes_boot",
                    len(booted) == n_nodes,
                    f"All {n_nodes} nodes booted",
                    f"Only {len(booted)}/{n_nodes} booted",
                    EXIT_FATAL if len(booted) == 0 else EXIT_FAIL)

        elif assert_name == "no_crashes":
            crash_pats = ["Guru Meditation", "assert failed", "abort()",
                          "panic", "LoadProhibited", "StoreProhibited"]
            crashed = [
                nid for nid, log in logs.items()
                if any(pat in log for pat in crash_pats)
            ]
            _check("no_crashes",
                    len(crashed) == 0,
                    "No crashes detected",
                    f"Crashes in nodes: {crashed}",
                    EXIT_FATAL)

        elif assert_name == "tdm_no_collision":
            slots: Dict[int, List[int]] = {}
            for nid, log in logs.items():
                m = re.search(r"TDM slot[=: ]+(\d+)", log, re.IGNORECASE)
                if m:
                    slot = int(m.group(1))
                    slots.setdefault(slot, []).append(nid)
            collisions = {s: ns for s, ns in slots.items() if len(ns) > 1}
            _check("tdm_no_collision",
                    len(collisions) == 0,
                    "No TDM slot collisions",
                    f"Collisions: {collisions}",
                    EXIT_FAIL)

        elif assert_name == "all_nodes_produce_frames":
            producing = []
            for nid, log in logs.items():
                node_cfg = next((n for n in cfg.nodes if n.node_id == nid), None)
                if node_cfg and node_cfg.role == "sensor":
                    if re.search(r"frame|CSI|emitted", log, re.IGNORECASE):
                        producing.append(nid)
            sensors = cfg.sensor_nodes()
            _check("all_nodes_produce_frames",
                    len(producing) == len(sensors),
                    f"All {len(sensors)} sensors producing frames",
                    f"Only {len(producing)}/{len(sensors)} sensors producing",
                    EXIT_FAIL)

        elif assert_name == "coordinator_receives_from_all":
            coord_logs = [
                logs.get(n.node_id, "") for n in cfg.coordinator_nodes()
            ]
            all_coord_text = "\n".join(coord_logs)
            received_from = set()
            for sensor in cfg.sensor_nodes():
                # Look for the sensor's node_id mentioned in coordinator logs
                if re.search(rf"node[_ ]?id[=: ]+{sensor.node_id}\b", all_coord_text, re.IGNORECASE):
                    received_from.add(sensor.node_id)
            sensor_ids = {s.node_id for s in cfg.sensor_nodes()}
            _check("coordinator_receives_from_all",
                    received_from == sensor_ids,
                    f"Coordinator received from all {len(sensor_ids)} sensors",
                    f"Missing: {sensor_ids - received_from}",
                    EXIT_FAIL)

        elif assert_name.startswith("fall_detected_by_node_"):
            target_id = int(assert_name.split("_")[-1])
            log_text = logs.get(target_id, "")
            found = bool(re.search(r"fall[_ ]?detect|fall[_ ]?event", log_text, re.IGNORECASE))
            _check(assert_name,
                    found,
                    f"Node {target_id} detected fall event",
                    f"Node {target_id} did not report fall detection",
                    EXIT_WARN)

        elif assert_name == "frame_rate_above":
            min_rate = int(assert_param) if assert_param else 10
            all_ok = True
            nodes_with_data = 0
            for nid, log in logs.items():
                m = re.search(r"frame[_ ]?rate[=: ]+([\d.]+)", log, re.IGNORECASE)
                if m:
                    nodes_with_data += 1
                    rate = float(m.group(1))
                    if rate < min_rate:
                        all_ok = False
            if nodes_with_data == 0:
                _check(f"frame_rate_above({min_rate})",
                        False,
                        "",
                        "No parseable frame rate data found in any node log",
                        EXIT_WARN)
            else:
                _check(f"frame_rate_above({min_rate})",
                        all_ok,
                        f"All nodes >= {min_rate} Hz",
                        f"Some nodes below {min_rate} Hz",
                        EXIT_WARN)

        elif assert_name == "max_boot_time_s":
            max_s = int(assert_param) if assert_param else 10
            all_ok = True
            nodes_with_data = 0
            for nid, log in logs.items():
                m = re.search(r"boot[_ ]?time[=: ]+([\d.]+)", log, re.IGNORECASE)
                if m:
                    nodes_with_data += 1
                    bt = float(m.group(1))
                    if bt > max_s:
                        all_ok = False
            if nodes_with_data == 0:
                _check(f"max_boot_time_s({max_s})",
                        False,
                        "",
                        "No parseable boot time data found in any node log",
                        EXIT_WARN)
            else:
                _check(f"max_boot_time_s({max_s})",
                        all_ok,
                        f"All nodes booted within {max_s}s",
                        f"Some nodes exceeded {max_s}s boot time",
                        EXIT_WARN)

        elif assert_name == "no_heap_errors":
            heap_pats = [
                r"HEAP_ERROR",
                r"heap_caps_alloc.*failed",
                r"out of memory",
                r"heap corruption",
                r"CORRUPT HEAP",
                r"malloc.*fail",
            ]
            found_in = [
                nid for nid, log in logs.items()
                if any(re.search(pat, log, re.IGNORECASE) for pat in heap_pats)
            ]
            _check("no_heap_errors",
                    len(found_in) == 0,
                    "No heap errors",
                    f"Heap errors in nodes: {found_in}",
                    EXIT_FAIL)

        else:
            warn(f"  Unknown assertion: {assert_name} (skipped)")

    print()
    verdict = {EXIT_PASS: "PASS", EXIT_WARN: "WARN", EXIT_FAIL: "FAIL", EXIT_FATAL: "FATAL"}
    print(f"  Verdict: {_c(verdict[worst], '32' if worst == 0 else '33' if worst == 1 else '1;31')}")
    print()

    return worst


# ---------------------------------------------------------------------------
# Orchestrator
# ---------------------------------------------------------------------------
class SwarmOrchestrator:
    """Manages the lifecycle of a QEMU swarm test."""

    def __init__(
        self,
        cfg: SwarmConfig,
        qemu_bin: str,
        output_dir: Path,
        skip_build: bool,
        dry_run: bool,
    ):
        self.cfg = cfg
        self.qemu_bin = qemu_bin
        self.output_dir = output_dir
        self.skip_build = skip_build
        self.dry_run = dry_run

        self.build_dir = FIRMWARE_DIR / "build"
        self.results_file = output_dir / "swarm_results.json"

        self.qemu_procs: List[subprocess.Popen] = []
        self.agg_proc: Optional[subprocess.Popen] = None
        self.net_state = NetworkState()

        # Register cleanup
        atexit.register(self.cleanup)
        signal.signal(signal.SIGTERM, self._signal_handler)
        signal.signal(signal.SIGINT, self._signal_handler)

    def _signal_handler(self, signum: int, frame: Any) -> None:
        info(f"Received signal {signum}, shutting down...")
        self.cleanup()
        sys.exit(EXIT_FATAL)

    def cleanup(self) -> None:
        """Kill all QEMU processes and tear down network."""
        for proc in self.qemu_procs:
            if proc.poll() is None:
                try:
                    proc.terminate()
                    proc.wait(timeout=5)
                except (subprocess.TimeoutExpired, OSError):
                    try:
                        proc.kill()
                    except OSError:
                        pass

        if self.agg_proc and self.agg_proc.poll() is None:
            try:
                self.agg_proc.terminate()
                self.agg_proc.wait(timeout=5)
            except (subprocess.TimeoutExpired, OSError):
                try:
                    self.agg_proc.kill()
                except OSError:
                    pass

        teardown_network(self.net_state)

    def run(self) -> int:
        """Execute the full swarm test. Returns exit code."""
        n = len(self.cfg.nodes)
        info(f"Swarm: {self.cfg.name}")
        info(f"Topology: {self.cfg.topology}")
        info(f"Nodes: {n}")
        info(f"Duration: {self.cfg.duration_s}s")
        info(f"Assertions: {len(self.cfg.assertions)}")
        info(f"Output: {self.output_dir}")
        print()

        if self.dry_run:
            return self._dry_run()

        # Ensure output dir exists
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.build_dir.mkdir(parents=True, exist_ok=True)

        # 1. Check prerequisites
        self._check_prerequisites()

        # 2. Provision each node
        info("--- Provisioning nodes ---")
        flash_images: Dict[int, Path] = {}
        aggregator_ip = "10.0.0.1"
        for node in self.cfg.nodes:
            flash_images[node.node_id] = provision_node(
                node=node,
                build_dir=self.build_dir,
                n_total=n,
                aggregator_ip=aggregator_ip,
                aggregator_port=self.cfg.aggregator_port,
            )
        print()

        # 3. Setup network topology
        info("--- Setting up network ---")
        node_net_args = setup_network(self.cfg, self.net_state)
        print()

        # 4. Start aggregator if needed
        if self.cfg.coordinator_nodes():
            info("--- Starting aggregator ---")
            agg_log = self.output_dir / "aggregator.log"
            self.agg_proc = start_aggregator(
                port=self.cfg.aggregator_port,
                n_nodes=n,
                output_file=self.results_file,
                log_file=agg_log,
            )
            if self.agg_proc:
                info(f"  Aggregator PID: {self.agg_proc.pid}")
            print()

        # 5. Launch QEMU instances
        info(f"--- Launching {n} QEMU nodes ---")
        for node in self.cfg.nodes:
            log_file = self.output_dir / f"qemu_node{node.node_id}.log"
            net_args = node_net_args.get(node.node_id, [])

            proc = launch_node(
                node=node,
                flash_image=flash_images[node.node_id],
                log_file=log_file,
                net_args=net_args,
                qemu_bin=self.qemu_bin,
            )
            self.qemu_procs.append(proc)
            info(f"  Node {node.node_id} ({node.role}): PID={proc.pid}, log={log_file}")
        print()

        # 6. Wait for test duration
        info(f"All nodes launched. Waiting {self.cfg.duration_s}s...")
        try:
            time.sleep(self.cfg.duration_s)
        except KeyboardInterrupt:
            warn("Interrupted by user.")

        # 7. Stop QEMU instances
        info("Duration elapsed. Stopping nodes...")
        for proc in self.qemu_procs:
            if proc.poll() is None:
                proc.terminate()
        # Give aggregator time to flush
        time.sleep(2)
        if self.agg_proc and self.agg_proc.poll() is None:
            self.agg_proc.terminate()
        print()

        # 8. Copy logs to output dir (they're already there via log_file paths)
        # Also copy from build_dir if assertions reference those paths
        for node in self.cfg.nodes:
            src = self.output_dir / f"qemu_node{node.node_id}.log"
            dst = self.build_dir / f"qemu_node{node.node_id}.log"
            if src.exists() and src != dst:
                shutil.copy2(str(src), str(dst))

        # 9. Run assertions
        exit_code = run_assertions(
            cfg=self.cfg,
            build_dir=self.output_dir,
            results_file=self.results_file,
        )

        # 10. Write JSON results summary
        self._write_summary(exit_code)

        return exit_code

    def _dry_run(self) -> int:
        """Show what would be launched without actually running anything."""
        print(_c("=== DRY RUN ===", "1;33"))
        print()
        print(f"Swarm: {self.cfg.name}")
        print(f"Topology: {self.cfg.topology}")
        print(f"Duration: {self.cfg.duration_s}s")
        print(f"Aggregator port: {self.cfg.aggregator_port}")
        print()

        print("Nodes:")
        for node in self.cfg.nodes:
            gw = " [GATEWAY]" if node.is_gateway else ""
            print(f"  node_id={node.node_id}  role={node.role}  scenario={node.scenario}  "
                  f"channel={node.channel}  tdm={node.tdm_slot}/{len(self.cfg.nodes)}  "
                  f"edge_tier={node.edge_tier}{gw}")
        print()

        print("Network:")
        if self.cfg.topology == "mesh":
            print("  Single bridge: all nodes on qemu-sw0")
        elif self.cfg.topology == "star":
            for i, s in enumerate(self.cfg.sensor_nodes()):
                print(f"  Bridge qemu-br{i}: sensor {s.node_id} <-> coordinator(s)")
        elif self.cfg.topology in ("line", "ring"):
            n = len(self.cfg.nodes)
            pairs = list(range(n - 1))
            if self.cfg.topology == "ring" and n > 2:
                pairs.append(n - 1)
            for p in range(len(pairs)):
                l = pairs[p]
                r = (pairs[p] + 1) % n
                print(f"  Bridge qemu-br{p}: node {self.cfg.nodes[l].node_id} "
                      f"<-> node {self.cfg.nodes[r].node_id}")
        print()

        print("QEMU command (per node):")
        print(f"  {self.qemu_bin} -machine esp32s3 -nographic "
              f"-drive file=<flash_image>,if=mtd,format=raw "
              f"-serial file:<log_file> -no-reboot <net_args>")
        print()

        print("Assertions:")
        for a in self.cfg.assertions:
            if isinstance(a, dict):
                name = list(a.keys())[0]
                param = a[name]
                print(f"  - {name}: {param}")
            else:
                print(f"  - {a}")
        print()

        return EXIT_PASS

    def _check_prerequisites(self) -> None:
        """Verify QEMU binary and build artifacts exist."""
        # Check QEMU binary
        try:
            result = subprocess.run(
                [self.qemu_bin, "--version"],
                capture_output=True, text=True, timeout=10,
            )
            if result.returncode != 0:
                fatal(f"QEMU binary returned error: {self.qemu_bin}")
                sys.exit(EXIT_FATAL)
        except FileNotFoundError:
            fatal(f"QEMU binary not found: {self.qemu_bin}")
            print("  Install: sudo apt install qemu-system-misc  # Debian/Ubuntu")
            print("  Or set --qemu-path to the qemu-system-xtensa binary.")
            sys.exit(EXIT_FATAL)
        except subprocess.TimeoutExpired:
            fatal(f"QEMU binary timed out: {self.qemu_bin}")
            sys.exit(EXIT_FATAL)

        # Check base flash image (accept either name)
        base = self.build_dir / "qemu_flash_base.bin"
        alt_base = self.build_dir / "qemu_flash.bin"
        if not base.exists() and not alt_base.exists():
            if self.skip_build:
                fatal(f"Base flash image not found: {base} or {alt_base}")
                fatal("Build the firmware first, or run without --skip-build.")
                sys.exit(EXIT_FATAL)
            else:
                warn("Base flash image not found; firmware build will create it.")

        # Check provision.py
        if not PROVISION_SCRIPT.exists():
            fatal(f"Provisioning script not found: {PROVISION_SCRIPT}")
            sys.exit(EXIT_FATAL)

    def _write_summary(self, exit_code: int) -> None:
        """Write JSON summary of the swarm test run."""
        verdict_map = {EXIT_PASS: "PASS", EXIT_WARN: "WARN",
                       EXIT_FAIL: "FAIL", EXIT_FATAL: "FATAL"}
        summary = {
            "swarm": self.cfg.name,
            "topology": self.cfg.topology,
            "node_count": len(self.cfg.nodes),
            "duration_s": self.cfg.duration_s,
            "verdict": verdict_map.get(exit_code, "UNKNOWN"),
            "exit_code": exit_code,
            "nodes": [
                {
                    "node_id": n.node_id,
                    "role": n.role,
                    "scenario": n.scenario,
                    "channel": n.channel,
                    "tdm_slot": n.tdm_slot,
                }
                for n in self.cfg.nodes
            ],
            "assertions": [
                str(a) if not isinstance(a, dict) else a
                for a in self.cfg.assertions
            ],
        }

        summary_path = self.output_dir / "swarm_summary.json"
        summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
        info(f"Summary written to {summary_path}")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="qemu_swarm.py",
        description="QEMU ESP32-S3 Swarm Configurator (ADR-062)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
Examples:
  python3 qemu_swarm.py --config swarm_presets/standard.yaml
  python3 qemu_swarm.py --preset smoke
  python3 qemu_swarm.py --preset standard --timeout 90
  python3 qemu_swarm.py --list-presets
  python3 qemu_swarm.py --config custom.yaml --dry-run

Exit codes:
  0  PASS  - all assertions passed
  1  WARN  - non-critical assertions failed
  2  FAIL  - critical assertions failed
  3  FATAL - infrastructure or build failure
""",
    )

    source = parser.add_mutually_exclusive_group()
    source.add_argument("--config", metavar="FILE",
                        help="Path to YAML swarm configuration file")
    source.add_argument("--preset", metavar="NAME",
                        help="Use a built-in preset (e.g. smoke, standard, large-mesh)")
    source.add_argument("--list-presets", action="store_true",
                        help="List available preset configurations and exit")

    parser.add_argument("--timeout", type=int, default=None,
                        help="Override swarm duration_s from config")
    parser.add_argument("--dry-run", action="store_true",
                        help="Show what would be launched without running")
    parser.add_argument("--qemu-path", default="qemu-system-xtensa",
                        help="Path to QEMU binary (default: qemu-system-xtensa)")
    parser.add_argument("--skip-build", action="store_true",
                        help="Skip firmware build step")
    parser.add_argument("--output-dir", metavar="DIR", default=None,
                        help="Directory for logs and results (default: build/swarm_<name>)")

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    # List presets
    if args.list_presets:
        presets = list_presets()
        if not presets:
            print(f"No presets found in {PRESETS_DIR}")
            return EXIT_PASS
        print("Available swarm presets:")
        print()
        for name, desc in presets:
            print(f"  {name:20s}  {desc}")
        print()
        print(f"Use: python3 qemu_swarm.py --preset <name>")
        return EXIT_PASS

    # Load config
    if args.config:
        config_path = Path(args.config)
        if not config_path.exists():
            fatal(f"Config file not found: {config_path}")
            return EXIT_FATAL
        raw = yaml.safe_load(config_path.read_text(encoding="utf-8"))
    elif args.preset:
        raw = load_preset(args.preset)
    else:
        parser.print_help()
        print()
        error("Provide --config FILE or --preset NAME (or use --list-presets)")
        return EXIT_FATAL

    cfg = validate_config(raw)

    # Apply overrides
    if args.timeout is not None:
        cfg.duration_s = args.timeout

    # Determine output directory
    if args.output_dir:
        output_dir = Path(args.output_dir)
    else:
        output_dir = FIRMWARE_DIR / "build" / f"swarm_{cfg.name.replace(' ', '_')}"

    # Run orchestrator
    orch = SwarmOrchestrator(
        cfg=cfg,
        qemu_bin=args.qemu_path,
        output_dir=output_dir,
        skip_build=args.skip_build,
        dry_run=args.dry_run,
    )

    return orch.run()


if __name__ == "__main__":
    sys.exit(main())

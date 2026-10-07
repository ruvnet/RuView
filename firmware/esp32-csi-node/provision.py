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
    pip install 'esptool>=5.0' esp-idf-nvs-partition-gen
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

    State is keyed by the board's MAC, read with esptool, so a board plugged
    into a port another board used doesn't inherit its node_id (#1755).
    Port-keyed files from earlier versions move to the board's record the
    first time that board is provisioned.

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
import getpass
import io
import json
import os
import stat
import re
import struct
import subprocess
import sys
import tempfile


# NVS partition table offset — default for ESP-IDF 4MB flash with standard
# partition scheme.  The "nvs" partition starts at 0x9000 (36864) and is
# 0x6000 (24576) bytes.
NVS_PARTITION_OFFSET = 0x9000
NVS_PARTITION_SIZE = 0x6000  # 24 KiB
NVS_U8_MAX = 0xFF
NVS_U16_MAX = 0xFFFF
NVS_U32_MAX = 0xFFFFFFFF
NVS_HOP_CHANNELS_MAX = 6


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


NVS_INT_RANGE_CHECKS = [
    ("target_port", "--target-port", 1, NVS_U16_MAX),
    ("node_id", "--node-id", 0, NVS_U8_MAX),
    ("tdm_slot", "--tdm-slot", 0, NVS_U8_MAX),
    ("tdm_total", "--tdm-total", 1, NVS_U8_MAX),
    ("pres_thresh", "--pres-thresh", 0, NVS_U16_MAX),
    ("fall_thresh", "--fall-thresh", 0, NVS_U16_MAX),
    ("vital_win", "--vital-win", 32, 256),
    ("vital_int", "--vital-int", 100, NVS_U16_MAX),
    ("subk_count", "--subk-count", 1, 32),
    ("swarm_hb", "--swarm-hb", 1, NVS_U16_MAX),
    ("swarm_ingest", "--swarm-ingest", 1, NVS_U16_MAX),
]


def has_config_value(args):
    """Return True when args include at least one NVS-writing config value."""
    return any(
        check(getattr(args, name, None))
        for name, check in CONFIG_VALUE_CHECKS
    )


def is_valid_wifi_channel(channel):
    """Return True for 2.4 GHz or 5 GHz WiFi channel numbers accepted by firmware."""
    return (1 <= channel <= 14) or (36 <= channel <= 177)


def parse_hop_channels(value):
    """Parse a comma-separated hop channel list into integers."""
    return [int(channel.strip()) for channel in value.split(",")]


def validate_int_range(args, parser, attr, flag, minimum, maximum):
    value = getattr(args, attr, None)
    if value is None:
        return
    if value < minimum or value > maximum:
        parser.error(f"{flag} must be in range {minimum}-{maximum}, got {value}")


def validate_config_ranges(args, parser):
    """Fail fast before generating an NVS image with values firmware cannot load."""
    for attr, flag, minimum, maximum in NVS_INT_RANGE_CHECKS:
        validate_int_range(args, parser, attr, flag, minimum, maximum)

    if args.channel is not None and not is_valid_wifi_channel(args.channel):
        parser.error(f"--channel must be 1-14 (2.4GHz) or 36-177 (5GHz), got {args.channel}")

    if args.hop_channels is None:
        return

    try:
        channels = parse_hop_channels(args.hop_channels)
    except ValueError:
        parser.error(f"--hop-channels must be comma-separated integers, got '{args.hop_channels}'")

    if not channels:
        parser.error("--hop-channels must include at least one channel")
    if len(channels) > NVS_HOP_CHANNELS_MAX:
        parser.error(
            f"--hop-channels supports at most {NVS_HOP_CHANNELS_MAX} channels, got {len(channels)}"
        )
    for channel in channels:
        if not is_valid_wifi_channel(channel):
            parser.error(
                f"--hop-channels entries must be 1-14 (2.4GHz) or 36-177 (5GHz), got {channel}"
            )
    validate_int_range(args, parser, "hop_dwell", "--hop-dwell", 10, NVS_U32_MAX)


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


# State files hold the WiFi password and seed token in cleartext (#1754), so the
# directory is owner-only and every file in it is 0600.
STATE_DIR_MODE = 0o700
STATE_FILE_MODE = 0o600

# Values `--state` hides unless `--show-secrets` is passed (#1754).
SECRET_ATTRS = ("password", "seed_token")


def _restrict_mode(path: str, mode: int) -> None:
    """chmod `path` to `mode` if it is a real file or dir owned by this user.

    Never raises: a read-only or unusual filesystem must not block provisioning,
    but a secret left readable by others is reported. Symlinks are not followed,
    so a planted link can't redirect the chmod.
    """
    try:
        st = os.lstat(path)
    except FileNotFoundError:
        return
    except OSError as exc:
        print(f"WARNING: could not stat {path}: {exc}", file=sys.stderr)
        return
    if stat.S_ISLNK(st.st_mode):
        print(f"WARNING: not changing permissions through symlink {path}", file=sys.stderr)
        return
    if hasattr(os, "getuid") and st.st_uid != os.getuid():
        print(f"WARNING: {path} is not owned by you; permissions left unchanged",
              file=sys.stderr)
        return
    if stat.S_IMODE(st.st_mode) == mode:
        return
    try:
        os.chmod(path, mode)
    except OSError as exc:
        print(f"WARNING: could not set {oct(mode)} on {path}: {exc}", file=sys.stderr)


def harden_state_dir(state_dir: str) -> None:
    """Make the state dir 0700 and its state/temp files 0600.

    Covers files written by earlier versions of this script, which used the
    default umask (0755 dir, 0644 files). POSIX only: chmod can't set
    owner-only ACLs on Windows.
    """
    if sys.platform == "win32" or not os.path.isdir(state_dir):
        return
    _restrict_mode(state_dir, STATE_DIR_MODE)
    try:
        names = os.listdir(state_dir)
    except OSError:
        return
    for name in names:
        if name.endswith((".json", ".tmp")):
            _restrict_mode(os.path.join(state_dir, name), STATE_FILE_MODE)


def _write_private(path: str, data: bytes) -> None:
    """Write a credential-bearing file (state, NVS CSV or binary) as 0600."""
    flags = (os.O_WRONLY | os.O_CREAT | os.O_TRUNC
             | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_BINARY", 0))
    try:
        fd = os.open(path, flags, STATE_FILE_MODE)
    except OSError as exc:
        # O_NOFOLLOW makes a pre-existing symlink fail here (ELOOP).
        if os.path.islink(path):
            raise SystemExit(
                f"ERROR: refusing to write credentials through a symlink: {path}. "
                f"Remove it and rerun.") from exc
        raise
    with os.fdopen(fd, "wb") as f:
        # O_CREAT ignores the mode when the file already exists.
        if hasattr(os, "fchmod"):
            os.fchmod(f.fileno(), STATE_FILE_MODE)
        f.write(data)


def redact_secrets(state: dict) -> dict:
    """Copy of `state` with secret values replaced by a fixed marker."""
    shown = dict(state)
    for name in SECRET_ATTRS:
        if shown.get(name) is not None:
            shown[name] = "(set)" if shown[name] else "(empty)"
    return shown


def read_password_file(path: str, allow_insecure: bool = False) -> str:
    """Read the WiFi password from `path`, dropping one trailing newline.

    Keeps the password out of argv and shell history (#1754). Refuses a file
    that group or others can read unless `allow_insecure` is set. Raises
    ValueError with a message meant for the user.
    """
    try:
        with open(path, "rb") as f:
            mode = stat.S_IMODE(os.fstat(f.fileno()).st_mode)
            data = f.read()
    except FileNotFoundError:
        raise ValueError(f"--password-file {path} does not exist") from None
    except OSError as exc:
        raise ValueError(f"could not read --password-file {path}: {exc}") from None
    if sys.platform != "win32" and mode & 0o044:
        if not allow_insecure:
            raise ValueError(
                f"--password-file {path} is readable by group or others "
                f"(mode {oct(mode)}). Run 'chmod 600 {path}', or pass "
                f"--allow-insecure-password-file."
            )
        print(f"WARNING: --password-file {path} is readable by group or others "
              f"(mode {oct(mode)}).", file=sys.stderr)
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        raise ValueError(f"--password-file {path} is not UTF-8 text") from None
    if text.endswith("\r\n"):
        text = text[:-2]
    elif text.endswith("\n"):
        text = text[:-1]
    if not text:
        raise ValueError(f"--password-file {path} is empty")
    return text


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
    """Write `state` to the per-port file, creating dirs as needed. Returns path.

    The file holds secrets, so it is written 0600 inside a 0700 dir (#1754).
    """
    os.makedirs(state_dir, mode=STATE_DIR_MODE, exist_ok=True)
    harden_state_dir(state_dir)
    path = _state_path_for(port, state_dir)
    # mkstemp picks a unique name and opens it O_EXCL with mode 0600, so a
    # stale temp file or a planted symlink can't receive the secret.
    fd, tmp = tempfile.mkstemp(
        dir=state_dir, prefix=os.path.basename(path) + ".", suffix=".tmp"
    )
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            if hasattr(os, "fchmod"):
                os.fchmod(f.fileno(), STATE_FILE_MODE)
            # Sort keys for deterministic on-disk content (easier to diff).
            json.dump(state, f, indent=2, sort_keys=True)
            f.write("\n")
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise
    return path


# ---------------------------------------------------------------------------
# Board identity (#1755)
# ---------------------------------------------------------------------------
#
# Serial port names get reused when boards are swapped, so a port-keyed state
# file hands one board's node_id to the next. State is keyed by the chip's
# base MAC instead. Each record also stores the MAC and the port it was last
# written from, so --state can find it without opening the port.

STATE_MAC_KEY = "_chip_mac"
STATE_PORT_KEY = "_port"

_ESPTOOL_MAC_RE = re.compile(
    r"^\s*(BASE MAC|MAC):\s*((?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2})\s*$", re.MULTILINE
)


def normalize_mac(value: str):
    """Return `value` as lowercase aa:bb:cc:dd:ee:ff, or None if it isn't a MAC."""
    parts = re.split(r"[:-]", value.strip())
    if len(parts) != 6 or not all(re.fullmatch(r"[0-9a-fA-F]{2}", p) for p in parts):
        return None
    return ":".join(p.lower() for p in parts)


def parse_esptool_mac(output: str):
    """Return the base MAC from `esptool read_mac` output, or None.

    Most chips print one 6-byte "MAC:" line. EUI-64 chips (C5, C6, H2) print an
    8-byte "MAC:" line, then "BASE MAC:" with the 6-byte address.
    """
    found = {label: mac.lower() for label, mac in _ESPTOOL_MAC_RE.findall(output)}
    return found.get("BASE MAC") or found.get("MAC")


def read_chip_mac(port: str, baud: int, chip: str):
    """Read the connected board's base MAC with esptool. None if it can't."""
    cmd = [
        sys.executable, "-m", "esptool",
        "--chip", chip,
        "--port", port,
        "--baud", str(baud),
        "read_mac",
    ]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return None
    if result.returncode != 0:
        return None
    return parse_esptool_mac(result.stdout)


def mac_state_key(mac: str) -> str:
    """State-file key for a board, e.g. mac-aabbccddeeff."""
    return "mac-" + mac.replace(":", "")


def boards_last_on_port(port: str, state_dir: str) -> list:
    """MACs of boards whose state was last written from `port`, newest first."""
    if not os.path.isdir(state_dir):
        return []
    found = []
    for name in os.listdir(state_dir):
        if not (name.startswith("mac-") and name.endswith(".json")):
            continue
        data = load_state(name[:-len(".json")], state_dir)
        mac = data.get(STATE_MAC_KEY)
        if data.get(STATE_PORT_KEY) == port and mac:
            found.append((os.path.getmtime(os.path.join(state_dir, name)), mac))
    return [mac for _, mac in sorted(found, reverse=True)]


def load_board_state(port: str, mac, state_dir: str):
    """Return (prior state, legacy path) for the board `mac` on `port`.

    Without a MAC this is the old port-keyed lookup. With one, the board's own
    record wins. If it has none but a port-keyed file from an earlier version
    exists, that file is returned for migration along with its path.
    """
    if mac is None:
        return load_state(port, state_dir), None
    prior = load_state(mac_state_key(mac), state_dir)
    if prior:
        return prior, None
    legacy = _state_path_for(port, state_dir)
    if os.path.isfile(legacy):
        return load_state(port, state_dir), legacy
    return {}, None


def save_board_state(port: str, mac, state_dir: str, state: dict, legacy=None) -> str:
    """Persist `state` under the board's MAC (or the port if the MAC is unknown).

    A migrated port-keyed file is removed afterwards, so no other board on that
    port can pick it up.
    """
    if mac is None:
        return save_state(port, state_dir, state)
    state = dict(state)
    state[STATE_MAC_KEY] = mac
    state[STATE_PORT_KEY] = port
    path = save_state(mac_state_key(mac), state_dir, state)
    if legacy and os.path.isfile(legacy):
        os.unlink(legacy)
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
        channels = parse_hop_channels(args.hop_channels)
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
    # Both files carry the WiFi password. A private 0700 dir keeps the
    # generator's output (written with the default umask) away from other users.
    work_dir = tempfile.mkdtemp(prefix="provision-nvs-")
    csv_path = os.path.join(work_dir, "nvs.csv")
    bin_path = os.path.join(work_dir, "nvs.bin")
    _write_private(csv_path, csv_content.encode("utf-8"))

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
            # Fixed interpreter/script plus an argv list (never a shell);
            # csv_path/bin_path are private NamedTemporaryFile paths.
            subprocess.check_call([  # nosemgrep: dangerous-subprocess-use-tainted-env-args
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
        try:
            os.rmdir(work_dir)
        except OSError:
            pass


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
            "write_flash",
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
    password_source = parser.add_mutually_exclusive_group()
    password_source.add_argument("--password",
                                 help="WiFi password. Visible in ps and shell history; "
                                 "prefer --password-file.")
    password_source.add_argument("--password-file", metavar="PATH",
                                 help="Read the WiFi password from PATH (one trailing newline "
                                 "is dropped). The file must not be readable by group or others.")
    parser.add_argument("--allow-insecure-password-file", action="store_true",
                        help="Accept a --password-file that group or others can read, with a "
                        "warning (e.g. a read-only 0444 secrets mount).")
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
                        help="Ignore this machine's per-port state file when merging; it is "
                        "replaced after a successful flash. "
                        "Use for first-time provisioning of a recycled board where "
                        "previously-staged keys should NOT be re-applied.")
    parser.add_argument("--state-dir", default=_default_state_dir(),
                        help="Override the per-user state directory (default: per-OS user config dir).")
    parser.add_argument("--mac", type=str,
                        help="Board MAC (AA:BB:CC:DD:EE:FF) whose state to use. Normally read "
                        "from the chip with esptool; --state and --dry-run never open the "
                        "port, so pass it there to pick a board.")
    parser.add_argument("--state", action="store_true",
                        help="Print the merged state that WOULD be flashed for this port and exit. "
                        "Useful for debugging which keys are about to land on the device. "
                        "The WiFi password and seed token are shown as (set)/(empty).")
    parser.add_argument("--show-secrets", action="store_true",
                        help="With --state, print the WiFi password and seed token in clear.")

    args = parser.parse_args()

    # State written by older versions may be 0644. Tighten it before any read
    # or early exit (#1754).
    harden_state_dir(args.state_dir)

    if args.password_file is not None:
        try:
            args.password = read_password_file(
                args.password_file, args.allow_insecure_password_file)
        except ValueError as exc:
            parser.error(str(exc))
    elif args.password is not None:
        print("WARNING: --password is visible in ps and shell history; "
              "use --password-file instead.", file=sys.stderr)
    cli_ssid, cli_password = args.ssid, args.password
    # --- Board identity (#1755) ---
    # State is keyed by the chip's MAC so a board swapped onto a reused port
    # doesn't inherit the previous board's node_id.
    if args.mac is not None:
        chip_mac = normalize_mac(args.mac)
        if chip_mac is None:
            parser.error(f"--mac must be in AA:BB:CC:DD:EE:FF format, got '{args.mac}'")
    elif args.state:
        # Inspection only: don't open the port; show the board last
        # provisioned from it.
        recent = boards_last_on_port(args.port, args.state_dir)
        chip_mac = recent[0] if recent else None
        if chip_mac:
            print(f"Showing board {chip_mac}, last provisioned on {args.port}. "
                  f"Pass --mac to pick another.", file=sys.stderr)
    elif args.dry_run:
        # No board involved; keep the port-keyed state as before.
        chip_mac = None
    else:
        chip_mac = read_chip_mac(args.port, args.baud, args.chip)
        if chip_mac:
            print(f"Board MAC: {chip_mac}")
        else:
            print(f"WARNING: could not read the board's MAC with esptool, so state "
                  f"stays keyed by port {args.port}. Pass --mac to key it by board.",
                  file=sys.stderr)
    legacy = None

    # --- Per-port state load + merge (additive-by-default, #391 / #574) ---
    if args.reset:
        # Don't delete the state file here: validation below can still fail,
        # and a failed run must not lose the record. A successful flash
        # overwrites it with the reset (CLI-only) state.
        path = _state_path_for(args.port, args.state_dir)
        if os.path.isfile(path):
            print(f"--reset: ignoring state file {path}", file=sys.stderr)
        prior = {}
    else:
        prior, legacy = load_board_state(args.port, chip_mac, args.state_dir)
        if legacy:
            print(f"Using port-keyed state {legacy} for board {chip_mac}; it moves to "
                  f"the board's own record when state is next saved. If this isn't the "
                  f"board last provisioned on {args.port}, rerun with --reset.",
                  file=sys.stderr)
    merged = merge_state_into_args(args, prior)

    # A new --ssid with no password given: ask for it on a terminal instead of
    # requiring it on the command line. The saved password is reused only when
    # it belongs to the same SSID. Without a terminal (scripts, CI) nothing is
    # asked and the WiFi-credential check below applies as before.
    if (not args.state and cli_ssid is not None and cli_password is None
            and (prior.get("password") is None or prior.get("ssid") != cli_ssid)
            and sys.stdin.isatty()):
        args.password = getpass.getpass(f"WiFi password for {cli_ssid}: ")
        merged["password"] = args.password

    if args.state:
        shown = merged if args.show_secrets else redact_secrets(merged)
        print(json.dumps(shown, indent=2, sort_keys=True))
        if shown != merged:
            print("Secrets hidden; pass --show-secrets to print them.", file=sys.stderr)
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
        state_key = mac_state_key(chip_mac) if chip_mac else args.port
        parser.error(
            f"Missing required WiFi credentials after merging prior state: "
            f"{', '.join(wifi_trio_missing)}.\n"
            f"\n"
            f"  No saved state at {_state_path_for(state_key, args.state_dir)}\n"
            f"  and the CLI didn't include them. Either pass --ssid + --password + --target-ip\n"
            f"  on this run, or add --force-partial to flash without WiFi.\n"
        )
    if args.force_partial and wifi_trio_missing:
        print(
            "WARNING: --force-partial is set and WiFi credentials are missing. "
            "The device will not connect to WiFi after flashing.",
            file=sys.stderr,
        )

    validate_config_ranges(args, parser)

    # Validate TDM: if one is given, both should be
    if (args.tdm_slot is not None) != (args.tdm_total is not None):
        parser.error("--tdm-slot and --tdm-total must be specified together")
    if args.tdm_slot is not None and args.tdm_slot >= args.tdm_total:
        parser.error(f"--tdm-slot ({args.tdm_slot}) must be less than --tdm-total ({args.tdm_total})")

    # ADR-060: Validate MAC filter
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
        _write_private(fallback_path, csv_content.encode("utf-8"))
        print(f"Saved NVS CSV to {fallback_path}", file=sys.stderr)
        print(f"Flash with: python $IDF_PATH/components/nvs_flash/"
              f"nvs_partition_generator/nvs_partition_gen.py generate "
              f"{fallback_path} nvs.bin 0x6000", file=sys.stderr)
        sys.exit(1)

    if args.dry_run:
        out = "nvs_provision.bin"
        _write_private(out, nvs_bin)
        print(f"NVS binary saved to {out} ({len(nvs_bin)} bytes)")
        print(f"Flash manually: python -m esptool --chip {args.chip} --port {args.port} "
              f"write_flash 0x9000 {out}")
        # Don't persist state on dry-run: nothing reached the device, so the
        # next real run must not merge on top of values that were never flashed.
        print("Dry run: state file not updated.")
        return

    flash_nvs(args.port, args.baud, nvs_bin, args.chip)
    # Persist merged state after a successful flash so future partial
    # invocations from this machine merge on top of what's actually on the
    # device. This is the heart of the additive-by-default fix (#391/#574).
    path = save_board_state(args.port, chip_mac, args.state_dir, merged, legacy)
    print(f"State persisted to {path}")


if __name__ == "__main__":
    main()

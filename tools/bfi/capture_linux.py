#!/usr/bin/env python3
"""Explicit, bounded own-AP capture with Ethernet preflight and Wi-Fi restoration.

Run as the normal user with passwordless sudo for iw/ip/nmcli/tcpdump. Raw
packets stay in a new private directory outside the source checkout. This tool
does not inject frames, install drivers, or configure an access point.
"""
import argparse
import datetime
try:
    import fcntl
except ImportError:  # Permit mocked tests on non-Linux development hosts.
    fcntl = None
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import signal
import subprocess
import tempfile
import time
import uuid as uuid_module


def command(args, privileged=False, timeout=30, check=True):
    result = subprocess.run((["sudo", "-n"] if privileged else []) + args,
                            capture_output=True, text=True, timeout=timeout)
    if check and result.returncode:
        raise RuntimeError(f"{shlex.join(args)} failed ({result.returncode}): {result.stderr.strip()}")
    return result


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def evidence_in_checkout(output):
    """Recognize real checkouts, including worktrees whose .git is a file.

    The script can be copied outside the repository for a hardware trial;
    its installation depth must never turn /var into a presumed checkout.
    """
    output = Path(output).resolve()
    return any((parent / ".git").exists() for parent in (output, *output.parents))


def release_wireless(iface):
    """Wait for observed NM and association teardown before changing type."""
    command(["nmcli", "--wait", "10", "device", "disconnect", iface], True, timeout=15)
    command(["nmcli", "device", "set", iface, "managed", "no"], True)
    deadline = time.monotonic() + 10
    state = link = ""
    while time.monotonic() < deadline:
        state = command(["nmcli", "-g", "GENERAL.STATE", "device", "show", iface], timeout=3).stdout.strip()
        link = command(["iw", "dev", iface, "link"], timeout=3).stdout.strip()
        if state.split(maxsplit=1)[:1] == ["10"] and link == "Not connected.":
            return {"nm_state": state, "link": link}
        time.sleep(0.25)
    raise RuntimeError(f"Wireless release did not verify: NM={state!r}, link={link!r}")


def restore(iface, uuid):
    # Stop NM first: it must not race the initial type change.
    command(["nmcli", "device", "set", iface, "managed", "no"], True)
    command(["ip", "link", "set", iface, "down"], True)
    command(["iw", "dev", iface, "set", "type", "managed"], True)
    command(["nmcli", "device", "set", iface, "managed", "yes"], True)
    command(["ip", "link", "set", iface, "up"], True)
    info = command(["iw", "dev", iface, "info"]).stdout
    if "type managed" not in info:
        # Observed on MT7927: the ownership handoff can restore the old
        # monitor type. Correct that specific state after NM owns the device.
        command(["ip", "link", "set", iface, "down"], True)
        command(["iw", "dev", iface, "set", "type", "managed"], True)
        command(["ip", "link", "set", iface, "up"], True)
    for _ in range(20):
        state = command(["nmcli", "-g", "GENERAL.STATE", "device", "show", iface]).stdout
        if not state.startswith(("10 ", "20 ")):
            break
        time.sleep(0.5)
    command(["nmcli", "--wait", "30", "connection", "up", "uuid", uuid,
             "ifname", iface], True, timeout=35)
    info = command(["iw", "dev", iface, "info"]).stdout
    link = command(["iw", "dev", iface, "link"]).stdout
    actual_uuid = command(["nmcli", "-g", "GENERAL.CON-UUID", "device", "show", iface]).stdout.strip()
    if "type managed" not in info or "Connected to " not in link or actual_uuid != uuid:
        raise RuntimeError("Wi-Fi restoration did not verify")
    return {"interface": info, "link": link}


def capture(cmd, packet_file, seconds, active_path=None):
    """Reap the complete privileged process group before radio restoration."""
    proc = subprocess.Popen(cmd, stdout=packet_file, stderr=subprocess.PIPE,
                            start_new_session=True)
    marker_created = False
    try:
        if active_path is not None:
            with active_path.open("x"):
                pass
            marker_created = True
        _, stderr = proc.communicate(timeout=seconds + 10)
        return subprocess.CompletedProcess(cmd, proc.returncode, stderr=stderr)
    except BaseException:
        # This group belongs only to this invocation. sudo/timeout/tcpdump
        # may run as root, so signaling the unprivileged wrapper is insufficient.
        command(["kill", "-INT", "--", f"-{proc.pid}"], True, check=False)
        try:
            proc.communicate(timeout=7)
        except subprocess.TimeoutExpired:
            command(["kill", "-KILL", "--", f"-{proc.pid}"], True, check=False)
            proc.communicate(timeout=5)
        raise
    finally:
        if marker_created:
            active_path.unlink(missing_ok=True)


def verify_monitor(info, frequency, width, center):
    if "type monitor" not in info or not re.search(rf"\({frequency} MHz\)", info):
        raise RuntimeError("Monitor interface type/frequency did not verify")
    if not re.search(rf"width: {width} MHz\b", info):
        raise RuntimeError("Monitor channel width did not verify")
    if width == 80 and not re.search(rf"center1: {center} MHz\b", info):
        raise RuntimeError("Monitor center frequency did not verify")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--interface", required=True)
    p.add_argument("--ethernet", required=True)
    p.add_argument("--management-ip", required=True)
    p.add_argument("--bssid", required=True)
    p.add_argument("--frequency", type=int, required=True)
    p.add_argument("--width", type=int, choices=(20, 80), default=80)
    p.add_argument("--center", type=int)
    p.add_argument("--seconds", type=int, default=30)
    p.add_argument("--packets", type=int, default=2000)
    frame_scope = p.add_mutually_exclusive_group()
    frame_scope.add_argument("--beacons-only", action="store_true")
    frame_scope.add_argument("--include-link-traffic", action="store_true",
                             help="Diagnostic: also retain own-AP control/data frames")
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--confirm-capture", action="store_true")
    a = p.parse_args()
    if fcntl is None:
        p.error("capture requires Linux")
    import ipaddress
    ipaddress.ip_address(a.management_ip)
    for name in (a.interface, a.ethernet):
        if not re.fullmatch(r"[A-Za-z0-9_.-]{1,15}", name):
            p.error("invalid interface name")
    if not a.confirm_capture:
        p.error("--confirm-capture is required for the radio change")
    if not re.fullmatch(r"(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}", a.bssid):
        p.error("invalid BSSID")
    # 2000 full 65535-byte records plus headers fit the decoder's 128 MiB limit.
    if not 1 <= a.seconds <= 120 or not 1 <= a.packets <= 2000:
        p.error("capture must be bounded to 1..120 seconds and 1..2000 packets")
    if not 2400 <= a.frequency <= 7125 or (a.width == 80 and not a.center):
        p.error("invalid frequency or missing 80 MHz center frequency")
    if a.center is not None and not 2400 <= a.center <= 7125:
        p.error("invalid center frequency")
    if a.width == 80 and abs(a.frequency - a.center) not in (10, 30):
        p.error("80 MHz primary/center frequencies must differ by 10 or 30 MHz")
    out = a.output.expanduser().resolve()
    if evidence_in_checkout(out):
        p.error("raw evidence must be outside the source checkout")
    os.umask(0o077)
    out.mkdir(mode=0o700, parents=False, exist_ok=False)
    # Per-user lock spans preflight, mutation, capture and restoration.
    lock_path = Path(tempfile.gettempdir()) / f"ruview-bfi-radio-{os.getuid()}.lock"
    fd = os.open(lock_path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        before = command(["iw", "dev", a.interface, "info"]).stdout
        if "type managed" not in before:
            raise RuntimeError("Expected a managed interface before capture")
        uuid = command(["nmcli", "-g", "GENERAL.CON-UUID", "device", "show", a.interface]).stdout.strip()
        try:
            uuid_module.UUID(uuid)
        except ValueError:
            raise RuntimeError("No active NetworkManager connection to restore")
        ethernet_type = command(["nmcli", "-g", "GENERAL.TYPE", "device", "show", a.ethernet]).stdout.strip()
        if ethernet_type != "ethernet":
            raise RuntimeError("Management interface is not Ethernet")
        route = command(["ip", "route", "get", a.management_ip]).stdout
        if f"dev {a.ethernet} " not in route or a.ethernet == a.interface:
            raise RuntimeError("Management route is not using the specified Ethernet interface")
        bssid = a.bssid.lower()
        addresses = f"(wlan addr1 {bssid} or wlan addr2 {bssid} or wlan addr3 {bssid})"
        filt = ("type mgt subtype beacon and wlan addr2 " + bssid if a.beacons_only else
                addresses if a.include_link_traffic else "type mgt and " + addresses)
        cmd = ["sudo", "-n", "timeout", "--signal=INT", "--kill-after=5s", str(a.seconds)+"s",
               "tcpdump", "-i", a.interface, "-y", "IEEE802_11_RADIO", "-s", "65535",
               "-c", str(a.packets), "-U", "-w", "-", filt]
        manifest = {"schema": 1, "before": before, "original_connection_uuid": uuid,
                    "route": route, "command": cmd,
                    "requested_seconds": a.seconds, "started_at": now()}
        changed = False
        error = None
        def interrupted(signum, frame):
            raise RuntimeError(f"capture interrupted by signal {signum}")
        old_handlers = {sig: signal.signal(sig, interrupted)
                        for sig in (signal.SIGTERM, signal.SIGHUP, signal.SIGINT)}
        try:
            changed = True
            manifest["released"] = release_wireless(a.interface)
            command(["ip", "link", "set", a.interface, "down"], True)
            command(["iw", "dev", a.interface, "set", "type", "monitor"], True)
            monitor_flags = ["otherbss", "control"] if a.include_link_traffic else ["otherbss"]
            command(["iw", "dev", a.interface, "set", "monitor"] + monitor_flags, True)
            command(["ip", "link", "set", a.interface, "up"], True)
            frequency = [str(a.frequency), "HT20"] if a.width == 20 else [str(a.frequency), "80", str(a.center)]
            command(["iw", "dev", a.interface, "set", "freq"] + frequency, True)
            manifest["monitor_before"] = command(["iw", "dev", a.interface, "info"]).stdout
            verify_monitor(manifest["monitor_before"], a.frequency, a.width, a.center)
            started = time.monotonic()
            manifest["capture_started_at"] = now()
            with (out / "capture.pcap").open("xb") as packet_file:
                result = capture(cmd, packet_file, a.seconds, out / "capture.active")
            manifest["capture_seconds"] = time.monotonic() - started
            manifest["capture_ended_at"] = now()
            manifest["capture_exit"] = result.returncode
            manifest["capture_log"] = result.stderr.decode(errors="replace")
            manifest["monitor_after"] = command(["iw", "dev", a.interface, "info"]).stdout
            verify_monitor(manifest["monitor_after"], a.frequency, a.width, a.center)
            with (out / "capture.pcap").open("rb") as evidence:
                manifest["sha256"] = hashlib.file_digest(evidence, "sha256").hexdigest()
            if result.returncode not in (0, 124):
                raise RuntimeError("tcpdump failed; see capture_log")
        except BaseException as exc:
            error = exc
            manifest["error"] = str(exc)
        finally:
            # A second terminal signal must not interrupt the restoration.
            for sig in old_handlers:
                signal.signal(sig, signal.SIG_IGN)
            if changed:
                try:
                    manifest["restored"] = restore(a.interface, uuid)
                    final_route = command(["ip", "route", "get", a.management_ip]).stdout
                    manifest["route_after"] = final_route
                    if f"dev {a.ethernet} " not in final_route:
                        raise RuntimeError("Ethernet management route changed")
                except Exception as exc:
                    manifest["restore_error"] = str(exc)
                    error = error or exc
            manifest["ended_at"] = now()
            (out / "manifest.json").write_text(json.dumps(manifest, indent=2)+"\n")
            for sig, handler in old_handlers.items():
                signal.signal(sig, handler)
        print(json.dumps({"output": str(out), "capture_log": manifest.get("capture_log"),
                          "restored": "restored" in manifest, "error": str(error) if error else None}))
        return 1 if error else 0


if __name__ == "__main__":
    raise SystemExit(main())

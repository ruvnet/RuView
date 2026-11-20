#!/usr/bin/env python3
"""Run an explicit own-link capture with bounded, synchronized UDP downlink.

Default profile: 45-second capture; at most 30 seconds of 250 packets/second,
1200-byte UDP payloads to port 9. Bounded rates/durations are configurable.
No listener, scanning, driver installation,
or network configuration is performed here. capture_linux.py owns restoration.
"""
import argparse
import ipaddress
import json
import os
from pathlib import Path
import re
import signal
import socket
import stat
import struct
import subprocess
import sys
import time

CAPTURE_SECONDS = 45
TRAFFIC_SECONDS = 30
PACKETS_PER_SECOND = 250
PAYLOAD_BYTES = 1200
DESTINATION_PORT = 9
MAX_DATAGRAMS = 60000
MAX_PAYLOAD_BYTES = 64 * 1024 * 1024
CAPTURE_RESERVE_SECONDS = 5


def validate_profile(pps, traffic_seconds, capture_seconds):
    if not all(type(value) is int for value in (pps, traffic_seconds, capture_seconds)):
        raise ValueError("Rate and durations must be integers")
    if not 1 <= pps <= 1000 or not 1 <= capture_seconds <= 120:
        raise ValueError("Rate must be 1..1000 pps and capture 1..120 seconds")
    if not 1 <= traffic_seconds <= capture_seconds - CAPTURE_RESERVE_SECONDS:
        raise ValueError("Traffic needs at least 5 seconds of capture reserve")
    planned = pps * traffic_seconds
    if planned > MAX_DATAGRAMS or planned * PAYLOAD_BYTES > MAX_PAYLOAD_BYTES:
        raise ValueError("Traffic exceeds 60000 datagrams or 64 MiB of payload")
    return planned


def ipv4(value):
    address = ipaddress.IPv4Address(value)
    if address.is_multicast or address.is_unspecified or address.is_loopback or address.is_reserved:
        raise ValueError("Expected an ordinary unicast IPv4 address")
    return address


def read_json(args):
    result = subprocess.run(args, capture_output=True, text=True, check=True, timeout=5)
    return json.loads(result.stdout)


def verify_link(ethernet, source_ip, client_ip, client_mac):
    """Read exact local-link evidence; never discover or guess a target."""
    source, client = ipv4(source_ip), ipv4(client_ip)
    if source == client:
        raise ValueError("Source and client must be distinct")
    interfaces = read_json(["ip", "-j", "-4", "address", "show", "dev", ethernet])
    networks = [ipaddress.IPv4Network(f"{source}/{item['prefixlen']}", strict=False)
                for interface in interfaces for item in interface.get("addr_info", [])
                if item.get("family") == "inet" and item.get("local") == str(source)]
    if not networks or not any(client in network and client not in
                               (network.network_address, network.broadcast_address)
                               for network in networks):
        raise RuntimeError("Source/client are not verified on the specified Ethernet subnet")
    routes = read_json(["ip", "-j", "-4", "route", "get", str(client), "from", str(source)])
    if len(routes) != 1 or routes[0].get("dev") != ethernet or "gateway" in routes[0] \
            or routes[0].get("type", "unicast") != "unicast":
        raise RuntimeError("Client route is not directly through the specified Ethernet interface")
    neighbors = read_json(["ip", "-j", "-4", "neigh", "show", "to", str(client), "dev", ethernet])
    matches = [entry for entry in neighbors if entry.get("dst") == str(client)
               and entry.get("dev", ethernet) == ethernet]
    if len(matches) != 1:
        raise RuntimeError("No unique client neighbor entry; no traffic was sent")
    entry = matches[0]
    states = entry.get("state", [])
    if isinstance(states, str):
        states = states.split()
    if len(states) != 1 or states[0] not in ("REACHABLE", "STALE", "DELAY", "PROBE", "PERMANENT") \
            or entry.get("lladdr", "").lower() != client_mac.lower():
        raise RuntimeError("Client neighbor is unresolved/failed or its MAC does not match; no traffic was sent")
    return {"source_verified_on_ethernet": True, "direct_route_verified": True,
            "expected_neighbor_verified": True, "neighbor_state": states[0],
            "neighbor_freshness": "recently_confirmed" if states[0] == "REACHABLE" else "cached_mapping"}


def wait_for_capture(proc, output, timeout=60):
    deadline = time.monotonic() + timeout
    packet_path = output / "capture.pcap"
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError("Capture helper exited before its PCAP header was ready")
        if (output / "capture.active").is_file() and packet_path.exists() and packet_path.stat().st_size >= 24:
            with packet_path.open("rb") as stream:
                header = stream.read(24)
            endian = {b"\xd4\xc3\xb2\xa1": "<", b"\xa1\xb2\xc3\xd4": ">",
                      b"\x4d\x3c\xb2\xa1": "<", b"\xa1\xb2\x3c\x4d": ">"}.get(header[:4])
            if endian is None or len(header) != 24:
                raise RuntimeError("Capture output has an invalid classic PCAP header")
            major, minor, _, _, snaplen, linktype = struct.unpack(endian + "HHIIII", header[4:])
            if (major, minor) != (2, 4) or not 1 <= snaplen <= 65535 or linktype != 127:
                raise RuntimeError("Capture output is not supported radiotap PCAP")
            if proc.poll() is not None:
                raise RuntimeError("Capture helper exited while arming traffic")
            return
        time.sleep(.1)
    raise RuntimeError("Capture did not become ready before the deadline")


def send_traffic(proc, source_ip, client_ip, evidence, check_link, ethernet, output,
                 pps=PACKETS_PER_SECOND, traffic_seconds=TRAFFIC_SECONDS,
                 capture_seconds=CAPTURE_SECONDS):
    planned = validate_profile(pps, traffic_seconds, capture_seconds)
    payload = bytes(PAYLOAD_BYTES)
    started = time.monotonic()
    interval = 1 / pps
    next_send = started
    elapsed_slots = 0
    lateness_total = 0.0
    evidence.update({"traffic_started_unix_ns": time.time_ns(), "sent_packets": 0,
                     "sent_payload_bytes": 0, "requested_seconds": traffic_seconds,
                     "maximum_packets_per_second": pps, "planned_datagrams": planned,
                     "pacing": "minimum_interval_without_catchup",
                     "late_schedules": 0, "missed_schedules": 0, "timed_send_attempts": 0,
                     "maximum_lateness_seconds": 0.0,
                     "payload_bytes": PAYLOAD_BYTES, "destination_port": DESTINATION_PORT})
    last_checked = started
    # Linux IP_PKTINFO selects the outgoing interface for each datagram without
    # requiring CAP_NET_RAW or changing the machine's routing configuration.
    pktinfo = struct.pack("=I4s4s", socket.if_nametoindex(ethernet),
                          socket.inet_aton(source_ip), bytes(4))
    evidence["periodic_link_checks"] = []
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as udp:
            udp.bind((source_ip, 0))
            udp.settimeout(1)
            while elapsed_slots < planned:
                if time.monotonic() - last_checked >= 5:
                    evidence["periodic_link_checks"].append(check_link())
                    last_checked = time.monotonic()
                remaining = min(next_send, started + traffic_seconds) - time.monotonic()
                while remaining > 0:
                    time.sleep(remaining)
                    remaining = min(next_send, started + traffic_seconds) - time.monotonic()
                if time.monotonic() - started >= traffic_seconds:
                    break
                if proc.poll() is not None or not (output / "capture.active").is_file():
                    raise RuntimeError("Capture ended during traffic generation")
                sent_at = time.monotonic()
                if sent_at >= started + traffic_seconds:
                    break
                lateness = max(0.0, sent_at - next_send)
                skipped = int(lateness / interval)
                elapsed_slots += skipped
                evidence["missed_schedules"] += skipped
                if elapsed_slots >= planned:
                    break
                evidence["late_schedules"] += lateness > 0
                evidence["timed_send_attempts"] += 1
                lateness_total += lateness
                evidence["maximum_lateness_seconds"] = max(evidence["maximum_lateness_seconds"], lateness)
                sent = udp.sendmsg([payload], [(socket.IPPROTO_IP, getattr(socket, "IP_PKTINFO", 8), pktinfo)],
                                   0, (client_ip, DESTINATION_PORT))
                if sent != len(payload):
                    raise RuntimeError("UDP payload was not sent completely")
                evidence["sent_packets"] += 1
                evidence["sent_payload_bytes"] += sent
                elapsed_slots += 1
                # Anchor to the send start, so checks/send overhead reduce the
                # sleep rather than the rate. Late slots never produce bursts.
                next_send = sent_at + interval
            remaining = started + traffic_seconds - time.monotonic()
            if elapsed_slots >= planned and remaining > 0:
                time.sleep(remaining)
    finally:
        elapsed = time.monotonic() - started
        evidence["traffic_elapsed_seconds"] = elapsed
        evidence["actual_packets_per_second"] = evidence["sent_packets"] / elapsed if elapsed else 0
        attempts = evidence["timed_send_attempts"]
        evidence["mean_lateness_seconds"] = lateness_total / attempts if attempts else 0
        evidence["traffic_ended_unix_ns"] = time.time_ns()


def stop_capture(proc):
    """Let the helper finish restoration; never SIGKILL its cleanup."""
    if proc.poll() is None:
        try:
            proc.send_signal(signal.SIGINT)
        except ProcessLookupError:
            pass
    proc.wait()


def save_evidence(output, evidence):
    # The helper must create its own private directory; do not create it early.
    if not output.exists():
        return
    mode = output.stat()
    if output.is_symlink() or not stat.S_ISDIR(mode.st_mode) or mode.st_uid != os.getuid() \
            or stat.S_IMODE(mode.st_mode) & 0o077:
        raise RuntimeError("Refusing evidence output outside the helper's private directory")
    with (output / "traffic.json").open("x", encoding="utf-8") as stream:
        json.dump(evidence, stream, indent=2)
        stream.write("\n")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("interface", "ethernet", "management-ip", "bssid", "client-ip", "client-mac", "source-ip"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--frequency", required=True, type=int)
    parser.add_argument("--width", type=int, choices=(20, 80), default=80)
    parser.add_argument("--center", type=int)
    parser.add_argument("--packets", type=int, default=2000)
    parser.add_argument("--pps", type=int, default=PACKETS_PER_SECOND, help="1..1000 packets/s; default 250")
    parser.add_argument("--traffic-seconds", type=int, default=TRAFFIC_SECONDS,
                        help="default 30; at least 5 below capture seconds; <=60000 datagrams and <=64 MiB payload")
    parser.add_argument("--capture-seconds", type=int, default=CAPTURE_SECONDS,
                        help="1..120 seconds; default 45")
    parser.add_argument("--include-link-traffic", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--confirm-capture", action="store_true")
    args = parser.parse_args(argv)
    if not sys.platform.startswith("linux"):
        parser.error("This trial requires Linux")
    if not args.confirm_capture:
        parser.error("--confirm-capture is required for capture and bounded traffic")
    try:
        validate_profile(args.pps, args.traffic_seconds, args.capture_seconds)
    except ValueError as exc:
        parser.error(str(exc))
    for iface in (args.interface, args.ethernet):
        if not re.fullmatch(r"[A-Za-z0-9_.-]{1,15}", iface):
            parser.error("Invalid interface name")
    if args.interface == args.ethernet:
        parser.error("Capture and Ethernet interfaces must differ")
    for address in (args.client_mac, args.bssid):
        if not re.fullmatch(r"(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}", address) \
                or int(address[:2], 16) & 1:
            parser.error("Expected unicast MAC addresses")
    try:
        ipv4(args.source_ip)
        ipv4(args.client_ip)
        ipaddress.ip_address(args.management_ip)
    except ValueError as exc:
        parser.error(str(exc))
    if not 1 <= args.packets <= 2000 or not 2400 <= args.frequency <= 7125:
        parser.error("Invalid packet bound or frequency")
    if args.width == 80 and (args.center is None or abs(args.center - args.frequency) not in (10, 30)):
        parser.error("80 MHz requires a center 10 or 30 MHz from the primary frequency")
    output = args.output.expanduser().resolve()
    if output.exists():
        parser.error("Output must be a new private directory created by the capture helper")
    os.umask(0o077)
    evidence = {"schema_version": 1,
                "profile": f"udp-downlink-{args.pps}pps-{PAYLOAD_BYTES}bytes-{args.traffic_seconds}s",
                "capture_seconds": args.capture_seconds, "delivery_claim": "UDP sends only; receiver delivery unverified"}
    evidence["preflight"] = verify_link(args.ethernet, args.source_ip, args.client_ip, args.client_mac)
    helper = Path(__file__).with_name("capture_linux.py")
    command = [sys.executable, str(helper), "--interface", args.interface, "--ethernet", args.ethernet,
               "--management-ip", args.management_ip, "--bssid", args.bssid,
               "--frequency", str(args.frequency), "--width", str(args.width),
               "--seconds", str(args.capture_seconds), "--packets", str(args.packets),
               "--output", str(output), "--confirm-capture"]
    if args.center is not None:
        command += ["--center", str(args.center)]
    if args.include_link_traffic:
        command.append("--include-link-traffic")
    proc = None
    error = None
    def interrupted(signum, frame):
        raise RuntimeError(f"Trial interrupted by signal {signum}")
    old_handlers = {sig: signal.signal(sig, interrupted)
                    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)}
    try:
        proc = subprocess.Popen(command, start_new_session=True)
        wait_for_capture(proc, output)
        evidence["armed_at_unix_ns"] = time.time_ns()
        evidence["before_traffic"] = verify_link(args.ethernet, args.source_ip, args.client_ip, args.client_mac)
        send_traffic(proc, args.source_ip, args.client_ip, evidence,
                     lambda: verify_link(args.ethernet, args.source_ip, args.client_ip, args.client_mac),
                     args.ethernet, output, args.pps, args.traffic_seconds, args.capture_seconds)
        if proc.wait(timeout=180) != 0:
            raise RuntimeError("Capture helper failed; inspect its private manifest")
        manifest = json.loads((output / "manifest.json").read_text())
        if not manifest.get("restored") or manifest.get("restore_error") or manifest.get("error"):
            raise RuntimeError("Capture helper did not verify successful restoration")
        evidence["restoration_verified"] = True
    except BaseException as exc:
        error = exc
        evidence["error"] = str(exc)
    finally:
        for sig in old_handlers:
            signal.signal(sig, signal.SIG_IGN)
        try:
            if proc is not None:
                try:
                    stop_capture(proc)
                    evidence["capture_exit"] = proc.returncode
                    manifest_path = output / "manifest.json"
                    if manifest_path.exists():
                        final_manifest = json.loads(manifest_path.read_text())
                        evidence["restoration_verified"] = bool(final_manifest.get("restored")) \
                            and not final_manifest.get("restore_error")
                except Exception as exc:
                    evidence["cleanup_verification_error"] = str(exc)
                    error = error or exc
            try:
                save_evidence(output, evidence)
            except Exception as exc:
                evidence["evidence_write_error"] = str(exc)
                error = error or exc
        finally:
            for sig, handler in old_handlers.items():
                signal.signal(sig, handler)
    print(json.dumps({"output": str(output), "sent_packets": evidence.get("sent_packets", 0),
                      "restoration_verified": evidence.get("restoration_verified", False),
                      "error": str(error) if error else None}))
    return 1 if error else 0


if __name__ == "__main__":
    raise SystemExit(main())

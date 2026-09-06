#!/usr/bin/env python3
"""Identify ESP32 boards by MAC, not by serial port.

Why this exists
---------------
`provision.py` keys its state files by **serial port**: COM3.json, COM5.json.
A port is a property of which USB socket you happened to use, not of the board
plugged into it. Cycling six boards through three ports makes a duplicate
node_id near-certain, and nothing errors -- the second board silently takes the
first one's identity, and you find out later when two nodes claim to be node 2
and the link table quietly merges them.

A MAC address is burned into the silicon. Keying on it means a board keeps its
identity no matter which socket it lands in, in what order, on which machine.

    python board_index.py                 # what is plugged in, and who is it
    python board_index.py --assign        # give any unrecognised board an id
    python board_index.py --watch         # sit and report boards as they mount

The index is `board_index.json` next to this script. It only ever *adds*:
an existing MAC's node_id is never rewritten, because that is the exact
accident this tool exists to prevent.

Reading the MAC resets the board into its bootloader, which is harmless when
you are about to flash it and disruptive when you are not -- use --passive to
sniff the boot log over serial instead, without touching the board.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
INDEX = os.path.join(HERE, "board_index.json")

# Espressif OUIs seen on this fleet, plus the locally-administered forms the
# ESP32 derives for its secondary interfaces.
MAC_RE = re.compile(r"\b([0-9a-fA-F]{2}(?::[0-9a-fA-F]{2}){5})\b")


def load_index():
    try:
        with open(INDEX, "r", encoding="utf-8") as fh:
            return json.load(fh)
    except Exception:
        return {"boards": {}}


def save_index(idx):
    with open(INDEX, "w", encoding="utf-8") as fh:
        json.dump(idx, fh, indent=2, sort_keys=True)


def ports():
    try:
        from serial.tools import list_ports
    except ImportError:
        sys.exit("pyserial not installed:  pip install pyserial")
    out = []
    for p in list_ports.comports():
        # Skip obvious non-boards (bluetooth bridges, virtual pairs) but do not
        # be clever about it -- a board on an unrecognised adapter still counts,
        # so only exclude what is definitely not a USB serial device.
        if p.vid is None and p.pid is None:
            continue
        out.append((p.device, (p.description or "").strip()))
    return sorted(out)


def mac_via_esptool(port, chip="esp32c6"):
    """Authoritative, but resets the board into the bootloader."""
    for form in ("read_mac", "read-mac"):     # esptool 4.x vs 5.x spelling
        try:
            r = subprocess.run(
                [sys.executable, "-m", "esptool", "--port", port,
                 "--chip", chip, form],
                capture_output=True, text=True, timeout=30)
        except Exception:
            continue
        if r.returncode == 0:
            # BASE MAC is the authoritative one; take nothing else if it is
            # present, because the other lines are derived forms.
            for line in r.stdout.splitlines():
                if "BASE MAC" in line.upper():
                    m = MAC_RE.search(line)
                    if m:
                        return m.group(1).lower()
            for line in r.stdout.splitlines():
                if "MAC" in line.upper() and "EXT" not in line.upper():
                    m = MAC_RE.search(line)
                    if m:
                        return m.group(1).lower()
            m = MAC_RE.search(r.stdout)
            if m:
                return m.group(1).lower()
    return None


def mac_via_log(port, seconds=12, baud=115200):
    """Passive: watch the boot log. Needs the board to say its MAC, which it
    does on the `mode : sta (..)` line shortly after boot -- so it only works
    if the board reboots inside the window, or logs it periodically."""
    try:
        import serial
    except ImportError:
        return None
    try:
        with serial.Serial(port, baud, timeout=0.5) as ser:
            end = time.time() + seconds
            buf = ""
            while time.time() < end:
                try:
                    buf += ser.read(512).decode("utf-8", "replace")
                except Exception:
                    break
                m = MAC_RE.search(buf)
                if m:
                    return m.group(1).lower()
    except Exception:
        return None
    return None


def next_free_id(idx):
    used = {b["node_id"] for b in idx["boards"].values()}
    n = 0
    while n in used:
        n += 1
    return n


def identify(port, passive, chip):
    return mac_via_log(port) if passive else mac_via_esptool(port, chip)


def report(idx, assign, passive, chip):
    found = ports()
    if not found:
        print("no serial boards mounted")
        return []

    rows = []
    for port, desc in found:
        mac = identify(port, passive, chip)
        if not mac:
            rows.append((port, desc, None, None, "unreadable"))
            continue
        entry = idx["boards"].get(mac)
        if entry:
            entry["last_port"] = port
            entry["last_seen"] = time.strftime("%Y-%m-%d %H:%M:%S")
            rows.append((port, desc, mac, entry["node_id"], "known"))
        elif assign:
            nid = next_free_id(idx)
            idx["boards"][mac] = {
                "node_id": nid,
                "label": "",
                "first_seen": time.strftime("%Y-%m-%d %H:%M:%S"),
                "last_seen": time.strftime("%Y-%m-%d %H:%M:%S"),
                "last_port": port,
            }
            rows.append((port, desc, mac, nid, "ASSIGNED"))
        else:
            rows.append((port, desc, mac, None, "NEW (run --assign)"))

    print("%-8s %-18s %-11s %-9s %s" % ("port", "mac", "node_id", "state", "adapter"))
    for port, desc, mac, nid, state in rows:
        print("%-8s %-18s %-11s %-9s %s" % (
            port, mac or "-", "-" if nid is None else str(nid), state, desc[:34]))

    # A duplicate node_id in the index is the failure this tool exists to
    # prevent, so say so loudly rather than printing a tidy table over it.
    seen = {}
    for mac, b in idx["boards"].items():
        seen.setdefault(b["node_id"], []).append(mac)
    dupes = {k: v for k, v in seen.items() if len(v) > 1}
    if dupes:
        print("\n*** DUPLICATE node_id IN INDEX ***")
        for nid, macs in sorted(dupes.items()):
            print("    node_id %d claimed by: %s" % (nid, ", ".join(macs)))
        print("    Fix board_index.json before provisioning anything.")

    if rows:
        print("\nprovision a known board with, e.g.:")
        for port, _, mac, nid, state in rows:
            if nid is not None:
                print("    python provision.py --port %s --chip %s --node-id %d "
                      "--tdm-slot %d --tdm-total 9 \\\n"
                      "        --ssid <SSID> --password <PSK> --target-ip <SERVER>"
                      % (port, "esp32c6", nid, nid))
                break
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--assign", action="store_true",
                    help="give any unrecognised board the lowest free node_id")
    ap.add_argument("--passive", action="store_true",
                    help="read the boot log instead of resetting the board")
    ap.add_argument("--watch", action="store_true",
                    help="poll and report each board as it is mounted")
    ap.add_argument("--chip", default="esp32c6")
    ap.add_argument("--label", nargs=2, metavar=("MAC", "TEXT"),
                    help="attach a human label to a MAC, e.g. --label aa:bb:.. kitchen")
    args = ap.parse_args()

    idx = load_index()

    if args.label:
        mac = args.label[0].lower()
        if mac not in idx["boards"]:
            sys.exit("unknown MAC %s -- mount it and run --assign first" % mac)
        idx["boards"][mac]["label"] = args.label[1]
        save_index(idx)
        print("labelled %s -> %s" % (mac, args.label[1]))
        return

    if args.watch:
        print("watching for boards -- Ctrl-C to stop")
        known_ports = set()
        try:
            while True:
                now = {p for p, _ in ports()}
                if now != known_ports:
                    new = now - known_ports
                    if new:
                        print("\n[%s] mounted: %s" %
                              (time.strftime("%H:%M:%S"), ", ".join(sorted(new))))
                        report(idx, args.assign, args.passive, args.chip)
                        save_index(idx)
                    known_ports = now
                time.sleep(2)
        except KeyboardInterrupt:
            print("\nstopped")
        return

    report(idx, args.assign, args.passive, args.chip)
    save_index(idx)


if __name__ == "__main__":
    main()

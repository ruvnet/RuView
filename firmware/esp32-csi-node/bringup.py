#!/usr/bin/env python3
"""Take every board currently plugged in from factory state to wall-ready.

Bringing up nine boards by hand is six commands each with an ordering that is
easy to get wrong in ways that do not announce themselves:

  * flashing without `0xf000 ota_data_initial.bin` leaves otadata pointing at
    the other OTA slot, so the board silently boots the PREVIOUS image and the
    flash looks like it did nothing (cost me a wasted cycle on 2026-08-30);
  * provisioning without --tdm-total leaves the ESP-NOW beacon period sized for
    the wrong fleet, and a single mismatched node took the whole mesh to zero
    delivery for five minutes;
  * a board provisioned without an OTA PSK is USB-update-only forever, which is
    only discovered later, from a ladder.

So this does the whole sequence and then VERIFIES it off the boot log rather
than trusting that each step worked. A board that does not print the expected
lines is reported as not ready.

    python bringup.py --net home --tdm-total 9
    python bringup.py --net home --tdm-total 9 --port COM7   # just one
"""

import argparse
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = os.path.join(HERE, "build")

FLASH_REGIONS = [
    ("0x0",     "bootloader/bootloader.bin"),
    ("0x8000",  "partition_table/partition-table.bin"),
    ("0xf000",  "ota_data_initial.bin"),          # never omit: see docstring
    ("0x20000", "esp32-csi-node.bin"),
]


def run(cmd, timeout=600):
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def ports():
    from serial.tools import list_ports
    return sorted(p.device for p in list_ports.comports() if p.vid or p.pid)


def flash(port):
    cmd = [sys.executable, "-m", "esptool", "--chip", "esp32c6", "--port", port,
           "--baud", "460800", "--before", "default-reset", "--after",
           "hard-reset", "write-flash", "--flash-mode", "dio",
           "--flash-size", "16MB", "--flash-freq", "80m"]
    for off, rel in FLASH_REGIONS:
        cmd += [off, os.path.join(BUILD, rel)]
    r = run(cmd)
    ok = r.stdout.lower().count("hash of data verified") == len(FLASH_REGIONS)
    return ok, (r.stderr or r.stdout)[-300:]


def provision(port, net, total):
    cmd = [sys.executable, os.path.join(HERE, "provision_node.py"),
           "--port", port, "--tdm-total", str(total)]
    if net:
        cmd += ["--net", net]
    r = run(cmd)
    return ("ok -- NVS written" in r.stdout), (r.stderr or r.stdout)[-300:]


def verify(port, expect_ssid, expect_fleet, seconds=22):
    """Read the boot log and confirm the four things that silently go wrong."""
    try:
        import serial
    except ImportError:
        return None, "pyserial missing"
    try:
        with serial.Serial(port, 115200, timeout=0.3) as ser:
            ser.setDTR(False); ser.setRTS(True); time.sleep(0.15)
            ser.setRTS(False); ser.reset_input_buffer()
            end = time.time() + seconds
            buf = b""
            while time.time() < end:
                buf += ser.read(4096)
    except Exception as e:
        return None, str(e)
    t = buf.decode("utf-8", "replace")
    checks = {
        "ssid":  ("ssid=%s" % expect_ssid) in t,
        "fleet": ("fleet=%d" % expect_fleet) in t,
        "psk":   "OTA PSK loaded from NVS" in t,
        "ip":    "Got IP:" in t,
    }
    ip = re.search(r"Got IP: ([0-9.]+)", t)
    return checks, (ip.group(1) if ip else "?")


def main():
    ap = argparse.ArgumentParser(description=__doc__,
            formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--net", help="network profile (provision_conf_<net>.json)")
    ap.add_argument("--tdm-total", type=int, required=True)
    ap.add_argument("--port", help="only this port; default is every board plugged in")
    a = ap.parse_args()

    if not os.path.exists(os.path.join(BUILD, "esp32-csi-node.bin")):
        sys.exit("no build at %s -- run idf.py build first" % BUILD)

    targets = [a.port] if a.port else ports()
    if not targets:
        sys.exit("no serial boards plugged in")

    # MAC-keyed identity first, so a board keeps its node_id regardless of which
    # USB socket it landed in. --assign only ever adds.
    idx = run([sys.executable, os.path.join(HERE, "board_index.py"), "--assign"])
    print(idx.stdout.strip().split("provision a known board")[0].strip())
    print()

    ssid = "?"
    conf = os.path.join(HERE, "provision_conf_%s.json" % a.net if a.net
                        else "provision_conf.json")
    if os.path.exists(conf):
        import json
        ssid = json.load(open(conf)).get("ssid", "?")

    ready, failed = [], []
    for port in targets:
        print("=" * 58)
        print("%s" % port)
        ok, msg = flash(port)
        print("  flash      %s" % ("ok" if ok else "FAILED: " + msg))
        if not ok:
            failed.append(port); continue
        ok, msg = provision(port, a.net, a.tdm_total)
        print("  provision  %s" % ("ok" if ok else "FAILED: " + msg))
        if not ok:
            failed.append(port); continue
        checks, ip = verify(port, ssid, a.tdm_total)
        if checks is None:
            print("  verify     could not read console (%s)" % ip)
            failed.append(port); continue
        print("  verify     ssid=%s fleet=%s psk=%s ip=%s (%s)"
              % (checks["ssid"], checks["fleet"], checks["psk"],
                 checks["ip"], ip))
        (ready if all(checks.values()) else failed).append(port)

    print("\n" + "=" * 58)
    print("READY:  %s" % (", ".join(ready) or "none"))
    if failed:
        print("FAILED: %s  <- do not mount these" % ", ".join(failed))


if __name__ == "__main__":
    main()

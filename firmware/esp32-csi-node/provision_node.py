#!/usr/bin/env python3
"""Provision a node without putting the WiFi password on a command line.

`provision.py` takes --password as an argument, so every invocation leaves the
credential in shell history, process listings, terminal scrollback and any
transcript of the session. That is a poor place for it and an easy thing to
forget you did.

This reads the password from a file, passes it to provision.py as a subprocess
argument (unavoidable -- that is provision.py's only interface), and never
prints or echoes it. The password still appears briefly in the child process's
argv, so this is not protection against a local attacker; it is protection
against the credential being permanently recorded somewhere it does not belong.

It also fills in the two things that are easy to get wrong by hand:

  * node_id comes from board_index.py's MAC index, so a board keeps its
    identity regardless of which USB socket it lands in.
  * --tdm-total is required, not optional. The firmware derives its ESP-NOW
    beacon period from it (c6_sync_espnow_period_ms), and a node provisioned
    without it reports `fleet=1` and falls back to the 80 ms default -- which
    is correct for three nodes and silently overruns the 50 Hz receive gate at
    nine. Measured on 2026-08-28: an over-fast beacon discarded 60-90% of peer
    frames at random and wedged a transmit queue.

  * --ota-psk is supplied from a file, generating one on first use. Without a
    provisioned key the node's OTA endpoint fails closed and the board can only
    ever be updated over USB -- which, once nine boards are screwed to walls,
    turns every firmware fix into a ladder job. Flashing NVS rewrites the whole
    partition, so the key has to be present on EVERY provisioning run or it is
    erased; that is exactly why it lives in a file this script always reads
    rather than a flag someone has to remember.

    python provision_node.py --port COM3 --tdm-total 9
    python provision_node.py --all --tdm-total 9      # every mounted board
"""

import argparse
import json
import ipaddress
import os
import secrets
import stat
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CONF = os.path.join(HERE, "provision_conf.json")
# Alternate network profiles live beside it as provision_conf_<name>.json,
# selected with --net <name>. Reverting a fleet to a known-good SSID is a
# diagnostic move that wants to be one flag, not an edit-and-remember-to-
# put-it-back on a file that also holds the path to a credential.
# NVS partition offset from partitions_4mb.csv. Nothing else is written, so a
# firmware image already on the board is untouched by provisioning.
NVS_OFFSET = "0x9000"


def load_conf(net=None):
    global CONF
    if net:
        CONF = os.path.join(HERE, "provision_conf_%s.json" % net)
    if not os.path.exists(CONF):
        sys.exit(
            "no %s.\n\nCreate it with:\n"
            '  {\n'
            '    "ssid": "thisismyssid",\n'
            '    "password_file": "~/secrets/thisismyssid.txt",\n'
            '    "target_ip": "192.168.1.10",\n'
            '    "ota_psk_file": "~/secrets/ota_psk.txt",\n'
            '    "chip": "esp32c6",\n'
            '    "edge_tier": 2\n'
            '  }\n\n'
            "password_file holds the passphrase and nothing else. Keep it "
            "outside this repo." % CONF)
    with open(CONF, "r", encoding="utf-8") as fh:
        return json.load(fh)


def secret_path(path):
    """Resolve a credential path from a profile.

    Expanded so a profile can say ~/onedrive/ota_psk.txt instead of
    D:/Users/<name>/onedrive/ota_psk.txt. The profile is tracked -- it holds
    only paths, never a credential -- so keeping a login name out of it costs
    nothing and stops the file naming a particular person's machine.

    Prefer `~`: expanduser resolves it on Windows and POSIX alike. %VAR% is
    also expanded, but only on Windows -- expandvars reads $VAR syntax on
    POSIX -- so a profile written with %USERPROFILE% breaks under WSL. An
    absolute path is returned unchanged, so existing profiles keep working.
    """
    return os.path.expandvars(os.path.expanduser(path))


def read_password(path):
    path = secret_path(path)
    try:
        with open(path, encoding="utf-8-sig") as fh:
            pw = fh.read().strip()
    except Exception as e:
        sys.exit("cannot read password file: %s" % e)
    if not pw:
        sys.exit("password file is empty")
    if len(pw) < 8:
        sys.exit("password is %d chars; WPA needs at least 8 -- is the file "
                 "holding something else?" % len(pw))
    return pw


def read_or_create_psk(conf):
    """Return the fleet's OTA pre-shared key, creating it on first use.

    One key for the whole fleet rather than one per node: the point of OTA here
    is pushing a build to every board at once, and per-node keys would mean
    tracking nine secrets to do it. The blast radius is the same as the WiFi
    passphrase, which any host on this VLAN already needs.

    Kept outside the repo for the same reason the passphrase is, and refused if
    it is not -- a secret inside a git worktree is one `git add -A` from being
    committed.
    """
    path = conf.get("ota_psk_file")
    if not path:
        # Default alongside the passphrase, which is already outside the repo.
        path = os.path.join(os.path.dirname(conf["password_file"]), "ota_psk.txt")
    path = secret_path(path)

    real = os.path.realpath(path)
    if real.startswith(os.path.realpath(HERE) + os.sep):
        sys.exit("ota_psk_file is inside the repository (%s). Move it outside "
                 "the worktree; secrets there are one 'git add -A' from being "
                 "committed." % real)

    if os.path.exists(path):
        try:
            with open(path, encoding="utf-8-sig") as fh:
                psk = fh.read().strip()
        except Exception as e:
            sys.exit("cannot read ota_psk_file: %s" % e)
        if psk:
            # ota_update.c caps the key at 65 bytes including the NUL.
            if len(psk) > 64:
                sys.exit("OTA PSK is %d chars; the firmware accepts at most 64"
                         % len(psk))
            return psk

    psk = secrets.token_hex(32)          # 64 hex chars
    try:
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(psk + "\n")
        os.chmod(path, stat.S_IRUSR | stat.S_IWUSR)
    except Exception as e:
        sys.exit("cannot create ota_psk_file at %s: %s" % (path, e))
    print("generated a new fleet OTA key at %s -- back this up; without it you "
          "cannot push firmware to these boards over the network." % path)
    return psk


def node_id_for(port):
    """Ask board_index.py who this board is, so identity follows the MAC."""
    idx_path = os.path.join(HERE, "board_index.json")
    try:
        with open(idx_path, encoding="utf-8") as fh:
            idx = json.load(fh)
    except Exception:
        return None
    for mac, b in idx.get("boards", {}).items():
        if b.get("last_port") == port:
            return b["node_id"], mac
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__,
            formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", help="serial port; omit with --all")
    ap.add_argument("--all", action="store_true",
                    help="provision every board currently in the index")
    ap.add_argument("--tdm-total", type=int, required=True,
                    help="TOTAL nodes in the fleet. Required: the firmware "
                         "derives its beacon period from this and gets it "
                         "wrong, silently, if left unset.")
    ap.add_argument("--node-id", type=int,
                    help="override the id from board_index.json")
    ap.add_argument("--net",
                    help="network profile name; reads provision_conf_<name>.json "
                         "instead of provision_conf.json (e.g. --net home)")
    ap.add_argument("--no-auto-reset", action="store_true",
                    help="board is ALREADY in the bootloader (BOOT held, "
                         "RESET tapped). Some units have a non-functional "
                         "auto-reset circuit: the CH343 enumerates and the "
                         "chip runs fine, but it never hears DTR/RTS, so "
                         "esptool reports 'No serial data received'. Skips "
                         "the reset so one button press covers the flash.")
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()

    conf = load_conf(a.net)
    pw = read_password(conf["password_file"])
    psk = read_or_create_psk(conf)

    targets = []
    if a.all:
        idx_path = os.path.join(HERE, "board_index.json")
        with open(idx_path, encoding="utf-8") as fh:
            idx = json.load(fh)
        for mac, b in sorted(idx["boards"].items(), key=lambda kv: kv[1]["node_id"]):
            if b.get("last_port"):
                targets.append((b["last_port"], b["node_id"], mac))
    else:
        if not a.port:
            sys.exit("--port or --all required")
        nid = a.node_id
        mac = "?"
        found = node_id_for(a.port)
        if found and nid is None:
            nid, mac = found
        if nid is None:
            sys.exit("no node_id for %s -- run board_index.py --assign first, "
                     "or pass --node-id" % a.port)
        targets.append((a.port, nid, mac))

    if a.tdm_total < len(targets):
        sys.exit("--tdm-total %d is smaller than the %d boards being "
                 "provisioned" % (a.tdm_total, len(targets)))

    # Validate everything that reaches an argv list before it gets there.
    #
    # These values come from a JSON config and CLI flags, which a scanner
    # rightly treats as untrusted. subprocess is called in list form with no
    # shell, so this is not a command-injection path -- but "no shell" is a
    # property of THIS call site, and the values also end up in esptool
    # arguments and a flash offset. Constraining them here means the safety
    # does not depend on remembering that, and a typo fails loudly at the top
    # instead of surfacing as a confusing esptool error nine boards in.
    CHIPS = {"esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c5", "esp32c6", "esp32h2"}
    chip = str(conf.get("chip", "esp32c6"))
    if chip not in CHIPS:
        sys.exit("unsupported chip %r in config (expected one of %s)"
                 % (chip, ", ".join(sorted(CHIPS))))

    ssid = str(conf["ssid"])
    if not ssid or len(ssid) > 32:
        sys.exit("ssid must be 1-32 characters (802.11 limit)")

    target_ip = str(conf["target_ip"])
    try:
        ipaddress.ip_address(target_ip)
    except ValueError:
        sys.exit("target_ip %r is not a valid IP address" % target_ip)

    try:
        edge_tier = int(conf.get("edge_tier", 2))
    except (TypeError, ValueError):
        sys.exit("edge_tier must be an integer")
    if edge_tier not in (0, 1, 2):
        sys.exit("edge_tier must be 0, 1 or 2 (got %d)" % edge_tier)

    # Ports arrive from --port or from board_index.json, so they are the one
    # remaining non-literal in the esptool argv. Real device nodes are
    # COM7, /dev/ttyUSB0, /dev/cu.usbserial-1420 -- none of which need a
    # character outside this set.
    for port, _nid, _mac in targets:
        if not port or len(port) > 64 or not all(
                c.isalnum() or c in "/\\.-_:" for c in port):
            sys.exit("refusing to use %r as a serial port: expected something "
                     "like COM7 or /dev/ttyUSB0" % port)

    for port, nid, mac in targets:
        cmd = [sys.executable, os.path.join(HERE, "provision.py"),
               "--port", port,
               "--chip", chip,
               "--ssid", ssid,
               "--password", pw,
               "--target-ip", target_ip,
               "--node-id", str(nid),
               "--tdm-slot", str(nid),
               "--tdm-total", str(a.tdm_total),
               "--edge-tier", str(edge_tier),
               "--ota-psk", psk]
        redact = {pw: "<password>", psk: "<ota-psk>"}
        shown = [redact.get(c, c) for c in cmd]
        print("\n%s  node %d  (%s)" % (port, nid, mac))
        print("  " + " ".join(shown[1:]))
        if a.dry_run:
            continue
        # Semgrep reports dangerous-subprocess-use-tainted-env-args here and
        # suggests shlex.quote(). Do not apply it: quoting is for building a
        # shell string, and this is the list form with no shell=True, so argv
        # goes straight to execve. shlex.quote would hand provision.py a port
        # named "'COM7'", quotes included, and break provisioning to satisfy a
        # scanner. There is no shell to inject into; a hostile value can only
        # ever become one bad argument, never a second command, and `port` is
        # range-checked above.
        #
        # The finding is left visible rather than suppressed. The suppression
        # pragma does not take effect in this workflow -- it was tried on the
        # line above the call and on the reported line itself, and the finding
        # survived both -- and the SAST job is `continue-on-error: true` by
        # design, so it does not gate the PR. The pragma is not even spelled
        # out here: on its own in a comment it is blanket-suppression syntax.
        r = subprocess.run(cmd, capture_output=True, text=True)
        # provision.py cannot build the NVS image without ESP-IDF on PATH, so
        # on a bare Windows host it writes nvs_config.csv and stops. Finish the
        # job here rather than leaving nine boards half-provisioned: generate
        # the partition in the IDF container, flash it at the NVS offset, and
        # delete the CSV, which holds the passphrase in clear text.
        csv = os.path.join(HERE, "nvs_config.csv")
        if not os.path.exists(csv):
            print("  FAILED -- provision.py produced no CSV")
            print("  " + (r.stderr or r.stdout or "").strip()[-400:])
            continue
        try:
            g = subprocess.run(
                ["docker", "run", "--rm", "-v", HERE + ":/project", "-w", "/project",
                 "espressif/idf:v5.4", "bash", "-c",
                 "python $IDF_PATH/components/nvs_flash/nvs_partition_generator/"
                 "nvs_partition_gen.py generate nvs_config.csv nvs.bin 0x6000"],
                capture_output=True, text=True, env={**os.environ, "MSYS_NO_PATHCONV": "1"})
            binp = os.path.join(HERE, "nvs.bin")
            if g.returncode != 0 or not os.path.exists(binp):
                print("  FAILED to build NVS image")
                print("  " + (g.stderr or g.stdout).strip()[-300:])
                continue
            esp = [sys.executable, "-m", "esptool", "--chip",
                   chip, "--port", port,
                   "--baud", "460800"]
            if a.no_auto_reset:
                esp += ["--before", "no-reset"]
            esp += ["write_flash", NVS_OFFSET, binp]
            # Same finding and same reasoning as the provision.py call above:
            # list form, no shell, shlex.quote inapplicable, and `chip` and
            # `port` are the only non-literals.
            f = subprocess.run(esp, capture_output=True, text=True)
            if "verified" in f.stdout or f.returncode == 0:
                print("  ok -- NVS written at %s. Confirm the boot log says "
                      "fleet=%d, the expected SSID, and 'OTA PSK loaded from "
                      "NVS'. If it instead says 'No OTA PSK in NVS', this board "
                      "is USB-update-only." % (NVS_OFFSET, a.tdm_total))
            else:
                print("  FAILED to flash NVS")
                print("  " + (f.stderr or f.stdout).strip()[-300:])
        finally:
            for f2 in (csv, os.path.join(HERE, "nvs.bin")):
                try: os.remove(f2)
                except OSError: pass


if __name__ == "__main__":
    main()

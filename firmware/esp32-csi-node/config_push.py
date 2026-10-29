#!/usr/bin/env python3
"""Read and change node configuration over the network -- no USB.

This is provisioning, not OTA. `ota_push.py` writes firmware images; this
writes the same NVS parameters provision.py writes over USB, to a running
node, through the authenticated server on port 8032.

    # what is this node set to?
    python config_push.py --node <node-ip> --get

    # a change that cannot orphan a node: written, then the node restarts so
    # it actually takes effect (config is only read at boot)
    python config_push.py --node <node-ip> --set tdm_node_count=9

    # stage it across the fleet instead, and restart on your own schedule
    python config_push.py --fleet nodes.txt --set tdm_node_count=9 --no-reboot

    # darken the bedroom node's LED -- takes effect at once, no restart
    python config_push.py --node <node-ip> --set led_mode=steady led_brightness=10
    python config_push.py --node <node-ip> --set led_mode=off

    # the whole fleet at once
    python config_push.py --fleet nodes.txt --set edge_tier=1

Parameters that affect WiFi association (ssid, password, channel) are applied
as a TRIAL: the node banks its current settings, reboots, and keeps the new
ones only if it manages to re-associate. If it cannot, it restores the bank
and reboots itself. That is what makes a remote SSID change safe; without it
one typo means a ladder and a USB cable for every node.

The PSK is read from a file and never printed, logged, or passed on argv.
"""
import argparse
import os
import json
import sys
import urllib.error
import urllib.request

# No default path: a secret location is site-specific, and baking one in
# means shipping somebody's directory layout to everyone else. Set
# RUVIEW_OTA_PSK_FILE once, or pass --psk-file.
PSK_DEFAULT = os.environ.get("RUVIEW_OTA_PSK_FILE")
PORT = 8032

# The onboard WS2812 defaults to a 40 Hz square-wave gamma stimulus at full
# brightness. Fine on a bench, hostile in a bedroom.
LED_MODES = {"off": 0, "steady": 1, "flicker": 2}


def call(host, psk, method, body=None, timeout=20):
    url = "http://%s:%d/config" % (host, PORT)
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", "Bearer " + psk)
    if data:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read().decode() or "{}"), None
    except urllib.error.HTTPError as e:
        return None, "HTTP %d: %s" % (e.code, e.read().decode()[:200])
    except Exception as e:                      # noqa: BLE001 - report, never raise
        return None, str(e)


def looks_armed(err):
    """A trial push that reboots can lose its response on a congested node.

    Observed on real hardware: the write succeeded, the node rebooted and
    reverted correctly, but curl saw nothing. Reporting that as a failure
    invites a re-push at exactly the wrong moment -- while a trial is pending
    and the banked values are still unproven.
    """
    e = (err or "").lower()
    return any(k in e for k in ("timed out", "reset", "aborted",
                                "expecting value", "remote end closed"))


def coerce(text):
    """Numbers stay numbers so the node's type check passes; else a string."""
    try:
        return int(text)
    except ValueError:
        pass
    try:
        return float(text)
    except ValueError:
        return text


def main():
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--node", help="node IP")
    g.add_argument("--fleet", help="file of node IPs, one per line")
    ap.add_argument("--get", action="store_true", help="read current config")
    ap.add_argument("--set", nargs="+", metavar="KEY=VALUE", default=None)
    ap.add_argument("--trial-seconds", type=int, default=None,
                    help="how long a node may take to re-associate before reverting")
    ap.add_argument("--psk-file", default=PSK_DEFAULT,
                    help="file holding the fleet OTA PSK; defaults to "
                         "$RUVIEW_OTA_PSK_FILE")
    ap.add_argument("--no-reboot", action="store_true",
                    help="write the values but leave the node running the old "
                         "ones; config is only read at boot, so nothing takes "
                         "effect until you restart it yourself")
    ap.add_argument("--yes", action="store_true",
                    help="skip the confirmation for association-affecting changes")
    a = ap.parse_args()

    if not a.get and not a.set:
        ap.error("choose --get or --set")

    if not a.psk_file:
        print("no PSK file: pass --psk-file or set RUVIEW_OTA_PSK_FILE",
              file=sys.stderr)
        return 2
    try:
        psk = open(a.psk_file).read().strip()
    except OSError as e:
        print("cannot read PSK file: %s" % e, file=sys.stderr)
        return 2
    if not psk:
        print("PSK file is empty", file=sys.stderr)
        return 2

    hosts = [a.node] if a.node else [
        l.strip() for l in open(a.fleet) if l.strip() and not l.startswith("#")]

    if a.get:
        for h in hosts:
            r, err = call(h, psk, "GET")
            if err:
                print("%-16s UNREACHABLE  %s" % (h, err))
                continue
            cfg = r.get("config", {})
            flag = "  [TRIAL PENDING]" if r.get("trial_pending") else ""
            print("\n%s%s" % (h, flag))
            for k in sorted(cfg):
                v = cfg[k]
                print("   %-22s %s" % (k, "-" if v is None else v))
        return 0

    body = {}
    for item in a.set:
        if "=" not in item:
            ap.error("--set takes KEY=VALUE, got %r" % item)
        k, v = item.split("=", 1)
        k, v = k.strip(), v.strip()
        if k == "led_mode" and v.lower() in LED_MODES:
            body[k] = LED_MODES[v.lower()]
        else:
            body[k] = coerce(v)
    if a.trial_seconds:
        body["trial_seconds"] = a.trial_seconds
    if a.no_reboot:
        body["reboot"] = False

    # Ask the first reachable node which parameters carry the orphaning risk,
    # rather than keeping a second copy of that list here that could drift
    # away from the firmware's.
    risky = []
    for h in hosts:
        probe, err = call(h, psk, "GET")
        if probe:
            risky = [k for k in probe.get("requires_trial", []) if k in body]
            break
    if risky and not a.yes:
        print("These change WiFi association: %s" % ", ".join(sorted(risky)))
        print("Each node will reboot and revert automatically if it cannot")
        print("re-associate. %d node(s) affected." % len(hosts))
        if input("proceed? [y/N] ").strip().lower() != "y":
            return 1

    fail = 0
    for h in hosts:
        r, err = call(h, psk, "POST", body)
        if err:
            if risky and looks_armed(err):
                print("%-16s UNCONFIRMED  the reply was lost; the trial has very "
                      "likely armed." % h)
                print("%-16s              Do NOT re-push. Wait ~%ds: it either "
                      "re-associates or reverts itself."
                      % ("", (a.trial_seconds or 120) + 30))
                continue
            print("%-16s FAILED    %s" % (h, err))
            fail += 1
            continue
        n = r.get("changed", 0)
        if r.get("trial"):
            print("%-16s TRIAL     %d field(s), reverts in %ds unless it re-associates"
                  % (h, n, r.get("trial_seconds", 0)))
        elif r.get("note", "").startswith("applied immediately"):
            print("%-16s LIVE      %d field(s) applied now, no restart" % (h, n))
        elif r.get("rebooting"):
            print("%-16s OK        %d field(s), rebooting to apply" % (h, n))
        else:
            print("%-16s STAGED    %d field(s) written, NOT in effect until restart"
                  % (h, n))
    return 1 if fail else 0


if __name__ == "__main__":
    sys.exit(main())

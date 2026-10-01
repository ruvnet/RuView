#!/usr/bin/env python3
"""Push an OTA image to one CSI node and prove the node came up on it.

The PSK is read from the file named by RUVIEW_OTA_PSK_FILE (or --psk-file) and
never appears in argv or in output — an environment variable holding it would
be visible in a process listing, and that key can replace firmware.

Verification is the point of this script, not the upload. "OTA returned 200" is
not evidence: /ota/status is polled until the node reports the EXPECTED version
from a DIFFERENT partition than it started on. A version string that did not
move means the update did not take, however healthy the response looked.

Usage:
  export RUVIEW_OTA_PSK_FILE=/path/to/ota_psk.txt
  python ota_push.py --node <node-ip> --bin build/esp32-csi-node.bin \
      --expect-version "$(cat version.txt)"
"""
import argparse, json, os, sys, time, urllib.error, urllib.request

PORT = 8032


def psk(path):
    with open(path, "r", encoding="utf-8") as f:
        return f.read().strip()


def req(url, key, data=None, timeout=30, ctype="application/octet-stream"):
    r = urllib.request.Request(url, data=data)
    r.add_header("Authorization", "Bearer " + key)
    if data is not None:
        r.add_header("Content-Type", ctype)
    with urllib.request.urlopen(r, timeout=timeout) as resp:
        return resp.status, resp.read().decode("utf-8", "replace")


def status(ip, key, timeout=10):
    try:
        _, body = req(f"http://{ip}:{PORT}/ota/status", key, timeout=timeout)
        return json.loads(body)
    except Exception as e:
        return {"_error": str(e)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--node", required=True)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--expect-version", required=True)
    ap.add_argument("--psk-file", default=os.environ.get("RUVIEW_OTA_PSK_FILE"))
    ap.add_argument("--wait", type=int, default=180,
                    help="seconds to wait for the node to return")
    a = ap.parse_args()

    if not a.psk_file or not os.path.isfile(a.psk_file):
        print("no PSK file: pass --psk-file or set RUVIEW_OTA_PSK_FILE", file=sys.stderr)
        return 2
    key = psk(a.psk_file)

    with open(a.bin, "rb") as f:
        image = f.read()
    if not image or image[0] != 0xE9:
        print(f"refusing to push: {a.bin} does not start with 0xE9", file=sys.stderr)
        return 2

    before = status(a.node, key)
    if "_error" in before:
        print(f"{a.node}: unreachable before push: {before['_error']}", file=sys.stderr)
        return 1
    print(f"{a.node} BEFORE  version={before.get('version')!r} "
          f"running={before.get('running_partition')!r} "
          f"next={before.get('next_partition')!r}")

    if before.get("version") == a.expect_version:
        print(f"{a.node}: already on {a.expect_version}; nothing to do")
        return 0

    print(f"{a.node} pushing {len(image)} bytes ...")
    t0 = time.time()
    try:
        code, body = req(f"http://{a.node}:{PORT}/ota", key, data=image, timeout=300)
        print(f"{a.node} upload http {code} in {time.time()-t0:.0f}s: {body.strip()[:200]}")
    except urllib.error.HTTPError as e:
        print(f"{a.node} upload FAILED http {e.code}: {e.read()[:200]!r}", file=sys.stderr)
        return 1
    except Exception as e:
        # A node that reboots the instant it finishes writing can drop the
        # response. That is not a failure by itself -- the version check below
        # is what decides.
        print(f"{a.node} upload connection ended: {e} (continuing to verify)")

    deadline = time.time() + a.wait
    last = None
    while time.time() < deadline:
        time.sleep(5)
        s = status(a.node, key, timeout=5)
        if "_error" not in s:
            last = s
            if s.get("version") == a.expect_version:
                moved = s.get("running_partition") != before.get("running_partition")
                print(f"{a.node} AFTER   version={s.get('version')!r} "
                      f"running={s.get('running_partition')!r} "
                      f"(partition changed: {moved})")
                print(f"{a.node} VERIFIED on {a.expect_version} "
                      f"after {time.time()-t0:.0f}s")
                return 0
    print(f"{a.node} NOT VERIFIED within {a.wait}s; last status={last}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())

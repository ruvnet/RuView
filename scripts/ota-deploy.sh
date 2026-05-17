#!/usr/bin/env python3
"""
scripts/ota-deploy.sh — push esp32-csi-node.bin to one or more sensor nodes
over WiFi. Talks to the on-device /ota endpoint (ADR-045, port 8032,
handler in firmware/esp32-csi-node/main/ota_update.c).

Usage:
    scripts/ota-deploy.sh                  # auto-discover via ARP, deploy to all
    scripts/ota-deploy.sh 192.168.0.100    # one node
    scripts/ota-deploy.sh 192.168.0.100 192.168.0.101
    scripts/ota-deploy.sh --build          # idf.py build first, then deploy
    scripts/ota-deploy.sh --no-verify ...  # skip post-reboot /ota/status check

Auth: set env OTA_PSK=<token> to send "Authorization: Bearer <token>"
(matches the on-device check in ota_update.c::ota_check_auth).

Exit codes:
    0  — all targeted nodes confirmed running_partition flipped
    1  — one or more nodes failed verification or were unreachable
    2  — build or argument error
"""

from __future__ import annotations

import argparse
import concurrent.futures as cf
import json
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Iterable

REPO_ROOT = Path(__file__).resolve().parent.parent
FW_DIR    = REPO_ROOT / "firmware" / "esp32-csi-node"
BIN_PATH  = FW_DIR / "build" / "esp32-csi-node.bin"
PORT      = 8032

UPLOAD_TIMEOUT_S = 120
REBOOT_WAIT_S    = 10
VERIFY_RETRIES   = 6
VERIFY_DELAY_S   = 3


# ---- ANSI logging helpers ----------------------------------------------------
def _c(code: str, msg: str) -> str:
    if not sys.stdout.isatty():
        return msg
    return f"\033[{code}m{msg}\033[0m"

def log(msg: str)  -> None: print(_c("36", "[ota-deploy] ") + msg, flush=True)
def warn(msg: str) -> None: print(_c("33", "[ota-deploy] ") + msg, file=sys.stderr, flush=True)
def err(msg: str)  -> None: print(_c("31", "[ota-deploy] ") + msg, file=sys.stderr, flush=True)


# ---- helpers -----------------------------------------------------------------
def http_get(url: str, timeout: float = 4.0) -> str | None:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            return r.read().decode("utf-8", errors="replace")
    except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, OSError):
        return None


def get_ota_status(ip: str) -> dict | None:
    body = http_get(f"http://{ip}:{PORT}/ota/status")
    if not body:
        return None
    try:
        return json.loads(body)
    except json.JSONDecodeError:
        return None


def local_subnet_prefix() -> str | None:
    """Return e.g. '192.168.0' from en0 (macOS) or first non-loopback IP."""
    try:
        out = subprocess.check_output(
            ["ipconfig", "getifaddr", "en0"], stderr=subprocess.DEVNULL, text=True
        ).strip()
        if out:
            return out.rsplit(".", 1)[0]
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    # Linux fallback
    try:
        out = subprocess.check_output(["hostname", "-I"], text=True).strip()
        if out:
            return out.split()[0].rsplit(".", 1)[0]
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    return None


def discover_nodes() -> list[str]:
    """ARP-prefilter + parallel /ota/status probe to find live sensor nodes."""
    prefix = local_subnet_prefix()
    if not prefix:
        err("could not determine local /24 — pass node IPs explicitly")
        return []
    log(f"scanning {prefix}.0/24 for /ota/status responders ...")

    candidates: list[str] = []
    try:
        arp_out = subprocess.check_output(
            ["arp", "-a", "-n"], text=True, stderr=subprocess.DEVNULL
        )
        for line in arp_out.splitlines():
            m = re.search(rf"\(({re.escape(prefix)}\.\d+)\)", line)
            if m and "incomplete" not in line:
                ip = m.group(1)
                if not ip.endswith(".1"):  # skip gateway
                    candidates.append(ip)
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    if not candidates:
        warn(f"no ARP hits — falling back to {prefix}.100-110 ping sweep")
        candidates = [f"{prefix}.{i}" for i in range(100, 111)]
    candidates = sorted(set(candidates))

    found: list[str] = []
    with cf.ThreadPoolExecutor(max_workers=32) as pool:
        futs = {pool.submit(get_ota_status, ip): ip for ip in candidates}
        for fut in cf.as_completed(futs):
            ip = futs[fut]
            try:
                if fut.result():
                    found.append(ip)
            except Exception:
                pass
    return sorted(found, key=lambda x: tuple(int(o) for o in x.split(".")))


def upload_one(ip: str, payload: bytes, psk: str | None) -> tuple[bool, float, str]:
    """POST the firmware to one node. Returns (success, elapsed_s, body_snippet)."""
    req = urllib.request.Request(
        f"http://{ip}:{PORT}/ota",
        data=payload,
        headers={"Content-Type": "application/octet-stream"},
        method="POST",
    )
    if psk:
        req.add_header("Authorization", f"Bearer {psk}")
    t0 = time.monotonic()
    try:
        with urllib.request.urlopen(req, timeout=UPLOAD_TIMEOUT_S) as r:
            body = r.read().decode("utf-8", errors="replace")[:200]
            return True, time.monotonic() - t0, body
    except (urllib.error.HTTPError, urllib.error.URLError,
            TimeoutError, ConnectionResetError, OSError) as e:
        # ConnectionReset is *expected* when the chip restarts before flushing
        # the response. We treat it as a soft pass and verify via /ota/status.
        return (isinstance(e, ConnectionResetError),
                time.monotonic() - t0,
                f"{type(e).__name__}: {e}")


def build_firmware() -> int:
    log("building firmware via idf.py ...")
    if "IDF_PATH" not in os.environ:
        export = Path.home() / "esp" / "esp-idf-v5.2" / "export.sh"
        if not export.is_file():
            err("IDF_PATH not set and ~/esp/esp-idf-v5.2/export.sh not found")
            return 2
        # source the env in a child shell
        rc = subprocess.call(
            ["bash", "-lc", f". '{export}' >/dev/null 2>&1 && cd '{FW_DIR}' && idf.py build"]
        )
    else:
        rc = subprocess.call(["idf.py", "build"], cwd=str(FW_DIR))
    if rc != 0:
        err("build failed")
        return 2
    return 0


# ---- main --------------------------------------------------------------------
def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(
        prog="ota-deploy.sh",
        description="Push esp32-csi-node.bin to one or more sensor nodes over WiFi.",
    )
    ap.add_argument("targets", nargs="*",
                    help="node IPs; auto-discover if omitted")
    ap.add_argument("--build", action="store_true",
                    help="idf.py build before deploying")
    ap.add_argument("--no-verify", action="store_true",
                    help="skip post-reboot /ota/status confirmation")
    args = ap.parse_args(argv)

    if args.build:
        rc = build_firmware()
        if rc != 0:
            return rc

    if not BIN_PATH.is_file():
        err(f"firmware binary not found: {BIN_PATH} — pass --build first")
        return 2
    payload = BIN_PATH.read_bytes()
    log(f"firmware: {BIN_PATH}  ({len(payload)} bytes)")

    targets = args.targets or discover_nodes()
    if not targets:
        err("no nodes given and none discovered")
        return 1
    log(f"targets: {' '.join(targets)}")

    # snapshot before
    before: dict[str, str] = {}
    for ip in targets:
        st = get_ota_status(ip)
        if not st:
            warn(f"{ip}: not reachable before upload")
            before[ip] = "UNREACHABLE"
            continue
        before[ip] = st.get("running_partition", "UNKNOWN")
        log(f"{ip} before: running_partition={before[ip]}  time={st.get('time')}")

    psk = os.environ.get("OTA_PSK") or None
    if psk:
        log("OTA_PSK set — sending Bearer token")

    # upload in parallel
    log("uploading in parallel ...")
    results: dict[str, tuple[bool, float, str]] = {}
    with cf.ThreadPoolExecutor(max_workers=max(2, len(targets))) as pool:
        futs = {pool.submit(upload_one, ip, payload, psk): ip for ip in targets}
        for fut in cf.as_completed(futs):
            ip = futs[fut]
            ok, dt, body = fut.result()
            results[ip] = (ok, dt, body)
            tag = _c("32", "ok") if ok else _c("31", "ERR")
            log(f"{ip} upload {tag} in {dt:.1f}s  body={body[:120]}")

    if args.no_verify:
        log("--no-verify — done")
        return 0 if all(v[0] for v in results.values()) else 1

    # verify
    log(f"waiting {REBOOT_WAIT_S}s for reboot ...")
    time.sleep(REBOOT_WAIT_S)
    fail = False
    for ip in targets:
        new_st: dict | None = None
        for _ in range(VERIFY_RETRIES):
            new_st = get_ota_status(ip)
            if new_st:
                break
            time.sleep(VERIFY_DELAY_S)
        if not new_st:
            err(f"{ip}: not reachable after reboot — DEAD or panic loop")
            fail = True
            continue
        new_part = new_st.get("running_partition", "?")
        new_time = new_st.get("time", "?")
        if new_part == before.get(ip):
            err(f"{ip}: running_partition still {new_part} — OTA did NOT take "
                "(likely panic on first boot from new slot)")
            fail = True
        else:
            log(f"{ip}: {before[ip]} → {_c('32', new_part)} (time={new_time})  ✓")
    return 1 if fail else 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except KeyboardInterrupt:
        err("interrupted")
        sys.exit(130)

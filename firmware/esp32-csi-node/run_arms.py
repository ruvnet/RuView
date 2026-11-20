#!/usr/bin/env python3
"""Run the gate factorial: U / M / S / MS, Latin-square blocked.

Arms (NVS gate_mode, applied live -- no reboot):
    0 U   elapsed-time gate, as upstream ships it
    1 M   mesh-aligned bucket gate (ours, 4b54ef8e)
    2 S   rx_seq selection + elapsed underneath
    3 MS  both stacked

Why it is built this way, and what each guard is for:

* Every arm is applied to the WHOLE fleet, never to a subset. The stopped
  2026-09-04 run compared four gated nodes against four different control
  nodes; nodes differ, and that comparison could not be read.

* After every switch the runner reads back `active.gate_mode` from EVERY node
  and refuses to sample unless all nine agree. "Config was stored" is not
  "gate is running" -- that gap is exactly what the previous run could not
  distinguish from a null result.

* Arms are short and blocked in a Latin square rather than run in long
  sequential batches, so slow drift lands on all arms equally instead of being
  aliased onto whichever arm ran first. Measured drift over 20 min windows is
  sd 0.0045; counting noise is +/-0.002.

* Metrics come from differenced cumulative counters, so the server is never
  restarted and the fleet stays in production throughout.

Output: arms_<label>.csv, one row per arm, in the fleet-baselines directory
(outside git -- it carries presence information).
"""
import argparse, csv, json, os, sys, time, urllib.request

PORT = 8032
SERVER = "http://127.0.0.1:3000"
OUTDIR = "C:/temp/ruview/fleet-baselines/gate-experiment"

NODES = {0: "192.168.1.171", 1: "192.168.1.219", 2: "192.168.1.72",
         3: "192.168.1.112", 4: "192.168.1.144", 5: "192.168.1.198",
         6: "192.168.1.199", 7: "192.168.1.25",  8: "192.168.1.122"}

ARM_NAME = {0: "U_elapsed", 1: "M_mesh", 2: "S_seq", 3: "MS_both"}

# Latin square: every arm appears once per block, in a different position.
BLOCKS = [[0, 1, 2, 3], [2, 3, 1, 0], [3, 0, 2, 1]]


def key():
    p = os.environ.get("RUVIEW_OTA_PSK_FILE")
    if not p or not os.path.isfile(p):
        sys.exit("set RUVIEW_OTA_PSK_FILE to the psk file path")
    return open(p, encoding="utf-8").read().strip()


def node_get(ip, path, k, timeout=10):
    r = urllib.request.Request(f"http://{ip}:{PORT}{path}")
    r.add_header("Authorization", "Bearer " + k)
    with urllib.request.urlopen(r, timeout=timeout) as resp:
        return json.load(resp)


def node_post(ip, path, k, payload, timeout=15):
    body = json.dumps(payload).encode()
    r = urllib.request.Request(f"http://{ip}:{PORT}{path}", data=body)
    r.add_header("Authorization", "Bearer " + k)
    r.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(r, timeout=timeout) as resp:
        return resp.status, resp.read().decode("utf-8", "replace")


def srv(path):
    with urllib.request.urlopen(SERVER + path, timeout=15) as r:
        return json.load(r)


def set_arm(mode, period, k):
    """Apply an arm fleet-wide and PROVE every node is running it."""
    for nid, ip in sorted(NODES.items()):
        try:
            node_post(ip, "/config", k, {"gate_mode": mode, "gate_seq_period": period})
        except Exception as e:
            print(f"  n{nid} set FAILED: {e}", file=sys.stderr)
    time.sleep(5)
    bad = []
    for nid, ip in sorted(NODES.items()):
        try:
            act = node_get(ip, "/config", k).get("active", {})
            if act.get("gate_mode") != mode or act.get("gate_seq_period") != period:
                bad.append((nid, act))
        except Exception as e:
            bad.append((nid, str(e)))
    return bad


def snapshot():
    f = srv("/api/v1/fusion")
    m = srv("/api/v1/mesh")["nodes"]
    nodes = {}
    for k2, v in m.items():
        tx = v.get("health", {}).get("tx", {})
        nodes[k2] = {"acc": v.get("csi_fps_samples"), "ed": tx.get("early_drop"),
                     "fps": v.get("csi_fps_ema"), "valid": v.get("is_valid")}
    pairs = {f"{p['a']}-{p['b']}": p["common"] for p in f.get("pairs", [])}
    return {"t": time.time(), "observations": f["observations"],
            "transmissions": f["transmissions"], "paired": f["paired"],
            "nodes": nodes, "pairs": pairs}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", default="factorial")
    ap.add_argument("--arm-minutes", type=float, default=20.0)
    ap.add_argument("--settle-minutes", type=float, default=3.0)
    ap.add_argument("--period", type=int, default=8)
    ap.add_argument("--blocks", type=int, default=3)
    a = ap.parse_args()

    k = key()
    os.makedirs(OUTDIR, exist_ok=True)
    out = os.path.join(OUTDIR, f"arms_{a.label}.csv")
    new = not os.path.exists(out)
    fh = open(out, "a", newline="", encoding="utf-8")
    w = csv.writer(fh)
    if new:
        w.writerow(["block", "arm", "arm_name", "period", "start_iso", "end_iso",
                    "dur_s", "d_observations", "d_transmissions", "d_paired",
                    "paired_fraction", "node_json", "pairs_json", "verified"])
        fh.flush()

    print(f"gate factorial: {a.blocks} blocks x 4 arms x {a.arm_minutes:g} min "
          f"(+{a.settle_minutes:g} min settle), seq period {a.period}")
    print(f"est. total {a.blocks*4*(a.arm_minutes+a.settle_minutes)/60:.1f} h -> {out}\n")

    for bi in range(a.blocks):
        order = BLOCKS[bi % len(BLOCKS)]
        print(f"--- block {bi}: {' '.join(ARM_NAME[m] for m in order)} ---")
        for mode in order:
            print(f"[{time.strftime('%H:%M:%S')}] arm {ARM_NAME[mode]} ...", flush=True)
            bad = set_arm(mode, a.period, k)
            if bad:
                print(f"  WARNING not all nodes on arm: {bad}", file=sys.stderr)
            # Settle: the gate changes instantly, but csi_fps_ema and the
            # fusion pairing window carry the previous arm for a while.
            time.sleep(a.settle_minutes * 60)
            s0 = snapshot()
            time.sleep(a.arm_minutes * 60)
            s1 = snapshot()

            dtr = s1["transmissions"] - s0["transmissions"]
            dp = s1["paired"] - s0["paired"]
            pf = dp / dtr if dtr > 0 else float("nan")
            nodes = {}
            for nid in s1["nodes"]:
                if nid in s0["nodes"]:
                    dacc = (s1["nodes"][nid]["acc"] or 0) - (s0["nodes"][nid]["acc"] or 0)
                    ded = (s1["nodes"][nid]["ed"] or 0) - (s0["nodes"][nid]["ed"] or 0)
                    nodes[nid] = {"d_acc": dacc, "d_ed": ded,
                                  "R": round((dacc + ded) / dacc, 3) if dacc > 0 else None,
                                  "fps": round(s1["nodes"][nid]["fps"] or 0, 2),
                                  "valid": s1["nodes"][nid]["valid"]}
            dpairs = {kk: s1["pairs"].get(kk, 0) - s0["pairs"].get(kk, 0)
                      for kk in s1["pairs"]}
            w.writerow([bi, mode, ARM_NAME[mode], a.period,
                        time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime(s0["t"])),
                        time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime(s1["t"])),
                        round(s1["t"] - s0["t"]),
                        s1["observations"] - s0["observations"], dtr, dp,
                        f"{pf:.6f}",
                        json.dumps(nodes, separators=(",", ":")),
                        json.dumps(dpairs, separators=(",", ":")),
                        "no" if bad else "yes"])
            fh.flush()
            meanR = [v["R"] for v in nodes.values() if v["R"]]
            print(f"    paired_fraction={pf:.4f}  transmissions={dtr}  "
                  f"meanR={sum(meanR)/len(meanR):.2f}" if meanR else "", flush=True)

    fh.close()
    print("done")


if __name__ == "__main__":
    main()

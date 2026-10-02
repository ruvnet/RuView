#!/usr/bin/env python3
"""Cross-chain phase stability of MediaTek mt76 vendor CSI (offline, read-only).

Reproducer for the MEASURED figures in docs/mediatek-router-csi.md. Input is a
capture made on the router by repeated `mt76-vendor <iface> dump csi <n> <file>`,
one dump (a JSON array of 13/14-element records) per line. Captures hold client
transmitter addresses and raw channel samples: keep them out of git. This script
never prints an address; transmitters are reported as TX-A, TX-B, ...

For every HT (rx_mode 2) frame that carries both receive chains of transmit
stream 0, it compares:
  raw     per-chain phase arg(H0[k]) between adjacent frames, and
  cross   cross-chain phase arg(H0[k] * conj(H1[k])), per bin and summed
          coherently over bins.
CFO, SFO and the per-packet phase offset are common to both chains of one
radio, so they cancel in the cross term. Adjacent means the frames are under
100 ms apart. Steps are wrapped to [-pi, pi], so p99 can never exceed pi; read
p50 against pi/2 (1.571 rad), which is what uniformly random phase gives.

usage: python3 mediatek-csi-phase-stability.py <capture.jsonl> [max_MB]
"""
import json
import sys

import numpy as np

U32 = 1 << 32
MAX_DT = 0.1  # s; consecutive frames closer than this count as adjacent
GUARD = set(range(29, 36))  # BW20 bins 29..35: guard tones
VALID = np.array([k for k in range(1, 64) if k not in GUARD])  # DC excluded
KPHYS = np.where(VALID < 32, VALID, VALID - 64).astype(float)


def iter_lines(path, max_bytes=None):
    read = 0
    with open(path, "rb") as fh:
        for raw in fh:
            read += len(raw)
            raw = raw.strip()
            if raw:
                try:
                    yield json.loads(raw)
                except json.JSONDecodeError:
                    pass  # truncated tail line
            if max_bytes and read >= max_bytes:
                return


def frames_from_line(recs, txidx):
    """Group a dump's records into PPDUs: per transmitter, consecutive records
    sharing ts, closed on chain_info BIT15 or a repeated (tx, rx) slot."""
    frames, open_ = [], {}
    for r in recs:
        ts = r[0] % U32
        if r[1] not in txidx:
            n = len(txidx)
            txidx[r[1]] = "TX-" + chr(ord("A") + n) if n < 26 else f"TX-{n}"
        tx = txidx[r[1]]
        key = (r[7], r[8])
        h = np.asarray(r[11], np.float64) + 1j * np.asarray(r[12], np.float64)
        cur = open_.get(tx)
        if cur is not None and (cur["ts"] != ts or key in cur["H"]):
            frames.append(cur)
            cur = None
        if cur is None:
            cur = {"tx": tx, "ts": ts, "rx_mode": r[6], "H": {}}
            open_[tx] = cur
        cur["H"][key] = h
        if r[9] & (1 << 15):
            frames.append(cur)
            del open_[tx]
    frames.extend(open_.values())
    return frames


def load(path, max_bytes=None):
    """Per transmitter: unwrapped time (s) and chains (0,0), (0,1) of HT frames.
    Within a dump the signed ts delta is used; between dumps the unsigned delta
    mod 2^32, so a gap over 71.6 min between dumps would alias."""
    txidx, per = {}, {}
    t_unwrapped, prev_ts = None, None
    for recs in iter_lines(path, max_bytes):
        first = True
        for f in frames_from_line(recs, txidx):
            ts = f["ts"]
            if prev_ts is None:
                t_unwrapped = 0
            elif first:
                t_unwrapped += (ts - prev_ts) % U32
            else:
                t_unwrapped += ((ts - prev_ts + (U32 >> 1)) % U32) - (U32 >> 1)
            prev_ts, first = ts, False
            H = f["H"]
            if (0, 0) not in H or (0, 1) not in H or f["rx_mode"] != 2:
                continue
            d = per.setdefault(f["tx"], {"t": [], "h0": [], "h1": []})
            d["t"].append(t_unwrapped / 1e6)
            d["h0"].append(H[(0, 0)][VALID].astype(np.complex64))
            d["h1"].append(H[(0, 1)][VALID].astype(np.complex64))
    out = {}
    for tx, d in per.items():
        t = np.asarray(d["t"])
        srt = np.argsort(t, kind="stable")
        out[tx] = {"t": t[srt], "h0": np.vstack(d["h0"])[srt], "h1": np.vstack(d["h1"])[srt]}
    return out


def wrap(x):
    return (x + np.pi) % (2 * np.pi) - np.pi


def sanitize(phase_rows):
    """Remove per-frame linear phase (offset + slope over subcarrier index)."""
    order = np.argsort(KPHYS)
    k = KPHYS[order]
    u = np.unwrap(phase_rows[:, order], axis=1)
    A = np.vstack([k, np.ones_like(k)]).T
    coef, *_ = np.linalg.lstsq(A, u.T, rcond=None)
    res = u - (A @ coef).T
    out = np.empty_like(res)
    out[:, order] = res
    return out


def step_stats(phase, t):
    dt = np.diff(t)
    ok = (dt > 0) & (dt < MAX_DT)
    st = np.abs(wrap(np.diff(phase, axis=0)))[ok].ravel()
    if st.size == 0:
        return None
    p = np.percentile(st, [50, 90, 99])
    return {"n": int(st.size), "p50": round(float(p[0]), 3), "p90": round(float(p[1]), 3),
            "p99": round(float(p[2]), 3), "frac_gt_pi_2": round(float((st > np.pi / 2).mean()), 4)}


def analyse(d):
    t, h0, h1 = d["t"], d["h0"].astype(complex), d["h1"].astype(complex)
    raw0 = np.angle(h0)
    x = h0 * np.conj(h1)
    dt = np.diff(t)
    adj = dt[(dt > 0) & (dt < MAX_DT)]
    return {
        "frames": int(len(t)),
        "span_s": round(float(t[-1] - t[0]), 1),
        "adjacent_pairs": int(adj.size),
        "adjacent_dt_ms_p50": round(float(np.median(adj) * 1e3), 2) if adj.size else None,
        "steps": {
            "raw_chain0_per_bin": step_stats(raw0, t),
            "sanitized_chain0_per_bin": step_stats(sanitize(raw0), t),
            "cross_per_bin": step_stats(np.angle(x), t),
            "cross_coherent_sum": step_stats(np.angle(x.sum(1)), t),
        },
    }


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    path = sys.argv[1]
    mb = float(sys.argv[2]) if len(sys.argv) > 2 else None
    data = load(path, int(mb * 1e6) if mb else None)
    out = {"sampled_MB": mb or "all", "tx": {}}
    for tx, d in sorted(data.items()):
        if len(d["t"]) >= 50:
            out["tx"][tx] = analyse(d)
    print(json.dumps(out, indent=1))

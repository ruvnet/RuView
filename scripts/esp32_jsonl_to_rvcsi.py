#!/usr/bin/env python3
"""Transcode an ESP32 .csi.jsonl recording into a .rvcsi capture (JSONL).

This is the moral equivalent of `rvcsi record --source esp32-jsonl` (which the
PR does not ship yet): parse each ESP32 frame, derive amplitude/phase from the
raw int8 I/Q pairs, run the same validation/quality logic rvcsi_core does, and
write a .rvcsi file whose first line is a CaptureHeader and every later line a
CsiFrame.  Rejected frames are dropped (quarantine), like the real pipeline.

Usage: esp32_jsonl_to_rvcsi.py <in.csi.jsonl> <out.rvcsi> [--limit N]
"""
import json
import math
import os
import sys

# --- rvcsi_core::ValidationPolicy::default() -------------------------------
MIN_SUBCARRIERS = 1
MAX_SUBCARRIERS = 4096
RSSI_LO, RSSI_HI = -110, 0
MIN_QUALITY = 0.25
RSSI_HARD_MARGIN = 30


def quality_and_status(amplitude, rssi_dbm):
    """Faithful port of rvcsi_core::validation::validate_frame soft scoring."""
    reasons = []
    q = 1.0
    sc = len(amplitude)
    # out-of-range (non-fatal) RSSI
    if rssi_dbm is not None and (rssi_dbm < RSSI_LO or rssi_dbm > RSSI_HI):
        q *= 0.6
        reasons.append(f"rssi {rssi_dbm} dBm outside [{RSSI_LO},{RSSI_HI}]")
    # dead subcarriers
    dead = sum(1 for a in amplitude if a < 1e-6)
    if dead > 0:
        frac = dead / max(sc, 1)
        q *= max(1.0 - frac, 0.05)
        reasons.append(f"{dead}/{sc} dead subcarriers")
    # amplitude spike vs median
    if sc >= 3:
        s = sorted(amplitude)
        median = max(s[sc // 2], 1e-9)
        mx = s[-1]
        if mx > median * 50.0:
            q *= 0.7
            reasons.append(f"amplitude spike: max {mx:.3f} vs median {median:.3f}")
    if rssi_dbm is None:
        q *= 0.95
        reasons.append("missing rssi")
    q = min(max(q, 0.0), 1.0)
    if q < MIN_QUALITY:
        status = "Degraded"          # degrade_instead_of_reject = true
    else:
        status = "Accepted"
    return q, status, reasons


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(2)
    in_path, out_path = sys.argv[1], sys.argv[2]
    # Check file identity before opening output: aliases can share one inode.
    if os.path.exists(out_path) and os.path.samefile(in_path, out_path):
        print("Input and output must refer to different files.", file=sys.stderr)
        sys.exit(2)
    limit = None
    if "--limit" in sys.argv:
        limit = int(sys.argv[sys.argv.index("--limit") + 1])

    source_id = "esp32-com7-rec"
    header = {
        "rvcsi_capture_version": 1,
        "session_id": 0,
        "source_id": source_id,
        "adapter_profile": {
            "adapter_kind": "Esp32",
            "chip": "ESP32-S3",
            "firmware_version": None,
            "driver_version": None,
            "supported_channels": [],
            "supported_bandwidths_mhz": [],
            "expected_subcarrier_counts": [],
            "supports_live_capture": True,
            "supports_injection": False,
            "supports_monitor_mode": False,
        },
        "validation_policy": {
            "min_subcarriers": MIN_SUBCARRIERS,
            "max_subcarriers": MAX_SUBCARRIERS,
            "rssi_dbm_bounds": [RSSI_LO, RSSI_HI],
            "strict_monotonic_time": False,
            "degrade_instead_of_reject": True,
            "min_quality": MIN_QUALITY,
        },
        "calibration_version": None,
        "runtime_config_json": "{}",
        "created_unix_ns": 0,
    }

    stats = {
        "read": 0, "written": 0,
        "rej_len": 0, "rej_sc": 0, "rej_nonfinite": 0, "rej_rssi": 0,
        "accepted": 0, "degraded": 0,
    }
    sc_hist = {}
    out = open(out_path, "w", newline="\n")
    out.write(json.dumps(header, separators=(",", ":")) + "\n")
    fid = 0
    with open(in_path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            d = json.loads(line)
            if d.get("type") != "raw_csi":
                continue
            stats["read"] += 1
            if limit is not None and stats["read"] > limit:
                stats["read"] -= 1
                break
            iq_hex = d.get("iq_hex", "")
            try:
                raw = bytes.fromhex(iq_hex)
            except (TypeError, ValueError):
                stats["rej_len"] += 1
                continue
            # A partial I/Q pair cannot be reconstructed from this recording.
            if len(raw) % 2:
                stats["rej_len"] += 1
                continue
            n_pairs = len(raw) // 2
            # ESP-IDF CSI buffer layout: [imag0, real0, imag1, real1, ...] as int8
            i_vals, q_vals, amp, ph = [], [], [], []
            for k in range(n_pairs):
                imag = raw[2 * k]
                real = raw[2 * k + 1]
                if imag >= 128:
                    imag -= 256
                if real >= 128:
                    real -= 256
                fi, fq = float(real), float(imag)
                i_vals.append(fi)
                q_vals.append(fq)
                amp.append(math.sqrt(fi * fi + fq * fq))
                ph.append(math.atan2(fq, fi))
            sc = n_pairs
            sc_hist[sc] = sc_hist.get(sc, 0) + 1
            # hard checks (mirror validate_frame)
            if sc < MIN_SUBCARRIERS or sc > MAX_SUBCARRIERS:
                stats["rej_sc"] += 1
                continue
            # int8 -> always finite, lengths consistent by construction
            # RSSI: the v1 collector's rssi byte is unreliable (sentinels 64/-128
            # etc.); only carry it through when it lands in a plausible band,
            # otherwise leave it None (a small quality penalty, not a reject).
            r = d.get("rssi")
            rssi_dbm = r if (isinstance(r, int) and -140 <= r <= 30) else None
            if rssi_dbm is not None and (rssi_dbm < RSSI_LO - RSSI_HARD_MARGIN or rssi_dbm > RSSI_HI + RSSI_HARD_MARGIN):
                stats["rej_rssi"] += 1
                continue
            if rssi_dbm is not None and not (-110 <= rssi_dbm <= 0):
                rssi_dbm = None  # implausible but not insane -> drop the field
            q, status, reasons = quality_and_status(amp, rssi_dbm)
            ch = d.get("channel", 0) or 0
            frame = {
                "frame_id": fid,
                "session_id": 0,
                "source_id": source_id,
                "adapter_kind": "Esp32",
                "timestamp_ns": int(d.get("ts_ns", 0)),
                "channel": int(ch),
                "bandwidth_mhz": 20,
                "rssi_dbm": rssi_dbm,
                "noise_floor_dbm": None,
                "antenna_index": 0,
                "tx_chain": None,
                "rx_chain": None,
                "subcarrier_count": sc,
                "i_values": i_vals,
                "q_values": q_vals,
                "amplitude": amp,
                "phase": ph,
                "validation": status,
                "quality_score": q,
            }
            if reasons:
                frame["quality_reasons"] = reasons
            frame["calibration_version"] = None
            out.write(json.dumps(frame, separators=(",", ":")) + "\n")
            fid += 1
            stats["written"] += 1
            stats[status.lower()] = stats.get(status.lower(), 0) + 1
    out.close()
    print("transcode stats:", json.dumps(stats))
    print("subcarrier-count histogram:", json.dumps(dict(sorted(sc_hist.items(), key=lambda x: -x[1]))))


if __name__ == "__main__":
    main()

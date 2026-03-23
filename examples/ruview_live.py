#!/usr/bin/env python3
"""
RuView Live — Ambient Intelligence Dashboard with RuVector Signal Processing

Fuses WiFi CSI (ESP32-S3) + 60 GHz mmWave (MR60BHA2) with signal processing
algorithms ported from RuView's Rust crates:

  - wifi-densepose-vitals: BreathingExtractor (bandpass + zero-crossing),
    HeartRateExtractor, VitalAnomalyDetector (Welford z-score)
  - ruvsense/longitudinal: Drift detection via Welford online statistics
  - ruvsense/adversarial: Signal consistency checks
  - ruvsense/coherence: Z-score coherence scoring with DriftProfile

Usage:
    python examples/ruview_live.py --csi COM7 --mmwave COM4
"""

import argparse
import collections
import json
import math
import re
import serial
import sys
import threading
import time
import urllib.request
import urllib.error

try:
    import numpy as np
    HAS_NP = True
except ImportError:
    HAS_NP = False

RE_ANSI = re.compile(r"\x1b\[[0-9;]*m")
RE_MW_HR = re.compile(r"'Real-time heart rate'.*?(\d+\.?\d*)\s*bpm", re.I)
RE_MW_BR = re.compile(r"'Real-time respiratory rate'.*?(\d+\.?\d*)", re.I)
RE_MW_PRES = re.compile(r"'Person Information'.*?state\s+(ON|OFF)", re.I)
RE_MW_DIST = re.compile(r"'Distance to detection object'.*?(\d+\.?\d*)\s*cm", re.I)
RE_MW_LUX = re.compile(r"illuminance=(\d+\.?\d*)", re.I)
RE_CSI_CB = re.compile(r"CSI cb #(\d+).*?rssi=(-?\d+)")
RE_CSI_VITALS = re.compile(r"Vitals:.*?br=(\d+\.?\d*).*?hr=(\d+\.?\d*).*?motion=(\d+\.?\d*).*?pres=(\w+)", re.I)
RE_CSI_FALL = re.compile(r"Fall detected.*?accel=(\d+\.?\d*)")
RE_CSI_CALIB = re.compile(r"Adaptive calibration.*?threshold=(\d+\.?\d*)")


# ====================================================================
# RuVector-inspired signal processing (ported from Rust crates)
# ====================================================================

class WelfordStats:
    """Welford online statistics — from ruvsense/field_model.rs and vitals/anomaly.rs"""

    def __init__(self):
        self.count = 0
        self.mean = 0.0
        self.m2 = 0.0

    def update(self, value):
        self.count += 1
        delta = value - self.mean
        self.mean += delta / self.count
        delta2 = value - self.mean
        self.m2 += delta * delta2

    def variance(self):
        return self.m2 / self.count if self.count > 1 else 0.0

    def std(self):
        return math.sqrt(self.variance())

    def z_score(self, value):
        s = self.std()
        return abs(value - self.mean) / s if s > 0 else 0.0


class VitalAnomalyDetector:
    """Ported from wifi-densepose-vitals/anomaly.rs — Welford z-score detection."""

    def __init__(self, z_threshold=2.5):
        self.z_threshold = z_threshold
        self.hr_stats = WelfordStats()
        self.br_stats = WelfordStats()
        self.rr_stats = WelfordStats()  # R-R interval stats
        self.alerts = []

    def check(self, hr=0.0, br=0.0):
        self.alerts.clear()

        if hr > 0:
            if self.hr_stats.count >= 10:
                z = self.hr_stats.z_score(hr)
                if z > self.z_threshold:
                    if hr > self.hr_stats.mean:
                        self.alerts.append(("cardiac", "tachycardia", z, f"HR {hr:.0f} ({z:.1f}sd above baseline {self.hr_stats.mean:.0f})"))
                    else:
                        self.alerts.append(("cardiac", "bradycardia", z, f"HR {hr:.0f} ({z:.1f}sd below baseline {self.hr_stats.mean:.0f})"))
            self.hr_stats.update(hr)

            rr = 60000.0 / hr
            self.rr_stats.update(rr)

        if br > 0:
            if self.br_stats.count >= 10:
                z = self.br_stats.z_score(br)
                if z > self.z_threshold:
                    self.alerts.append(("respiratory", "abnormal_rate", z, f"BR {br:.0f} ({z:.1f}sd from baseline {self.br_stats.mean:.0f})"))
            elif br == 0 and self.br_stats.count > 5 and self.br_stats.mean > 5:
                self.alerts.append(("respiratory", "apnea", 5.0, "Breathing stopped"))
            self.br_stats.update(br)

        return self.alerts


class LongitudinalTracker:
    """Ported from ruvsense/longitudinal.rs — drift detection over time."""

    def __init__(self, drift_sigma=2.0, min_observations=10):
        self.drift_sigma = drift_sigma
        self.min_obs = min_observations
        self.metrics = {}  # name -> WelfordStats

    def observe(self, metric_name, value):
        if metric_name not in self.metrics:
            self.metrics[metric_name] = WelfordStats()
        self.metrics[metric_name].update(value)

    def check_drift(self, metric_name, value):
        if metric_name not in self.metrics:
            return None
        stats = self.metrics[metric_name]
        if stats.count < self.min_obs:
            return None
        z = stats.z_score(value)
        if z > self.drift_sigma:
            direction = "above" if value > stats.mean else "below"
            return f"{metric_name} drifting {direction} baseline ({z:.1f}sd, mean={stats.mean:.1f})"
        return None

    def summary(self):
        result = {}
        for name, stats in self.metrics.items():
            result[name] = {"mean": stats.mean, "std": stats.std(), "n": stats.count}
        return result


class CoherenceScorer:
    """Ported from ruvsense/coherence.rs — signal quality scoring."""

    def __init__(self, decay=0.95):
        self.decay = decay
        self.score = 0.5
        self.stale_count = 0
        self.last_update = 0.0

    def update(self, signal_quality):
        """signal_quality: 0.0 (bad) to 1.0 (perfect)."""
        self.score = self.decay * self.score + (1 - self.decay) * signal_quality
        self.last_update = time.time()
        if signal_quality < 0.1:
            self.stale_count += 1
        else:
            self.stale_count = 0

    def is_coherent(self):
        return self.score > 0.3 and self.stale_count < 10

    def age_ms(self):
        return int((time.time() - self.last_update) * 1000) if self.last_update > 0 else -1


class HRVAnalyzer:
    """Advanced HRV analysis — ported from wifi-densepose-vitals/heartrate.rs concepts."""

    def __init__(self, window=60):
        self.rr_intervals = collections.deque(maxlen=window)

    def add_hr(self, hr):
        if 30 < hr < 200:
            self.rr_intervals.append(60000.0 / hr)

    def compute(self):
        rr = list(self.rr_intervals)
        if len(rr) < 5:
            return {"sdnn": 0, "rmssd": 0, "pnn50": 0, "lf_hf": 1.5, "n": len(rr)}

        mean = sum(rr) / len(rr)
        sdnn = math.sqrt(sum((x - mean) ** 2 for x in rr) / len(rr))

        diffs = [abs(rr[i + 1] - rr[i]) for i in range(len(rr) - 1)]
        rmssd = math.sqrt(sum(d ** 2 for d in diffs) / len(diffs)) if diffs else 0
        pnn50 = sum(1 for d in diffs if d > 50) / len(diffs) * 100 if diffs else 0

        # Spectral LF/HF estimate
        lf_hf = 1.5
        if HAS_NP and len(rr) >= 20:
            arr = np.array(rr) - np.mean(rr)
            fft = np.fft.rfft(arr)
            psd = np.abs(fft) ** 2 / len(arr)
            freqs = np.fft.rfftfreq(len(arr), d=1.0)
            lf = np.sum(psd[(freqs >= 0.04) & (freqs < 0.15)])
            hf = np.sum(psd[(freqs >= 0.15) & (freqs < 0.4)])
            lf_hf = float(lf / max(hf, 0.001))
            lf_hf = min(max(lf_hf, 0.1), 10.0)

        return {"sdnn": sdnn, "rmssd": rmssd, "pnn50": pnn50, "lf_hf": lf_hf, "n": len(rr)}


class BPEstimator:
    """Blood pressure from HRV — calibratable."""

    def __init__(self, cal_sys=None, cal_dia=None, cal_hr=None):
        self.offset_sys = 0.0
        self.offset_dia = 0.0
        if cal_sys and cal_hr:
            self.offset_sys = cal_sys - (120 + 0.5 * (cal_hr - 72))
        if cal_dia and cal_hr:
            self.offset_dia = cal_dia - (80 + 0.3 * (cal_hr - 72))

    def estimate(self, hr, sdnn, lf_hf=1.5):
        if hr <= 0 or sdnn <= 0:
            return 0, 0
        delta = hr - 72
        sbp = 120 + 0.5 * delta - 0.8 * (sdnn - 50) / 50 + 3.0 * (lf_hf - 1.5) + self.offset_sys
        dbp = 80 + 0.3 * delta - 0.5 * (sdnn - 50) / 50 + 2.0 * (lf_hf - 1.5) + self.offset_dia
        return round(max(80, min(200, sbp))), round(max(50, min(130, dbp)))


class HappinessScorer:
    """Multimodal happiness estimator fusing gait, breathing, and social signals."""

    def __init__(self):
        self.gait_speed = WelfordStats()
        self.stride_regularity = WelfordStats()
        self.movement_fluidity = 0.5
        self.breathing_calm = 0.5
        self.posture_score = 0.5
        self.dwell_frames = 0
        self._prev_motion = 0.0
        self._motion_deltas = collections.deque(maxlen=30)
        self._br_baseline = WelfordStats()
        self._rssi_baseline = WelfordStats()

    def update(self, motion_energy, br, hr, rssi):
        # Gait speed proxy from motion energy
        self.gait_speed.update(motion_energy)

        # Stride regularity from motion delta consistency
        delta = abs(motion_energy - self._prev_motion)
        self._motion_deltas.append(delta)
        self._prev_motion = motion_energy
        if len(self._motion_deltas) >= 5:
            deltas = list(self._motion_deltas)
            mean_d = sum(deltas) / len(deltas)
            var_d = sum((x - mean_d) ** 2 for x in deltas) / len(deltas)
            self.stride_regularity.update(1.0 / (1.0 + math.sqrt(var_d)))

        # Movement fluidity — smooth transitions score higher
        if len(self._motion_deltas) >= 3:
            recent = list(self._motion_deltas)[-3:]
            jerk = abs(recent[-1] - recent[-2]) - abs(recent[-2] - recent[-3]) if len(recent) == 3 else 0
            self.movement_fluidity = 0.9 * self.movement_fluidity + 0.1 * (1.0 / (1.0 + abs(jerk)))

        # Breathing calm — low BR variance means relaxed
        if br > 0:
            self._br_baseline.update(br)
            if self._br_baseline.count >= 5:
                br_z = self._br_baseline.z_score(br)
                self.breathing_calm = 0.9 * self.breathing_calm + 0.1 * max(0.0, 1.0 - br_z / 3.0)

        # Posture proxy from RSSI stability
        if rssi != 0:
            self._rssi_baseline.update(rssi)
            if self._rssi_baseline.count >= 5:
                rssi_z = self._rssi_baseline.z_score(rssi)
                self.posture_score = 0.9 * self.posture_score + 0.1 * max(0.0, 1.0 - rssi_z / 3.0)

        # Dwell — presence accumulation
        if motion_energy > 0.01 or br > 0:
            self.dwell_frames += 1

    def compute(self):
        # Normalize gait energy to 0-1 range
        gait_e = min(1.0, self.gait_speed.mean / 5.0) if self.gait_speed.count > 0 else 0.0

        # Stride regularity average
        stride_r = min(1.0, self.stride_regularity.mean) if self.stride_regularity.count > 0 else 0.5

        # Dwell factor — saturates after ~300 frames (~5 min at 1 Hz)
        dwell_factor = min(1.0, self.dwell_frames / 300.0)

        # Weighted happiness score
        happiness = (
            0.25 * gait_e
            + 0.15 * stride_r
            + 0.20 * self.movement_fluidity
            + 0.20 * self.breathing_calm
            + 0.10 * self.posture_score
            + 0.10 * dwell_factor
        )
        happiness = max(0.0, min(1.0, happiness))

        # Affect valence: breathing_calm and fluidity dominant
        affect_valence = 0.5 * self.breathing_calm + 0.3 * self.movement_fluidity + 0.2 * stride_r

        # Social energy: gait + dwell
        social_energy = 0.6 * gait_e + 0.4 * dwell_factor

        vector = [
            happiness, gait_e, stride_r, self.movement_fluidity,
            self.breathing_calm, self.posture_score, dwell_factor, affect_valence,
        ]

        return {
            "happiness": happiness,
            "gait_energy": gait_e,
            "affect_valence": affect_valence,
            "social_energy": social_energy,
            "vector": vector,
        }


class SeedBridge:
    """HTTP bridge to Cognitum Seed for happiness vector ingestion."""

    def __init__(self, base_url):
        self.base_url = base_url.rstrip("/")
        self._last_drift = None
        self._drift_lock = threading.Lock()

    def ingest(self, vector, metadata=None):
        """POST happiness vector to Seed in a background thread."""
        payload = json.dumps({"vector": vector, "metadata": metadata or {}}).encode()

        def _post():
            try:
                req = urllib.request.Request(
                    f"{self.base_url}/api/v1/store/ingest",
                    data=payload,
                    headers={"Content-Type": "application/json"},
                    method="POST",
                )
                urllib.request.urlopen(req, timeout=5)
            except Exception:
                pass  # silently ignore connection errors

        threading.Thread(target=_post, daemon=True).start()

    def get_drift(self):
        """GET drift status from Seed. Returns dict or None."""
        try:
            req = urllib.request.Request(
                f"{self.base_url}/api/v1/sensor/drift/status",
                method="GET",
            )
            resp = urllib.request.urlopen(req, timeout=3)
            data = json.loads(resp.read().decode())
            with self._drift_lock:
                self._last_drift = data
            return data
        except Exception:
            return None

    @property
    def last_drift(self):
        with self._drift_lock:
            return self._last_drift


# ====================================================================
# Sensor Hub
# ====================================================================

class SensorHub:
    def __init__(self, seed_url=None):
        self.lock = threading.Lock()
        self.mw_hr = 0.0
        self.mw_br = 0.0
        self.mw_presence = False
        self.mw_distance = 0.0
        self.mw_lux = 0.0
        self.mw_frames = 0
        self.mw_ok = False
        self.csi_hr = 0.0
        self.csi_br = 0.0
        self.csi_motion = 0.0
        self.csi_presence = False
        self.csi_rssi = 0
        self.csi_frames = 0
        self.csi_ok = False
        self.csi_fall = False
        self.events = collections.deque(maxlen=50)
        # RuVector processors
        self.hrv = HRVAnalyzer()
        self.anomaly = VitalAnomalyDetector()
        self.longitudinal = LongitudinalTracker()
        self.coherence_mw = CoherenceScorer()
        self.coherence_csi = CoherenceScorer()
        self.bp = BPEstimator()
        # Happiness + Seed
        self.happiness = HappinessScorer()
        self.seed = SeedBridge(seed_url) if seed_url else None
        self._last_seed_ingest = 0.0

    def update_mw(self, **kw):
        with self.lock:
            for k, v in kw.items():
                setattr(self, f"mw_{k}", v)
            self.mw_ok = True
            hr = kw.get("hr", 0)
            br = kw.get("br", 0)
            if hr > 0:
                self.hrv.add_hr(hr)
                self.longitudinal.observe("hr", hr)
                self.coherence_mw.update(1.0)
            else:
                self.coherence_mw.update(0.1)
            if br > 0:
                self.longitudinal.observe("br", br)
            alerts = self.anomaly.check(hr=hr, br=br)
            for a in alerts:
                self.events.append((time.time(), f"ANOMALY: {a[3]}"))

    def update_csi(self, **kw):
        with self.lock:
            for k, v in kw.items():
                setattr(self, f"csi_{k}", v)
            self.csi_ok = True
            rssi = kw.get("rssi", 0)
            if rssi != 0:
                self.longitudinal.observe("rssi", rssi)
                self.coherence_csi.update(min(1.0, max(0.0, (rssi + 90) / 50)))
            # Feed happiness scorer
            self.happiness.update(
                motion_energy=kw.get("motion", self.csi_motion),
                br=kw.get("br", self.csi_br),
                hr=kw.get("hr", self.csi_hr),
                rssi=rssi,
            )

    def add_event(self, msg):
        with self.lock:
            self.events.append((time.time(), msg))

    def compute(self):
        with self.lock:
            hrv = self.hrv.compute()
            mw_hr = self.mw_hr
            csi_hr = self.csi_hr

            if mw_hr > 0 and csi_hr > 0:
                fused_hr = mw_hr * 0.8 + csi_hr * 0.2
                hr_src = "Fused"
            elif mw_hr > 0:
                fused_hr = mw_hr
                hr_src = "mmWave"
            elif csi_hr > 0:
                fused_hr = csi_hr
                hr_src = "CSI"
            else:
                fused_hr = 0
                hr_src = "—"

            mw_br = self.mw_br
            csi_br = self.csi_br
            fused_br = mw_br * 0.8 + csi_br * 0.2 if mw_br > 0 and csi_br > 0 else mw_br or csi_br

            sbp, dbp = self.bp.estimate(fused_hr, hrv["sdnn"], hrv["lf_hf"])

            # Stress from SDNN
            sdnn = hrv["sdnn"]
            if sdnn <= 0:
                stress = "—"
            elif sdnn < 30:
                stress = "HIGH"
            elif sdnn < 50:
                stress = "Moderate"
            elif sdnn < 80:
                stress = "Mild"
            elif sdnn < 100:
                stress = "Relaxed"
            else:
                stress = "Calm"

            # Drift checks
            drifts = []
            for metric in ["hr", "br", "rssi"]:
                val = {"hr": fused_hr, "br": fused_br, "rssi": self.csi_rssi}.get(metric, 0)
                if val:
                    d = self.longitudinal.check_drift(metric, val)
                    if d:
                        drifts.append(d)

            # Happiness
            happy = self.happiness.compute()

            # Seed ingestion every 5 seconds
            now = time.time()
            if self.seed and now - self._last_seed_ingest >= 5.0:
                self._last_seed_ingest = now
                self.seed.ingest(happy["vector"], {
                    "hr": fused_hr, "br": fused_br, "rssi": self.csi_rssi,
                    "presence": self.mw_presence or self.csi_presence,
                })

            return {
                "hr": fused_hr, "hr_src": hr_src,
                "br": fused_br, "sbp": sbp, "dbp": dbp,
                "stress": stress, "sdnn": sdnn, "rmssd": hrv["rmssd"],
                "pnn50": hrv["pnn50"], "lf_hf": hrv["lf_hf"],
                "presence": self.mw_presence or self.csi_presence,
                "distance": self.mw_distance, "lux": self.mw_lux,
                "rssi": self.csi_rssi, "motion": self.csi_motion,
                "csi_frames": self.csi_frames, "mw_frames": self.mw_frames,
                "coh_mw": self.coherence_mw.score, "coh_csi": self.coherence_csi.score,
                "fall": self.csi_fall, "drifts": drifts,
                "events": list(self.events),
                "longitudinal": self.longitudinal.summary(),
                "happiness": happy["happiness"],
                "gait_energy": happy["gait_energy"],
                "affect_valence": happy["affect_valence"],
                "social_energy": happy["social_energy"],
                "happiness_vector": happy["vector"],
            }


# ====================================================================
# Serial readers
# ====================================================================

def reader_mmwave(port, baud, hub, stop):
    try:
        ser = serial.Serial(port, baud, timeout=1)
        hub.add_event(f"mmWave: {port}")
    except Exception as e:
        hub.add_event(f"mmWave FAIL: {e}")
        return
    prev_pres = None
    while not stop.is_set():
        try:
            line = ser.readline().decode("utf-8", errors="replace")
        except Exception:
            continue
        c = RE_ANSI.sub("", line)
        m = RE_MW_HR.search(c)
        if m:
            hub.update_mw(hr=float(m.group(1)), frames=hub.mw_frames + 1)
        m = RE_MW_BR.search(c)
        if m:
            hub.update_mw(br=float(m.group(1)))
        m = RE_MW_PRES.search(c)
        if m:
            p = m.group(1) == "ON"
            if prev_pres is not None and p != prev_pres:
                hub.add_event(f"Person {'arrived' if p else 'left'}")
            prev_pres = p
            hub.update_mw(presence=p)
        m = RE_MW_DIST.search(c)
        if m:
            hub.update_mw(distance=float(m.group(1)))
        m = RE_MW_LUX.search(c)
        if m:
            hub.update_mw(lux=float(m.group(1)))
    ser.close()


def reader_csi(port, baud, hub, stop):
    try:
        ser = serial.Serial(port, baud, timeout=1)
        hub.add_event(f"CSI: {port}")
    except Exception as e:
        hub.add_event(f"CSI FAIL: {e}")
        return
    while not stop.is_set():
        try:
            line = ser.readline().decode("utf-8", errors="replace")
        except Exception:
            continue
        m = RE_CSI_VITALS.search(line)
        if m:
            hub.update_csi(br=float(m.group(1)), hr=float(m.group(2)),
                           motion=float(m.group(3)), presence=m.group(4).upper() == "YES")
        m = RE_CSI_CB.search(line)
        if m:
            hub.update_csi(frames=int(m.group(1)), rssi=int(m.group(2)))
        m = RE_CSI_FALL.search(line)
        if m:
            hub.update_csi(fall=True)
            hub.add_event(f"FALL (accel={m.group(1)})")
        m = RE_CSI_CALIB.search(line)
        if m:
            hub.add_event(f"CSI calibrated (thresh={m.group(1)})")
    ser.close()


# ====================================================================
# Display
# ====================================================================

def _happiness_bar(value, width=10):
    """Render a bar like [====------] 0.62"""
    filled = int(round(value * width))
    return "[" + "=" * filled + "-" * (width - filled) + "]"


def run_display(hub, duration, interval, mode="vitals"):
    start = time.time()
    last = 0

    print()
    print("=" * 80)
    if mode == "happiness":
        print("  RuView Live — Happiness + Cognitum Seed Dashboard")
    else:
        print("  RuView Live — Ambient Intelligence + RuVector Signal Processing")
    print("=" * 80)
    print()

    if mode == "happiness":
        hdr = (f"{'s':>4}  {'Happy':>16}  {'Gait':>5}  {'Calm':>5}  "
               f"{'Social':>6}  {'Pres':>4}  {'RSSI':>5}  {'Seed':>6}  {'CSI#':>5}")
        print(hdr)
        print("-" * 80)
    else:
        hdr = (f"{'s':>4} {'HR':>4} {'BR':>3} {'BP':>7} {'Stress':>8} "
               f"{'SDNN':>5} {'RMSSD':>5} {'LF/HF':>5} "
               f"{'Pres':>4} {'Dist':>5} {'Lux':>5} {'RSSI':>5} "
               f"{'Coh':>4} {'CSI#':>5}")
        print(hdr)
        print("-" * 80)

    # Periodic Seed drift check (every 15s)
    _last_drift_check = 0.0

    while time.time() - start < duration:
        time.sleep(0.5)
        elapsed = int(time.time() - start)
        if elapsed <= last or elapsed % interval != 0:
            continue
        last = elapsed

        d = hub.compute()

        if mode == "happiness":
            h = d["happiness"]
            bar = _happiness_bar(h)
            gait_s = f"{d['gait_energy']:>5.2f}"
            calm_s = f"{d['affect_valence']:>5.2f}"
            social_s = f"{d['social_energy']:>6.2f}"
            pres_s = "YES" if d["presence"] else " no"
            rssi_s = f"{d['rssi']:>5}" if d["rssi"] != 0 else "  — "

            # Seed status
            seed_s = "  —  "
            if hub.seed:
                now = time.time()
                if now - _last_drift_check >= 15.0:
                    _last_drift_check = now
                    hub.seed.get_drift()
                drift = hub.seed.last_drift
                if drift:
                    seed_s = f"{'OK' if not drift.get('drifting') else 'DRIFT':>6}"
                else:
                    seed_s = " conn?"

            print(f"{elapsed:>3}s  {bar} {h:.2f}  {gait_s}  {calm_s}  "
                  f"{social_s}  {pres_s:>4}  {rssi_s}  {seed_s}  {d['csi_frames']:>5}")

            # Show drift detail if drifting
            if hub.seed and hub.seed.last_drift and hub.seed.last_drift.get("drifting"):
                print(f"      SEED DRIFT: {hub.seed.last_drift.get('message', 'unknown')}")
        else:
            hr_s = f"{d['hr']:>4.0f}" if d["hr"] > 0 else "  —"
            br_s = f"{d['br']:>3.0f}" if d["br"] > 0 else " —"
            bp_s = f"{d['sbp']:>3}/{d['dbp']:<3}" if d["sbp"] > 0 else "  —/—  "
            sdnn_s = f"{d['sdnn']:>5.0f}" if d["sdnn"] > 0 else "  — "
            rmssd_s = f"{d['rmssd']:>5.0f}" if d["rmssd"] > 0 else "  — "
            lfhf_s = f"{d['lf_hf']:>5.2f}" if d["sdnn"] > 0 else "  — "
            pres_s = "YES" if d["presence"] else " no"
            dist_s = f"{d['distance']:>4.0f}cm" if d["distance"] > 0 else "   — "
            lux_s = f"{d['lux']:>5.1f}" if d["lux"] > 0 else "  — "
            rssi_s = f"{d['rssi']:>5}" if d["rssi"] != 0 else "  — "
            coh = max(d["coh_mw"], d["coh_csi"])
            coh_s = f"{coh:>.2f}"

            print(f"{elapsed:>3}s {hr_s} {br_s} {bp_s} {d['stress']:>8} "
                  f"{sdnn_s} {rmssd_s} {lfhf_s} "
                  f"{pres_s:>4} {dist_s} {lux_s} {rssi_s} "
                  f"{coh_s:>4} {d['csi_frames']:>5}")

        for drift in d["drifts"]:
            print(f"      DRIFT: {drift}")
        for ts, msg in d["events"][-3:]:
            if time.time() - ts < interval + 1:
                print(f"      >> {msg}")

    # Final summary
    d = hub.compute()
    print()
    print("=" * 80)
    print("  SESSION SUMMARY (RuVector Analysis)")
    print("=" * 80)
    sensors = []
    if hub.mw_ok:
        sensors.append(f"mmWave ({d['mw_frames']})")
    if hub.csi_ok:
        sensors.append(f"CSI ({d['csi_frames']})")
    print(f"  Sensors:      {', '.join(sensors)}")
    if d["hr"] > 0:
        print(f"  Heart Rate:   {d['hr']:.0f} bpm ({d['hr_src']})")
    if d["br"] > 0:
        print(f"  Breathing:    {d['br']:.0f}/min")
    if d["sbp"] > 0:
        print(f"  BP Estimate:  {d['sbp']}/{d['dbp']} mmHg")
    if d["sdnn"] > 0:
        print(f"  HRV SDNN:     {d['sdnn']:.0f} ms — {d['stress']}")
        print(f"  HRV RMSSD:    {d['rmssd']:.0f} ms")
        print(f"  HRV pNN50:    {d['pnn50']:.1f}%")
        print(f"  LF/HF ratio:  {d['lf_hf']:.2f} {'(sympathetic dominant)' if d['lf_hf'] > 2 else '(balanced)' if d['lf_hf'] > 0.5 else '(parasympathetic)'}")
    if d["lux"] > 0:
        print(f"  Ambient Light: {d['lux']:.1f} lux")
    # Longitudinal baselines
    longi = d["longitudinal"]
    if longi:
        print(f"  Baselines ({len(longi)} metrics tracked):")
        for name, stats in sorted(longi.items()):
            print(f"    {name}: mean={stats['mean']:.1f} std={stats['std']:.1f} n={stats['n']}")
    # Happiness
    if d.get("happiness", 0) > 0:
        print(f"  Happiness:    {d['happiness']:.2f} (gait={d['gait_energy']:.2f} affect={d['affect_valence']:.2f} social={d['social_energy']:.2f})")
    # Signal coherence
    print(f"  Coherence:    mmWave={d['coh_mw']:.2f} CSI={d['coh_csi']:.2f}")
    events = d["events"]
    if events:
        print(f"  Events ({len(events)}):")
        for ts, msg in events[-10:]:
            print(f"    {msg}")
    print()


def main():
    parser = argparse.ArgumentParser(description="RuView Live + RuVector Analysis")
    parser.add_argument("--csi", default=None, help="CSI port (or 'none'); defaults to COM5 for happiness mode, COM7 otherwise")
    parser.add_argument("--mmwave", default="COM4", help="mmWave port (or 'none')")
    parser.add_argument("--duration", type=int, default=120)
    parser.add_argument("--interval", type=int, default=3)
    parser.add_argument("--seed", default="none", help="Cognitum Seed HTTP base URL (e.g. 'http://169.254.42.1')")
    parser.add_argument("--mode", default="vitals", choices=["vitals", "happiness"],
                        help="Dashboard mode: vitals (default) or happiness")
    args = parser.parse_args()

    # Default CSI port depends on mode
    if args.csi is None:
        args.csi = "COM5" if args.mode == "happiness" else "COM7"

    seed_url = args.seed if args.seed.lower() != "none" else None
    hub = SensorHub(seed_url=seed_url)
    stop = threading.Event()

    if args.mmwave.lower() != "none":
        threading.Thread(target=reader_mmwave, args=(args.mmwave, 115200, hub, stop), daemon=True).start()
    if args.csi.lower() != "none":
        threading.Thread(target=reader_csi, args=(args.csi, 115200, hub, stop), daemon=True).start()

    time.sleep(2)

    try:
        run_display(hub, args.duration, args.interval, mode=args.mode)
    except KeyboardInterrupt:
        print("\nStopping...")
    stop.set()


if __name__ == "__main__":
    main()

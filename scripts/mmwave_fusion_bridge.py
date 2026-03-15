#!/usr/bin/env python3
"""
ADR-063 Phase 6: Real-time mmWave + WiFi CSI Fusion Bridge

Reads two serial ports simultaneously:
  - COM7 (ESP32-S3): WiFi CSI edge processing vitals
  - COM4 (ESP32-C6 + MR60BHA2): 60 GHz mmWave HR/BR via ESPHome

Fuses heart rate and breathing rate using weighted Kalman-style averaging
and displays the combined output in real-time.

Usage:
    python scripts/mmwave_fusion_bridge.py --csi-port COM7 --mmwave-port COM4
"""

import argparse
import re
import serial
import sys
import threading
import time
from dataclasses import dataclass, field


@dataclass
class SensorState:
    """Thread-safe sensor state."""
    heart_rate: float = 0.0
    breathing_rate: float = 0.0
    presence: bool = False
    distance_cm: float = 0.0
    last_update: float = 0.0
    frame_count: int = 0
    lock: threading.Lock = field(default_factory=threading.Lock)

    def update(self, **kwargs):
        with self.lock:
            for k, v in kwargs.items():
                setattr(self, k, v)
            self.last_update = time.time()
            self.frame_count += 1

    def snapshot(self):
        with self.lock:
            return {
                "hr": self.heart_rate,
                "br": self.breathing_rate,
                "presence": self.presence,
                "distance_cm": self.distance_cm,
                "age_ms": int((time.time() - self.last_update) * 1000) if self.last_update else -1,
                "frames": self.frame_count,
            }


# ESPHome log patterns for MR60BHA2
RE_HR = re.compile(r"'Real-time heart rate'.*?(\d+\.?\d*)\s*bpm", re.IGNORECASE)
RE_BR = re.compile(r"'Real-time respiratory rate'.*?(\d+\.?\d*)", re.IGNORECASE)
RE_PRESENCE = re.compile(r"'Person Information'.*?state\s+(ON|OFF)", re.IGNORECASE)
RE_DISTANCE = re.compile(r"'Distance to detection object'.*?(\d+\.?\d*)\s*cm", re.IGNORECASE)

# CSI edge_proc patterns
RE_CSI_VITALS = re.compile(
    r"Vitals:.*?br=(\d+\.?\d*).*?hr=(\d+\.?\d*).*?motion=(\d+\.?\d*).*?pres=(\w+)",
    re.IGNORECASE,
)
RE_CSI_PRESENCE = re.compile(r"presence.*?(YES|no)", re.IGNORECASE)
RE_CSI_ADAPTIVE = re.compile(r"Adaptive calibration complete.*?threshold=(\d+\.?\d*)")


def read_mmwave_serial(port: str, baud: int, state: SensorState, stop: threading.Event):
    """Read ESPHome debug output from MR60BHA2 on ESP32-C6."""
    try:
        ser = serial.Serial(port, baud, timeout=1)
        print(f"[mmWave] Connected to {port} at {baud} baud")
    except Exception as e:
        print(f"[mmWave] Failed to open {port}: {e}")
        return

    while not stop.is_set():
        try:
            line = ser.readline().decode("utf-8", errors="replace").strip()
            if not line:
                continue

            # Remove ANSI escape codes
            clean = re.sub(r"\x1b\[[0-9;]*m", "", line)

            m = RE_HR.search(clean)
            if m:
                state.update(heart_rate=float(m.group(1)))

            m = RE_BR.search(clean)
            if m:
                state.update(breathing_rate=float(m.group(1)))

            m = RE_PRESENCE.search(clean)
            if m:
                state.update(presence=(m.group(1).upper() == "ON"))

            m = RE_DISTANCE.search(clean)
            if m:
                state.update(distance_cm=float(m.group(1)))

        except Exception:
            pass

    ser.close()


def read_csi_serial(port: str, baud: int, state: SensorState, stop: threading.Event):
    """Read edge_proc vitals from ESP32-S3 CSI node."""
    try:
        ser = serial.Serial(port, baud, timeout=1)
        print(f"[CSI]    Connected to {port} at {baud} baud")
    except Exception as e:
        print(f"[CSI]    Failed to open {port}: {e}")
        return

    while not stop.is_set():
        try:
            line = ser.readline().decode("utf-8", errors="replace").strip()
            if not line:
                continue

            clean = re.sub(r"\x1b\[[0-9;]*m", "", line)

            m = RE_CSI_VITALS.search(clean)
            if m:
                state.update(
                    breathing_rate=float(m.group(1)),
                    heart_rate=float(m.group(2)),
                    presence=(m.group(4).upper() == "YES"),
                )

        except Exception:
            pass

    ser.close()


def fuse_and_display(mmwave: SensorState, csi: SensorState, stop: threading.Event):
    """Kalman-style fusion: mmWave 80% + CSI 20% when both available."""
    print("\n" + "=" * 70)
    print("  ADR-063 Real-Time Sensor Fusion (mmWave + WiFi CSI)")
    print("=" * 70)
    print(f"  {'Metric':<20} {'mmWave':>10} {'CSI':>10} {'Fused':>10} {'Source':>12}")
    print("-" * 70)

    while not stop.is_set():
        mw = mmwave.snapshot()
        cs = csi.snapshot()

        # Fuse heart rate
        mw_hr = mw["hr"]
        cs_hr = cs["hr"]
        if mw_hr > 0 and cs_hr > 0:
            fused_hr = mw_hr * 0.8 + cs_hr * 0.2
            hr_src = "Kalman 80/20"
        elif mw_hr > 0:
            fused_hr = mw_hr
            hr_src = "mmWave only"
        elif cs_hr > 0:
            fused_hr = cs_hr
            hr_src = "CSI only"
        else:
            fused_hr = 0.0
            hr_src = "—"

        # Fuse breathing rate
        mw_br = mw["br"]
        cs_br = cs["br"]
        if mw_br > 0 and cs_br > 0:
            fused_br = mw_br * 0.8 + cs_br * 0.2
            br_src = "Kalman 80/20"
        elif mw_br > 0:
            fused_br = mw_br
            br_src = "mmWave only"
        elif cs_br > 0:
            fused_br = cs_br
            br_src = "CSI only"
        else:
            fused_br = 0.0
            br_src = "—"

        # Fuse presence (OR gate — either sensor detecting = present)
        fused_presence = mw["presence"] or cs["presence"]

        # Build display
        lines = [
            f"  {'Heart Rate':.<20} {mw_hr:>8.1f}bpm {cs_hr:>8.1f}bpm {fused_hr:>8.1f}bpm {hr_src:>12}",
            f"  {'Breathing':.<20} {mw_br:>8.1f}/m  {cs_br:>8.1f}/m  {fused_br:>8.1f}/m  {br_src:>12}",
            f"  {'Presence':.<20} {'YES' if mw['presence'] else 'no':>10} {'YES' if cs['presence'] else 'no':>10} {'YES' if fused_presence else 'no':>10} {'OR gate':>12}",
            f"  {'Distance':.<20} {mw['distance_cm']:>8.0f}cm  {'—':>10} {mw['distance_cm']:>8.0f}cm  {'mmWave':>12}",
            f"  {'Data age':.<20} {mw['age_ms']:>8}ms  {cs['age_ms']:>8}ms",
            f"  {'Frames':.<20} {mw['frames']:>10}   {cs['frames']:>10}",
        ]

        # Clear and redraw
        sys.stdout.write(f"\033[{len(lines) + 1}A\033[J")
        for line in lines:
            print(line)
        print()

        time.sleep(1)


def main():
    parser = argparse.ArgumentParser(description="ADR-063 mmWave + CSI Fusion Bridge")
    parser.add_argument("--csi-port", default="COM7", help="ESP32-S3 CSI serial port")
    parser.add_argument("--mmwave-port", default="COM4", help="ESP32-C6 mmWave serial port")
    parser.add_argument("--csi-baud", type=int, default=115200)
    parser.add_argument("--mmwave-baud", type=int, default=115200)
    args = parser.parse_args()

    mmwave_state = SensorState()
    csi_state = SensorState()
    stop = threading.Event()

    # Start reader threads
    t_mw = threading.Thread(
        target=read_mmwave_serial,
        args=(args.mmwave_port, args.mmwave_baud, mmwave_state, stop),
        daemon=True,
    )
    t_csi = threading.Thread(
        target=read_csi_serial,
        args=(args.csi_port, args.csi_baud, csi_state, stop),
        daemon=True,
    )

    t_mw.start()
    t_csi.start()

    # Wait for both to connect
    time.sleep(2)

    # Print initial blank lines for the display area
    for _ in range(8):
        print()

    try:
        fuse_and_display(mmwave_state, csi_state, stop)
    except KeyboardInterrupt:
        print("\nStopping...")
        stop.set()


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
Live WiFi sensing monitor — collects RSSI from Windows WiFi and classifies
presence/motion in real-time using the ADR-013 commodity sensing pipeline.

Usage:
    python v1/tests/integration/live_sense_monitor.py

Walk around the room (especially between laptop and router) to trigger detection.
Press Ctrl+C to stop.
"""
import sys
import time

from v1.src.sensing.rssi_collector import WindowsWifiCollector
from v1.src.sensing.feature_extractor import RssiFeatureExtractor
from v1.src.sensing.classifier import PresenceClassifier

SAMPLE_RATE = 2.0       # Hz (netsh is slow, 2 Hz is practical max)
WINDOW_SEC = 15.0        # Analysis window
REPORT_INTERVAL = 3.0    # Print classification every N seconds


def main():
    collector = WindowsWifiCollector(interface="Wi-Fi", sample_rate_hz=SAMPLE_RATE)
    extractor = RssiFeatureExtractor(window_seconds=WINDOW_SEC)
    classifier = PresenceClassifier(
        presence_variance_threshold=0.3,   # Lower threshold for netsh quantization
        motion_energy_threshold=0.05,
    )

    print("=" * 65)
    print("  WiFi-DensePose Live Sensing Monitor (ADR-013)")
    print("  Pipeline: WindowsWifiCollector -> Extractor -> Classifier")
    print("=" * 65)
    print(f"  Sample rate:  {SAMPLE_RATE} Hz")
    print(f"  Window:       {WINDOW_SEC}s")
    print(f"  Report every: {REPORT_INTERVAL}s")
    print()
    print("  Collecting baseline... walk around after 15s to test detection.")
    print("  Press Ctrl+C to stop.")
    print("-" * 65)

    collector.start()

    try:
        last_report = 0.0
        while True:
            time.sleep(0.5)
            now = time.time()
            if now - last_report < REPORT_INTERVAL:
                continue
            last_report = now

            samples = collector.get_samples()
            n = len(samples)
            if n < 4:
                print(f"  [{time.strftime('%H:%M:%S')}] Buffering... ({n} samples)")
                continue

            rssi_vals = [s.rssi_dbm for s in samples]
            features = extractor.extract(samples)
            result = classifier.classify(features)

            # Motion bar visualization
            bar_len = min(40, max(0, int(features.variance * 20)))
            bar = "#" * bar_len + "." * (40 - bar_len)

            level_icon = {
                "absent": "  ",
                "present_still": "🧍",
                "active": "🏃",
            }.get(result.motion_level.value, "??")

            print(
                f"  [{time.strftime('%H:%M:%S')}] "
                f"RSSI: {features.mean:6.1f} dBm | "
                f"var: {features.variance:6.3f} | "
                f"motion_e: {features.motion_band_power:7.4f} | "
                f"breath_e: {features.breathing_band_power:7.4f} | "
                f"{result.motion_level.value:14s} {level_icon} "
                f"({result.confidence:.0%})"
            )
            print(f"           [{bar}] n={n} rssi=[{min(rssi_vals):.0f}..{max(rssi_vals):.0f}]")

    except KeyboardInterrupt:
        print()
        print("-" * 65)
        print("  Stopped. Final sample count:", len(collector.get_samples()))

        # Print summary
        samples = collector.get_samples()
        if len(samples) >= 4:
            features = extractor.extract(samples)
            result = classifier.classify(features)
            rssi_vals = [s.rssi_dbm for s in samples]
            print()
            print("  SUMMARY")
            print(f"    Duration:       {samples[-1].timestamp - samples[0].timestamp:.1f}s")
            print(f"    Total samples:  {len(samples)}")
            print(f"    RSSI range:     {min(rssi_vals):.1f} to {max(rssi_vals):.1f} dBm")
            print(f"    RSSI variance:  {features.variance:.4f}")
            print(f"    Motion energy:  {features.motion_band_power:.4f}")
            print(f"    Breath energy:  {features.breathing_band_power:.4f}")
            print(f"    Change points:  {features.n_change_points}")
            print(f"    Final verdict:  {result.motion_level.value} ({result.confidence:.0%})")
        print("=" * 65)
    finally:
        collector.stop()


if __name__ == "__main__":
    main()

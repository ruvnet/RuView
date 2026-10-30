#!/usr/bin/env python3
"""Tests for the ESP32 firmware release qualification contract (RuView#2092).

These tests cover the parsers and the verdict, and they prove the properties
that make the verdict mean something: a cumulative counter that was already
non-zero before the window does not fail the window, a restart inside the
window does, and a run with no device side is never a pass.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from esp32_firmware_qualification import (  # noqa: E402
    build_result,
    evaluate,
    find_node_ids,
    parse_console,
    render_markdown,
)

# A console capture in the shape the node writes: an ESP-IDF log line with
# the milliseconds since boot, the 1 Hz controller tick, and the mesh sync
# counters.
CAPTURE = """\
10:00:00.100 I (1000) adaptive_ctrl: medium tick: state=6 yield=28pps dsp=20.0Hz motion=0.10 presence=0.20 rssi=-52
10:00:01.100 I (2000) adaptive_ctrl: medium tick: state=6 yield=30pps dsp=20.0Hz motion=0.10 presence=0.20 rssi=-52
10:00:02.100 I (3000) c6_espnow: tx#120 (fail=96) rx#0 (match=0) leader=1 offset_us=0 smoothed=0
10:00:03.100 I (4000) adaptive_ctrl: medium tick: state=8 yield=4pps dsp=20.0Hz motion=0.10 presence=0.20 rssi=-52
10:00:04.100 I (5000) c6_espnow: tx#121 (fail=96) rx#0 (match=0) leader=1 offset_us=0 smoothed=0
"""


class TestParseConsole(unittest.TestCase):
    def test_yield_samples_and_mean(self) -> None:
        counters = parse_console(CAPTURE)
        self.assertEqual(counters["tick_samples"], 3)
        self.assertAlmostEqual(counters["yield_pps_mean"], 20.6666666, places=1)
        self.assertEqual(counters["yield_pps_min"], 4)
        self.assertEqual(counters["yield_pps_max"], 30)
        self.assertAlmostEqual(counters["dsp_hz_mean"], 20.0, places=2)

    def test_degraded_ticks_are_counted(self) -> None:
        counters = parse_console(CAPTURE)
        self.assertEqual(counters["degraded_ticks"], 1)

    def test_a_cumulative_failure_counter_is_differenced(self) -> None:
        # The node had already failed 96 sends before the window; that is not
        # a failure of this window.
        counters = parse_console(CAPTURE)
        self.assertEqual(counters["mesh_send_failures"], 0)

    def test_a_failure_inside_the_window_is_counted(self) -> None:
        capture = CAPTURE.replace("tx#121 (fail=96)", "tx#121 (fail=97)")
        counters = parse_console(capture)
        self.assertEqual(counters["mesh_send_failures"], 1)

    def test_a_restart_is_detected_from_the_boot_counter(self) -> None:
        capture = CAPTURE + (
            "10:00:05.100 I (200) adaptive_ctrl: medium tick: state=6 yield=27pps "
            "dsp=20.0Hz motion=0.10 presence=0.20 rssi=-52\n"
        )
        counters = parse_console(capture)
        self.assertEqual(counters["restarts"], 1)

    def test_enomem_and_watchdog_lines_are_counted(self) -> None:
        capture = CAPTURE + (
            "10:00:05.100 W (6000) csi_collector: sendto ENOMEM — backing off\n"
            "10:00:06.100 E (7000) uplink: uplink watchdog: no successful send for 12 s\n"
        )
        counters = parse_console(capture)
        self.assertEqual(counters["enomem_backoffs"], 1)
        self.assertEqual(counters["uplink_watchdogs"], 1)

    def test_an_empty_capture_reports_no_samples(self) -> None:
        counters = parse_console("")
        self.assertEqual(counters["tick_samples"], 0)
        self.assertEqual(counters["yield_pps_mean"], 0)
        self.assertEqual(counters["restarts"], 0)


class TestFindNodeIds(unittest.TestCase):
    def test_node_id_in_a_top_level_list(self) -> None:
        frame = {"nodes": [{"node_id": 1, "rssi_dbm": -52}]}
        self.assertEqual(find_node_ids(frame), {1})

    def test_node_id_in_a_nested_shape(self) -> None:
        frame = {"node_features": [{"node_id": 3}], "room": {"contributors": [{"nodeId": 2}]}}
        self.assertEqual(find_node_ids(frame), {2, 3})

    def test_a_frame_without_a_node(self) -> None:
        self.assertEqual(find_node_ids({"nodes": []}), set())

    def test_a_string_node_id_is_read(self) -> None:
        self.assertEqual(find_node_ids({"node_id": "4"}), {4})


def good_server():
    return {
        "seconds_observed": 300.0,
        "frames": 5900,
        "frames_with_node": 5900,
        "source_offline_frames": 0,
        "json_parse_errors": 0,
        "websocket_errors": 0,
        "early_closes": 0,
        "node_frame_coverage_percent": 100.0,
        "max_frame_gap_ms": 112.0,
    }


def good_device():
    return {
        "tick_samples": 300,
        "yield_pps_mean": 28.03,
        "yield_pps_min": 22,
        "yield_pps_max": 34,
        "degraded_ticks": 0,
        "dsp_hz_mean": 20.0,
        "mesh_send_failures": 0,
        "send_fail_warnings": 0,
        "enomem_backoffs": 0,
        "uplink_watchdogs": 0,
        "restarts": 0,
        "reset_reasons": [],
    }


class TestEvaluate(unittest.TestCase):
    def test_a_clean_run_passes(self) -> None:
        verdict = evaluate(good_server(), good_device(), 20.0, 99.0)
        self.assertTrue(verdict["pass"])
        self.assertEqual(verdict["failed"], 0)

    def test_a_missing_device_capture_never_passes(self) -> None:
        verdict = evaluate(good_server(), None, 20.0, 99.0)
        self.assertFalse(verdict["pass"])
        names = [f["criterion"] for f in verdict["findings"] if not f["pass"]]
        self.assertIn("device counters", names)

    def test_a_low_yield_fails(self) -> None:
        device = good_device()
        device["yield_pps_mean"] = 12.0
        verdict = evaluate(good_server(), device, 20.0, 99.0)
        self.assertFalse(verdict["pass"])
        names = [f["criterion"] for f in verdict["findings"] if not f["pass"]]
        self.assertEqual(names, ["raw CSI yield"])

    def test_a_restart_fails(self) -> None:
        device = good_device()
        device["restarts"] = 1
        verdict = evaluate(good_server(), device, 20.0, 99.0)
        self.assertFalse(verdict["pass"])

    def test_offline_frames_fail(self) -> None:
        server = good_server()
        server["source_offline_frames"] = 3
        server["node_frame_coverage_percent"] = 99.5
        verdict = evaluate(server, good_device(), 20.0, 99.0)
        self.assertFalse(verdict["pass"])

    def test_the_threshold_is_configurable(self) -> None:
        device = good_device()
        device["yield_pps_mean"] = 12.0
        self.assertFalse(evaluate(good_server(), device, 20.0, 99.0)["pass"])
        self.assertTrue(evaluate(good_server(), device, 10.0, 99.0)["pass"])


class TestRenderMarkdown(unittest.TestCase):
    def test_a_pass_report_carries_the_pass_marker(self) -> None:
        result = build_result(1, "board, AP, room", good_server(), good_device(),
                              20.0, 99.0, "0.8.16")
        report = render_markdown(result)
        self.assertIn("Result: **PASS**", report)
        self.assertIn("Environment: board, AP, room", report)
        self.assertIn("ruview:esp32-firmware-qualification version=0.8.16", report)
        self.assertIn("verdict=PASS", report)

    def test_a_fail_report_carries_the_fail_marker(self) -> None:
        device = good_device()
        device["yield_pps_mean"] = 3.0
        result = build_result(1, "board, AP, room", good_server(), device,
                              20.0, 99.0, "0.8.16")
        report = render_markdown(result)
        self.assertIn("Result: **FAIL**", report)
        self.assertNotIn("Result: **PASS**", report)
        self.assertIn("verdict=FAIL", report)

    def test_the_marker_names_the_version_under_test(self) -> None:
        result = build_result(2, "board", good_server(), good_device(),
                              20.0, 99.0, "0.8.15")
        report = render_markdown(result)
        self.assertIn("version=0.8.15", report)
        self.assertIn("node=2", report)


if __name__ == "__main__":
    unittest.main()

"""Tests for scripts/radar_window_logger.py (SYNTHETIC log lines)."""

import json
import unittest

import radar_window_logger as rwl

PRESENT = "[D][binary_sensor:036]: 'Person Information': Sending state ON"
ABSENT = "[D][binary_sensor:036]: 'Person Information': Sending state OFF"
DIST = "[D][sensor:093]: 'Distance to detection object': Sending state 142.00000 cm"
NOISE = "[D][sensor:093]: 'Real-time heart rate': Sending state 72.00000 bpm"


def cfg(**kw):
    base = dict(room="office", zone="desk", window_ms=1000, stale_after_ms=3000)
    base.update(kw)
    return rwl.WindowConfig(**base)


class WindowTests(unittest.TestCase):
    def test_presence_carries_forward_between_reports(self):
        lines = [(0, PRESENT), (100, DIST), (2500, NOISE)]
        ws = list(rwl.windows_from_lines(lines, cfg()))
        self.assertEqual([w["window_start_ms"] for w in ws], [0, 1000, 2000])
        self.assertTrue(all(w["presence"] is True for w in ws))
        self.assertEqual(ws[0]["distance_cm"], 142.0)
        self.assertEqual(ws[0]["reports_in_window"], 2)
        self.assertEqual(ws[1]["reports_in_window"], 0)

    def test_silence_becomes_stale_not_empty(self):
        lines = [(0, PRESENT), (6000, "")]
        ws = list(rwl.windows_from_lines(lines, cfg()))
        self.assertEqual(ws[0]["health"], "ok")
        stale = [w for w in ws if w["health"] == "stale"]
        self.assertTrue(stale)
        self.assertTrue(all("presence" not in w for w in stale))

    def test_absent_has_no_distance(self):
        lines = [(0, DIST), (10, ABSENT)]
        (w,) = list(rwl.windows_from_lines(lines, cfg()))
        self.assertIs(w["presence"], False)
        self.assertNotIn("distance_cm", w)

    def test_no_radar_line_yet_is_stale(self):
        (w,) = list(rwl.windows_from_lines([(0, NOISE)], cfg()))
        self.assertEqual(w["health"], "stale")
        self.assertNotIn("presence", w)

    def test_room_zone_and_source_fields(self):
        (w,) = list(rwl.windows_from_lines([(0, PRESENT)], cfg(verified=False)))
        self.assertEqual((w["room"], w["zone"]), ("office", "desk"))
        self.assertEqual(w["source"], {"verified": False, "simulated": False})
        json.dumps(w)  # serialisable

    def test_ansi_colour_codes_are_ignored(self):
        (w,) = list(rwl.windows_from_lines([(0, "\x1b[0;36m" + PRESENT + "\x1b[0m")], cfg()))
        self.assertIs(w["presence"], True)

    def test_replay_timestamp_prefix(self):
        self.assertEqual(rwl.parse_replay_line("1700000000123\thello", 5), (1700000000123, "hello"))
        self.assertEqual(rwl.parse_replay_line("no tab here", 5), (5, "no tab here"))

    def test_bad_window_rejected(self):
        with self.assertRaises(ValueError):
            list(rwl.windows_from_lines([(0, PRESENT)], cfg(window_ms=0)))

    def test_names_are_validated(self):
        self.assertEqual(rwl.validate_name("room", "living_room-2"), "living_room-2")
        for bad in ["", "a b", "../etc", "x" * 65]:
            with self.assertRaises(Exception):
                rwl.validate_name("room", bad)


if __name__ == "__main__":
    unittest.main()

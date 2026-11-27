"""Tests for input_activity_logger.py (ADR-426 D3). Clock and idle time are faked."""

import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import input_activity_logger as m  # noqa: E402


class FakeClock:
    def __init__(self):
        self.now = 1_000_000

    def ms(self):
        return self.now

    def sleep(self, seconds):
        self.now += int(seconds * 1000)


class Windows(unittest.TestCase):
    def test_recent_input_is_occupied_and_silence_is_unknown_never_empty(self):
        busy = m.window(5000, 1000, 400, 3000, "office", "desk")
        self.assertEqual((busy["health"], busy["presence"], busy["idle_ms"]), ("ok", True, 400))
        self.assertEqual((busy["window_start_ms"], busy["timestamp_ms"]), (4000, 5000))
        self.assertEqual(busy["sensor"], "input_activity")
        for idle in (3000, 600_000, None):
            quiet = m.window(5000, 1000, idle, 3000, "office", "desk")
            self.assertEqual(quiet["health"], "unknown")
            self.assertNotIn("presence", quiet)

    def test_run_uses_the_longest_idle_in_each_window(self):
        clock = FakeClock()
        # Idle grows 100 ms per poll, then the user types at t = 2.5 s.
        def idle():
            elapsed = clock.now - 1_000_000
            return 2900 + elapsed if elapsed < 2500 else elapsed - 2500
        out = list(m.run(idle, clock.ms, clock.sleep, room="office", zone="desk",
                         window_ms=1000, active_ms=3000, duration_s=5.05))
        self.assertEqual(len(out), 5)
        # Window 1 crosses the 3 s line; window 3 still contains the pre-typing
        # idle; from window 4 on only fresh input is seen.
        self.assertEqual([w["health"] for w in out], ["unknown", "unknown", "unknown", "ok", "ok"])
        self.assertTrue(all(w["timestamp_ms"] - w["window_start_ms"] == 1000 for w in out))

    def test_a_failed_read_makes_the_window_unknown(self):
        clock = FakeClock()
        calls = {"n": 0}

        def idle():
            calls["n"] += 1
            if calls["n"] == 3:
                raise OSError("no session")
            return 10
        out = list(m.run(idle, clock.ms, clock.sleep, room="office", zone="desk",
                         window_ms=1000, active_ms=3000, duration_s=2.05))
        self.assertEqual([w["health"] for w in out], ["unknown", "ok"])

    def test_bad_names_are_refused(self):
        with self.assertRaises(SystemExit):
            m.main(["--room", "../x", "--zone", "desk", "--out", os.devnull])


if __name__ == "__main__":
    unittest.main()

"""Offline trial synchronization, routing, traffic bounds and cleanup tests."""
import importlib.util
import io
from pathlib import Path
import signal
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location("run_trial_linux", Path(__file__).with_name("run_trial_linux.py"))
trial = importlib.util.module_from_spec(spec)
spec.loader.exec_module(trial)
SOURCE, CLIENT, MAC = "192.0.2.10", "192.0.2.11", "02:00:00:00:00:11"
HEADER = struct.pack("<IHHIIII", 0xa1b2c3d4, 2, 4, 0, 0, 65535, 127)


class LinkTests(unittest.TestCase):
    def fixtures(self):
        return [[{"addr_info": [{"family": "inet", "local": SOURCE, "prefixlen": 24}]}],
                [{"dev": "eth0", "from": SOURCE}],
                [{"dst": CLIENT, "lladdr": MAC, "state": ["STALE"]}]]

    def test_matching_cached_mac_without_dev_is_explicitly_recorded(self):
        with patch.object(trial, "read_json", side_effect=self.fixtures()):
            result = trial.verify_link("eth0", SOURCE, CLIENT, MAC)
        self.assertEqual(result["neighbor_state"], "STALE")
        self.assertEqual(result["neighbor_freshness"], "cached_mapping")

    def test_wrong_mac_failed_neighbor_gateway_wrong_interface_rejected(self):
        for mutation in (lambda f: f[2][0].update(lladdr="02:00:00:00:00:99"),
                         lambda f: f[2][0].update(state=["FAILED"]),
                         lambda f: f[1][0].update(gateway="192.0.2.1"),
                         lambda f: f[1][0].update(dev="wlan0")):
            fixtures = self.fixtures()
            mutation(fixtures)
            with patch.object(trial, "read_json", side_effect=fixtures):
                with self.assertRaises(RuntimeError):
                    trial.verify_link("eth0", SOURCE, CLIENT, MAC)

    def test_nonlocal_or_broadcast_target_rejected(self):
        for target in ("198.51.100.1", "192.0.2.255", SOURCE):
            with patch.object(trial, "read_json", side_effect=self.fixtures()):
                with self.assertRaises((RuntimeError, ValueError)):
                    trial.verify_link("eth0", SOURCE, target, MAC)


class SynchronizationTests(unittest.TestCase):
    def test_main_failure_cleans_up_and_preserves_original_error(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "new-trial"
            argv = ["--interface", "wlan0", "--ethernet", "eth0", "--management-ip", SOURCE,
                    "--bssid", MAC, "--client-ip", CLIENT, "--client-mac", MAC,
                    "--source-ip", SOURCE, "--frequency", "5180", "--center", "5210",
                    "--output", str(output), "--confirm-capture"]
            proc = Mock(returncode=0)
            proc.poll.return_value = None
            with patch.object(trial.sys, "platform", "linux"), \
                    patch.object(trial.signal, "SIGHUP", 1, create=True), \
                    patch.object(trial.signal, "signal"), \
                    patch.object(trial.os, "umask"), \
                    patch.object(trial, "verify_link", return_value={}), \
                    patch.object(trial.subprocess, "Popen", return_value=proc), \
                    patch.object(trial, "wait_for_capture", side_effect=RuntimeError("arming failed")), \
                    patch.object(trial, "send_traffic") as sender, \
                    patch.object(trial, "save_evidence", side_effect=OSError("receipt failed")), \
                    patch.object(trial.sys, "stdout", new_callable=io.StringIO) as printed:
                self.assertEqual(trial.main(argv), 1)
            sender.assert_not_called()
            proc.send_signal.assert_called_once_with(signal.SIGINT)
            proc.wait.assert_called_once_with()
            self.assertIn("arming failed", printed.getvalue())
            self.assertNotIn('"error": "receipt failed"', printed.getvalue())

    def test_valid_header_requires_capture_active_marker(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "capture.pcap").write_bytes(HEADER)
            proc = Mock()
            proc.poll.return_value = None
            with patch.object(trial.time, "monotonic", side_effect=[0, 0, 61]), \
                    patch.object(trial.time, "sleep"):
                with self.assertRaisesRegex(RuntimeError, "deadline"):
                    trial.wait_for_capture(proc, output)
            (output / "capture.active").touch()
            trial.wait_for_capture(proc, output)

    def test_invalid_dlt_and_exited_helper_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "capture.active").touch()
            (output / "capture.pcap").write_bytes(HEADER[:-4] + struct.pack("<I", 1))
            proc = Mock()
            proc.poll.return_value = None
            with self.assertRaisesRegex(RuntimeError, "radiotap"):
                trial.wait_for_capture(proc, output)
            proc.poll.return_value = 1
            with self.assertRaisesRegex(RuntimeError, "exited"):
                trial.wait_for_capture(proc, output)

    def test_cleanup_interrupts_and_waits_for_helper_not_kill(self):
        proc = Mock()
        proc.poll.return_value = None
        trial.stop_capture(proc)
        proc.send_signal.assert_called_once_with(signal.SIGINT)
        proc.wait.assert_called_once_with()
        proc.kill.assert_not_called()


class TrafficTests(unittest.TestCase):
    def pacing_run(self, pps, send_cost=0.0, oversleep=0.0):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "capture.active").touch()
            clock, sends, sleeps = [0.0], [], []
            def sleep(duration):
                self.assertGreater(duration, 0)
                sleeps.append(duration)
                clock[0] += duration + oversleep
            def send(*args):
                sends.append(clock[0])
                clock[0] += send_cost
                return 1200
            udp = Mock()
            udp.sendmsg.side_effect = send
            context = Mock(__enter__=Mock(return_value=udp), __exit__=Mock(return_value=False))
            proc = Mock()
            proc.poll.return_value = None
            evidence = {}
            with patch.object(trial.socket, "socket", return_value=context), \
                    patch.object(trial.socket, "if_nametoindex", return_value=7, create=True), \
                    patch.object(trial.time, "monotonic", side_effect=lambda: clock[0]), \
                    patch.object(trial.time, "sleep", side_effect=sleep):
                trial.send_traffic(proc, SOURCE, CLIENT, evidence, Mock(), "eth0", output,
                                   pps=pps, traffic_seconds=1, capture_seconds=6)
            return evidence, sends, sleeps

    def test_processing_cost_does_not_accumulate_in_sleep_at_both_rates(self):
        for rate in (250, 1000):
            evidence, sends, sleeps = self.pacing_run(rate, send_cost=.0002)
            self.assertEqual(len(sends), rate)
            self.assertAlmostEqual(evidence["actual_packets_per_second"], rate, places=5)
            self.assertEqual(evidence["missed_schedules"], 0)
            self.assertTrue(sleeps)
            self.assertLess(max(sleeps), 1 / rate + 1e-9)
            self.assertTrue(all(b - a >= 1 / rate - 1e-9 for a, b in zip(sends, sends[1:])))

    def test_scheduler_pause_skips_slots_without_catchup_bursts(self):
        evidence, sends, sleeps = self.pacing_run(1000, oversleep=.003)
        self.assertLess(len(sends), 300)
        self.assertGreater(evidence["missed_schedules"], 500)
        self.assertGreaterEqual(evidence["maximum_lateness_seconds"], .003 - 1e-9)
        self.assertTrue(all(b - a >= .001 - 1e-9 for a, b in zip(sends, sends[1:])))
        self.assertLessEqual(evidence["sent_payload_bytes"], 1000 * 1200)
        self.assertLessEqual(len(sleeps), 1001)

    def test_payload_count_interface_and_partial_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            marker = output / "capture.active"
            marker.touch()
            clock = [0.0]
            def sleep(duration):
                clock[0] += duration
            udp = Mock()
            udp.sendmsg.side_effect = [1200, RuntimeError("synthetic send failure")]
            context = Mock()
            context.__enter__ = Mock(return_value=udp)
            context.__exit__ = Mock(return_value=False)
            proc = Mock()
            proc.poll.return_value = None
            evidence = {}
            with patch.object(trial.socket, "socket", return_value=context), \
                    patch.object(trial.socket, "if_nametoindex", return_value=7, create=True), \
                    patch.object(trial.time, "monotonic", side_effect=lambda: clock[0]), \
                    patch.object(trial.time, "sleep", side_effect=sleep):
                with self.assertRaisesRegex(RuntimeError, "synthetic send failure"):
                    trial.send_traffic(proc, SOURCE, CLIENT, evidence, Mock(), "eth0", output)
            args = udp.sendmsg.call_args_list[0].args
            self.assertEqual(args[0], [bytes(1200)])
            self.assertEqual(struct.unpack("=I4s4s", args[1][0][2])[0], 7)
            self.assertEqual(args[3], (CLIENT, 9))
            self.assertEqual(evidence["sent_packets"], 1)
            self.assertIn("traffic_ended_unix_ns", evidence)
            self.assertIn("actual_packets_per_second", evidence)

    def test_no_traffic_after_capture_marker_removed(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = {}
            proc = Mock()
            proc.poll.return_value = None
            with patch.object(trial.socket, "socket") as socket_mock, \
                    patch.object(trial.socket, "if_nametoindex", return_value=7, create=True):
                with self.assertRaisesRegex(RuntimeError, "Capture ended"):
                    trial.send_traffic(proc, SOURCE, CLIENT, evidence, Mock(), "eth0", Path(directory))
            socket_mock.return_value.__enter__.return_value.sendmsg.assert_not_called()
            self.assertEqual(evidence["sent_packets"], 0)


class ProfileTests(unittest.TestCase):
    def test_defaults_and_controlled_high_rate_profile(self):
        self.assertEqual(trial.validate_profile(250, 30, 45), 7500)
        self.assertEqual(trial.validate_profile(1000, 30, 45), 30000)
        self.assertEqual(trial.validate_profile(1000, 55, 60), 55000)

    def test_all_resource_and_duration_limits(self):
        for profile in ((0, 30, 45), (1001, 30, 45), (250, 30, 121), (250, 0, 45),
                        (250, 41, 45), (1000, 61, 120), (1000, 56, 120), (1.5, 1, 6)):
            with self.subTest(profile=profile), self.assertRaises(ValueError):
                trial.validate_profile(*profile)

    def test_invalid_send_profile_cannot_open_socket(self):
        with patch.object(trial.socket, "socket") as opened:
            with self.assertRaises(ValueError):
                trial.send_traffic(Mock(), SOURCE, CLIENT, {}, Mock(), "eth0", Path("unused"),
                                   pps=1001)
        opened.assert_not_called()


if __name__ == "__main__":
    unittest.main()

"""Offline regression tests: no commands or radios are accessed."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location("capture_linux", Path(__file__).with_name("capture_linux.py"))
capture_linux = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture_linux)

UUID = "01234567-89ab-cdef-0123-456789abcdef"


class RestoreTests(unittest.TestCase):
    def commands(self, reverted=False, wrong_uuid=False):
        info_calls = 0
        calls = []

        def run(args, *unused, **kwargs):
            nonlocal info_calls
            calls.append(args)
            output = ""
            if args == ["iw", "dev", "wlan0", "info"]:
                info_calls += 1
                output = "type monitor" if reverted and info_calls == 1 else "type managed"
            elif args == ["iw", "dev", "wlan0", "link"]:
                output = "Connected to 02:00:00:00:00:01"
            elif "GENERAL.STATE" in args:
                output = "30 (disconnected)"
            elif "GENERAL.CON-UUID" in args:
                output = "different" if wrong_uuid else UUID
            return subprocess.CompletedProcess(args, 0, output, "")
        return calls, run

    def test_nm_reversion_corrected_after_ownership_handoff(self):
        calls, run = self.commands(reverted=True)
        with patch.object(capture_linux, "command", side_effect=run):
            capture_linux.restore("wlan0", UUID)
        managed = ["iw", "dev", "wlan0", "set", "type", "managed"]
        self.assertEqual(calls.count(managed), 2)
        handoff = calls.index(["nmcli", "device", "set", "wlan0", "managed", "yes"])
        self.assertIn(managed, calls[handoff + 1:])
        self.assertIn(["nmcli", "--wait", "30", "connection", "up", "uuid", UUID,
                       "ifname", "wlan0"], calls)

    def test_no_retry_without_observed_type_reversion(self):
        calls, run = self.commands()
        with patch.object(capture_linux, "command", side_effect=run):
            capture_linux.restore("wlan0", UUID)
        self.assertEqual(calls.count(["iw", "dev", "wlan0", "set", "type", "managed"]), 1)

    def test_associated_wrong_profile_is_not_restoration(self):
        _, run = self.commands(wrong_uuid=True)
        with patch.object(capture_linux, "command", side_effect=run):
            with self.assertRaisesRegex(RuntimeError, "restoration"):
                capture_linux.restore("wlan0", UUID)


class ProcessTests(unittest.TestCase):
    def test_active_marker_only_exists_during_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "capture.active"
            proc = Mock(pid=1234, returncode=0)
            def communicate(**kwargs):
                self.assertTrue(marker.is_file())
                return None, b"done"
            proc.communicate.side_effect = communicate
            with patch.object(capture_linux.subprocess, "Popen", return_value=proc):
                capture_linux.capture(["capture"], Mock(), 10, marker)
            self.assertFalse(marker.exists())

    def test_interrupted_capture_removes_marker_after_reaping(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "capture.active"
            proc = Mock(pid=1234)
            count = [0]
            def communicate(**kwargs):
                self.assertTrue(marker.is_file())
                count[0] += 1
                if count[0] == 1:
                    raise KeyboardInterrupt()
                return None, b"stopped"
            proc.communicate.side_effect = communicate
            with patch.object(capture_linux.subprocess, "Popen", return_value=proc), \
                    patch.object(capture_linux, "command"):
                with self.assertRaises(KeyboardInterrupt):
                    capture_linux.capture(["capture"], Mock(), 10, marker)
            self.assertEqual(count[0], 2)
            self.assertFalse(marker.exists())

    def test_failure_names_complete_command(self):
        result = subprocess.CompletedProcess([], 16, "", "Device or resource busy")
        with patch.object(capture_linux.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(RuntimeError, "iw dev wlan0 set type monitor failed"):
                capture_linux.command(["iw", "dev", "wlan0", "set", "type", "monitor"])

    def test_interruption_reaps_capture_group_before_returning(self):
        proc = Mock(pid=1234)
        proc.communicate.side_effect = [KeyboardInterrupt(), (None, b"stopped")]
        with patch.object(capture_linux.subprocess, "Popen", return_value=proc), \
                patch.object(capture_linux, "command") as command:
            with self.assertRaises(KeyboardInterrupt):
                capture_linux.capture(["capture"], Mock(), 10)
        command.assert_called_once_with(["kill", "-INT", "--", "-1234"], True, check=False)
        self.assertEqual(proc.communicate.call_count, 2)

    def test_stuck_group_is_killed_and_reaped(self):
        proc = Mock(pid=1234)
        proc.communicate.side_effect = [KeyboardInterrupt(), subprocess.TimeoutExpired("capture", 7),
                                        (None, b"stopped")]
        with patch.object(capture_linux.subprocess, "Popen", return_value=proc), \
                patch.object(capture_linux, "command") as command:
            with self.assertRaises(KeyboardInterrupt):
                capture_linux.capture(["capture"], Mock(), 10)
        self.assertEqual(command.call_args_list[-1].args[0], ["kill", "-KILL", "--", "-1234"])
        self.assertEqual(proc.communicate.call_count, 3)


class MonitorTests(unittest.TestCase):
    def test_frequency_width_and_center_must_match(self):
        info = "type monitor\nchannel 36 (5180 MHz), width: 80 MHz, center1: 5210 MHz"
        capture_linux.verify_monitor(info, 5180, 80, 5210)
        for wrong in (info.replace("monitor", "managed"), info.replace("5180", "5765"),
                      info.replace("80 MHz,", "20 MHz,"), info.replace("5210", "5775")):
            with self.assertRaises(RuntimeError):
                capture_linux.verify_monitor(wrong, 5180, 80, 5210)


class ReleaseTests(unittest.TestCase):
    def test_waits_for_both_nm_and_link_release(self):
        responses = iter(["", "", "30 (disconnected)", "Connected to 02:00:00:00:00:01",
                          "10 (unmanaged)", "Connected to 02:00:00:00:00:01",
                          "10 (unmanaged)", "Not connected."])
        def run(args, *unused, **kwargs):
            return subprocess.CompletedProcess(args, 0, next(responses), "")
        with patch.object(capture_linux, "command", side_effect=run) as cmd, \
                patch.object(capture_linux.time, "sleep") as sleep:
            result = capture_linux.release_wireless("wlan0")
        self.assertEqual(result["link"], "Not connected.")
        self.assertEqual(sleep.call_count, 2)
        self.assertEqual(cmd.call_args_list[0].args[0],
                         ["nmcli", "--wait", "10", "device", "disconnect", "wlan0"])
        self.assertFalse(any("type" in call.args[0] for call in cmd.call_args_list))

    def test_timeout_stops_before_type_change(self):
        result = subprocess.CompletedProcess([], 0, "100 (connected)", "")
        with patch.object(capture_linux, "command", return_value=result), \
                patch.object(capture_linux.time, "monotonic", side_effect=[0, 11]):
            with self.assertRaisesRegex(RuntimeError, "release did not verify"):
                capture_linux.release_wireless("wlan0")


class EvidencePathTests(unittest.TestCase):
    def test_standalone_copy_does_not_infer_checkout_from_script_depth(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            copied_script = base / "trial" / "capture_linux.py"
            copied_script.parent.mkdir()
            copied_script.write_text("# standalone copy\n")
            with patch.object(capture_linux, "__file__", str(copied_script)):
                self.assertFalse(capture_linux.evidence_in_checkout(base / "private-evidence"))

    def test_checkout_and_worktree_markers_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            (base / ".git").write_text("gitdir: /some/worktree\n")
            self.assertTrue(capture_linux.evidence_in_checkout(base))
            self.assertTrue(capture_linux.evidence_in_checkout(base / "evidence"))
            (base / ".git").unlink()
            (base / ".git").mkdir()
            self.assertTrue(capture_linux.evidence_in_checkout(base / "evidence"))


if __name__ == "__main__":
    unittest.main()

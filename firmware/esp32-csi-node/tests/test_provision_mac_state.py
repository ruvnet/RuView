"""Tests for keying provision.py state by board MAC instead of serial port (#1755).

main() runs in-process. esptool's MAC read, the NVS generator and the flasher
are all stubbed, so no serial port is opened and nothing is flashed. MACs and
credentials are placeholders.
"""

import contextlib
import csv
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

PROVISION_PATH = Path(__file__).resolve().parents[1] / "provision.py"
SPEC = importlib.util.spec_from_file_location("provision", PROVISION_PATH)
provision = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provision)

PORT = "/dev/cu.usbmodem-test"
OTHER_PORT = "/dev/cu.usbmodem-other"
MAC_A = "02:00:00:00:00:0a"
MAC_B = "02:00:00:00:00:0b"
CREDS = ("--ssid", "test-ssid", "--password", "fake-pass-not-real",
         "--target-ip", "192.0.2.10")

# The password is never cached, so reruns that rely on saved state still pass it.
PW = ("--password", "fake-pass-not-real")

# Shapes of `esptool read_mac` output (esptool/cmds.py read_mac in 4.x and 5.x).
ESPTOOL_V5_S3 = "Connected to ESP32-S3 on /dev/x:\nMAC:                02:00:00:00:00:0a\n"
ESPTOOL_V4_S3 = "Chip is ESP32-S3\nMAC: 02:00:00:00:00:0a\nHard resetting via RTS pin...\n"
ESPTOOL_V5_C6 = (
    "MAC:                02:00:00:ff:fe:00:00:0a\n"
    "BASE MAC:           02:00:00:00:00:0a\n"
    "MAC_EXT:            ff:fe\n"
)


class TestParseEsptoolMac(unittest.TestCase):
    def test_six_byte_mac_line(self):
        self.assertEqual(provision.parse_esptool_mac(ESPTOOL_V5_S3), MAC_A)
        self.assertEqual(provision.parse_esptool_mac(ESPTOOL_V4_S3), MAC_A)

    def test_eui64_chips_use_the_base_mac(self):
        # C5/C6 print an 8-byte "MAC:" line first; that isn't the base MAC.
        self.assertEqual(provision.parse_esptool_mac(ESPTOOL_V5_C6), MAC_A)

    def test_no_mac_in_output(self):
        self.assertIsNone(provision.parse_esptool_mac("A fatal error occurred\n"))

    def test_normalize_mac(self):
        self.assertEqual(provision.normalize_mac("02-00-00-00-00-0A"), MAC_A)
        self.assertIsNone(provision.normalize_mac("02:00:00:00:00"))
        self.assertIsNone(provision.normalize_mac("02:00:00:00:00:zz"))


class TestReadChipMac(unittest.TestCase):
    def test_runs_esptool_read_mac_on_the_port(self):
        done = subprocess.CompletedProcess([], 0, stdout=ESPTOOL_V5_C6, stderr="")
        with mock.patch.object(provision.subprocess, "run", return_value=done) as run:
            self.assertEqual(provision.read_chip_mac(PORT, 115200, "auto"), MAC_A)
        cmd = run.call_args.args[0]
        self.assertIn("read_mac", cmd)
        self.assertEqual(cmd[cmd.index("--port") + 1], PORT)

    def test_failure_returns_none(self):
        failed = subprocess.CompletedProcess([], 2, stdout="", stderr="no port")
        with mock.patch.object(provision.subprocess, "run", return_value=failed):
            self.assertIsNone(provision.read_chip_mac(PORT, 115200, "auto"))
        with mock.patch.object(provision.subprocess, "run",
                               side_effect=subprocess.TimeoutExpired("esptool", 30)):
            self.assertIsNone(provision.read_chip_mac(PORT, 115200, "auto"))


class _MainCase(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp(prefix="provision-mac-")
        self.state_dir = os.path.join(self.root, "state")
        self.flashed_csv = []

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def run_main(self, *argv, mac=None):
        """Run main() with the board `mac` attached; return (code, stdout, stderr)."""
        def fake_generate(csv_content, _size):
            self.flashed_csv.append(csv_content)
            return b"nvs"

        out, err = io.StringIO(), io.StringIO()
        code = 0
        old_cwd = os.getcwd()
        os.chdir(self.root)
        try:
            with mock.patch.object(sys, "argv", ["provision.py", *argv,
                                                 "--state-dir", self.state_dir]), \
                    mock.patch.object(provision, "read_chip_mac", return_value=mac,
                                      create=True) as read, \
                    mock.patch.object(provision, "generate_nvs_binary",
                                      side_effect=fake_generate), \
                    mock.patch.object(provision, "flash_nvs"), \
                    contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                self.read_mac = read
                try:
                    provision.main()
                except SystemExit as exc:
                    code = exc.code
        finally:
            os.chdir(old_cwd)
        return code, out.getvalue(), err.getvalue()

    def last_flashed(self):
        rows = csv.DictReader(io.StringIO(self.flashed_csv[-1]))
        return {row["key"]: row["value"] for row in rows}

    def board_file(self, mac):
        return provision._state_path_for(provision.mac_state_key(mac), self.state_dir)


class TestBoardKeyedState(_MainCase):
    def test_second_board_on_same_port_does_not_inherit_node_id(self):
        self.assertEqual(self.run_main("--port", PORT, *CREDS, "--node-id", "2", mac=MAC_A)[0], 0)

        code, _, _ = self.run_main("--port", PORT, *CREDS, mac=MAC_B)

        self.assertEqual(code, 0)
        self.assertNotIn("node_id", self.last_flashed())

    def test_second_board_without_credentials_is_not_given_the_first_boards(self):
        self.run_main("--port", PORT, *CREDS, "--node-id", "2", mac=MAC_A)

        code, _, err = self.run_main("--port", PORT, "--node-id", "3", mac=MAC_B)

        self.assertEqual(code, 2)
        self.assertIn("Missing required WiFi credentials", err)

    def test_board_keeps_its_state_on_a_different_port(self):
        self.run_main("--port", PORT, *CREDS, "--node-id", "2", mac=MAC_A)

        code, _, _ = self.run_main("--port", OTHER_PORT, *PW, "--zone", "lab", mac=MAC_A)

        self.assertEqual(code, 0)
        flashed = self.last_flashed()
        self.assertEqual(flashed["node_id"], "2")
        self.assertEqual(flashed["ssid"], "test-ssid")

    def test_record_carries_board_identity(self):
        self.run_main("--port", PORT, *CREDS, mac=MAC_A)

        with open(self.board_file(MAC_A), encoding="utf-8") as f:
            saved = json.load(f)
        self.assertEqual(saved[provision.STATE_MAC_KEY], MAC_A)
        self.assertEqual(saved[provision.STATE_PORT_KEY], PORT)
        self.assertNotIn("_chip_mac", self.flashed_csv[-1])

    def test_unreadable_mac_falls_back_to_port_keyed_state(self):
        code, _, err = self.run_main("--port", PORT, *CREDS, mac=None)

        self.assertEqual(code, 0)
        self.assertIn("could not read the board's MAC", err)
        self.assertTrue(os.path.isfile(provision._state_path_for(PORT, self.state_dir)))

    def test_reset_ignores_the_boards_record(self):
        self.run_main("--port", PORT, *CREDS, "--node-id", "2", mac=MAC_A)

        self.run_main("--port", PORT, "--reset", *CREDS, mac=MAC_A)

        self.assertNotIn("node_id", self.last_flashed())

    def test_invalid_mac_flag_is_rejected(self):
        code, _, err = self.run_main("--port", PORT, "--mac", "not-a-mac", "--state")
        self.assertEqual(code, 2)
        self.assertIn("--mac", err)


class TestMigration(_MainCase):
    def write_legacy(self, port, state):
        os.makedirs(self.state_dir, exist_ok=True)
        path = provision._state_path_for(port, self.state_dir)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(state, f)
        return path

    def test_port_keyed_record_moves_to_the_board_and_is_retired(self):
        legacy = self.write_legacy(PORT, {
            "ssid": "test-ssid", "password": "fake-pass-not-real",
            "target_ip": "192.0.2.10", "node_id": 5,
        })

        code, _, err = self.run_main("--port", PORT, *PW, "--zone", "lab", mac=MAC_A)

        self.assertEqual(code, 0)
        self.assertIn("port-keyed state", err)
        self.assertEqual(self.last_flashed()["node_id"], "5")
        self.assertFalse(os.path.exists(legacy))
        with open(self.board_file(MAC_A), encoding="utf-8") as f:
            self.assertEqual(json.load(f)["node_id"], 5)

        # The next board on that port starts clean.
        self.run_main("--port", PORT, *CREDS, mac=MAC_B)
        self.assertNotIn("node_id", self.last_flashed())

    def test_board_record_wins_over_a_port_keyed_file(self):
        self.run_main("--port", OTHER_PORT, *CREDS, "--node-id", "2", mac=MAC_A)
        self.write_legacy(PORT, {"node_id": 9})

        code, _, _ = self.run_main("--port", PORT, *PW, "--zone", "lab", mac=MAC_A)

        self.assertEqual(code, 0)
        self.assertEqual(self.last_flashed()["node_id"], "2")


class TestNoSerialForInspection(_MainCase):
    def test_state_shows_last_board_on_port_without_opening_it(self):
        self.run_main("--port", PORT, *CREDS, "--node-id", "2", mac=MAC_A)

        code, out, err = self.run_main("--port", PORT, "--state", mac=MAC_B)

        self.assertEqual(code, 0)
        self.read_mac.assert_not_called()
        self.assertEqual(json.loads(out)["node_id"], 2)
        self.assertIn(MAC_A, err)

    def test_state_with_mac_flag_selects_that_board(self):
        self.run_main("--port", PORT, *CREDS, "--node-id", "2", mac=MAC_A)
        self.run_main("--port", PORT, *CREDS, "--node-id", "3", mac=MAC_B)

        code, out, _ = self.run_main("--port", PORT, "--state", "--mac", MAC_A.upper())

        self.assertEqual(code, 0)
        self.assertEqual(json.loads(out)["node_id"], 2)

    def test_dry_run_does_not_open_the_port(self):
        code, _, _ = self.run_main("--port", PORT, *CREDS, "--dry-run", mac=MAC_A)

        self.assertEqual(code, 0)
        self.read_mac.assert_not_called()


if __name__ == "__main__":
    unittest.main()

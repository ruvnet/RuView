"""Side-effect tests for provision.py --dry-run and --reset (#2096).

main() runs in-process with the NVS generator and the flasher stubbed out, so
no serial port is opened and no ESP-IDF tooling is needed. State lives in a
per-test temp dir passed via --state-dir.
"""

import contextlib
import importlib.util
import io
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

PORT = "/dev/null-test"
FAKE_NVS = b"\xab" * 64
PRIOR = {
    "ssid": "TESTSSID",
    "target_ip": "127.0.0.1",
    "node_id": 1,
}
WIFI = ["--ssid", "TESTSSID", "--password", "TESTPASS", "--target-ip", "127.0.0.1"]


class ProvisionSideEffectTests(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp(prefix="provision-side-effects-")
        self.state_dir = os.path.join(self.root, "state")
        self.cwd = os.path.join(self.root, "cwd")
        os.makedirs(self.cwd)
        self.state_path = provision._state_path_for(PORT, self.state_dir)
        self.flash = mock.Mock()

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def run_main(self, *argv):
        """Run provision.main() with stubs; return the SystemExit code or None."""
        old_cwd = os.getcwd()
        os.chdir(self.cwd)
        try:
            with mock.patch.object(sys, "argv", ["provision.py", "--port", PORT,
                                                 "--state-dir", self.state_dir, *argv]), \
                    mock.patch.object(provision, "generate_nvs_binary",
                                      return_value=FAKE_NVS), \
                    mock.patch.object(provision, "flash_nvs", self.flash), \
                    contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()):
                provision.main()
        except SystemExit as exc:
            return exc.code
        finally:
            os.chdir(old_cwd)
        return None

    def seed_state(self):
        provision.save_state(PORT, self.state_dir, PRIOR)
        with open(self.state_path, "rb") as f:
            return f.read()

    def read_state_bytes(self):
        with open(self.state_path, "rb") as f:
            return f.read()

    def test_dry_run_does_not_create_state(self):
        self.assertIsNone(self.run_main(*WIFI, "--node-id", "1", "--dry-run"))
        self.assertFalse(os.path.exists(self.state_path))
        self.assertFalse(os.path.exists(self.state_dir))
        self.flash.assert_not_called()

    def test_dry_run_does_not_modify_existing_state(self):
        before = self.seed_state()
        self.assertIsNone(self.run_main("--password", "TESTPASS", "--node-id", "7", "--dry-run"))
        self.assertEqual(self.read_state_bytes(), before)
        self.flash.assert_not_called()

    def test_dry_run_still_writes_requested_binary(self):
        # The binary is the dry-run's documented output (ADR-061,
        # scripts/qemu-mesh-test.sh), so it is kept.
        self.run_main(*WIFI, "--dry-run")
        with open(os.path.join(self.cwd, "nvs_provision.bin"), "rb") as f:
            self.assertEqual(f.read(), FAKE_NVS)

    def test_failed_reset_keeps_state(self):
        before = self.seed_state()
        # The issue's repro: no WiFi trio after reset, so validation fails.
        self.assertEqual(self.run_main("--reset", "--node-id", "3"), 2)
        self.assertEqual(self.read_state_bytes(), before)
        self.flash.assert_not_called()

    def test_reset_dry_run_keeps_state(self):
        before = self.seed_state()
        self.assertIsNone(self.run_main("--reset", *WIFI, "--node-id", "3", "--dry-run"))
        self.assertEqual(self.read_state_bytes(), before)

    def test_reset_with_failed_flash_keeps_state(self):
        before = self.seed_state()
        self.flash.side_effect = subprocess.CalledProcessError(2, "esptool")
        with self.assertRaises(subprocess.CalledProcessError):
            self.run_main("--reset", *WIFI, "--node-id", "3")
        self.assertEqual(self.read_state_bytes(), before)

    def test_reset_with_successful_flash_replaces_state(self):
        provision.save_state(PORT, self.state_dir, {**PRIOR, "zone": "lobby"})
        self.assertIsNone(self.run_main("--reset", *WIFI, "--node-id", "3"))
        self.flash.assert_called_once()
        state = provision.load_state(PORT, self.state_dir)
        self.assertEqual(state["node_id"], 3)
        self.assertNotIn("zone", state)

    def test_real_flash_still_persists_merged_state(self):
        self.seed_state()
        self.assertIsNone(self.run_main("--password", "TESTPASS", "--zone", "lobby"))
        self.flash.assert_called_once()
        state = provision.load_state(PORT, self.state_dir)
        self.assertEqual(state["zone"], "lobby")
        for key, value in PRIOR.items():
            self.assertEqual(state[key], value)
        # The password is supplied per run and never cached.
        self.assertNotIn("password", state)
        self.assertNotIn("TESTPASS", self.read_state_bytes().decode())


if __name__ == "__main__":
    unittest.main()

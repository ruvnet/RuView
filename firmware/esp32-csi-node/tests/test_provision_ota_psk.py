"""OTA provisioning regressions; all hardware and NVS generation are mocked."""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location(
    "provision_ota", Path(__file__).resolve().parents[1] / "provision.py")
provision = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provision)
FAKE_PSK = "ab" * 32


class OtaProvisionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.state = str(self.root / "state")
        self.base = ["--port", "COM7", "--state-dir", self.state,
                     "--ssid", "test-network", "--password", "synthetic-password",
                     "--target-ip", "192.0.2.1"]
        self.csv = []

    def tearDown(self):
        self.temp.cleanup()

    def key_file(self, value=FAKE_PSK):
        path = self.root / "key.txt"
        path.write_bytes(value.encode("ascii"))
        path.chmod(0o600)
        return str(path)

    def run_main(self, *options, generator_error=None):
        def generate(csv, _size):
            self.csv.append(csv)
            if generator_error:
                raise generator_error
            return b"synthetic-nvs"

        out, err = io.StringIO(), io.StringIO()
        old_cwd = os.getcwd()
        try:
            os.chdir(self.root)
            with mock.patch.object(sys, "argv", ["provision.py", *self.base, *options]), \
                    mock.patch.object(provision, "read_chip_mac", return_value=None) as read_mac, \
                    mock.patch.object(provision, "flash_nvs") as flash, \
                    mock.patch.object(provision, "generate_nvs_binary", side_effect=generate), \
                    contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = 0
                try:
                    provision.main()
                except SystemExit as exc:
                    code = exc.code
            return code, out.getvalue(), err.getvalue(), read_mac, flash
        finally:
            os.chdir(old_cwd)

    def test_key_reaches_security_namespace_but_never_state_or_output(self):
        code, out, err, _, flash = self.run_main("--ota-psk-file", self.key_file())
        self.assertEqual(code, 0)
        flash.assert_called_once()
        rows = list(provision.csv.reader(io.StringIO(self.csv[-1])))
        namespace = rows.index(["security", "namespace", "", ""])
        self.assertEqual(rows[namespace + 1], ["ota_psk", "data", "string", FAKE_PSK])
        saved = Path(provision._state_path_for("COM7", self.state)).read_text()
        self.assertNotIn(FAKE_PSK, saved + out + err)
        self.assertNotIn('"ota_psk"', saved)
        self.assertTrue(json.loads(saved)[provision.OTA_PSK_STATE_KEY])
        code, out, err, read_mac, flash = self.run_main("--state", "--show-secrets")
        self.assertEqual(code, 0)
        self.assertNotIn(FAKE_PSK, out + err)
        read_mac.assert_not_called()
        flash.assert_not_called()

    def test_known_key_must_be_resupplied_or_explicitly_removed(self):
        self.run_main("--ota-psk-file", self.key_file())
        self.csv.clear()
        code, _, err, _, flash = self.run_main("--node-id", "2")
        self.assertEqual(code, 2)
        self.assertIn("keys are not cached", err)
        self.assertEqual(self.csv, [])
        flash.assert_not_called()
        code, _, _, _, flash = self.run_main("--clear-ota-psk")
        self.assertEqual(code, 0)
        flash.assert_called_once()
        self.assertNotIn("security,namespace", self.csv[-1])
        self.assertFalse(provision.load_state("COM7", self.state)[provision.OTA_PSK_STATE_KEY])

    def test_legacy_key_is_hidden_on_read_and_scrubbed_only_on_successful_write(self):
        os.makedirs(self.state)
        path = Path(provision._state_path_for("COM7", self.state))
        path.write_text(json.dumps({"ota_psk": FAKE_PSK, "node_id": 7}))
        before = path.read_bytes()
        with mock.patch.object(provision, "save_state", side_effect=PermissionError("read-only")) as save:
            code, out, err, _, _ = self.run_main("--state", "--show-secrets")
            save.assert_not_called()
        self.assertEqual(code, 0)
        self.assertNotIn(FAKE_PSK, out + err)
        self.assertEqual(path.read_bytes(), before)
        self.assertTrue(json.loads(out)[provision.OTA_PSK_STATE_KEY])
        self.assertEqual(json.loads(out)["node_id"], 7)
        code, _, _, _, flash = self.run_main()
        self.assertEqual(code, 2)
        flash.assert_not_called()
        self.assertEqual(path.read_bytes(), before)
        code, _, _, _, _ = self.run_main("--ota-psk-file", self.key_file())
        self.assertEqual(code, 0)
        self.assertNotIn(FAKE_PSK, path.read_text())

    def test_direct_key_validation_rejects_before_hardware_without_echo(self):
        for value in ["", "a", "a" * 63, "a" * 65, "z" * 64, " " + FAKE_PSK,
                      FAKE_PSK + "\n", "é" * 64]:
            with self.subTest(length=len(value)):
                code, out, err, read_mac, flash = self.run_main("--ota-psk", value)
                self.assertEqual(code, 2)
                self.assertIn("exactly 64 hexadecimal", err)
                if len(value) > 20:
                    self.assertNotIn(value, out + err)
                read_mac.assert_not_called()
                flash.assert_not_called()

    def test_direct_key_and_standalone_key_count_as_configuration(self):
        code, out, err, _, _ = self.run_main("--ota-psk", FAKE_PSK)
        self.assertEqual(code, 0)
        self.assertIn("process arguments", err)
        self.assertNotIn(FAKE_PSK, out + err)
        args = type("Args", (), {"ota_psk": FAKE_PSK})()
        self.assertTrue(provision.has_config_value(args))

    def test_file_validation_allows_one_newline_and_rejects_oversize(self):
        for suffix in ["", "\n", "\r\n"]:
            self.assertEqual(provision.read_ota_psk_file(self.key_file(FAKE_PSK + suffix)), FAKE_PSK)
        for value in [FAKE_PSK + "\n\n", "a" * 10000, "short"]:
            with self.assertRaises(ValueError):
                provision.read_ota_psk_file(self.key_file(value))
        with self.assertRaises(ValueError):
            provision.read_ota_psk_file(str(self.root))
        with self.assertRaises(ValueError):
            provision.read_ota_psk_file(str(self.root / "missing"))

    @unittest.skipIf(sys.platform == "win32", "POSIX permission/symlink model")
    def test_file_rejects_insecure_permissions_and_symlinks(self):
        key = Path(self.key_file())
        key.chmod(0o644)
        with self.assertRaisesRegex(ValueError, "owner-only"):
            provision.read_ota_psk_file(str(key))
        key.chmod(0o600)
        link = self.root / "key-link"
        link.symlink_to(key)
        with self.assertRaisesRegex(ValueError, "not a symlink"):
            provision.read_ota_psk_file(str(link))

    def test_dry_run_has_no_hardware_or_state_side_effects(self):
        code, out, err, read_mac, flash = self.run_main("--ota-psk-file", self.key_file(), "--dry-run")
        self.assertEqual(code, 0)
        self.assertNotIn(FAKE_PSK, out + err)
        read_mac.assert_not_called()
        flash.assert_not_called()
        self.assertFalse(Path(provision._state_path_for("COM7", self.state)).exists())

    def test_generator_failure_redacts_key_from_diagnostic(self):
        code, out, err, _, flash = self.run_main(
            "--ota-psk-file", self.key_file(), generator_error=RuntimeError(FAKE_PSK))
        self.assertEqual(code, 1)
        self.assertNotIn(FAKE_PSK, out + err)
        flash.assert_not_called()

    def test_state_writer_strips_key_as_backstop(self):
        path = provision.save_state("COM7", self.state, {"ota_psk": FAKE_PSK})
        self.assertNotIn(FAKE_PSK, Path(path).read_text())


if __name__ == "__main__":
    unittest.main()

"""Tests for how provision.py protects the WiFi password and seed token (#1754).

The state file, the --dry-run NVS binary and the fallback CSV all hold the
secrets in clear, so they must be owner-only, and --state must not print them
unless asked. main() runs in-process with the NVS generator stubbed, so no
serial port is opened and no ESP-IDF tooling is needed. All credentials here
are placeholders.
"""

import contextlib
import importlib.util
import io
import json
import os
import shutil
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

PROVISION_PATH = Path(__file__).resolve().parents[1] / "provision.py"
SPEC = importlib.util.spec_from_file_location("provision", PROVISION_PATH)
provision = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provision)

FAKE_PASSWORD = "fake-pass-not-real"
FAKE_TOKEN = "fake-token-not-real"
POSIX_ONLY = unittest.skipIf(sys.platform == "win32", "POSIX permission model only")


def mode_of(path):
    return stat.S_IMODE(os.lstat(path).st_mode)


class _TempDirCase(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp(prefix="provision-secrets-")
        self.state_dir = os.path.join(self.root, "state")
        # The default umask is what made files 0644 in the first place.
        self.old_umask = os.umask(0o022)

    def tearDown(self):
        os.umask(self.old_umask)
        shutil.rmtree(self.root, ignore_errors=True)

    def run_main(self, *argv, cwd=None):
        """Run provision.main() with argv; return (exit code, stdout, stderr)."""
        out, err = io.StringIO(), io.StringIO()
        code = 0
        old_cwd = os.getcwd()
        try:
            os.chdir(cwd or self.root)
            with mock.patch.object(sys, "argv", ["provision.py", *argv]), \
                    contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                try:
                    provision.main()
                except SystemExit as exc:
                    code = exc.code
        finally:
            os.chdir(old_cwd)
        return code, out.getvalue(), err.getvalue()


@POSIX_ONLY
class TestStateFilePermissions(_TempDirCase):
    def test_new_state_dir_and_file_are_owner_only(self):
        path = provision.save_state("COM7", self.state_dir, {"password": FAKE_PASSWORD})
        self.assertEqual(mode_of(self.state_dir), 0o700)
        self.assertEqual(mode_of(path), 0o600)

    def test_save_tightens_files_left_by_an_older_version(self):
        os.makedirs(self.state_dir)
        os.chmod(self.state_dir, 0o755)
        legacy = provision._state_path_for("/dev/ttyUSB0", self.state_dir)
        stale_tmp = legacy + ".tmp"
        for path in (legacy, stale_tmp):
            with open(path, "w", encoding="utf-8") as f:
                json.dump({"password": FAKE_PASSWORD}, f)
            os.chmod(path, 0o644)

        provision.save_state("COM7", self.state_dir, {"ssid": "test-ssid"})

        self.assertEqual(mode_of(self.state_dir), 0o700)
        self.assertEqual(mode_of(legacy), 0o600)
        self.assertEqual(mode_of(stale_tmp), 0o600)

    def test_planted_symlink_at_old_temp_path_does_not_receive_secret(self):
        os.makedirs(self.state_dir, mode=0o700)
        victim = os.path.join(self.root, "victim.txt")
        with open(victim, "w", encoding="utf-8") as f:
            f.write("original")
        os.symlink(victim, provision._state_path_for("COM7", self.state_dir) + ".tmp")

        provision.save_state("COM7", self.state_dir, {"password": FAKE_PASSWORD})

        with open(victim, encoding="utf-8") as f:
            self.assertEqual(f.read(), "original")

    def test_hardening_does_not_chmod_through_a_symlink(self):
        os.makedirs(self.state_dir, mode=0o700)
        victim = os.path.join(self.root, "victim.txt")
        with open(victim, "w", encoding="utf-8") as f:
            f.write("not a state file")
        os.chmod(victim, 0o644)
        os.symlink(victim, os.path.join(self.state_dir, "link.json"))

        with contextlib.redirect_stderr(io.StringIO()):
            provision.harden_state_dir(self.state_dir)

        self.assertEqual(mode_of(victim), 0o644)

    def test_state_inspection_repairs_legacy_permissions(self):
        # --state never writes, so the repair has to happen before the read.
        os.makedirs(self.state_dir)
        os.chmod(self.state_dir, 0o755)
        legacy = provision._state_path_for("COM7", self.state_dir)
        with open(legacy, "w", encoding="utf-8") as f:
            json.dump({"password": FAKE_PASSWORD}, f)
        os.chmod(legacy, 0o644)

        code, _, _ = self.run_main("--port", "COM7", "--state-dir", self.state_dir, "--state")

        self.assertEqual(code, 0)
        self.assertEqual(mode_of(self.state_dir), 0o700)
        self.assertEqual(mode_of(legacy), 0o600)


class TestStateOutputMasking(_TempDirCase):
    """--state output, including a file left by a version that cached secrets."""

    def setUp(self):
        super().setUp()
        # Written directly: save_state() would strip the secrets, and the point
        # is what happens to a file an older version already wrote.
        os.makedirs(self.state_dir, exist_ok=True)
        self.legacy_path = provision._state_path_for("COM7", self.state_dir)
        with open(self.legacy_path, "w", encoding="utf-8") as f:
            json.dump({
                "ssid": "test-ssid",
                "password": FAKE_PASSWORD,
                "seed_token": FAKE_TOKEN,
                "target_ip": "192.0.2.10",
            }, f)

    def test_state_never_shows_secrets_from_a_legacy_file(self):
        code, out, err = self.run_main(
            "--port", "COM7", "--state-dir", self.state_dir, "--state")

        self.assertEqual(code, 0)
        self.assertNotIn(FAKE_PASSWORD, out)
        self.assertNotIn(FAKE_TOKEN, out)
        shown = json.loads(out)
        self.assertNotIn("password", shown)
        self.assertNotIn("seed_token", shown)
        self.assertEqual(shown["ssid"], "test-ssid")

    def test_show_secrets_has_nothing_to_show(self):
        code, out, _ = self.run_main(
            "--port", "COM7", "--state-dir", self.state_dir, "--state", "--show-secrets")

        self.assertEqual(code, 0)
        shown = json.loads(out)
        self.assertNotIn("password", shown)
        self.assertNotIn("seed_token", shown)

    def test_reading_a_legacy_file_scrubs_it_on_disk(self):
        state = provision.load_state("COM7", self.state_dir)

        self.assertEqual(state, {"ssid": "test-ssid", "target_ip": "192.0.2.10"})
        with open(self.legacy_path, encoding="utf-8") as f:
            raw = f.read()
        self.assertNotIn(FAKE_PASSWORD, raw)
        self.assertNotIn(FAKE_TOKEN, raw)

    def test_redact_hides_values_it_is_given(self):
        shown = provision.redact_secrets({"password": FAKE_PASSWORD, "seed_token": FAKE_TOKEN})
        self.assertEqual(shown, {"password": "(set)", "seed_token": "(set)"})

    def test_redact_marks_empty_and_leaves_absent_alone(self):
        shown = provision.redact_secrets({"password": "", "ssid": "test-ssid"})
        self.assertEqual(shown, {"password": "(empty)", "ssid": "test-ssid"})


@POSIX_ONLY
class TestIntermediateArtifacts(_TempDirCase):
    ARGS = ("--port", "COM7", "--ssid", "test-ssid", "--password", FAKE_PASSWORD,
            "--target-ip", "192.0.2.10", "--dry-run")

    def test_dry_run_binary_is_owner_only_even_if_it_already_existed(self):
        out_path = os.path.join(self.root, "nvs_provision.bin")
        with open(out_path, "wb") as f:
            f.write(b"old")
        os.chmod(out_path, 0o644)

        with mock.patch.object(provision, "generate_nvs_binary",
                               return_value=b"nvs-with-" + FAKE_PASSWORD.encode()):
            code, _, _ = self.run_main(*self.ARGS, "--state-dir", self.state_dir)

        self.assertIn(code, (0, None))
        self.assertEqual(mode_of(out_path), 0o600)

    def test_fallback_csv_is_owner_only(self):
        with mock.patch.object(provision, "generate_nvs_binary",
                               side_effect=RuntimeError("generator missing")):
            code, _, _ = self.run_main(*self.ARGS, "--state-dir", self.state_dir)

        self.assertEqual(code, 1)
        csv_path = os.path.join(self.root, "nvs_config.csv")
        self.assertEqual(mode_of(csv_path), 0o600)

    def test_generator_files_live_in_a_private_dir(self):
        seen = {}

        def fake_generator(cmd, **_kwargs):
            csv_path, bin_path = cmd[-3], cmd[-2]
            seen["dir_mode"] = mode_of(os.path.dirname(bin_path))
            seen["csv_mode"] = mode_of(csv_path)
            with open(bin_path, "wb") as f:
                f.write(b"nvs")

        # A shared temp dir like Linux /tmp, rather than macOS's per-user one.
        shared_tmp = os.path.join(self.root, "shared-tmp")
        os.makedirs(shared_tmp)
        os.chmod(shared_tmp, 0o755)
        with mock.patch.object(provision.tempfile, "tempdir", shared_tmp), \
                mock.patch.object(provision.subprocess, "check_call",
                                  side_effect=fake_generator):
            self.assertEqual(provision.generate_nvs_binary("key,type\n", 0x6000), b"nvs")

        self.assertEqual(seen["dir_mode"], 0o700)
        self.assertEqual(seen["csv_mode"], 0o600)


class TestPasswordInput(_TempDirCase):
    """Ways to give the password without putting it on the command line."""

    BASE = ("--port", "COM7", "--target-ip", "192.0.2.10")

    def setUp(self):
        super().setUp()
        self.flashed_csv = []

    def password_file(self, content, mode=0o600):
        path = os.path.join(self.root, "wifi-pass.txt")
        with open(path, "w", encoding="utf-8", newline="") as f:
            f.write(content)
        os.chmod(path, mode)
        return path

    def provision(self, *argv, tty=False, typed=None):
        """Run main() with flashing stubbed; return (code, stderr, getpass mock)."""
        def fake_generate(csv_content, _size):
            self.flashed_csv.append(csv_content)
            return b"nvs"

        stdin = mock.Mock()
        stdin.isatty.return_value = tty
        # read_chip_mac only exists once state is keyed by board MAC (#1755);
        # stub it so these tests never reach esptool or a serial port.
        with mock.patch.object(provision, "generate_nvs_binary", side_effect=fake_generate), \
                mock.patch.object(provision, "flash_nvs"), \
                mock.patch.object(provision, "read_chip_mac", return_value=None,
                                  create=True), \
                mock.patch.object(sys, "stdin", stdin), \
                mock.patch("getpass.getpass", return_value=typed) as prompt:
            code, _, err = self.run_main(*self.BASE, *argv, "--state-dir", self.state_dir)
        return code, err, prompt

    def flashed_password(self):
        rows = provision.csv.DictReader(io.StringIO(self.flashed_csv[-1]))
        return {row["key"]: row["value"] for row in rows}.get("password")

    def test_password_file_is_read_and_one_newline_dropped(self):
        path = self.password_file(FAKE_PASSWORD + "  \n\n")

        code, err, _ = self.provision("--ssid", "test-ssid", "--password-file", path)

        self.assertEqual(code, 0)
        # Only one newline goes; trailing spaces and the second newline stay.
        self.assertEqual(self.flashed_password(), FAKE_PASSWORD + "  \n")
        self.assertNotIn("visible in ps", err)

    def test_password_file_crlf_counts_as_one_newline(self):
        path = self.password_file(FAKE_PASSWORD + "\r\n")
        self.provision("--ssid", "test-ssid", "--password-file", path)
        self.assertEqual(self.flashed_password(), FAKE_PASSWORD)

    @POSIX_ONLY
    def test_group_or_world_readable_password_file_is_refused(self):
        for mode in (0o644, 0o640, 0o604):
            with self.subTest(mode=oct(mode)):
                path = self.password_file(FAKE_PASSWORD + "\n", mode)
                code, err, _ = self.provision("--ssid", "test-ssid", "--password-file", path)
                self.assertEqual(code, 2)
                self.assertIn("chmod 600", err)

    @POSIX_ONLY
    def test_insecure_password_file_allowed_with_flag_and_warning(self):
        path = self.password_file(FAKE_PASSWORD + "\n", 0o644)

        code, err, _ = self.provision("--ssid", "test-ssid", "--password-file", path,
                                      "--allow-insecure-password-file")

        self.assertEqual(code, 0)
        self.assertIn("readable by group or others", err)
        self.assertEqual(self.flashed_password(), FAKE_PASSWORD)

    def test_missing_or_empty_password_file_is_an_error(self):
        missing = os.path.join(self.root, "nope.txt")
        code, err, _ = self.provision("--ssid", "test-ssid", "--password-file", missing)
        self.assertEqual(code, 2)
        self.assertIn("does not exist", err)

        empty = self.password_file("\n")
        code, err, _ = self.provision("--ssid", "test-ssid", "--password-file", empty)
        self.assertEqual(code, 2)
        self.assertIn("is empty", err)

    def test_password_and_password_file_are_mutually_exclusive(self):
        path = self.password_file(FAKE_PASSWORD)

        code, err, _ = self.provision("--ssid", "test-ssid", "--password", FAKE_PASSWORD,
                                      "--password-file", path)

        self.assertEqual(code, 2)
        self.assertIn("not allowed with argument", err)

    def test_password_on_command_line_warns(self):
        code, err, _ = self.provision("--ssid", "test-ssid", "--password", FAKE_PASSWORD)

        self.assertEqual(code, 0)
        self.assertIn("--password is visible in ps and shell history", err)
        self.assertIn("--password-file", err)

    def test_terminal_prompts_for_password_of_a_new_ssid(self):
        code, _, prompt = self.provision("--ssid", "test-ssid", tty=True, typed=FAKE_PASSWORD)

        self.assertEqual(code, 0)
        prompt.assert_called_once()
        self.assertEqual(self.flashed_password(), FAKE_PASSWORD)
        self.assertNotIn("password", provision.load_state("COM7", self.state_dir))

    def test_saved_ssid_alone_still_prompts_for_the_password(self):
        # The password is never cached, so a known SSID in state is not enough.
        provision.save_state("COM7", self.state_dir, {"ssid": "test-ssid"})

        code, _, prompt = self.provision(tty=True, typed=FAKE_PASSWORD)

        self.assertEqual(code, 0)
        prompt.assert_called_once()
        self.assertEqual(self.flashed_password(), FAKE_PASSWORD)

    def test_no_prompt_when_password_is_given(self):
        code, _, prompt = self.provision("--ssid", "test-ssid", "--password", FAKE_PASSWORD,
                                         tty=True, typed="unused")

        self.assertEqual(code, 0)
        prompt.assert_not_called()
        self.assertEqual(self.flashed_password(), FAKE_PASSWORD)

    def test_full_run_leaves_no_secret_in_the_state_file(self):
        code, _, _ = self.provision(
            "--ssid", "test-ssid", "--password", FAKE_PASSWORD,
            "--seed-url", "http://192.0.2.10:8080", "--seed-token", FAKE_TOKEN,
            "--ota-psk", "deadbeef")

        self.assertEqual(code, 0)
        self.assertEqual(self.flashed_password(), FAKE_PASSWORD)  # it did reach the NVS image
        files = [n for n in os.listdir(self.state_dir) if n.endswith(".json")]
        self.assertTrue(files)
        for name in files:
            with open(os.path.join(self.state_dir, name), encoding="utf-8") as f:
                raw = f.read()
            data = json.loads(raw)
            for key in ("password", "seed_token", "ota_psk"):
                self.assertNotIn(key, data)
            for secret in (FAKE_PASSWORD, FAKE_TOKEN, "deadbeef"):
                self.assertNotIn(secret, raw)

    def test_no_prompt_without_a_terminal(self):
        code, err, prompt = self.provision("--ssid", "test-ssid", tty=False, typed="unused")

        prompt.assert_not_called()
        self.assertEqual(code, 2)
        self.assertIn("--password", err)


if __name__ == "__main__":
    unittest.main()

"""Tests for provision.py's additive-by-default merge behaviour (#391, #574)."""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
import unittest

# Allow `python -m unittest` from anywhere in the repo.
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

import provision  # noqa: E402  — sibling import after sys.path tweak


def _mk_args(**overrides) -> argparse.Namespace:
    """Build a Namespace with every mergeable attr set to None unless overridden."""
    base = {name: None for name in provision.MERGEABLE_ATTRS}
    base.update(overrides)
    return argparse.Namespace(**base)


class TestStateFile(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="provision-state-")

    def tearDown(self):
        import shutil
        shutil.rmtree(self.dir, ignore_errors=True)

    def test_load_state_empty_when_missing(self):
        self.assertEqual(provision.load_state("COM7", self.dir), {})

    def test_save_then_load_roundtrip(self):
        provision.save_state("COM7", self.dir, {"ssid": "x", "password": "y"})
        self.assertEqual(
            provision.load_state("COM7", self.dir),
            {"ssid": "x", "password": "y"},
        )

    def test_save_creates_per_port_files(self):
        provision.save_state("COM7", self.dir, {"ssid": "a"})
        provision.save_state("/dev/ttyUSB0", self.dir, {"ssid": "b"})
        self.assertEqual(provision.load_state("COM7", self.dir), {"ssid": "a"})
        self.assertEqual(provision.load_state("/dev/ttyUSB0", self.dir), {"ssid": "b"})

    def test_load_state_handles_corrupt_json(self):
        path = provision._state_path_for("COM7", self.dir)
        os.makedirs(self.dir, exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write("{not valid json")
        # Should warn but not raise.
        self.assertEqual(provision.load_state("COM7", self.dir), {})

    @unittest.skipIf(sys.platform == "win32",
                      "POSIX permission bits; Windows has no equivalent to assert on")
    def test_save_creates_dir_and_file_with_restrictive_perms(self):
        # RuView#1754: a state file holds the WiFi password in cleartext and
        # must not be world- or group-readable.
        path = provision.save_state("COM7", self.dir, {"ssid": "x", "password": "y"})
        self.assertEqual(os.stat(self.dir).st_mode & 0o777, 0o700)
        self.assertEqual(os.stat(path).st_mode & 0o777, 0o600)

    @unittest.skipIf(sys.platform == "win32",
                      "POSIX permission bits; Windows has no equivalent to assert on")
    def test_save_tightens_permissions_left_by_an_earlier_version(self):
        # Simulate a directory/file created by the pre-#1754 code path
        # (os.makedirs/open with no mode, so whatever the umask allowed).
        os.makedirs(self.dir, exist_ok=True)
        os.chmod(self.dir, 0o755)
        path = provision._state_path_for("COM7", self.dir)
        with open(path, "w", encoding="utf-8") as f:
            f.write("{}")
        os.chmod(path, 0o644)

        provision.save_state("COM7", self.dir, {"ssid": "x", "password": "y"})

        self.assertEqual(os.stat(self.dir).st_mode & 0o777, 0o700)
        self.assertEqual(os.stat(path).st_mode & 0o777, 0o600)

    @unittest.skipIf(sys.platform == "win32",
                      "POSIX permission bits; Windows has no equivalent to assert on")
    def test_save_tightens_sibling_state_files_not_being_rewritten(self):
        # A second port's state file, left at 0644 by an earlier version,
        # must be tightened too when *any* save_state call hardens the dir —
        # not just the file the current call happens to write.
        os.makedirs(self.dir, exist_ok=True)
        other_path = provision._state_path_for("/dev/ttyUSB0", self.dir)
        with open(other_path, "w", encoding="utf-8") as f:
            f.write("{}")
        os.chmod(other_path, 0o644)

        provision.save_state("COM7", self.dir, {"ssid": "x"})

        self.assertEqual(os.stat(other_path).st_mode & 0o777, 0o600)


class TestMerge(unittest.TestCase):
    def test_cli_wins_over_prior(self):
        args = _mk_args(ssid="new-ssid")
        prior = {"ssid": "old-ssid", "password": "abc"}
        merged = provision.merge_state_into_args(args, prior)
        self.assertEqual(args.ssid, "new-ssid")  # CLI value preserved
        self.assertEqual(args.password, "abc")    # filled from prior
        self.assertEqual(merged["ssid"], "new-ssid")
        self.assertEqual(merged["password"], "abc")

    def test_prior_fills_missing_cli(self):
        args = _mk_args()  # all None
        prior = {
            "ssid": "MyWiFi",
            "password": "secret",
            "target_ip": "192.168.1.20",
            "node_id": 3,
        }
        merged = provision.merge_state_into_args(args, prior)
        self.assertEqual(args.ssid, "MyWiFi")
        self.assertEqual(args.password, "secret")
        self.assertEqual(args.target_ip, "192.168.1.20")
        self.assertEqual(args.node_id, 3)
        for key, val in prior.items():
            self.assertEqual(merged[key], val)

    def test_partial_invocation_does_not_drop_unrelated_keys(self):
        # The exact #391 scenario: user previously provisioned WiFi, now adds
        # only --seed-url. Old behaviour wiped SSID. New behaviour keeps it.
        args = _mk_args(seed_url="http://10.1.10.236")
        prior = {
            "ssid": "ruv.net",
            "password": "<secret>",
            "target_ip": "192.168.1.20",
        }
        merged = provision.merge_state_into_args(args, prior)
        self.assertEqual(args.ssid, "ruv.net")
        self.assertEqual(args.password, "<secret>")
        self.assertEqual(args.target_ip, "192.168.1.20")
        self.assertEqual(args.seed_url, "http://10.1.10.236")
        # And the on-disk merged dict carries all four keys.
        self.assertEqual(set(merged.keys()),
                         {"ssid", "password", "target_ip", "seed_url"})

    def test_empty_prior_is_noop(self):
        args = _mk_args(ssid="x")
        merged = provision.merge_state_into_args(args, {})
        self.assertEqual(merged, {"ssid": "x"})

    def test_falsy_but_not_none_cli_value_overrides_prior(self):
        # node_id=0 is a legal value; must NOT be replaced by prior["node_id"]=5.
        args = _mk_args(node_id=0)
        prior = {"node_id": 5}
        merged = provision.merge_state_into_args(args, prior)
        self.assertEqual(args.node_id, 0)
        self.assertEqual(merged["node_id"], 0)


class TestStatePathSanitization(unittest.TestCase):
    def test_slashes_in_port_are_safe(self):
        path = provision._state_path_for("/dev/ttyUSB0", "/tmp/x")
        # Must not contain a raw slash in the basename
        self.assertNotIn("/", os.path.basename(path))

    def test_windows_com_port_is_safe(self):
        path = provision._state_path_for("COM7", "/tmp/x")
        self.assertTrue(path.endswith("COM7.json"))


if __name__ == "__main__":
    unittest.main()

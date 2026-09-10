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
    """Build a Namespace with every mergeable attr set to None unless overridden.

    The secret attributes are included even though they are deliberately NOT
    mergeable: argparse always defines them (they are still real flags), so a
    Namespace without them would let a test pass against code that would
    raise AttributeError in the real CLI.
    """
    base = {name: None for name in provision.MERGEABLE_ATTRS}
    base.update({name: None for name in provision.SECRET_ATTRS})
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
        provision.save_state("COM7", self.dir, {"ssid": "x", "target_ip": "1.2.3.4"})
        self.assertEqual(
            provision.load_state("COM7", self.dir),
            {"ssid": "x", "target_ip": "1.2.3.4"},
        )

    def test_save_drops_secrets_it_is_handed(self):
        # The caller passing a secret is exactly the route MERGEABLE_ATTRS
        # cannot police, so save_state has to filter independently.
        path = provision.save_state("COM7", self.dir, {
            "ssid": "x",
            "password": "hunter2",
            "seed_token": "tok",
            "ota_psk": "deadbeef",
        })
        self.assertEqual(provision.load_state("COM7", self.dir), {"ssid": "x"})
        with open(path, encoding="utf-8") as fh:
            raw = fh.read()
        for secret in ("hunter2", "tok", "deadbeef"):
            self.assertNotIn(secret, raw)

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


class TestMerge(unittest.TestCase):
    def test_cli_wins_over_prior(self):
        args = _mk_args(ssid="new-ssid")
        prior = {"ssid": "old-ssid", "target_ip": "10.0.0.1"}
        merged = provision.merge_state_into_args(args, prior)
        self.assertEqual(args.ssid, "new-ssid")  # CLI value preserved
        self.assertEqual(args.target_ip, "10.0.0.1")  # filled from prior
        self.assertEqual(merged["ssid"], "new-ssid")
        self.assertEqual(merged["target_ip"], "10.0.0.1")

    def test_prior_fills_missing_cli(self):
        args = _mk_args()  # all None
        prior = {
            "ssid": "MyWiFi",
            "target_ip": "192.168.1.20",
            "node_id": 3,
        }
        merged = provision.merge_state_into_args(args, prior)
        self.assertEqual(args.ssid, "MyWiFi")
        self.assertEqual(args.target_ip, "192.168.1.20")
        self.assertEqual(args.node_id, 3)
        for key, val in prior.items():
            self.assertEqual(merged[key], val)

    def test_prior_secret_is_not_merged_back_into_args(self):
        # A state file written by an older revision still carries the
        # passphrase. It must not be resurrected into the args the CSV
        # builder reads -- otherwise "not persisted" would be cosmetic and
        # the credential would keep flowing from disk into every flash.
        args = _mk_args()
        prior = {"ssid": "MyWiFi", "password": "old-secret",
                 "seed_token": "old-token"}
        provision.merge_state_into_args(args, prior)
        self.assertEqual(args.ssid, "MyWiFi")
        self.assertIsNone(args.password)
        self.assertIsNone(args.seed_token)

    def test_partial_invocation_does_not_drop_unrelated_keys(self):
        # The exact #391 scenario: user previously provisioned WiFi, now adds
        # only --seed-url. Old behaviour wiped SSID. New behaviour keeps it.
        args = _mk_args(seed_url="http://10.1.10.236")
        prior = {
            "ssid": "ruv.net",
            "target_ip": "192.168.1.20",
        }
        merged = provision.merge_state_into_args(args, prior)
        self.assertEqual(args.ssid, "ruv.net")
        self.assertEqual(args.target_ip, "192.168.1.20")
        self.assertEqual(args.seed_url, "http://10.1.10.236")
        # And the on-disk merged dict carries all three keys.
        self.assertEqual(set(merged.keys()),
                         {"ssid", "target_ip", "seed_url"})

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


class TestLegacySecretBearingState(unittest.TestCase):
    """State files written before secrets were excluded (ruvnet review, #1832).

    Seeds a file in the old format -- credentials in cleartext -- and pins
    that reading it both hides the credential from the caller and removes it
    from disk, rather than leaving it for a future run that may never come.
    """

    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="provision-legacy-")
        self.path = provision._state_path_for("COM7", self.dir)
        os.makedirs(self.dir, exist_ok=True)
        self.legacy = {
            "ssid": "ruv.net",
            "password": "old-passphrase",
            "seed_token": "old-token",
            "ota_psk": "0123456789abcdef",
            "target_ip": "192.168.1.20",
            "node_id": 3,
        }
        with open(self.path, "w", encoding="utf-8") as fh:
            json.dump(self.legacy, fh)

    def tearDown(self):
        import shutil
        shutil.rmtree(self.dir, ignore_errors=True)

    def _load_quietly(self):
        import contextlib
        import io as _io
        buf = _io.StringIO()
        with contextlib.redirect_stderr(buf):
            state = provision.load_state("COM7", self.dir)
        return state, buf.getvalue()

    def test_load_strips_every_secret(self):
        state, _ = self._load_quietly()
        for name in provision.SECRET_ATTRS:
            self.assertNotIn(name, state)

    def test_load_keeps_the_non_secret_settings(self):
        state, _ = self._load_quietly()
        self.assertEqual(state["ssid"], "ruv.net")
        self.assertEqual(state["target_ip"], "192.168.1.20")
        self.assertEqual(state["node_id"], 3)

    def test_load_rewrites_the_file_without_the_credential(self):
        self._load_quietly()
        with open(self.path, encoding="utf-8") as fh:
            raw = fh.read()
        for secret in ("old-passphrase", "old-token", "0123456789abcdef"):
            self.assertNotIn(secret, raw)
        # And the scrub is durable: a second read sees an already-clean file.
        again, err = self._load_quietly()
        self.assertNotIn("password", again)
        self.assertEqual(err, "")

    def test_load_says_what_it_removed(self):
        _, err = self._load_quietly()
        self.assertIn("password", err)
        self.assertIn(self.path, err)

    def test_scrubbed_state_leaves_the_wifi_trio_incomplete(self):
        # The point of the failure-injection case: after the scrub a run that
        # relied on the cached passphrase has no password at all. provision.py
        # must refuse rather than silently flash a board with no credential.
        state, _ = self._load_quietly()
        args = _mk_args()
        provision.merge_state_into_args(args, state)
        missing = [n for n, v in (("--ssid", args.ssid),
                                  ("--password", args.password),
                                  ("--target-ip", args.target_ip))
                   if v is None or v == ""]
        self.assertEqual(missing, ["--password"])


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

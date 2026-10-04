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




class TestStateFilePermissions(unittest.TestCase):
    """Issue #1754: the state file holds the WiFi password in cleartext."""

    def setUp(self):
        self.dir = os.path.join(tempfile.mkdtemp(prefix="provision-perm-"), "state")

    def tearDown(self):
        import shutil
        shutil.rmtree(os.path.dirname(self.dir), ignore_errors=True)

    @unittest.skipIf(sys.platform == "win32", "POSIX permission model only")
    def test_state_file_is_owner_only(self):
        path = provision.save_state("COM7", self.dir, {"ssid": "x", "password": "secret"})
        self.assertEqual(os.stat(path).st_mode & 0o777, 0o600)

    @unittest.skipIf(sys.platform == "win32", "POSIX permission model only")
    def test_state_dir_is_owner_only(self):
        provision.save_state("COM7", self.dir, {"password": "secret"})
        self.assertEqual(os.stat(self.dir).st_mode & 0o777, 0o700)

    @unittest.skipIf(sys.platform == "win32", "POSIX permission model only")
    def test_tightens_a_directory_left_by_an_earlier_version(self):
        # makedirs cannot tighten an existing directory; an explicit chmod must.
        os.makedirs(self.dir, exist_ok=True)
        os.chmod(self.dir, 0o755)
        provision.save_state("COM7", self.dir, {"password": "secret"})
        self.assertEqual(os.stat(self.dir).st_mode & 0o777, 0o700)


class TestChipIdentityBinding(unittest.TestCase):
    """Issue #1755: port paths are reused when boards are swapped."""

    def test_matches_when_identity_absent_on_either_side(self):
        # Permissive: pre-existing state files carry no identity, and a board
        # that will not answer must still be provisionable.
        self.assertTrue(provision.identity_matches({}, "80:b5:4e:c1:b5:68"))
        self.assertTrue(provision.identity_matches(
            {provision.STATE_IDENTITY_KEY: "80:b5:4e:c1:b5:68"}, None))

    def test_matches_same_board(self):
        self.assertTrue(provision.identity_matches(
            {provision.STATE_IDENTITY_KEY: "80:b5:4e:c1:b5:68"}, "80:b5:4e:c1:b5:68"))

    def test_rejects_a_different_board_on_the_same_port(self):
        self.assertFalse(provision.identity_matches(
            {provision.STATE_IDENTITY_KEY: "80:b5:4e:c1:b5:68"}, "80:b5:4e:c1:c4:f0"))


class TestOtaPskNamespace(unittest.TestCase):
    """Issue #1753: the OTA PSK lives in its own `security` NVS namespace."""

    def test_security_namespace_emitted_with_psk(self):
        args = _mk_args(ssid="net", ota_psk="a" * 64)
        csv_text = provision.build_nvs_csv(args)
        self.assertIn("security,namespace", csv_text.replace(", ", ","))
        self.assertIn("ota_psk", csv_text)

    def test_no_security_namespace_without_psk(self):
        args = _mk_args(ssid="net")
        csv_text = provision.build_nvs_csv(args)
        self.assertNotIn("security", csv_text)
        self.assertNotIn("ota_psk", csv_text)

    def test_csi_cfg_namespace_still_first(self):
        args = _mk_args(ssid="net", ota_psk="b" * 64)
        rows = [r.split(",") for r in provision.build_nvs_csv(args).strip().splitlines()]
        namespaces = [r[0] for r in rows if len(r) > 1 and r[1] == "namespace"]
        self.assertEqual(namespaces, ["csi_cfg", "security"])


class TestReworkedAfterReview(unittest.TestCase):
    """Regressions for defects found reviewing the first cut of this change."""

    def setUp(self):
        self.dir = os.path.join(tempfile.mkdtemp(prefix="provision-rework-"), "state")

    def tearDown(self):
        import shutil
        shutil.rmtree(os.path.dirname(self.dir), ignore_errors=True)

    def test_ota_psk_alone_counts_as_a_config_value(self):
        # The headline flag must be able to provision on its own; previously
        # has_config_value() rejected it before NVS generation.
        args = _mk_args(ota_psk="a" * 64)
        self.assertTrue(provision.has_config_value(args))

    @unittest.skipIf(sys.platform == "win32", "POSIX symlink semantics")
    def test_symlink_at_the_temp_path_cannot_capture_the_secret(self):
        # The temp path is predictable. Without O_EXCL/O_NOFOLLOW, os.open with
        # O_CREAT|O_TRUNC follows a planted symlink and writes the cleartext
        # password through it. Asserting the *final* file mode does not catch
        # this — the trailing chmod fixes that either way — so assert the victim
        # never receives the secret.
        os.makedirs(self.dir, mode=0o700, exist_ok=True)
        victim = os.path.join(os.path.dirname(self.dir), "victim.txt")
        with open(victim, "w", encoding="utf-8") as f:
            f.write("original")
        tmp = provision._state_path_for("COM7", self.dir) + ".tmp"
        os.symlink(victim, tmp)

        try:
            provision.save_state("COM7", self.dir, {"password": "s3cret"})
        except OSError:
            pass  # refusing outright is an acceptable outcome

        with open(victim, encoding="utf-8") as f:
            self.assertEqual(f.read(), "original",
                             "secret was written through a planted symlink")

    @unittest.skipIf(sys.platform == "win32", "POSIX permission model only")
    def test_read_path_repairs_permissions_left_by_an_earlier_version(self):
        os.makedirs(self.dir, exist_ok=True)
        provision.save_state("COM7", self.dir, {"password": "secret"})
        # Simulate state written before this change.
        os.chmod(self.dir, 0o755)
        os.chmod(provision._state_path_for("COM7", self.dir), 0o644)
        provision._harden_existing_state("COM7", self.dir)
        self.assertEqual(os.stat(self.dir).st_mode & 0o777, 0o700)
        self.assertEqual(
            os.stat(provision._state_path_for("COM7", self.dir)).st_mode & 0o777, 0o600)

    def test_harden_survives_a_chmod_failure(self):
        # A read-only or exotic filesystem must warn, not abort provisioning.
        missing = os.path.join(self.dir, "definitely-not-there.json")
        provision._harden(missing, 0o600)  # must not raise


if __name__ == "__main__":
    unittest.main()

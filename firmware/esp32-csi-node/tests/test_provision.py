import csv
import importlib.util
import io
import argparse
import types
import unittest
from pathlib import Path


PROVISION_PATH = Path(__file__).resolve().parents[1] / "provision.py"
SPEC = importlib.util.spec_from_file_location("provision", PROVISION_PATH)
provision = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provision)


def make_args(**overrides):
    values = {name: None for name, _ in provision.CONFIG_VALUE_CHECKS}
    values["hop_dwell"] = 200
    values.update(overrides)
    return types.SimpleNamespace(**values)


def csv_rows(content):
    return list(csv.DictReader(io.StringIO(content)))


class RaisingArgumentParser(argparse.ArgumentParser):
    def error(self, message):
        raise ValueError(message)


class ProvisionConfigValueTests(unittest.TestCase):
    def test_swarm_and_hopping_flags_count_as_config_values(self):
        cases = [
            {"hop_channels": "1,6,11"},
            {"seed_token": "token-123"},
            {"swarm_hb": 15},
            {"swarm_ingest": 3},
        ]

        for values in cases:
            with self.subTest(values=values):
                self.assertTrue(provision.has_config_value(make_args(**values)))

    def test_operational_flags_alone_do_not_count_as_config_values(self):
        self.assertFalse(provision.has_config_value(make_args()))

    def test_swarm_and_hopping_values_are_written_to_csv(self):
        args = make_args(
            hop_channels="1,6,11",
            hop_dwell=250,
            seed_token="token-123",
            swarm_hb=15,
            swarm_ingest=3,
        )

        rows = csv_rows(provision.build_nvs_csv(args))
        values_by_key = {row["key"]: row["value"] for row in rows}

        self.assertEqual(values_by_key["hop_count"], "3")
        self.assertEqual(values_by_key["chan_list"], "01060b")
        self.assertEqual(values_by_key["dwell_ms"], "250")
        self.assertEqual(values_by_key["seed_token"], "token-123")
        self.assertEqual(values_by_key["swarm_hb"], "15")
        self.assertEqual(values_by_key["swarm_ingest"], "3")


class ProvisionValidationTests(unittest.TestCase):
    def setUp(self):
        self.parser = RaisingArgumentParser(prog="provision.py")

    def assert_range_error(self, **overrides):
        with self.assertRaises(ValueError):
            provision.validate_config_ranges(make_args(**overrides), self.parser)

    def test_rejects_values_that_do_not_fit_nvs_integer_types(self):
        invalid_cases = [
            {"target_port": 0},
            {"target_port": 65536},
            {"node_id": -1},
            {"node_id": 256},
            {"tdm_slot": -1},
            {"tdm_slot": 256},
            {"tdm_total": 0},
            {"tdm_total": 256},
            {"pres_thresh": -1},
            {"pres_thresh": 65536},
            {"fall_thresh": -1},
            {"fall_thresh": 65536},
            {"vital_win": 31},
            {"vital_win": 257},
            {"vital_int": 99},
            {"vital_int": 65536},
            {"subk_count": 0},
            {"subk_count": 33},
            {"swarm_hb": 0},
            {"swarm_hb": 65536},
            {"swarm_ingest": 0},
            {"swarm_ingest": 65536},
        ]

        for values in invalid_cases:
            with self.subTest(values=values):
                self.assert_range_error(**values)

    def test_accepts_valid_nvs_integer_boundaries(self):
        args = make_args(
            target_port=65535,
            node_id=255,
            tdm_slot=254,
            tdm_total=255,
            pres_thresh=65535,
            fall_thresh=65535,
            vital_win=256,
            vital_int=65535,
            subk_count=32,
            swarm_hb=65535,
            swarm_ingest=65535,
        )

        provision.validate_config_ranges(args, self.parser)

    def test_rejects_invalid_hop_channel_lists(self):
        invalid_cases = [
            {"hop_channels": "1,6,11,36,40,44,48"},
            {"hop_channels": "1,abc"},
            {"hop_channels": "0"},
            {"hop_channels": "178"},
            {"hop_channels": "1,6", "hop_dwell": 9},
        ]

        for values in invalid_cases:
            with self.subTest(values=values):
                self.assert_range_error(**values)

    def test_accepts_six_valid_hop_channels(self):
        args = make_args(hop_channels="1,6,11,36,149,177", hop_dwell=10)

        provision.validate_config_ranges(args, self.parser)
        rows = csv_rows(provision.build_nvs_csv(args))
        values_by_key = {row["key"]: row["value"] for row in rows}

        self.assertEqual(values_by_key["hop_count"], "6")
        self.assertEqual(values_by_key["chan_list"], "01060b2495b1")
        self.assertEqual(values_by_key["dwell_ms"], "10")


if __name__ == "__main__":
    unittest.main()

"""Tests for the Windows WiFi quick start contract.

The contract is what keeps the guide true: the guide states a fact, and the
repository either produces that fact or the check fails. These tests cover the
parsers and prove that a changed guide is caught.
"""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from windows_wifi_quickstart_contract import (  # noqa: E402
    EVIDENCE,
    GUIDE,
    IMPORT_PREFIX,
    V1,
    code_blocks,
    inline_code,
    norm,
    read,
    table_with,
    tables,
    test_functions,
    verify,
)


class Parsers(unittest.TestCase):
    def test_tables_are_parsed(self):
        found = tables("| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n")
        self.assertEqual(len(found), 1)
        header, rows = found[0]
        self.assertEqual(header, ["A", "B"])
        self.assertEqual(rows, [["1", "2"], ["3", "4"]])

    def test_a_table_without_a_separator_row_is_not_a_table(self):
        self.assertEqual(tables("| A | B |\nnot a table\n"), [])

    def test_table_with_raises_for_an_unknown_header(self):
        with self.assertRaises(KeyError):
            table_with("| A |\n|---|\n| 1 |\n", "Nothing")

    def test_inline_code_and_code_blocks_are_extracted(self):
        text = "a `one` b\n```\nblock\n```\n"
        self.assertEqual(inline_code(text), ["one"])
        self.assertEqual(len(code_blocks(text)), 1)

    def test_norm_collapses_whitespace(self):
        self.assertEqual(norm("a  b\n c"), "a b c")

    def test_test_functions_counts_methods_and_functions(self):
        self.assertEqual(
            test_functions(ROOT / V1 / "tests/unit/test_rssi_collector_gate.py"),
            5,
        )


class Contract(unittest.TestCase):
    def setUp(self):
        self.guide = read(ROOT / GUIDE)

    def test_the_guide_matches_the_repository(self):
        self.assertEqual(verify(ROOT), [])

    def test_a_wrong_test_count_is_caught(self):
        changed = self.guide.replace("45 tests pass", "36 tests pass")
        self.assertNotEqual(changed, self.guide)
        failures = verify(ROOT, changed)
        self.assertTrue(
            any("unit test count" in name for name, _, _ in failures), failures
        )

    def test_the_wrong_python_path_is_caught(self):
        changed = self.guide.replace(
            'PYTHONPATH = "archive"', 'PYTHONPATH = "archive/v1"'
        )
        failures = verify(ROOT, changed)
        self.assertTrue(
            any("does not put archive/v1" in name for name, _, _ in failures),
            failures,
        )

    def test_a_missing_path_is_caught(self):
        changed = self.guide.replace(
            "`archive/v1/src/sensing/rssi_collector.py`",
            "`archive/v1/src/sensing/rssi_collector-typo.py`",
        )
        failures = verify(ROOT, changed)
        self.assertTrue(any("path " in name for name, _, _ in failures), failures)

    def test_a_claim_the_capture_does_not_hold_is_caught(self):
        changed = self.guide.replace("VERDICT: PASS", "VERDICT: MAYBE")
        failures = verify(ROOT, changed)
        self.assertTrue(
            any("the guide states" in name for name, _, _ in failures), failures
        )

    def test_the_import_style_is_the_one_the_repository_uses(self):
        monitor = read(ROOT / V1 / "tests/integration/live_sense_monitor.py")
        self.assertIn("from %s." % IMPORT_PREFIX, monitor)


if __name__ == "__main__":
    unittest.main()

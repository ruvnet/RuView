"""Tests for the ESP32-S3 quick start contract.

The contract is what keeps the guide true: the guide states a fact, and the
repository either produces that fact or the check fails. These tests cover the
parsers and prove that a changed guide is caught.
"""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from esp32_quickstart_contract import (  # noqa: E402
    FIRMWARE,
    GUIDE,
    SERVER,
    code_blocks,
    dashboard_strings,
    inline_code,
    log_records,
    norm,
    partition_offsets,
    python_flags,
    read,
    rust_flags,
    server_flags,
    table_with,
    tables,
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

    def test_rust_flags_read_the_default_values(self):
        flags = rust_flags(
            '#[arg(long, default_value = "8080")]\n'
            "pub http_port: u16,\n"
            '#[arg(long)]\n'
            "pub other: bool,\n"
        )
        self.assertEqual(flags["--http-port"], "8080")
        self.assertIsNone(flags["--other"])

    def test_python_flags_read_the_long_options(self):
        flags = python_flags(
            'parser.add_argument("--port", required=True)\n'
            'g.add_argument("--node", help="node IP")\n'
        )
        self.assertEqual(flags, {"--port", "--node"})

    def test_workflow_matrix_reads_every_release_variant(self):
        source = (
            "      matrix:\n"
            "        include:\n"
            "          - variant: 8mb\n"
            "            target: esp32s3\n"
            "            # a comment between the keys\n"
            "            artifact_app: esp32-csi-node.bin\n"
        )
        matrix = __import__("esp32_quickstart_contract").workflow_matrix(source)
        self.assertEqual(matrix["8mb"]["target"], "esp32s3")
        self.assertEqual(matrix["8mb"]["artifact_app"], "esp32-csi-node.bin")

    def test_partition_offsets_read_a_table(self):
        offsets = partition_offsets(ROOT / FIRMWARE / "partitions_4mb.csv")
        self.assertEqual(offsets["ota_0"], "0x20000")
        self.assertEqual(offsets["otadata"], "0xF000")

    def test_log_records_rejoin_a_wrapped_line(self):
        text = (
            "```\n"
            "W (4529) stream_sender: sendto ENOMEM to 192.168.0.57:5005 - the aggregator\n"
            "may not be answering on that address\n"
            "```\n"
        )
        records = log_records(text)
        self.assertEqual(len(records), 1)
        self.assertTrue(records[0].endswith("on that address"))

    def test_inline_code_and_code_blocks_are_extracted(self):
        text = "a `one` b\n```\nblock\n```\n"
        self.assertEqual(inline_code(text), ["one"])
        self.assertEqual(len(code_blocks(text)), 1)

    def test_norm_collapses_whitespace(self):
        self.assertEqual(norm("a  b\n c"), "a b c")

    def test_dashboard_strings_decode_the_unicode_escapes(self):
        strings = dashboard_strings(ROOT)
        self.assertIn("LIVE — ESP32 Hardware Connected", strings)

    def test_the_binary_flags_come_from_main_rs(self):
        flags = server_flags(ROOT)
        for flag in ("--udp-bind", "--udp-allow", "--udp-insecure-lan",
                     "--http-port", "--ws-port", "--udp-port", "--ui-path",
                     "--source", "--bind-addr"):
            self.assertIn(flag, flags)
        self.assertEqual(flags["--http-port"], "8080")
        self.assertEqual(flags["--ws-port"], "8765")
        self.assertEqual(flags["--udp-port"], "5005")
        self.assertEqual(flags["--ui-path"], "../ui")
        self.assertEqual(flags["--source"], "auto")


class Contract(unittest.TestCase):
    def setUp(self):
        self.guide = read(ROOT / GUIDE)

    def test_the_guide_matches_the_repository(self):
        self.assertEqual(verify(ROOT), [])

    def test_a_wrong_flash_offset_is_caught(self):
        changed = self.guide.replace("| `0x20000` |", "| `0x30000` |", 1)
        self.assertNotEqual(changed, self.guide)
        failures = verify(ROOT, changed)
        self.assertTrue(any("ota_0 offset" in name for name, _, _ in failures))

    def test_a_wrong_firmware_version_is_caught(self):
        changed = self.guide.replace("Firmware version: `0.8.15`", "Firmware version: `9.9.9`")
        failures = verify(ROOT, changed)
        self.assertTrue(any("firmware version" in name for name, _, _ in failures))

    def test_an_unknown_flag_is_caught(self):
        changed = self.guide.replace("--udp-port 5005", "--udp-port 5005 --not-a-flag")
        failures = verify(ROOT, changed)
        self.assertTrue(any("--not-a-flag" in name for name, _, _ in failures))

    def test_a_banner_the_dashboard_does_not_use_is_caught(self):
        changed = self.guide.replace(
            "RECONNECTING TO SERVER...", "RECONNECTING TO NOWHERE..."
        )
        failures = verify(ROOT, changed)
        self.assertTrue(any("dashboard banner" in name for name, _, _ in failures))

    def test_a_quote_missing_from_the_evidence_is_caught(self):
        changed = self.guide.replace(
            "sendto suppressed (ENOMEM backoff, 1 dropped)",
            "sendto suppressed (nothing was dropped)",
        )
        failures = verify(ROOT, changed)
        self.assertTrue(any("quoted log line" in name for name, _, _ in failures))

    def test_a_wrong_container_port_is_caught(self):
        changed = self.guide.replace("`127.0.0.1:3000`", "`127.0.0.1:3002`")
        failures = verify(ROOT, changed)
        self.assertTrue(any("container port" in name for name, _, _ in failures))

    def test_a_missing_path_is_caught(self):
        changed = self.guide.replace(
            "`firmware/esp32-csi-node/provision.py`",
            "`firmware/esp32-csi-node/provision-typo.py`",
        )
        failures = verify(ROOT, changed)
        self.assertTrue(any("path " in name for name, _, _ in failures))

    def test_the_onboarding_commands_are_not_environment_variables(self):
        # The node's serial onboarding commands share the RUVIEW_ prefix and
        # are firmware protocol names, not variables the repository reads.
        failures = verify(ROOT, self.guide)
        self.assertFalse(
            any("RUVIEW_HELLO_V1" in name for name, _, _ in failures),
            failures,
        )
        self.assertFalse(
            any("RUVIEW_CONFIG_V1" in name for name, _, _ in failures),
            failures,
        )

    def test_the_evidence_carries_no_raw_identifier(self):
        failures = verify(ROOT, self.guide)
        self.assertFalse(
            any("raw MAC" in name for name, _, _ in failures), failures
        )
        self.assertFalse(
            any("unmasked password" in name for name, _, _ in failures), failures
        )

    def test_the_evidence_carries_no_person_data(self):
        # Real CSI is person data (ADR-299): presence, motion level, the
        # person count, the band powers, the subcarrier amplitude vector and
        # the 17-keypoint pose must not survive into the published capture.
        failures = verify(ROOT, self.guide)
        self.assertFalse(
            any("person data" in name for name, _, _ in failures), failures
        )


if __name__ == "__main__":
    unittest.main()

"""PDML contract and subprocess bounds. Actual tshark is checked by --self-test."""
import sys
import unittest
from unittest.mock import patch

import bfi_decode as decoder
import compare_tshark as oracle


def pdml(first_phi=5, include_last=True):
    fields = []
    controls = {"ncindex": "1", "nrindex": "1", "chanwidth": "2", "grouping": "2",
                "codebookinfo": "0", "feedbacktype": "0", "sounding_dialog_token_nbr": "0"}
    for name, value in controls.items():
        fields.append(f'<field name="wlan.vht.mimo_control.{name}" show="{value}"/>')
    for snr in (-128, 127):
        fields.append(f'<field name="wlan.vht.compressed_beamforming_report.snr" show="{snr}"/>')
    pairs = [(first_phi, 2), (9, 1)] + [(0, 0)] * (60 if include_last else 59)
    for number, (phi, psi) in enumerate(pairs):
        fields.append(f'<field name="wlan.vht.compressed_beamforming_report.scidx" '
                      f'show="{number}, φ11:{phi}, ψ21:{psi}"/>')
    return ('<?xml version="1.0"?><pdml><packet>' + ''.join(fields) + '</packet></pdml>').encode()


class OracleTests(unittest.TestCase):
    def test_every_integer_compared_not_only_count(self):
        _, _, report = next(oracle.golden_packets())
        result = oracle.pdml_values(pdml(), report)
        self.assertTrue(result["angles_match"])
        self.assertTrue(result["controls_match"])
        self.assertTrue(result["snr_match"])
        changed = oracle.pdml_values(pdml(first_phi=6), report)
        self.assertFalse(changed["angles_match"])
        self.assertEqual(changed["oracle_angle_count"], changed["decoder_angle_count"])
        missing = oracle.pdml_values(pdml(include_last=False), report)
        self.assertFalse(missing["angles_match"])

    def test_unsafe_and_malformed_xml_rejected(self):
        _, _, report = next(oracle.golden_packets())
        for data in (b'<!DOCTYPE pdml><pdml/>', b'<!ENTITY foo "bar"><pdml/>', b'<pdml>'):
            with self.assertRaises(decoder.DecodeError):
                oracle.pdml_values(data, report)

    def test_all_literal_fixtures_keep_nonzero_prefix(self):
        for _, _, report in oracle.golden_packets():
            self.assertEqual([pair[:2] for pair in report["angles"][:2]], [(5, 2), (9, 1)])
            self.assertEqual(report["snr_codes"], [-128, 127])

    def test_bounded_process(self):
        self.assertEqual(oracle.run_bounded([sys.executable, "-c", "print('ok')"]), b"ok\r\n" if sys.platform == "win32" else b"ok\n")
        with self.assertRaisesRegex(decoder.DecodeError, "oracle_timeout"):
            oracle.run_bounded([sys.executable, "-c", "import time; time.sleep(5)"], timeout=.05)
        with patch.object(oracle, "MAX_OUTPUT", 64):
            with self.assertRaisesRegex(decoder.DecodeError, "oracle_output_limit_exceeded"):
                oracle.run_bounded([sys.executable, "-c", "print('x'*1024)"])


if __name__ == "__main__":
    unittest.main()

import copy
import json
from pathlib import Path
import unittest
from unittest import mock
from audit_rust_advisories import classify as classify_report, REGISTRY, fetch_primary, decision, WASMTIME_ARCHIVE_SHA256

def classify(report, lock, ignored=(), primary=None):
    if primary is None:
        primary = json.loads((Path(__file__).parent / "fixtures/wasmtime-primary-scopes-20261119.json").read_text())
    return classify_report(report, lock, ignored, primary)


class AuditMetadataTests(unittest.TestCase):
    def setUp(self):
        self.findings = json.loads((Path(__file__).parent / "fixtures/wasmtime-rustsec-metadata-20261119.json").read_text())
        self.package = {"name": "wasmtime", "version": "36.0.21", "source": REGISTRY, "checksum": WASMTIME_ARCHIVE_SHA256}
        for finding in self.findings:
            finding["package"] = self.package.copy()
        self.lock = {"package": [self.package.copy()]}

    def report(self, findings=None):
        findings = self.findings if findings is None else findings
        return {"vulnerabilities": {"found": bool(findings), "count": len(findings), "list": findings}, "settings": {"ignore": []}}

    def test_exact_reviewed_false_matches_are_reported_separately(self):
        result = classify(self.report(), self.lock)
        self.assertEqual(len(result["metadata_corrections"]), 2)
        self.assertEqual(result["raw_vulnerability_count"], 2)
        self.assertEqual(result["unresolved"], [])

    def test_old_or_future_affected_versions_cannot_use_the_correction(self):
        for version in ("36.0.20", "36.0.22", "39.0.0", "47.0.0", "50.0.1"):
            with self.subTest(version=version):
                findings = copy.deepcopy(self.findings)
                for finding in findings:
                    finding["package"]["version"] = version
                lock = {"package": [{**self.package, "version": version}]}
                self.assertEqual(len(classify(self.report(findings), lock)["unresolved"]), 2)

    def test_wasi_presence_prevents_wrong_package_coverage(self):
        for name in ("wasmtime-wasi", "wasmtime-wasi-http"):
            with self.subTest(package=name):
                lock = {"package": [self.package.copy(), {"name": name, "version": "36.0.20"}]}
                self.assertEqual(len(classify(self.report(), lock)["unresolved"]), 2)

    def test_actual_wasi_findings_remain_unresolved(self):
        findings = copy.deepcopy(self.findings)
        findings[0]["package"]["name"] = "wasmtime-wasi"
        self.assertEqual(len(classify(self.report(findings), self.lock)["unresolved"]), 1)

    def test_changed_advisory_selectors_fail_closed(self):
        for change in ("versions", "affected", "aliases", "url"):
            findings = copy.deepcopy(self.findings)
            if change == "versions": findings[0]["versions"]["patched"] = [">=99.0.0"]
            elif change == "affected": findings[0]["affected"] = {"os": ["macos"]}
            elif change == "aliases": findings[0]["advisory"]["aliases"] = ["GHSA-new-advisory"]
            else: findings[0]["advisory"]["url"] = "https://example.invalid/unreviewed"
            self.assertEqual(len(classify(self.report(findings), self.lock)["unresolved"]), 1)

    def test_unknown_or_version_applicable_advisory_is_not_covered(self):
        for advisory in ("RUSTSEC-2099-0001", "RUSTSEC-2026-0569"):
            findings = copy.deepcopy(self.findings)
            findings[0]["advisory"]["id"] = advisory
            self.assertEqual(len(classify(self.report(findings), self.lock)["unresolved"]), 1)

    def test_registry_identity_and_checksum_must_match_the_lockfile(self):
        for change in ("checksum", "source"):
            findings = copy.deepcopy(self.findings)
            findings[0]["package"][change] = "unreviewed"
            self.assertEqual(len(classify(self.report(findings), self.lock)["unresolved"]), 1)
        self.assertEqual(len(classify(self.report(), {"package": []})["unresolved"]), 2)

    def test_coordinated_report_and_lock_checksum_tamper_is_rejected(self):
        findings = copy.deepcopy(self.findings)
        for finding in findings: finding["package"]["checksum"] = "b" * 64
        lock = {"package": [{**self.package, "checksum": "b" * 64}]}
        self.assertEqual(len(classify(self.report(findings), lock)["unresolved"]), 2)

    def test_malformed_or_omitted_findings_fail_closed(self):
        for report in ({}, {"vulnerabilities": {"found": False, "count": 0, "list": self.findings}}):
            with self.assertRaises(ValueError): classify(report, self.lock)

    def test_existing_policy_is_honored_but_cannot_be_broadened_by_report(self):
        findings = copy.deepcopy(self.findings)
        findings[0]["advisory"]["id"] = "RUSTSEC-2099-0001"
        report = self.report(findings)
        report["settings"]["ignore"] = ["RUSTSEC-2099-0001"]
        with self.assertRaises(ValueError): classify(report, self.lock)
        result = classify(report, self.lock, ["RUSTSEC-2099-0001"])
        self.assertEqual(result["unresolved"], [])
        self.assertEqual(result["existing_policy_ignored"], ["RUSTSEC-2099-0001"])

    def test_missing_primary_evidence_does_not_cover_any_finding(self):
        self.assertEqual(len(classify(self.report(), self.lock, primary={})["unresolved"]), 2)

    def test_changed_primary_package_or_range_revokes_coverage(self):
        for field in ("package", "vulnerable_version_range"):
            primary = json.loads((Path(__file__).parent / "fixtures/wasmtime-primary-scopes-20261119.json").read_text())
            target = primary["GHSA-2674-w29h-wqvj"]["vulnerabilities"][0]
            if field == "package": target["package"]["name"] = "other-package"
            else: target[field] = ">=36.0.0,<48.0.9"
            self.assertEqual(len(classify(self.report(), self.lock, primary=primary)["unresolved"]), 1)

    def test_unknown_advisory_cannot_choose_a_network_location(self):
        with mock.patch("audit_rust_advisories.subprocess.run") as execute:
            with self.assertRaises(ValueError): fetch_primary("https://untrusted.invalid/advisory")
            execute.assert_not_called()

    def test_live_primary_failure_has_no_offline_success_fallback(self):
        with mock.patch("audit_rust_advisories.subprocess.run", return_value=mock.Mock(returncode=1, stdout="")):
            with self.assertRaises(ValueError): fetch_primary("GHSA-2674-w29h-wqvj")

    def test_native_warning_denial_cannot_be_overridden(self):
        result = classify(self.report(), self.lock)
        self.assertEqual(decision(1, result, {"output": {"deny": ["warnings"]}}), 1)
        self.assertEqual(decision(1, result, {}), 0)
        with self.assertRaises(ValueError): decision(1, classify(self.report([]), self.lock), {})

    def test_native_clean_report_remains_clean(self):
        self.assertEqual(classify(self.report([]), self.lock)["unresolved"], [])


if __name__ == "__main__":
    unittest.main()

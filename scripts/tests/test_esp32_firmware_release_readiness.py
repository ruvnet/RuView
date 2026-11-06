#!/usr/bin/env python3
"""Tests for the ESP32 firmware stable-release readiness contract (RuView#2092).

The gate is what keeps a stable release honest, so these tests cover the
cases that must block: a version that does not match the tree, a missing
release note, a missing qualification, and a qualification that failed.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from esp32_firmware_release_readiness import (  # noqa: E402
    evaluate_readiness,
    parse_qualification_marker,
    workflow_accepts_tag,
)

CANDIDATE = "0.8.16"
TAG = "v0.8.16-esp32"


def marker(version: str = CANDIDATE, verdict: str = "PASS") -> str:
    """The machine-readable line a qualification record carries."""
    return (f"<!-- ruview:esp32-firmware-qualification version={version} "
            f"node=1 verdict={verdict} -->\n")

WORKFLOW = """\
name: firmware
on:
  push:
    branches:
      - '**'
    tags:
      # ESP32 firmware release tags — build + version-consistency guard.
      - 'v*-esp32'
    paths:
      - 'firmware/**'
jobs:
  build:
    runs-on: ubuntu-latest
"""


class Tree:
    """A scratch tree with the files the readiness gate reads."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.version_file = root / "firmware" / "esp32-csi-node" / "version.txt"
        self.releases = root / "docs" / "releases"
        self.validation = root / "docs" / "validation"
        self.changelog = root / "CHANGELOG.md"
        self.workflow = root / ".github" / "workflows" / "firmware-ci.yml"
        for path in (self.version_file, self.releases, self.validation,
                     self.changelog.parent, self.workflow.parent):
            path.parent.mkdir(parents=True, exist_ok=True)

    def write_version(self, value: str) -> None:
        self.version_file.parent.mkdir(parents=True, exist_ok=True)
        self.version_file.write_text(value + "\n", encoding="utf-8")

    def write_note(self, version: str = CANDIDATE) -> None:
        self.releases.mkdir(parents=True, exist_ok=True)
        (self.releases / f"v{version}-esp32.md").write_text(
            f"# RuView ESP32 firmware {version}\n\nTag: v{version}-esp32\n",
            encoding="utf-8")

    def write_record(self, name: str, body: str) -> None:
        self.validation.mkdir(parents=True, exist_ok=True)
        (self.validation / name).write_text(body, encoding="utf-8")

    def write_workflow(self, text: str = WORKFLOW) -> None:
        self.workflow.parent.mkdir(parents=True, exist_ok=True)
        self.workflow.write_text(text, encoding="utf-8")

    def write_changelog(self, text: str = "# Changelog\n") -> None:
        self.changelog.write_text(text, encoding="utf-8")

    def evaluate(self, version: str = CANDIDATE):
        return evaluate_readiness(
            version,
            version_file=self.version_file,
            releases_dir=self.releases,
            validation_dir=self.validation,
            changelog=self.changelog,
            workflow=self.workflow,
        )


class TestReadinessGate(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.tree = Tree(Path(self.tmp.name))

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def _ready_tree(self) -> Tree:
        self.tree.write_version(CANDIDATE)
        self.tree.write_note()
        self.tree.write_record(
            "2026-10-23-esp32-qualification.md",
            f"# Qualification\n\n{marker()}Firmware {CANDIDATE}.\n\nResult: **PASS**\n")
        self.tree.write_workflow()
        self.tree.write_changelog()
        return self.tree

    def test_a_complete_tree_is_ready(self) -> None:
        self._ready_tree()
        result = self.tree.evaluate()
        self.assertTrue(result["ready"], result["findings"])
        self.assertEqual(result["errors"], 0)

    def test_a_version_mismatch_blocks(self) -> None:
        self._ready_tree()
        self.tree.write_version("0.8.15")
        result = self.tree.evaluate()
        self.assertFalse(result["ready"])
        failed = [f["check"] for f in result["findings"] if not f["pass"]]
        self.assertIn("version.txt", failed)

    def test_a_missing_release_note_blocks(self) -> None:
        self._ready_tree()
        (self.tree.releases / f"v{CANDIDATE}-esp32.md").unlink()
        result = self.tree.evaluate()
        self.assertFalse(result["ready"])
        failed = [f["check"] for f in result["findings"] if not f["pass"]]
        self.assertIn("release note", failed)

    def test_a_missing_qualification_blocks(self) -> None:
        self._ready_tree()
        for path in self.tree.validation.glob("*.md"):
            path.unlink()
        result = self.tree.evaluate()
        self.assertFalse(result["ready"])
        failed = [f["check"] for f in result["findings"] if not f["pass"]]
        self.assertIn("hardware qualification", failed)

    def test_a_failed_qualification_blocks(self) -> None:
        self._ready_tree()
        for path in self.tree.validation.glob("*.md"):
            path.unlink()
        self.tree.write_record(
            "2026-10-23-esp32-qualification.md",
            f"# Qualification\n\n{marker(verdict='FAIL')}Firmware {CANDIDATE}.\n\n"
            f"Result: **FAIL**\n")
        result = self.tree.evaluate()
        self.assertFalse(result["ready"])
        failed = [f["check"] for f in result["findings"] if not f["pass"]]
        self.assertIn("hardware qualification", failed)

    def test_a_record_without_a_verdict_blocks(self) -> None:
        self._ready_tree()
        for path in self.tree.validation.glob("*.md"):
            path.unlink()
        self.tree.write_record(
            "2026-10-23-esp32-qualification.md",
            f"# Qualification\n\n{marker(verdict='')}Firmware {CANDIDATE}.\n")
        result = self.tree.evaluate()
        self.assertFalse(result["ready"])
        failed = [f["check"] for f in result["findings"] if not f["pass"]]
        self.assertIn("hardware qualification", failed)

    def test_a_record_for_another_version_does_not_count(self) -> None:
        self._ready_tree()
        for path in self.tree.validation.glob("*.md"):
            path.unlink()
        self.tree.write_record(
            "2026-08-31-esp32-qualification.md",
            f"# Qualification\n\n{marker(version='0.8.8')}Result: **PASS**\n")
        result = self.tree.evaluate()
        self.assertFalse(result["ready"])

    def test_a_record_without_the_marker_does_not_count(self) -> None:
        self._ready_tree()
        for path in self.tree.validation.glob("*.md"):
            path.unlink()
        self.tree.write_record(
            "2026-08-31-esp32-qualification.md",
            f"# Qualification\n\nFirmware {CANDIDATE}.\n\nResult: **PASS**\n")
        result = self.tree.evaluate()
        self.assertFalse(result["ready"])

    def test_a_tag_the_workflow_rejects_blocks(self) -> None:
        self._ready_tree()
        self.tree.write_workflow(WORKFLOW.replace("- 'v*-esp32'", "- 'v*-firmware'"))
        result = self.tree.evaluate()
        self.assertFalse(result["ready"])
        failed = [f["check"] for f in result["findings"] if not f["pass"]]
        self.assertIn("release workflow", failed)

    def test_a_missing_changelog_is_only_advisory(self) -> None:
        self._ready_tree()
        self.tree.changelog.unlink()
        result = self.tree.evaluate()
        self.assertTrue(result["ready"])


class TestParseQualificationMarker(unittest.TestCase):
    def test_a_marker_is_read(self) -> None:
        fields = parse_qualification_marker(marker())
        self.assertEqual(fields["version"], CANDIDATE)
        self.assertEqual(fields["verdict"], "PASS")
        self.assertEqual(fields["node"], "1")

    def test_a_record_without_a_marker_reads_as_none(self) -> None:
        self.assertIsNone(parse_qualification_marker("# Qualification\n\nResult: **PASS**\n"))


class TestWorkflowTag(unittest.TestCase):
    def test_the_esp32_pattern_accepts_the_tag(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "firmware-ci.yml"
            path.write_text(WORKFLOW, encoding="utf-8")
            self.assertTrue(workflow_accepts_tag(TAG, path))

    def test_a_different_pattern_rejects_the_tag(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "firmware-ci.yml"
            path.write_text(WORKFLOW.replace("- 'v*-esp32'", "- 'v*-firmware'"),
                            encoding="utf-8")
            self.assertFalse(workflow_accepts_tag(TAG, path))

    def test_a_missing_workflow_is_unknown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "absent.yml"
            self.assertIsNone(workflow_accepts_tag(TAG, path))


if __name__ == "__main__":
    unittest.main()

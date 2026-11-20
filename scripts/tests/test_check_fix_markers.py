#!/usr/bin/env python3
"""Unit tests for scripts/check_fix_markers.py."""

from __future__ import annotations

import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.check_fix_markers import (
    ManifestError,
    _pattern_found,
    check_marker,
    load_manifest,
    main,
)


class TestCheckFixMarkers(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp_dir.name)

    def tearDown(self) -> None:
        self.tmp_dir.cleanup()

    def test_load_manifest_valid(self) -> None:
        manifest_path = self.root / "fix-markers.json"
        data = {
            "schema_version": 1,
            "markers": [
                {"id": "Test#1", "files": ["foo.txt"], "require": ["bar"]},
                {"id": "Test#2", "files": ["baz.txt"], "require": ["qux"]},
            ],
        }
        manifest_path.write_text(json.dumps(data), encoding="utf-8")
        loaded = load_manifest(manifest_path)
        self.assertEqual(len(loaded["markers"]), 2)
        self.assertEqual(loaded["markers"][0]["id"], "Test#1")

    def test_load_manifest_not_found(self) -> None:
        non_existent = self.root / "does_not_exist.json"
        with self.assertRaises(ManifestError) as ctx:
            load_manifest(non_existent)
        self.assertIn("manifest not found", str(ctx.exception))

    def test_load_manifest_invalid_json(self) -> None:
        manifest_path = self.root / "bad.json"
        manifest_path.write_text("not json content", encoding="utf-8")
        with self.assertRaises(ManifestError) as ctx:
            load_manifest(manifest_path)
        self.assertIn("manifest is not valid JSON", str(ctx.exception))

    def test_load_manifest_missing_markers(self) -> None:
        manifest_path = self.root / "no_markers.json"
        manifest_path.write_text(json.dumps({"schema_version": 1}), encoding="utf-8")
        with self.assertRaises(ManifestError) as ctx:
            load_manifest(manifest_path)
        self.assertIn("must be an object with a 'markers' array", str(ctx.exception))

    def test_load_manifest_duplicate_ids(self) -> None:
        manifest_path = self.root / "dupes.json"
        data = {
            "markers": [
                {"id": "Dup#1", "files": ["a.txt"]},
                {"id": "Dup#1", "files": ["b.txt"]},
            ]
        }
        manifest_path.write_text(json.dumps(data), encoding="utf-8")
        with self.assertRaises(ManifestError) as ctx:
            load_manifest(manifest_path)
        self.assertIn("duplicate marker ids", str(ctx.exception))

    def test_pattern_found_literal(self) -> None:
        self.assertTrue(_pattern_found("hello world", "world"))
        self.assertFalse(_pattern_found("hello world", "earth"))

    def test_pattern_found_regex(self) -> None:
        self.assertTrue(_pattern_found("rate = 120.0", r"/rate\s*=\s*\d+\.\d+/"))
        self.assertFalse(_pattern_found("rate = invalid", r"/rate\s*=\s*\d+\.\d+/"))

    def test_pattern_found_invalid_regex(self) -> None:
        with self.assertRaises(ManifestError) as ctx:
            _pattern_found("some text", "/(unclosed paren/")
        self.assertIn("invalid regex in pattern", str(ctx.exception))

    def test_check_marker_pass(self) -> None:
        target_file = self.root / "test.py"
        target_file.write_text("def test_fix():\n    return 'safe_value'\n", encoding="utf-8")
        marker = {
            "id": "Test#Pass",
            "files": ["test.py"],
            "require": ["safe_value"],
            "forbid": ["unsafe_pattern"],
        }
        ok, problems = check_marker(marker, root=self.root)
        self.assertTrue(ok)
        self.assertEqual(problems, [])

    def test_check_marker_missing_file(self) -> None:
        marker = {
            "id": "Test#MissingFile",
            "files": ["non_existent.py"],
            "require": ["anything"],
        }
        ok, problems = check_marker(marker, root=self.root)
        self.assertFalse(ok)
        self.assertTrue(any("missing file: non_existent.py" in p for p in problems))

    def test_check_marker_empty_files(self) -> None:
        marker = {"id": "Test#NoFiles", "files": []}
        ok, problems = check_marker(marker, root=self.root)
        self.assertFalse(ok)
        self.assertIn("marker lists no files", problems)

    def test_check_marker_missing_require(self) -> None:
        target_file = self.root / "test.py"
        target_file.write_text("def test_fix():\n    pass\n", encoding="utf-8")
        marker = {
            "id": "Test#MissingReq",
            "files": ["test.py"],
            "require": ["required_constant"],
        }
        ok, problems = check_marker(marker, root=self.root)
        self.assertFalse(ok)
        self.assertTrue(any("required marker absent" in p for p in problems))

    def test_check_marker_forbid_violation(self) -> None:
        target_file = self.root / "test.py"
        target_file.write_text("legacy_bug_reintroduced = True\n", encoding="utf-8")
        marker = {
            "id": "Test#ForbidViolated",
            "files": ["test.py"],
            "require": [],
            "forbid": ["legacy_bug_reintroduced"],
        }
        ok, problems = check_marker(marker, root=self.root)
        self.assertFalse(ok)
        self.assertTrue(any("forbidden pattern re-appeared" in p for p in problems))

    def test_main_cli_options(self) -> None:
        sample_file = self.root / "code.txt"
        sample_file.write_text("shipped_fix_verified", encoding="utf-8")
        manifest_path = self.root / "manifest.json"
        manifest_data = {
            "schema_version": 1,
            "markers": [
                {
                    "id": "M1",
                    "title": "Marker One",
                    "files": ["code.txt"],
                    "require": ["shipped_fix_verified"],
                },
                {
                    "id": "M2",
                    "title": "Marker Two",
                    "files": ["code.txt"],
                    "require": ["missing_pattern"],
                },
            ],
        }
        manifest_path.write_text(json.dumps(manifest_data), encoding="utf-8")

        # --list
        with patch("sys.stdout", new=io.StringIO()) as out:
            code = main(["--manifest", str(manifest_path), "--list"])
            self.assertEqual(code, 0)
            self.assertIn("2 fix markers tracked", out.getvalue())

        # --only M1 (should pass)
        with patch("sys.stdout", new=io.StringIO()):
            code = main(["--manifest", str(manifest_path), "--root", str(self.root), "--only", "M1"])
            self.assertEqual(code, 0)

        # --only M2 (should fail because missing_pattern is absent)
        with patch("sys.stdout", new=io.StringIO()):
            code = main(["--manifest", str(manifest_path), "--root", str(self.root), "--only", "M2"])
            self.assertEqual(code, 1)

        # --only unknown
        with patch("sys.stderr", new=io.StringIO()):
            code = main(["--manifest", str(manifest_path), "--only", "UnknownID"])
            self.assertEqual(code, 2)

        # --json flag with M1
        with patch("sys.stdout", new=io.StringIO()) as out:
            code = main(["--manifest", str(manifest_path), "--root", str(self.root), "--only", "M1", "--json"])
            self.assertEqual(code, 0)
            payload = json.loads(out.getvalue())
            self.assertTrue(payload["ok"])
            self.assertEqual(payload["checked"], 1)
            self.assertEqual(payload["failed"], 0)


if __name__ == "__main__":
    unittest.main()

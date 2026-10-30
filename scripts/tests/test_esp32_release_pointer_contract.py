"""Tests for the ESP32 firmware release pointer contract.

The contract is what keeps the documentation pointed at the release this tree
builds: the guide states a version, and the repository either produces that
version or the check fails. These tests cover the parsers and prove that a
pointer left on an older prerelease is caught.

Nothing here pins a firmware version. The current release is read from the tree,
so a new release does not invalidate the suite - which is the failure this file
had when 0.8.15 was written into it and the tree moved on.
"""
from pathlib import Path
import re
import shutil
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from esp32_release_pointer_contract import (  # noqa: E402
    ARCHIVE,
    POINTER_DOCS,
    RELEASE_NOTES,
    RELEASE_WORKFLOW,
    VERSION_FILE,
    build_variants,
    current_tag,
    current_version,
    read,
    release_archives,
    release_versions,
    verify,
    version_tuple,
)

CURRENT_VERSION = current_version(ROOT)
CURRENT_TAG = current_tag(ROOT)


def bumped_version(version):
    """Return the next patch version above the one given."""
    parts = version.split(".")
    parts[-1] = str(int(parts[-1]) + 1)
    return ".".join(parts)


def stale_tag():
    """Return the newest release tag the documents name below the current one."""
    tags = release_versions(read(ROOT / POINTER_DOCS[0]))
    older = [tag for tag in tags if tag != version_tuple(CURRENT_VERSION)]
    if not older:
        raise AssertionError("no older release tag is named in %s" % POINTER_DOCS[0])
    newest = max(older)
    return "v%s-esp32" % ".".join(str(part) for part in newest)


STALE_TAG = stale_tag()


def clone(target):
    """Copy the files the contract reads into a scratch tree."""
    for relative in (VERSION_FILE, RELEASE_WORKFLOW) + POINTER_DOCS:
        source = ROOT / relative
        destination = target / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    relative = RELEASE_NOTES + "/v%s-esp32.md" % CURRENT_VERSION
    source = ROOT / relative
    destination = target / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    return target


def names(failures):
    return [name for name, _observed, _required in failures]


class Parsers(unittest.TestCase):
    def test_version_tuple_parses_a_dotted_version(self):
        self.assertEqual(version_tuple("0.8.15"), (0, 8, 15))
        self.assertEqual(version_tuple(" 0.4.3.1 \n"), (0, 4, 3, 1))

    def test_version_tuple_rejects_a_non_version(self):
        self.assertIsNone(version_tuple(""))
        self.assertIsNone(version_tuple("0.8.x"))

    def test_release_versions_reads_the_tags_in_a_document(self):
        found = release_versions("see v0.8.15-esp32 and v0.8.8-esp32")
        self.assertEqual(sorted(found), [(0, 8, 8), (0, 8, 15)])

    def test_release_versions_ignores_an_alpha_tag(self):
        self.assertEqual(release_versions("v0.3.0-alpha-esp32"), [])

    def test_the_workflow_variants_are_the_published_archives(self):
        variants = build_variants(ROOT)
        self.assertTrue(variants)
        self.assertEqual(
            release_archives(ROOT),
            tuple(
                "esp32-csi-node-firmware-%s.tar.gz" % variant for variant in variants
            ),
        )
        for relative in POINTER_DOCS:
            named = set(ARCHIVE.findall(read(ROOT / relative)))
            if not named:
                # A document that names no archive is not judged on archives.
                continue
            self.assertEqual(
                named,
                set(release_archives(ROOT)),
                relative,
            )


class TheRepository(unittest.TestCase):
    def test_the_tree_passes_its_own_contract(self):
        self.assertEqual(verify(ROOT), [])

    def test_the_documents_name_the_current_release(self):
        for relative in POINTER_DOCS:
            self.assertIn(CURRENT_TAG, read(ROOT / relative))


class Drift(unittest.TestCase):
    def test_a_pointer_left_on_an_older_prerelease_is_caught(self):
        with tempfile.TemporaryDirectory() as directory:
            tree = clone(Path(directory))
            for relative in POINTER_DOCS:
                path = tree / relative
                path.write_text(
                    read(path).replace(CURRENT_TAG, STALE_TAG), encoding="utf-8"
                )
            failures = verify(tree)
            self.assertTrue(failures)
            self.assertTrue(
                any("newest release tag" in name for name in names(failures)),
                names(failures),
            )

    def test_a_bumped_version_without_a_pointer_update_is_caught(self):
        with tempfile.TemporaryDirectory() as directory:
            tree = clone(Path(directory))
            bumped = bumped_version(CURRENT_VERSION)
            (tree / VERSION_FILE).write_text(bumped + "\n", encoding="utf-8")
            failures = verify(tree)
            self.assertTrue(failures)
            self.assertIn("release notes for %s" % bumped, names(failures))

    def test_a_missing_release_note_is_caught(self):
        with tempfile.TemporaryDirectory() as directory:
            tree = clone(Path(directory))
            (tree / RELEASE_NOTES / ("v%s-esp32.md" % CURRENT_VERSION)).unlink()
            failures = verify(tree)
            self.assertTrue(failures)
            self.assertIn("release notes for %s" % CURRENT_VERSION, names(failures))

    def test_an_archive_the_workflow_does_not_publish_is_caught(self):
        with tempfile.TemporaryDirectory() as directory:
            tree = clone(Path(directory))
            relative = POINTER_DOCS[0]
            path = tree / relative
            path.write_text(
                read(path).replace(
                    "esp32-csi-node-firmware-8mb.tar.gz",
                    "esp32-csi-node-v0.8.8-s3-8mb-flash-bundle.zip",
                ),
                encoding="utf-8",
            )
            failures = verify(tree)
            self.assertTrue(failures)
            self.assertTrue(
                any("archive names" in name for name in names(failures)),
                names(failures),
            )

    def test_a_document_that_stops_naming_the_release_is_caught(self):
        with tempfile.TemporaryDirectory() as directory:
            tree = clone(Path(directory))
            relative = POINTER_DOCS[0]
            path = tree / relative
            path.write_text(
                read(path).replace(CURRENT_TAG, "the current release"),
                encoding="utf-8",
            )
            failures = verify(tree)
            self.assertTrue(failures)
            self.assertTrue(
                any("names %s" % CURRENT_TAG in name for name in names(failures)),
                names(failures),
            )


if __name__ == "__main__":
    unittest.main()

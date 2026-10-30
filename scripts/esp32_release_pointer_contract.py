"""The contract between the ESP32 firmware documentation and the release it points at.

``firmware/esp32-csi-node/version.txt`` names the firmware version this source
tree builds, and the published release that carries that version is the one a
reader has to download. Every document that tells a reader which bundle to
fetch therefore has to name that release: a pointer left on an older
prerelease sends the reader to a flash image that is not the firmware the
surrounding source describes.

The checks live here so they can be unit tested; ``verify-esp32-release-
pointer.py`` is the command line entry point.
"""
from pathlib import Path
import re

FIRMWARE = "firmware/esp32-csi-node"
VERSION_FILE = FIRMWARE + "/version.txt"
RELEASE_NOTES = "docs/releases"
RELEASE_WORKFLOW = ".github/workflows/firmware-ci.yml"

# The documents that tell a reader which firmware to download.
POINTER_DOCS = (
    FIRMWARE + "/README.md",
    FIRMWARE + "/release_bins/README.md",
    "docs/user-guide.md",
)

# A release tag as it appears in a link or in a table cell.
TAG = re.compile(r"v(\d+(?:\.\d+)+)-esp32")

# Any firmware archive name, including the ones older releases published.
ARCHIVE = re.compile(r"esp32-csi-node[-\w.]*\.(?:tar\.gz|zip)")

# A build variant in the release workflow matrix.
VARIANT = re.compile(r"^\s*- variant: (\S+)\s*$", re.MULTILINE)


def read(path):
    return path.read_text(encoding="utf-8")


def version_tuple(text):
    """Return a comparable tuple for a dotted version, or None."""
    parts = text.strip().split(".")
    if not parts or not all(part.isdigit() for part in parts):
        return None
    return tuple(int(part) for part in parts)


def release_versions(text):
    """Return the versions of every ESP32 release tag mentioned in `text`."""
    found = []
    for match in TAG.finditer(text):
        parsed = version_tuple(match.group(1))
        if parsed is not None:
            found.append(parsed)
    return found


def build_variants(root):
    """Return the firmware variants the release workflow builds."""
    workflow = read(root / RELEASE_WORKFLOW)
    return tuple(VARIANT.findall(workflow))


def release_archives(root):
    """Return the archive names the release workflow publishes."""
    return tuple(
        "esp32-csi-node-firmware-%s.tar.gz" % variant
        for variant in build_variants(root)
    )


def current_version(root):
    """Return the firmware version this tree builds."""
    return read(root / VERSION_FILE).strip()


def current_tag(root):
    """Return the release tag that carries this tree's firmware version."""
    return "v%s-esp32" % current_version(root)


def verify(root):
    """Return a list of (name, observed, required) failures."""
    failures = []

    def check(name, observed, required):
        if observed != required:
            failures.append((name, observed, required))

    version = current_version(root)
    parsed = version_tuple(version)
    if parsed is None:
        failures.append(("firmware version", version, "a dotted version"))
        return failures

    tag = "v%s-esp32" % version

    # 1. The release notes for the version this tree builds are in the tree.
    notes = root / RELEASE_NOTES / ("v%s-esp32.md" % version)
    check("release notes for %s" % version, notes.exists(), True)
    if notes.exists():
        heading = read(notes).splitlines()[0]
        check("release notes heading", version in heading, True)

    # 2. Every document that names a bundle names the current release.
    documents = {}
    for relative in POINTER_DOCS:
        path = root / relative
        if not path.exists():
            failures.append(("document", relative, "present"))
            continue
        documents[relative] = read(path)
        check("%s names %s" % (relative, tag), tag in documents[relative], True)

    # 3. No document points at a superseded prerelease.
    for relative, text in documents.items():
        versions = release_versions(text)
        if versions:
            check("%s newest release tag" % relative, max(versions), parsed)

    # 4. The archives named are the ones the release workflow publishes.
    published = release_archives(root)
    for relative, text in documents.items():
        named = sorted(set(ARCHIVE.findall(text)))
        if not named:
            continue
        check("%s archive names" % relative, tuple(named), tuple(sorted(published)))
    return failures

#!/usr/bin/env python3
"""ESP32 firmware stable-release readiness contract (RuView#2092).

This module holds the logic behind
``scripts/check-firmware-release-readiness.py``. It is importable so the
contract can be unit tested.

The ESP32 firmware release has two channels, and they are not the same thing:

* **prerelease** -- the channel the CI workflow publishes. It is built and
  host-tested by CI, and it carries no physical-board qualification and no
  sensing-accuracy claim;
* **stable** -- a release that a person publishes after the firmware has
  passed a hardware qualification on real boards.

The stable channel is the one users are told to flash, so its preconditions
have to be checkable rather than remembered. This module is that check. It
answers one question: *can the candidate version be published as a stable
ESP32 firmware release, on the evidence that is in this tree?*

The preconditions it enforces:

* the firmware version in the tree is the candidate version, because the
  release workflow's version guard refuses a tag that does not match
  ``firmware/esp32-csi-node/version.txt``;
* a release note exists for the candidate, because a stable release is a
  curated note, not a build log;
* a hardware qualification record for the candidate exists in
  ``docs/validation/`` and reports a pass, because a stable release without
  measured hardware evidence is not a stable release;
* the release workflow accepts the candidate's tag form, because otherwise
  pushing the tag publishes nothing.

Findings that do not block the release are reported as advisories.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Dict, List, Optional

REPO_ROOT = Path(__file__).resolve().parent.parent
VERSION_FILE = REPO_ROOT / "firmware" / "esp32-csi-node" / "version.txt"
RELEASES_DIR = REPO_ROOT / "docs" / "releases"
VALIDATION_DIR = REPO_ROOT / "docs" / "validation"
CHANGELOG = REPO_ROOT / "CHANGELOG.md"
RELEASE_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "firmware-ci.yml"

# The machine-readable line the qualification harness writes into its report.
# A record is a qualification for the version it names, so the gate reads
# that line instead of guessing from prose.
MARKER_RE = re.compile(
    r"<!--\s*ruview:esp32-firmware-qualification\s+(?P<fields>[^>]*?)-->")

VERSION_RE = re.compile(r"^v(\d+\.\d+\.\d+)$")


def finding(severity: str, name: str, ok: bool, detail: str) -> Dict[str, Any]:
    return {"severity": severity, "check": name, "pass": bool(ok), "detail": detail}


def read_version(version_file: Path = VERSION_FILE) -> Optional[str]:
    if not version_file.exists():
        return None
    return version_file.read_text(encoding="utf-8").strip()


def release_note_path(version: str, releases_dir: Path = RELEASES_DIR) -> Path:
    return releases_dir / f"v{version}-esp32.md"


def parse_qualification_marker(text: str) -> Optional[Dict[str, str]]:
    """The version, node, and verdict a qualification record declares."""
    match = MARKER_RE.search(text)
    if not match:
        return None
    fields = {}
    for item in match.group("fields").split():
        if "=" in item:
            key, _, value = item.partition("=")
            fields[key] = value
    return fields


def find_qualification_records(version: str, validation_dir: Path = VALIDATION_DIR) -> List[Path]:
    """The validation records that declare a qualification for this version."""
    if not validation_dir.exists():
        return []
    found = []
    for path in sorted(validation_dir.glob("*.md")):
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        marker = parse_qualification_marker(text)
        if marker and marker.get("version") == version:
            found.append(path)
    return found


def workflow_accepts_tag(tag: str, workflow: Path = RELEASE_WORKFLOW) -> Optional[bool]:
    """Whether the release workflow's tag trigger matches the candidate tag."""
    if not workflow.exists():
        return None
    text = workflow.read_text(encoding="utf-8", errors="replace")
    # The trigger block is a literal list of glob patterns under `tags:`.
    match = re.search(r"tags:\s*\n(?P<body>(?:\s*#.*\n)*\s*-\s*'[^']+'\n)+", text)
    if not match:
        return None
    patterns = re.findall(r"-\s*'([^']+)'", match.group("body"))
    for pattern in patterns:
        # The only ESP32 pattern in use is a trailing glob such as v*-esp32.
        prefix = pattern.split("*", 1)[0]
        suffix = pattern.split("*", 1)[-1]
        if tag.startswith(prefix) and tag.endswith(suffix):
            return True
    return False


def check_version_matches(candidate: str, version_file: Path = VERSION_FILE) -> Dict[str, Any]:
    actual = read_version(version_file)
    if actual is None:
        return finding("error", "version.txt", False, f"{version_file} is missing")
    if actual == candidate:
        return finding("error", "version.txt", True, f"the tree builds firmware {actual}")
    return finding("error", "version.txt", False,
                   f"the tree builds firmware {actual}, not {candidate}; the release "
                   f"workflow's version guard refuses a tag that does not match")


def check_release_note(candidate: str, releases_dir: Path = RELEASES_DIR) -> Dict[str, Any]:
    path = release_note_path(candidate, releases_dir)
    try:
        name = path.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        name = path.as_posix()
    if not path.exists():
        return finding("error", "release note", False,
                       f"{name} is missing; a stable release needs a curated note, "
                       f"not a build log")
    text = path.read_text(encoding="utf-8", errors="replace")
    tag = f"v{candidate}-esp32"
    if tag not in text:
        return finding("advisory", "release note", True, f"{name} does not name {tag}")
    return finding("error", "release note", True, f"{name} exists and names {tag}")


def check_qualification(candidate: str, validation_dir: Path = VALIDATION_DIR) -> Dict[str, Any]:
    records = find_qualification_records(candidate, validation_dir)
    try:
        names = ", ".join(p.relative_to(REPO_ROOT).as_posix() for p in records)
    except ValueError:
        names = ", ".join(p.as_posix() for p in records)
    if not records:
        try:
            where = validation_dir.relative_to(REPO_ROOT).as_posix()
        except ValueError:
            where = validation_dir.as_posix()
        return finding("error", "hardware qualification", False,
                       f"no record in {where} declares a qualification for {candidate}; "
                       f"a stable release needs measured hardware evidence")
    verdicts = []
    for path in records:
        text = path.read_text(encoding="utf-8", errors="replace")
        marker = parse_qualification_marker(text) or {}
        verdicts.append(marker.get("verdict", "").upper())
    if any(verdict == "PASS" for verdict in verdicts):
        return finding("error", "hardware qualification", True, f"{names} reports a pass")
    if any(verdict == "FAIL" for verdict in verdicts):
        return finding("error", "hardware qualification", False,
                       f"{names} reports a failure; a stable release needs a "
                       f"qualification that passes")
    return finding("error", "hardware qualification", False,
                   f"{names} does not carry a qualification verdict; run "
                   f"scripts/qualify-esp32-firmware.py --print-md and paste its "
                   f"report into the record")


def check_workflow_tag(candidate: str, workflow: Path = RELEASE_WORKFLOW) -> Dict[str, Any]:
    tag = f"v{candidate}-esp32"
    accepted = workflow_accepts_tag(tag, workflow)
    if accepted is None:
        return finding("advisory", "release workflow", True,
                       "the release workflow's tag trigger could not be read")
    if accepted:
        return finding("error", "release workflow", True,
                       f"the release workflow accepts {tag}")
    return finding("error", "release workflow", False,
                   f"the release workflow's tag trigger does not accept {tag}; "
                   f"pushing it publishes nothing")


def check_changelog(candidate: str, changelog: Path = CHANGELOG) -> Dict[str, Any]:
    if not changelog.exists():
        return finding("advisory", "changelog", True, "the changelog is missing")
    text = changelog.read_text(encoding="utf-8", errors="replace")
    if f"## [v{candidate}-esp32]" in text:
        return finding("advisory", "changelog", True,
                       f"the changelog has a heading for v{candidate}-esp32")
    return finding("advisory", "changelog", True,
                   f"the changelog has no heading for v{candidate}-esp32; the entry "
                   f"is still under Unreleased")


def verify_records(validation_dir: Path = VALIDATION_DIR) -> Dict[str, Any]:
    """Check that every declared qualification record is self-consistent.

    A record that declares a verdict has to carry the matching report, and
    the report has to agree with the verdict. Without this, a record could
    declare a pass and carry a failed run.
    """
    findings: List[Dict[str, Any]] = []
    if not validation_dir.exists():
        return {"records": 0, "findings": findings, "ok": True}
    for path in sorted(validation_dir.glob("*.md")):
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        marker = parse_qualification_marker(text)
        if marker is None:
            continue
        name = path.name
        verdict = (marker.get("verdict") or "").upper()
        version = marker.get("version") or ""
        if verdict not in ("PASS", "FAIL"):
            findings.append(finding("error", name, False,
                                    f"the record declares verdict {verdict!r}; "
                                    f"only PASS or FAIL is a verdict"))
            continue
        if not VERSION_RE.match(f"v{version}"):
            findings.append(finding("error", name, False,
                                    f"the record declares version {version!r}, "
                                    f"which is not a firmware version"))
            continue
        expected = f"Result: **{verdict}**"
        if expected not in text:
            findings.append(finding("error", name, False,
                                    f"the record declares {verdict} but does not "
                                    f"carry the matching report line {expected!r}"))
            continue
        findings.append(finding("error", name, True,
                                f"the record declares a {verdict} for firmware "
                                f"{version} and carries the matching report"))
    return {"records": len(findings), "findings": findings,
            "ok": all(f["pass"] for f in findings)}


def evaluate_readiness(candidate: str, version_file: Path = VERSION_FILE,
                       releases_dir: Path = RELEASES_DIR,
                       validation_dir: Path = VALIDATION_DIR,
                       changelog: Path = CHANGELOG,
                       workflow: Path = RELEASE_WORKFLOW) -> Dict[str, Any]:
    """Run every readiness check for the candidate version."""
    findings = [
        check_version_matches(candidate, version_file),
        check_release_note(candidate, releases_dir),
        check_qualification(candidate, validation_dir),
        check_workflow_tag(candidate, workflow),
        check_changelog(candidate, changelog),
    ]
    errors = [f for f in findings if f["severity"] == "error" and not f["pass"]]
    return {
        "version": candidate,
        "tag": f"v{candidate}-esp32",
        "ready": not errors,
        "findings": findings,
        "errors": len(errors),
    }

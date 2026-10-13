#!/usr/bin/env python3
"""Fix-marker regression guard for RuView.

Reads ``scripts/fix-markers.json`` and asserts that every previously-shipped
fix is still present in the codebase:

* every file listed in a marker must exist;
* every ``require`` pattern must appear in at least one of the marker's files
  (a missing pattern means the fix was probably reverted);
* no ``forbid`` pattern may appear in any of the marker's files
  (a re-appearing anti-pattern means the bug was re-introduced).

A pattern is a literal substring by default. Wrap it in ``/.../`` to treat it
as a (multiline, case-sensitive) regular expression, e.g. ``"/fall_thresh\\s*=\\s*2\\.0/"``.

This is a stdlib-only script — no dependencies, runs anywhere Python 3.8+ does.

Usage::

    python scripts/check_fix_markers.py            # check everything (CI)
    python scripts/check_fix_markers.py --list     # list all markers
    python scripts/check_fix_markers.py --json      # machine-readable result
    python scripts/check_fix_markers.py --only RuView#396 RuView#521
    python scripts/check_fix_markers.py --manifest path/to/markers.json

Exit codes: 0 = all markers OK, 1 = one or more regressions, 2 = bad manifest.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = REPO_ROOT / "scripts" / "fix-markers.json"

# Best-effort UTF-8 stdout (Windows consoles default to cp1252); harmless on
# Linux/CI where it's already UTF-8. We still keep all symbols ASCII below so
# the script works even if reconfigure() is unavailable.
try:  # pragma: no cover - environment-dependent
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

# ANSI colours — disabled automatically when stdout isn't a TTY (CI logs are
# plain either way, but keep them readable locally).
_TTY = sys.stdout.isatty()
def _c(code: str, s: str) -> str:
    return f"\033[{code}m{s}\033[0m" if _TTY else s
GREEN = lambda s: _c("32", s)
RED = lambda s: _c("31", s)
YELLOW = lambda s: _c("33", s)
DIM = lambda s: _c("2", s)
BOLD = lambda s: _c("1", s)

OK_MARK = "PASS"
BAD_MARK = "FAIL"
ARROW = "->"


class ManifestError(Exception):
    pass


def load_manifest(path: Path | None = None) -> dict:
    target_path = path or MANIFEST_PATH
    if not target_path.exists():
        raise ManifestError(f"manifest not found: {target_path}")
    try:
        data = json.loads(target_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        raise ManifestError(f"manifest is not valid JSON: {e}") from e
    if not isinstance(data, dict) or not isinstance(data.get("markers"), list):
        raise ManifestError("manifest must be an object with a 'markers' array")
    ids = [m.get("id") for m in data["markers"]]
    dupes = {i for i in ids if ids.count(i) > 1}
    if dupes:
        raise ManifestError(f"duplicate marker ids: {sorted(dupes)}")
    return data


def _pattern_found(text: str, pattern: str) -> bool:
    if len(pattern) >= 2 and pattern.startswith("/") and pattern.endswith("/"):
        try:
            return re.search(pattern[1:-1], text, re.MULTILINE) is not None
        except re.error as e:
            raise ManifestError(f"invalid regex in pattern {pattern!r}: {e}") from e
    return pattern in text


def check_marker(marker: dict, root: Path | None = None) -> tuple[bool, list[str]]:
    """Return (ok, problems) for a single marker."""
    repo_root = root or REPO_ROOT
    problems: list[str] = []
    files = marker.get("files", [])
    require = marker.get("require", [])
    forbid = marker.get("forbid", [])

    if not files:
        problems.append("marker lists no files")
        return False, problems

    contents: dict[str, str] = {}
    for rel in files:
        p = repo_root / rel
        if not p.exists():
            problems.append(f"missing file: {rel}")
            continue
        try:
            contents[rel] = p.read_text(encoding="utf-8", errors="replace")
        except OSError as e:
            problems.append(f"cannot read {rel}: {e}")

    haystack = "\n".join(contents.values())
    try:
        for pat in require:
            if not _pattern_found(haystack, pat):
                problems.append(f"required marker absent (fix likely reverted): {pat!r}")
        for pat in forbid:
            for rel, text in contents.items():
                if _pattern_found(text, pat):
                    problems.append(f"forbidden pattern re-appeared in {rel} (bug re-introduced?): {pat!r}")
    except ManifestError as e:
        problems.append(str(e))

    return (len(problems) == 0), problems


def cmd_list(manifest: dict) -> int:
    print(BOLD(f"{len(manifest['markers'])} fix markers tracked:\n"))
    for m in manifest["markers"]:
        print(f"  {BOLD(m['id']):<28} {m.get('title', '')}")
        if m.get("ref"):
            print(DIM(f"      {m['ref']}"))
        for f in m.get("files", []):
            print(DIM(f"      - {f}"))
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--list", action="store_true", help="list all markers and exit")
    ap.add_argument("--json", action="store_true", help="emit a JSON result object")
    ap.add_argument("--only", nargs="+", metavar="ID", help="only check the given marker ids")
    ap.add_argument("--manifest", type=Path, default=MANIFEST_PATH, help="path to fix-markers manifest (default: scripts/fix-markers.json)")
    ap.add_argument("--root", type=Path, default=REPO_ROOT, help="path to repository root (default: repo root)")
    args = ap.parse_args(argv)

    try:
        manifest = load_manifest(args.manifest)
    except ManifestError as e:
        print(RED(f"[manifest error] {e}"), file=sys.stderr)
        return 2

    if args.list:
        return cmd_list(manifest)

    markers = manifest["markers"]
    if args.only:
        wanted = set(args.only)
        markers = [m for m in markers if m["id"] in wanted]
        unknown = wanted - {m["id"] for m in markers}
        if unknown:
            print(RED(f"[error] unknown marker id(s): {sorted(unknown)}"), file=sys.stderr)
            return 2

    results = []
    failed = 0
    for m in markers:
        ok, problems = check_marker(m, root=args.root)
        results.append({"id": m["id"], "title": m.get("title", ""), "ok": ok, "problems": problems})
        if not ok:
            failed += 1

    if args.json:
        print(json.dumps({"ok": failed == 0, "checked": len(markers), "failed": failed, "markers": results}, indent=2))
        return 0 if failed == 0 else 1

    print(BOLD(f"Fix-marker regression guard - {len(markers)} marker(s)\n"))
    for r in results:
        if r["ok"]:
            print(f"  {GREEN('[' + OK_MARK + ']')} {r['id']:<28} {DIM(r['title'])}")
        else:
            print(f"  {RED('[' + BAD_MARK + ']')} {BOLD(r['id']):<28} {r['title']}")
            for p in r["problems"]:
                print(f"        {RED(ARROW)} {p}")
    print()
    if failed:
        print(RED(BOLD(f"{failed}/{len(markers)} marker(s) regressed.")))
        print(DIM("  A reverted fix is a regression. Restore the marker, or - if the change is"))
        print(DIM("  intentional - update scripts/fix-markers.json in the same PR with a rationale."))
        return 1
    print(GREEN(BOLD(f"All {len(markers)} fix markers present.")))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

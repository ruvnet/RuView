"""The contract between the Windows WiFi quick start and the repository it documents.

Every fact the guide states has a producer in this repository: the import
style comes from the scripts the repository ships, the verdict string from the
proof, the error message from the collector, and the test counts from the test
files themselves. The checks live here so they can be unit tested;
``verify-windows-wifi-quickstart.py`` is the command line entry point.
"""
from pathlib import Path
import re

GUIDE = "docs/tutorials/windows-wifi-quickstart.md"
EVIDENCE = "docs/tutorials/evidence/windows-wifi-quickstart-2026-10-22"
V1 = "archive/v1"

# The import prefix the repository's own scripts use.
IMPORT_PREFIX = "v1.src.sensing"


def read(path):
    return path.read_text(encoding="utf-8")


def norm(text):
    """Collapse whitespace so a quote can be compared across line wrapping."""
    return re.sub(r"\s+", " ", text).strip()


def cells(line):
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def tables(text):
    """Return every markdown table in `text` as (header, rows) pairs."""
    found = []
    lines = text.splitlines()
    index = 0
    while index < len(lines):
        head = lines[index].strip()
        after = lines[index + 1].strip() if index + 1 < len(lines) else ""
        if head.startswith("|") and re.fullmatch(r"\|[\s:|-]+\|", after):
            header = cells(head)
            rows = []
            cursor = index + 2
            while cursor < len(lines) and lines[cursor].strip().startswith("|"):
                rows.append(cells(lines[cursor]))
                cursor += 1
            found.append((header, rows))
            index = cursor
        else:
            index += 1
    return found


def table_with(guide, first_header):
    """Return the (header, rows) pair of the table whose first column is named."""
    for header, rows in tables(guide):
        if header and header[0] == first_header:
            return header, rows
    raise KeyError(first_header)


def inline_code(text):
    return re.findall(r"`([^`\n]+)`", text)


def code_blocks(text):
    return re.findall(r"```[a-z]*\n(.*?)```", text, re.S)


def test_functions(path):
    """Return the number of test functions defined in a test file."""
    return len(re.findall(r"^[ \t]*def test_", read(path), re.MULTILINE))


def verify(root, guide=None):
    """Return a list of (check, observed, required) failures for the guide."""
    root = Path(root)
    if guide is None:
        guide = read(root / GUIDE)
    failures = []

    def check(name, observed, required):
        if observed != required:
            failures.append((name, str(observed), str(required)))

    # 1. Every repository path the guide names exists.
    for path in sorted(set(inline_code(guide))):
        if not re.match(r"^(docs|archive|v2|ui|scripts|docker|verify)", path):
            continue
        target = root / (path.rstrip("/") if path.endswith("/") else path)
        check("path %s" % path, target.exists(), True)

    # 2. The import style the guide teaches is the one the repository uses.
    monitor = read(root / V1 / "tests/integration/live_sense_monitor.py")
    unit = read(root / V1 / "tests/unit/test_sensing.py")
    check("the monitor imports %s" % IMPORT_PREFIX,
          "from %s." % IMPORT_PREFIX in monitor, True)
    check("the unit tests import %s" % IMPORT_PREFIX,
          "from %s." % IMPORT_PREFIX in unit, True)
    check("the guide teaches %s" % IMPORT_PREFIX,
          "from %s." % IMPORT_PREFIX in guide, True)

    # 3. The path the guide puts on PYTHONPATH is the one that holds the
    #    package: the directory that contains "v1".
    check("the v1 package sits under archive",
          (root / "archive/v1/__init__.py").exists(), True)
    check("the guide puts archive on the path",
          'PYTHONPATH = "archive"' in guide, True)
    check("the guide does not put archive/v1 on the path",
          'PYTHONPATH = "archive/v1"' in guide, False)

    # 4. The verdict string the guide quotes is the one the proof prints.
    proof = read(root / V1 / "data/proof/verify.py")
    verdict = "VERDICT: PASS"
    check("the proof prints the verdict", verdict in proof, True)
    check("the guide quotes the verdict", verdict in guide, True)

    # 5. The error the guide quotes is the one the collector raises.
    collector = read(root / V1 / "src/sensing/rssi_collector.py")
    check("the collector refuses a gated netsh",
          "netsh cannot read the WLAN interfaces" in collector, True)
    check("the guide quotes the refusal",
          "netsh cannot read the WLAN interfaces" in guide, True)
    check("the collector refuses a missing signal line",
          "no signal data" in collector, True)
    check("the collector reads the rssi unit",
          'strip().split()[0]' in collector, True)

    # 6. The test counts the guide states are the counts in the files.
    unit_count = test_functions(root / V1 / "tests/unit/test_sensing.py")
    gate_count = test_functions(root / V1 / "tests/unit/test_rssi_collector_gate.py")
    live_count = test_functions(
        root / V1 / "tests/integration/test_windows_live_sensing.py"
    )
    check("the unit test count the guide states",
          "%d tests pass" % unit_count in guide, True)
    check("the gate test count the guide states",
          "%d passed" % gate_count in guide, True)
    check("the live test count the guide states",
          "%d tests, skipped" % live_count in guide, True)

    # 7. Every claim the guide makes about the capture is in the committed
    #    evidence. The guide's code blocks are commands, not quotations, so
    #    the check runs over the claims rather than over every line.
    corpus = norm(read(root / EVIDENCE / "before-after.txt"))
    claims = (
        "OSError: [WinError 126]",
        "RSSI: -80.0 dBm, Quality: 0%",
        "VERDICT: PASS",
        "f8e76f21a0f9852b70b6d9dd5318239f6b20cbcb4cdd995863263cecdc446f7a",
        "netsh cannot read the WLAN interfaces",
        "Requires Windows with connected WiFi",
        "Network shell commands need location permission",
    )
    for claim in claims:
        check("the capture holds %r" % claim[:40], norm(claim) in corpus, True)
        check("the guide states %r" % claim[:40], norm(claim) in norm(guide), True)

    # 8. The capture carries no raw identifier and no person data.
    person_keys = (
        "presence", "motion_level", "person_count", "estimated_persons",
        "breathing_band_power", "amplitude", "persons", "keypoints", "bbox",
    )
    person_field = re.compile(
        r'"(?:%s)":\s*(?!\[?"<redacted>")' % "|".join(person_keys)
    )
    for path in sorted((root / EVIDENCE).glob("*.txt")):
        evidence = read(path)
        raw_mac = re.search(r"\b(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}\b", evidence)
        check("no raw MAC address in %s" % path.name,
              raw_mac.group(0) if raw_mac else None, None)
        person = person_field.search(evidence)
        check("no person data in %s" % path.name,
              person.group(0) if person else None, None)

    return failures

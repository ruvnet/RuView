"""The contract between the ESP32-S3 quick start and the repository it documents.

Every fact the guide states has a producer in this repository: the firmware
version comes from ``version.txt``, the release table from the firmware
release workflow, the flash offsets from the partition tables, the flags from
the two CLIs, the container ports from the compose file, the environment
variables from the sources that read them, the dashboard banners from the
dashboard, and every quoted log line from the captured evidence committed
next to the guide.

The checks live here so they can be unit tested; ``verify-esp32-quickstart.py``
is the command line entry point.
"""
from pathlib import Path
import re

GUIDE = "docs/tutorials/esp32-s3-csi-quickstart.md"
EVIDENCE = "docs/tutorials/evidence/esp32-s3-quickstart-2026-10-22"
FIRMWARE = "firmware/esp32-csi-node"
SERVER = "v2/crates/wifi-densepose-sensing-server/src"

# Partition table used for each flash size, as the firmware release workflow
# selects them.
FLASH_TABLES = {
    "4 MB": "partitions_4mb.csv",
    "8 MB": "partitions_display.csv",
    "16 MB": "partitions_16mb.csv",
}

# Cargo's own switches, which the guide uses in front of the server command.
CARGO_FLAGS = {"--release", "--locked", "--offline"}

# Requests the guide makes to the server, which are not log quotations.
HTTP_VERBS = ("GET", "POST", "PUT", "DELETE", "PATCH")


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


def code_blocks(text):
    return re.findall(r"```[a-z]*\n(.*?)```", text, re.S)


def inline_code(text):
    return re.findall(r"`([^`\n]+)`", text)


def partition_offsets(path):
    """Return {partition: offset} for one partition table CSV."""
    offsets = {}
    for line in read(path).splitlines():
        line = line.strip()
        if not line or line.startswith("#") or line.lower().startswith("name,"):
            continue
        fields = [field.strip() for field in line.split(",")]
        if len(fields) < 4:
            continue
        offsets[fields[0]] = fields[3]
    return offsets


def rust_flags(source):
    """Return {--flag: default or None} for every clap argument in `source`."""
    flags = {}
    for match in re.finditer(
        r"#\[arg\((?P<attr>[^)]*)\)\]\s*(?:pub\s+)?(?P<name>\w+)\s*:\s*[^,]+,",
        source,
    ):
        attribute = match.group("attr")
        if "long" not in attribute:
            continue
        default = re.search(r'default_value\s*=\s*"([^"]*)"', attribute)
        flags["--" + match.group("name").replace("_", "-")] = (
            default.group(1) if default else None
        )
    return flags


def server_flags(root):
    """Return the flags of the sensing-server binary and its library CLI.

    The binary's own arguments live in ``main.rs``; ``cli.rs`` holds the
    library-side groups it reuses.
    """
    flags = {}
    for name in ("main.rs", "cli.rs"):
        flags.update(rust_flags(read(root / SERVER / name)))
    return flags


def python_flags(source):
    """Return the set of long options declared by an argparse script."""
    return set(re.findall(r'add_argument\(\s*"(--[a-z0-9][a-z0-9-]*)"', source))


def workflow_matrix(source):
    """Return {variant: {key: value}} for the firmware release matrix."""
    matrix = {}
    for match in re.finditer(
        r"- variant: (\S+)\n((?:[ \t]+(?:\w+|#).*\n)+)", source
    ):
        entry = dict(
            re.findall(r"^\s+(\w+):\s*(\S+)\s*$", match.group(2), re.MULTILINE)
        )
        matrix[match.group(1)] = entry
    return matrix


def dashboard_strings(root):
    """Return the dashboard's user-visible strings, with its escapes decoded."""
    dashboard = "\n".join(
        path.read_text(encoding="utf-8", errors="replace")
        for path in (root / "ui").rglob("*.js")
    )
    return re.sub(
        r"\\u([0-9a-fA-F]{4})", lambda m: chr(int(m.group(1), 16)), dashboard
    )


def log_records(text):
    """Return the log lines quoted in `text`, rejoined across line wrapping."""
    records = []
    for block in code_blocks(text):
        record = None
        for line in block.splitlines():
            stripped = line.strip()
            if re.match(r"^(?:[IWEVD]\s*\(\d+\)|ERROR:|WARN:|INFO:)", stripped):
                if record:
                    records.append(record)
                record = stripped
            elif record and stripped:
                record += " " + stripped
            else:
                record = None
        if record:
            records.append(record)
    return records


def verify(root, guide=None):
    """Return a list of (check, observed, required) failures for the guide.

    `guide` overrides the file on disk, which lets a test ask what a changed
    guide would fail on.
    """
    root = Path(root)
    if guide is None:
        guide = read(root / GUIDE)
    corpus = norm(
        "\n".join(
            path.read_text(encoding="utf-8", errors="replace")
            for path in sorted((root / EVIDENCE).glob("*.txt"))
        )
    )
    failures = []

    def check(name, observed, required):
        if observed != required:
            failures.append((name, str(observed), str(required)))

    # 1. The firmware version the guide names is the version the build uses.
    version = re.search(r"Firmware version: `([0-9]+\.[0-9]+\.[0-9]+)`", guide)
    built = read(root / FIRMWARE / "version.txt").strip()
    check("firmware version", version.group(1) if version else None, built)

    # 2. The release table is the table the release workflow builds.
    matrix = workflow_matrix(read(root / ".github/workflows/firmware-ci.yml"))
    _, rows = table_with(guide, "Asset")
    listed = {}
    for row in rows:
        asset = row[0].strip("`")
        listed[asset.replace("esp32-csi-node-firmware-", "").replace(".tar.gz", "")] = row
    check("release variants", ", ".join(sorted(listed)), ", ".join(sorted(matrix)))
    for variant, row in sorted(listed.items()):
        entry = matrix.get(variant)
        if entry is None:
            continue
        check("release partition table (%s)" % variant,
              row[2].strip("`"), entry["partition_table_name"])
        check("release app image (%s)" % variant,
              row[3].strip("`"), entry["artifact_app"])
        check("release partition image (%s)" % variant,
              row[4].strip("`"), entry["artifact_pt"])
    check("release checksum file", "SHA256SUMS.txt" in guide, True)

    # 3. The flash offsets are the offsets the partition tables define.
    header, rows = table_with(guide, "Flash size")
    partitions = [column.strip("`") for column in header[2:]]
    for row in rows:
        size, table = row[0], row[1].strip("`")
        check("partition table for %s" % size, table, FLASH_TABLES.get(size))
        offsets = partition_offsets(root / FIRMWARE / table)
        for position, partition in enumerate(partitions):
            cell = row[2 + position].strip("`")
            if cell in ("", "-"):
                continue
            check("%s offset (%s)" % (partition, size),
                  cell.lower(), offsets.get(partition, "").lower())
    for partition, offset in re.findall(r"`(\w+)` at `(0x[0-9a-f]+)`", guide):
        values = {
            partition_offsets(root / FIRMWARE / name).get(partition, "").lower()
            for name in FLASH_TABLES.values()
        }
        check("extra partition offset (%s)" % partition, offset.lower() in values, True)

    # 4. The flags the guide uses exist, and the defaults it quotes are real.
    flags = server_flags(root)
    provision_flags = python_flags(read(root / FIRMWARE / "provision.py"))
    push_flags = python_flags(read(root / FIRMWARE / "config_push.py"))
    for block in code_blocks(guide):
        used = set(re.findall(r"(?<![\w-])--[a-z][a-z0-9-]+", block)) - CARGO_FLAGS
        if "esptool" in block:
            continue
        if "provision.py" in block:
            allowed = provision_flags
        elif "config_push.py" in block:
            allowed = push_flags
        else:
            allowed = flags
        for flag in sorted(used):
            check("flag %s" % flag, flag in allowed, True)
    for flag, value in re.findall(r"`(--[a-z][a-z0-9-]*) ([^`]+)`", guide):
        if flag in flags and flags[flag] is not None:
            check("default for %s" % flag, value, flags[flag])

    # 5. The container publishes the ports the guide names. Only the paragraph
    # that states them counts; a quoted server log also carries an address.
    compose = read(root / "docker/docker-compose.yml")
    paragraphs = [p for p in guide.split("\n\n") if "publishes" in p]
    stated = sorted(set(re.findall(r"127\.0\.0\.1:\d+", " ".join(paragraphs))))
    check("container ports stated", bool(stated), True)
    for port in stated:
        check("container port %s" % port, port in compose, True)

    # 6. Every environment variable the guide names is read by the repository.
    # A name counts as a variable when the guide presents it as one: it is
    # quoted inline or it is assigned somewhere. The node's serial onboarding
    # commands share the RUVIEW_ prefix and are not variables.
    sources = [read(root / SERVER / "cli.rs"),
               read(root / "docker/docker-entrypoint.sh"), compose]
    sources += [read(path) for path in (root / SERVER).glob("*.rs")]
    known = "\n".join(sources)
    quoted = set(inline_code(guide))
    assigned = set(re.findall(r"\b(RUVIEW_[A-Z0-9_]+|CSI_SOURCE)\s*=", guide))
    for variable in sorted(
        set(re.findall(r"\b(?:RUVIEW|CSI_SOURCE)[A-Z0-9_]*\b", guide))
    ):
        if variable not in quoted and variable not in assigned:
            continue
        check("environment variable %s" % variable, variable in known, True)

    # 7. The dashboard banners the guide names are the ones the dashboard uses.
    dashboard = dashboard_strings(root)
    _, rows = table_with(guide, "Dashboard banner")
    for row in rows:
        banner = row[0].strip("`")
        check("dashboard banner %s" % banner, banner in dashboard, True)

    # 8. Every repository path the guide names exists.
    for path in sorted(set(inline_code(guide))):
        if not re.match(r"^(docs|firmware|v2|ui|scripts|docker|\.github)/", path):
            continue
        target = root / (path.rstrip("/") if path.endswith("/") else path)
        check("path %s" % path, target.exists(), True)

    # 9. The captured evidence carries no raw identifier and no person data.
    person_keys = (
        "presence", "motion_level", "person_count", "person_count_valid",
        "estimated_persons", "breathing_band_power", "motion_band_power",
        "spectral_power", "amplitude", "persons", "keypoints", "bbox",
        "room_inference", "signal_field", "node_inference", "classification",
    )
    person_field = re.compile(
        r'"(?:%s)":\s*(?!\[?"<redacted>")' % "|".join(person_keys)
    )
    for path in sorted((root / EVIDENCE).glob("*.txt")):
        evidence = read(path)
        raw_mac = re.search(r"\b(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}\b", evidence)
        check("no raw MAC address in %s" % path.name,
              raw_mac.group(0) if raw_mac else None, None)
        raw_password = re.search(r"(?i)\bpassword\s*=\s*(?!\*\*\*)\S", evidence)
        check("no unmasked password in %s" % path.name,
              raw_password.group(0) if raw_password else None, None)
        person = person_field.search(evidence)
        check("no person data in %s" % path.name,
              person.group(0) if person else None, None)

    # 10. Every quoted log line is in the captured evidence.
    for record in log_records(guide):
        text = re.sub(r"^(?:ERROR|WARN|INFO):\s*", "", record)
        check("quoted log line", norm(text) in corpus, True)

    # 11. Every inline quote taken from a log or from the API is in the evidence.
    # Only the provenance column of the measurement table and the "not accepted"
    # column of the acceptance table claim what a capture actually showed.
    provenance = []
    _, rows = table_with(guide, "Fact")
    for row in rows:
        provenance += inline_code(row[2])
    _, rows = table_with(guide, "Question")
    for row in rows:
        provenance += inline_code(row[3])
    for span in provenance:
        if "..." in span or "…" in span:
            continue
        if span.startswith(HTTP_VERBS):
            continue
        check("inline quote %s" % span[:48], norm(span) in corpus, True)

    return failures

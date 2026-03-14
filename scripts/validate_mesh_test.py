#!/usr/bin/env python3
"""
QEMU Multi-Node Mesh Validation (ADR-061 Layer 3)

Validates the output of a multi-node mesh simulation run by qemu-mesh-test.sh.
Parses the aggregator results JSON and per-node UART logs, then runs 6 checks:

  1. All nodes booted          - every node log contains a boot indicator
  2. TDM ordering              - slot assignments are sequential 0..N-1
  3. No slot collision         - no two nodes share a TDM slot
  4. Frame count balance       - per-node frame counts within +/-10%
  5. ADR-018 compliance        - magic 0xC5110001 present in frames
  6. Vitals per node           - each node produced vitals output

Usage:
    python3 validate_mesh_test.py --nodes N [results.json] [--log node0.log] ...

Exit codes:
    0  All checks passed (or only SKIP-level)
    1  Warnings (non-critical checks failed)
    2  Errors (critical checks failed)
    3  Fatal (crash or missing nodes)
"""

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from enum import IntEnum
from pathlib import Path
from typing import Dict, List, Optional


# ---------------------------------------------------------------------------
# Severity / reporting (matches validate_qemu_output.py pattern)
# ---------------------------------------------------------------------------

class Severity(IntEnum):
    PASS = 0
    SKIP = 1
    WARN = 2
    ERROR = 3
    FATAL = 4


USE_COLOR = sys.stdout.isatty()


def color(text: str, code: str) -> str:
    if not USE_COLOR:
        return text
    return f"\033[{code}m{text}\033[0m"


def green(text: str) -> str:
    return color(text, "32")


def yellow(text: str) -> str:
    return color(text, "33")


def red(text: str) -> str:
    return color(text, "31")


def bold_red(text: str) -> str:
    return color(text, "1;31")


@dataclass
class CheckResult:
    name: str
    severity: Severity
    message: str
    count: int = 0


@dataclass
class ValidationReport:
    checks: List[CheckResult] = field(default_factory=list)

    def add(self, name: str, severity: Severity, message: str, count: int = 0):
        self.checks.append(CheckResult(name, severity, message, count))

    @property
    def max_severity(self) -> Severity:
        if not self.checks:
            return Severity.PASS
        return max(c.severity for c in self.checks)

    def print_report(self):
        print("\n" + "=" * 60)
        print("  Multi-Node Mesh Validation Report (ADR-061 Layer 3)")
        print("=" * 60 + "\n")

        for check in self.checks:
            if check.severity == Severity.PASS:
                icon = green("PASS")
            elif check.severity == Severity.SKIP:
                icon = yellow("SKIP")
            elif check.severity == Severity.WARN:
                icon = yellow("WARN")
            elif check.severity == Severity.ERROR:
                icon = red("FAIL")
            else:
                icon = bold_red("FATAL")

            count_str = f" (count={check.count})" if check.count > 0 else ""
            print(f"  [{icon}] {check.name}: {check.message}{count_str}")

        print()

        passed = sum(1 for c in self.checks if c.severity <= Severity.SKIP)
        total = len(self.checks)
        summary = f"  {passed}/{total} checks passed"

        max_sev = self.max_severity
        if max_sev <= Severity.SKIP:
            print(green(summary))
        elif max_sev == Severity.WARN:
            print(yellow(summary + " (with warnings)"))
        elif max_sev == Severity.ERROR:
            print(red(summary + " (with errors)"))
        else:
            print(bold_red(summary + " (FATAL issues detected)"))

        print()


# ---------------------------------------------------------------------------
# Log parsing helpers
# ---------------------------------------------------------------------------

def check_node_booted(log_text: str) -> bool:
    """Return True if the log shows a boot indicator."""
    boot_patterns = [r"app_main\(\)", r"main_task:", r"main:", r"ESP32-S3 CSI Node"]
    return any(re.search(p, log_text) for p in boot_patterns)


def check_node_crashed(log_text: str) -> Optional[str]:
    """Return first crash line or None."""
    crash_patterns = [
        r"Guru Meditation", r"assert failed", r"abort\(\)",
        r"panic", r"LoadProhibited", r"StoreProhibited",
        r"InstrFetchProhibited", r"IllegalInstruction",
    ]
    for line in log_text.splitlines():
        for pat in crash_patterns:
            if re.search(pat, line):
                return line.strip()[:120]
    return None


def extract_node_id_from_log(log_text: str) -> Optional[int]:
    """Try to extract the node_id from UART log lines."""
    patterns = [
        r"node_id[=: ]+(\d+)",
        r"Node ID[=: ]+(\d+)",
        r"TDM slot[=: ]+(\d+)",
    ]
    for line in log_text.splitlines():
        for pat in patterns:
            m = re.search(pat, line, re.IGNORECASE)
            if m:
                try:
                    return int(m.group(1))
                except (ValueError, IndexError):
                    pass
    return None


def check_vitals_in_log(log_text: str) -> bool:
    """Return True if the log contains vitals output."""
    vitals_patterns = [r"vitals", r"breathing", r"breathing_bpm",
                       r"heart_rate", r"heartrate"]
    return any(
        re.search(p, line, re.IGNORECASE)
        for line in log_text.splitlines()
        for p in vitals_patterns
    )


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------

def validate_mesh(
    n_nodes: int,
    results_path: Optional[Path],
    log_paths: List[Path],
) -> ValidationReport:
    """Run all 6 mesh validation checks."""
    report = ValidationReport()

    # Load aggregator results if available
    results: Optional[dict] = None
    if results_path:
        if not results_path.exists():
            print(f"WARNING: Aggregator results file not found: {results_path}",
                  file=sys.stderr)
            report.add("Results JSON", Severity.WARN,
                        f"Results file not found: {results_path}")
        else:
            try:
                results = json.loads(results_path.read_text(encoding="utf-8"))
            except (json.JSONDecodeError, OSError) as exc:
                report.add("Results JSON", Severity.ERROR,
                            f"Failed to parse results: {exc}")

    # Load per-node logs
    node_logs: Dict[int, str] = {}
    for idx, lp in enumerate(log_paths):
        if lp.exists():
            node_logs[idx] = lp.read_text(encoding="utf-8", errors="replace")
        else:
            node_logs[idx] = ""

    # ---- Check 1: All nodes booted ----
    booted = []
    not_booted = []
    crashed = []
    for idx in range(n_nodes):
        log_text = node_logs.get(idx, "")
        if not log_text.strip():
            not_booted.append(idx)
            continue
        crash_line = check_node_crashed(log_text)
        if crash_line:
            crashed.append((idx, crash_line))
        if check_node_booted(log_text):
            booted.append(idx)
        else:
            not_booted.append(idx)

    if crashed:
        crash_desc = "; ".join(f"node {i}: {msg}" for i, msg in crashed)
        report.add("All nodes booted", Severity.FATAL,
                    f"Crash detected: {crash_desc}", count=len(crashed))
    elif len(booted) == n_nodes:
        report.add("All nodes booted", Severity.PASS,
                    f"All {n_nodes} nodes booted successfully", count=n_nodes)
    elif len(booted) == 0:
        report.add("All nodes booted", Severity.FATAL,
                    f"No nodes booted (expected {n_nodes})")
    else:
        missing = ", ".join(str(i) for i in not_booted)
        report.add("All nodes booted", Severity.ERROR,
                    f"{len(booted)}/{n_nodes} booted; missing: [{missing}]",
                    count=len(booted))

    # ---- Check 2: TDM ordering ----
    # Extract TDM slots either from aggregator results or from logs
    tdm_slots: Dict[int, int] = {}

    # Try aggregator results first
    if results and "nodes" in results:
        for node_entry in results["nodes"]:
            nid = node_entry.get("node_id")
            slot = node_entry.get("tdm_slot")
            if nid is not None and slot is not None:
                tdm_slots[int(nid)] = int(slot)

    # Fall back to log extraction
    if not tdm_slots:
        for idx in range(n_nodes):
            log_text = node_logs.get(idx, "")
            nid = extract_node_id_from_log(log_text)
            if nid is not None:
                tdm_slots[idx] = nid

    if len(tdm_slots) == n_nodes:
        expected = list(range(n_nodes))
        actual = [tdm_slots.get(i, -1) for i in range(n_nodes)]
        if actual == expected:
            report.add("TDM ordering", Severity.PASS,
                        f"Slots sequential 0..{n_nodes - 1}")
        else:
            report.add("TDM ordering", Severity.ERROR,
                        f"Expected slots {expected}, got {actual}")
    elif len(tdm_slots) > 0:
        report.add("TDM ordering", Severity.WARN,
                    f"Only {len(tdm_slots)}/{n_nodes} TDM slots detected",
                    count=len(tdm_slots))
    else:
        report.add("TDM ordering", Severity.SKIP,
                    "No TDM slot info found in results or logs")

    # ---- Check 3: No slot collision ----
    if tdm_slots:
        slot_to_nodes: Dict[int, List[int]] = {}
        for nid, slot in tdm_slots.items():
            slot_to_nodes.setdefault(slot, []).append(nid)

        collisions = {s: nodes for s, nodes in slot_to_nodes.items() if len(nodes) > 1}
        if not collisions:
            report.add("No slot collision", Severity.PASS,
                        f"All {len(tdm_slots)} slots unique")
        else:
            desc = "; ".join(f"slot {s}: nodes {ns}" for s, ns in collisions.items())
            report.add("No slot collision", Severity.ERROR,
                        f"Slot collisions: {desc}", count=len(collisions))
    else:
        report.add("No slot collision", Severity.SKIP,
                    "No TDM slot data to check for collisions")

    # ---- Check 4: Frame count balance (within +/-10%) ----
    frame_counts: Dict[int, int] = {}

    # Try aggregator results
    if results and "nodes" in results:
        for node_entry in results["nodes"]:
            nid = node_entry.get("node_id")
            fc = node_entry.get("frame_count", node_entry.get("frames", 0))
            if nid is not None:
                frame_counts[int(nid)] = int(fc)

    # Fall back to log extraction
    if not frame_counts:
        for idx in range(n_nodes):
            log_text = node_logs.get(idx, "")
            frame_pats = [
                r"frame[_ ]count[=: ]+(\d+)",
                r"frames?[=: ]+(\d+)",
                r"emitted[=: ]+(\d+)",
            ]
            max_fc = 0
            for line in log_text.splitlines():
                for pat in frame_pats:
                    m = re.search(pat, line, re.IGNORECASE)
                    if m:
                        try:
                            max_fc = max(max_fc, int(m.group(1)))
                        except (ValueError, IndexError):
                            pass
            if max_fc > 0:
                frame_counts[idx] = max_fc

    if len(frame_counts) >= 2:
        counts = list(frame_counts.values())
        avg = sum(counts) / len(counts)
        if avg > 0:
            max_deviation = max(abs(c - avg) / avg for c in counts)
            details = ", ".join(f"node {nid}={fc}" for nid, fc in sorted(frame_counts.items()))
            if max_deviation <= 0.10:
                report.add("Frame count balance", Severity.PASS,
                            f"Within +/-10% (avg={avg:.0f}): {details}",
                            count=int(avg))
            elif max_deviation <= 0.25:
                report.add("Frame count balance", Severity.WARN,
                            f"Deviation {max_deviation:.0%} exceeds 10%: {details}",
                            count=int(avg))
            else:
                report.add("Frame count balance", Severity.ERROR,
                            f"Severe imbalance {max_deviation:.0%}: {details}",
                            count=int(avg))
        else:
            report.add("Frame count balance", Severity.ERROR,
                        "All frame counts are zero")
    elif len(frame_counts) == 1:
        report.add("Frame count balance", Severity.WARN,
                    f"Only 1 node reported frames: {frame_counts}")
    else:
        report.add("Frame count balance", Severity.WARN,
                    "No frame count data found")

    # ---- Check 5: ADR-018 compliance (magic 0xC5110001) ----
    ADR018_MAGIC = "c5110001"
    magic_found = False

    # Check aggregator results
    if results:
        results_str = json.dumps(results).lower()
        if ADR018_MAGIC in results_str or "0xc5110001" in results_str:
            magic_found = True
        # Also check a dedicated field
        if results.get("adr018_magic") or results.get("magic"):
            magic_found = True
        # Check per-node entries
        if "nodes" in results:
            for node_entry in results["nodes"]:
                magic = node_entry.get("magic", "")
                if isinstance(magic, str) and ADR018_MAGIC in magic.lower():
                    magic_found = True
                elif isinstance(magic, int) and magic == 0xC5110001:
                    magic_found = True

    # Check logs for serialization/ADR-018 markers
    if not magic_found:
        for idx in range(n_nodes):
            log_text = node_logs.get(idx, "")
            adr018_pats = [
                r"0xC5110001",
                r"c5110001",
                r"ADR-018",
                r"magic[=: ]+0x[Cc]5110001",
            ]
            if any(re.search(p, log_text, re.IGNORECASE) for p in adr018_pats):
                magic_found = True
                break

    if magic_found:
        report.add("ADR-018 compliance", Severity.PASS,
                    "Magic 0xC5110001 found in frame data")
    else:
        report.add("ADR-018 compliance", Severity.WARN,
                    "Magic 0xC5110001 not found (may require deeper frame inspection)")

    # ---- Check 6: Vitals per node ----
    vitals_nodes = []
    no_vitals_nodes = []
    for idx in range(n_nodes):
        log_text = node_logs.get(idx, "")
        if check_vitals_in_log(log_text):
            vitals_nodes.append(idx)
        else:
            no_vitals_nodes.append(idx)

    # Also check aggregator results for vitals data
    if results and "nodes" in results:
        for node_entry in results["nodes"]:
            nid = node_entry.get("node_id")
            has_vitals = (
                node_entry.get("vitals") is not None
                or node_entry.get("breathing_bpm") is not None
                or node_entry.get("heart_rate") is not None
            )
            if has_vitals and nid is not None and int(nid) not in vitals_nodes:
                vitals_nodes.append(int(nid))
                if int(nid) in no_vitals_nodes:
                    no_vitals_nodes.remove(int(nid))

    if len(vitals_nodes) == n_nodes:
        report.add("Vitals per node", Severity.PASS,
                    f"All {n_nodes} nodes produced vitals output",
                    count=n_nodes)
    elif len(vitals_nodes) > 0:
        missing = ", ".join(str(i) for i in no_vitals_nodes)
        report.add("Vitals per node", Severity.WARN,
                    f"{len(vitals_nodes)}/{n_nodes} nodes have vitals; "
                    f"missing: [{missing}]",
                    count=len(vitals_nodes))
    else:
        report.add("Vitals per node", Severity.WARN,
                    "No vitals output found from any node")

    return report


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Validate multi-node mesh QEMU test output (ADR-061 Layer 3)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python3 validate_mesh_test.py --nodes 3 --results mesh_results.json\n"
            "  python3 validate_mesh_test.py --nodes 4 --log node0.log --log node1.log"
        ),
    )
    parser.add_argument("--results", default=None,
                        help="Path to mesh_test_results.json from aggregator")
    parser.add_argument("--nodes", "-n", type=int, required=True,
                        help="Expected number of mesh nodes")
    parser.add_argument("--log", action="append", default=[],
                        help="Path to a per-node QEMU log (can be repeated)")

    args = parser.parse_args()

    if args.nodes < 2:
        print("ERROR: --nodes must be >= 2", file=sys.stderr)
        sys.exit(3)

    results_path = Path(args.results) if args.results else None
    log_paths = [Path(lp) for lp in args.log]

    # If no log files given, try the conventional paths
    if not log_paths:
        for i in range(args.nodes):
            candidate = Path(f"build/qemu_node{i}.log")
            if candidate.exists():
                log_paths.append(candidate)

    report = validate_mesh(args.nodes, results_path, log_paths)
    report.print_report()

    # Map max severity to exit code
    max_sev = report.max_severity
    if max_sev <= Severity.SKIP:
        sys.exit(0)
    elif max_sev == Severity.WARN:
        sys.exit(1)
    elif max_sev == Severity.ERROR:
        sys.exit(2)
    else:
        sys.exit(3)


if __name__ == "__main__":
    main()

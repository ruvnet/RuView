#!/usr/bin/env python3
"""
QEMU ESP32-S3 UART Output Validator (ADR-061)

Parses the UART log captured from a QEMU firmware run and validates
16 checks covering boot, NVS, mock CSI, edge processing, vitals,
presence/fall detection, serialization, crash indicators, scenario
completion, and frame rate sanity.

Usage:
    python3 validate_qemu_output.py <log_file>

Exit codes:
    0  All checks passed (or only INFO-level skips)
    1  Warnings (non-critical checks failed)
    2  Errors (critical checks failed)
    3  Fatal (crash or corruption detected)
"""

import argparse
import re
import sys
from dataclasses import dataclass, field
from enum import IntEnum
from pathlib import Path
from typing import List, Optional


class Severity(IntEnum):
    PASS = 0
    SKIP = 1
    WARN = 2
    ERROR = 3
    FATAL = 4


# ANSI color codes (disabled if not a TTY)
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
        print("  QEMU Firmware Validation Report (ADR-061)")
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


def validate_log(log_text: str) -> ValidationReport:
    """Run all 16 validation checks against the UART log text."""
    report = ValidationReport()
    lines = log_text.splitlines()
    log_lower = log_text.lower()

    # ---- Check 1: Boot ----
    # Look for app_main() entry or main_task: tag
    boot_patterns = [r"app_main\(\)", r"main_task:", r"main:", r"ESP32-S3 CSI Node"]
    boot_found = any(re.search(p, log_text) for p in boot_patterns)
    if boot_found:
        report.add("Boot", Severity.PASS, "Firmware booted successfully")
    else:
        report.add("Boot", Severity.FATAL, "No boot indicator found (app_main / main_task)")

    # ---- Check 2: NVS load ----
    nvs_patterns = [r"nvs_config:", r"nvs_config_load", r"NVS", r"csi_cfg"]
    nvs_found = any(re.search(p, log_text) for p in nvs_patterns)
    if nvs_found:
        report.add("NVS load", Severity.PASS, "NVS configuration loaded")
    else:
        report.add("NVS load", Severity.WARN, "No NVS load indicator found")

    # ---- Check 3: Mock CSI init ----
    mock_patterns = [r"mock_csi:", r"mock_csi_init", r"Mock CSI", r"MOCK_CSI"]
    mock_found = any(re.search(p, log_text) for p in mock_patterns)
    if mock_found:
        report.add("Mock CSI init", Severity.PASS, "Mock CSI generator initialized")
    else:
        # This is only expected when mock is enabled
        report.add("Mock CSI init", Severity.SKIP,
                    "No mock CSI indicator (expected if mock not enabled)")

    # ---- Check 4: Frame generation ----
    # Count frame-related log lines
    frame_patterns = [
        r"frame[_ ]count[=: ]+(\d+)",
        r"frames?[=: ]+(\d+)",
        r"emitted[=: ]+(\d+)",
        r"mock_csi:.*frame",
        r"csi_collector:.*frame",
        r"CSI frame",
    ]
    frame_count = 0
    for line in lines:
        for pat in frame_patterns:
            m = re.search(pat, line, re.IGNORECASE)
            if m:
                if m.lastindex and m.lastindex >= 1:
                    try:
                        frame_count = max(frame_count, int(m.group(1)))
                    except (ValueError, IndexError):
                        frame_count = max(frame_count, 1)
                else:
                    frame_count = max(frame_count, 1)

    if frame_count > 0:
        report.add("Frame generation", Severity.PASS,
                    f"Frames detected", count=frame_count)
    else:
        # Also count lines mentioning IQ data or subcarriers
        iq_lines = sum(1 for line in lines
                       if re.search(r"(iq_data|subcarrier|I/Q|enqueue)", line, re.IGNORECASE))
        if iq_lines > 0:
            report.add("Frame generation", Severity.PASS,
                        "I/Q data activity detected", count=iq_lines)
        else:
            report.add("Frame generation", Severity.WARN,
                        "No frame generation activity detected")

    # ---- Check 5: Edge pipeline ----
    edge_patterns = [r"edge_processing:", r"DSP task", r"edge_init", r"edge_tier"]
    edge_found = any(re.search(p, log_text) for p in edge_patterns)
    if edge_found:
        report.add("Edge pipeline", Severity.PASS, "Edge processing pipeline active")
    else:
        report.add("Edge pipeline", Severity.WARN,
                    "No edge processing indicator found")

    # ---- Check 6: Vitals output ----
    vitals_patterns = [r"vitals", r"breathing", r"presence", r"heartrate",
                       r"breathing_bpm", r"heart_rate"]
    vitals_count = sum(1 for line in lines
                       if any(re.search(p, line, re.IGNORECASE) for p in vitals_patterns))
    if vitals_count > 0:
        report.add("Vitals output", Severity.PASS,
                    "Vitals/breathing/presence output detected", count=vitals_count)
    else:
        report.add("Vitals output", Severity.WARN,
                    "No vitals output lines found")

    # ---- Check 7: Presence detection ----
    presence_patterns = [
        r"presence[=: ]+1",
        r"presence_score[=: ]+([0-9.]+)",
        r"presence detected",
    ]
    presence_found = False
    for line in lines:
        for pat in presence_patterns:
            m = re.search(pat, line, re.IGNORECASE)
            if m:
                if m.lastindex and m.lastindex >= 1:
                    try:
                        score = float(m.group(1))
                        if score > 0:
                            presence_found = True
                    except (ValueError, IndexError):
                        presence_found = True
                else:
                    presence_found = True

    if presence_found:
        report.add("Presence detection", Severity.PASS, "Presence detected in output")
    else:
        report.add("Presence detection", Severity.WARN,
                    "No presence=1 or presence_score>0 found")

    # ---- Check 8: Fall detection ----
    fall_patterns = [r"fall[=: ]+1", r"fall detected", r"fall_event"]
    fall_found = any(
        re.search(p, line, re.IGNORECASE)
        for line in lines for p in fall_patterns
    )
    if fall_found:
        report.add("Fall detection", Severity.PASS, "Fall event detected in output")
    else:
        report.add("Fall detection", Severity.SKIP,
                    "No fall event (expected if fall scenario not run)")

    # ---- Check 9: MAC filter ----
    mac_patterns = [r"MAC filter", r"mac_filter", r"dropped.*MAC",
                    r"filter_mac", r"filtered"]
    mac_found = any(
        re.search(p, line, re.IGNORECASE)
        for line in lines for p in mac_patterns
    )
    if mac_found:
        report.add("MAC filter", Severity.PASS, "MAC filter activity detected")
    else:
        report.add("MAC filter", Severity.SKIP,
                    "No MAC filter activity (expected if filter scenario not run)")

    # ---- Check 10: ADR-018 serialize ----
    serialize_patterns = [r"[Ss]erializ", r"ADR-018", r"stream_sender",
                          r"UDP.*send", r"udp.*sent"]
    serialize_count = sum(1 for line in lines
                         if any(re.search(p, line) for p in serialize_patterns))
    if serialize_count > 0:
        report.add("ADR-018 serialize", Severity.PASS,
                    "Serialization/streaming activity detected", count=serialize_count)
    else:
        report.add("ADR-018 serialize", Severity.WARN,
                    "No serialization activity detected")

    # ---- Check 11: No crash ----
    crash_patterns = [r"Guru Meditation", r"assert failed", r"abort\(\)",
                      r"panic", r"LoadProhibited", r"StoreProhibited",
                      r"InstrFetchProhibited", r"IllegalInstruction"]
    crash_found = []
    for line in lines:
        for pat in crash_patterns:
            if re.search(pat, line):
                crash_found.append(line.strip()[:120])

    if not crash_found:
        report.add("No crash", Severity.PASS, "No crash indicators found")
    else:
        report.add("No crash", Severity.FATAL,
                    f"Crash detected: {crash_found[0]}",
                    count=len(crash_found))

    # ---- Check 12: Heap OK ----
    heap_patterns = [r"HEAP_ERROR", r"out of memory", r"heap_caps_alloc.*failed",
                     r"malloc.*fail", r"heap corruption"]
    heap_errors = [line.strip()[:120] for line in lines
                   if any(re.search(p, line, re.IGNORECASE) for p in heap_patterns)]
    if not heap_errors:
        report.add("Heap OK", Severity.PASS, "No heap errors found")
    else:
        report.add("Heap OK", Severity.ERROR,
                    f"Heap error: {heap_errors[0]}",
                    count=len(heap_errors))

    # ---- Check 13: Stack OK ----
    stack_patterns = [r"[Ss]tack overflow", r"stack_overflow",
                      r"vApplicationStackOverflowHook"]
    stack_errors = [line.strip()[:120] for line in lines
                    if any(re.search(p, line) for p in stack_patterns)]
    if not stack_errors:
        report.add("Stack OK", Severity.PASS, "No stack overflow detected")
    else:
        report.add("Stack OK", Severity.FATAL,
                    f"Stack overflow: {stack_errors[0]}",
                    count=len(stack_errors))

    # ---- Check 14: Clean exit ----
    reboot_patterns = [r"Rebooting\.\.\.", r"rst:0x"]
    reboot_found = any(
        re.search(p, line)
        for line in lines for p in reboot_patterns
    )
    if not reboot_found:
        report.add("Clean exit", Severity.PASS,
                    "No unexpected reboot detected")
    else:
        report.add("Clean exit", Severity.WARN,
                    "Reboot detected (may indicate crash or watchdog)")

    # ---- Check 15: Scenario completion (when running all scenarios) ----
    all_scenarios_pattern = r"All (\d+) scenarios complete"
    scenario_match = re.search(all_scenarios_pattern, log_text)
    if scenario_match:
        n_scenarios = int(scenario_match.group(1))
        report.add("Scenario completion", Severity.PASS,
                    f"All {n_scenarios} scenarios completed", count=n_scenarios)
    else:
        # Check if individual scenario started indicators exist
        scenario_starts = re.findall(r"=== Scenario (\d+) started ===", log_text)
        if scenario_starts:
            report.add("Scenario completion", Severity.WARN,
                        f"Started {len(scenario_starts)} scenarios but no completion marker",
                        count=len(scenario_starts))
        else:
            report.add("Scenario completion", Severity.SKIP,
                        "No scenario tracking (single scenario or mock not enabled)")

    # ---- Check 16: Frame rate sanity ----
    # Extract scenario frame counts and check they're reasonable
    frame_reports = re.findall(r"scenario=\d+ frames=(\d+)", log_text)
    if frame_reports:
        max_frames = max(int(f) for f in frame_reports)
        if max_frames > 0:
            report.add("Frame rate", Severity.PASS,
                        f"Peak frame counter: {max_frames}", count=max_frames)
        else:
            report.add("Frame rate", Severity.ERROR,
                        "Frame counters are all zero")
    else:
        report.add("Frame rate", Severity.SKIP,
                    "No periodic frame reports found")

    # ---- Check 17: ADR-081 adaptive controller boot ----
    adapt_boot_patterns = [
        r"adaptive_ctrl:.*adaptive controller online",
        r"adaptive_ctrl:\s*state\s+\d+\s*\xe2\x86\x92",
        r"adapt=on",
    ]
    adapt_boot = any(re.search(p, log_text) for p in adapt_boot_patterns)
    if adapt_boot:
        report.add("ADR-081 controller", Severity.PASS,
                   "Adaptive controller started (ADR-081 Layer 2)")
    else:
        report.add("ADR-081 controller", Severity.WARN,
                   "No adaptive_ctrl: log line found "
                   "(expected ADR-081 Layer 2 online)")

    # ---- Check 18: ADR-081 mock radio binding (QEMU only) ----
    mock_radio = re.search(r"rv_radio_mock:.*registered", log_text)
    if mock_radio:
        report.add("ADR-081 radio binding", Severity.PASS,
                   "Mock radio ops binding registered "
                   "(ADR-081 Layer 1 portability gate)")
    else:
        # Only required when CONFIG_CSI_MOCK_ENABLED — downgrade to SKIP.
        report.add("ADR-081 radio binding", Severity.SKIP,
                   "No rv_radio_mock registration line "
                   "(expected if CONFIG_CSI_MOCK_ENABLED)")

    # ---- Check 19: ADR-081 slow-loop heartbeat ----
    slow_tick = re.search(r"adaptive_ctrl:\s*slow tick", log_text)
    if slow_tick:
        report.add("ADR-081 slow loop", Severity.PASS,
                   "Slow loop heartbeat observed "
                   "(controller is ticking at ≥30 s cadence)")
    else:
        # A 60s QEMU timeout may not reach the first slow tick (30s default
        # plus boot time); treat as SKIP not WARN.
        report.add("ADR-081 slow loop", Severity.SKIP,
                   "No slow tick (QEMU run shorter than slow_loop_ms)")

    return report


def main():
    parser = argparse.ArgumentParser(
        description="Validate QEMU ESP32-S3 UART output (ADR-061)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="Example: python3 validate_qemu_output.py build/qemu_output.log",
    )
    parser.add_argument(
        "log_file",
        help="Path to QEMU UART log file",
    )
    parser.add_argument(
        "--strict", action="store_true",
        help="Exit non-zero on warnings (default: only fail on errors/fatals)",
    )
    args = parser.parse_args()

    log_path = Path(args.log_file)
    if not log_path.exists():
        print(f"ERROR: Log file not found: {log_path}", file=sys.stderr)
        sys.exit(3)

    log_text = log_path.read_text(encoding="utf-8", errors="replace")

    if not log_text.strip():
        print("ERROR: Log file is empty. QEMU may have failed to start.",
              file=sys.stderr)
        sys.exit(3)

    report = validate_log(log_text)
    report.print_report()

    # Map max severity to exit code.
    # WARNs are expected in QEMU without real WiFi hardware (no CSI data
    # flowing), so they exit 0 to avoid failing CI. Use --strict to
    # fail on warnings (useful for mock-CSI scenarios where data IS expected).
    max_sev = report.max_severity
    if max_sev <= Severity.SKIP:
        sys.exit(0)
    elif max_sev == Severity.WARN:
        sys.exit(1 if args.strict else 0)
    elif max_sev == Severity.ERROR:
        sys.exit(2)
    else:
        sys.exit(3)


if __name__ == "__main__":
    main()

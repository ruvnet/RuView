#!/usr/bin/env python3
"""
QEMU Post-Fault Health Checker — ADR-061 Layer 9

Reads a log segment captured after a fault injection and checks whether
the firmware is still healthy. Used by qemu-chaos-test.sh after each
fault in the chaos testing loop.

Health checks:
    1. No crash patterns (Guru Meditation, assert, panic, abort)
    2. No heap errors (OOM, heap corruption, alloc failure)
    3. No stack overflow (FreeRTOS stack overflow hook)
    4. Firmware still producing frames (CSI frame activity)

Exit codes:
    0  HEALTHY   — all checks pass
    1  DEGRADED  — no crash, but missing expected activity
    2  UNHEALTHY — crash, heap error, or stack overflow detected

Usage:
    python3 check_health.py --log /path/to/fault_segment.log --after-fault wifi_kill
"""

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import List


# ANSI colors
USE_COLOR = sys.stdout.isatty()


def color(text: str, code: str) -> str:
    if not USE_COLOR:
        return text
    return f"\033[{code}m{text}\033[0m"


def green(t: str) -> str:
    return color(t, "32")


def yellow(t: str) -> str:
    return color(t, "33")


def red(t: str) -> str:
    return color(t, "1;31")


@dataclass
class HealthCheck:
    name: str
    passed: bool
    message: str
    severity: int  # 0=pass, 1=degraded, 2=unhealthy


def check_no_crash(lines: List[str]) -> HealthCheck:
    """Check for crash indicators in the log."""
    crash_patterns = [
        r"Guru Meditation",
        r"assert failed",
        r"abort\(\)",
        r"panic",
        r"LoadProhibited",
        r"StoreProhibited",
        r"InstrFetchProhibited",
        r"IllegalInstruction",
        r"Unhandled debug exception",
        r"Fatal exception",
    ]

    for line in lines:
        for pat in crash_patterns:
            if re.search(pat, line):
                return HealthCheck(
                    name="No crash",
                    passed=False,
                    message=f"Crash detected: {line.strip()[:120]}",
                    severity=2,
                )

    return HealthCheck(
        name="No crash",
        passed=True,
        message="No crash indicators found",
        severity=0,
    )


def check_no_heap_errors(lines: List[str]) -> HealthCheck:
    """Check for heap/memory errors."""
    heap_patterns = [
        r"HEAP_ERROR",
        r"out of memory",
        r"heap_caps_alloc.*failed",
        r"malloc.*fail",
        r"heap corruption",
        r"CORRUPT HEAP",
        r"multi_heap",
        r"heap_lock",
    ]

    for line in lines:
        for pat in heap_patterns:
            if re.search(pat, line, re.IGNORECASE):
                return HealthCheck(
                    name="No heap errors",
                    passed=False,
                    message=f"Heap error: {line.strip()[:120]}",
                    severity=2,
                )

    return HealthCheck(
        name="No heap errors",
        passed=True,
        message="No heap errors found",
        severity=0,
    )


def check_no_stack_overflow(lines: List[str]) -> HealthCheck:
    """Check for FreeRTOS stack overflow."""
    stack_patterns = [
        r"[Ss]tack overflow",
        r"stack_overflow",
        r"vApplicationStackOverflowHook",
        r"stack smashing",
    ]

    for line in lines:
        for pat in stack_patterns:
            if re.search(pat, line):
                return HealthCheck(
                    name="No stack overflow",
                    passed=False,
                    message=f"Stack overflow: {line.strip()[:120]}",
                    severity=2,
                )

    return HealthCheck(
        name="No stack overflow",
        passed=True,
        message="No stack overflow detected",
        severity=0,
    )


def check_frame_activity(lines: List[str]) -> HealthCheck:
    """Check that the firmware is still producing CSI frames."""
    frame_patterns = [
        r"frame",
        r"CSI",
        r"mock_csi",
        r"iq_data",
        r"subcarrier",
        r"csi_collector",
        r"enqueue",
        r"presence",
        r"vitals",
        r"breathing",
    ]

    activity_lines = 0
    for line in lines:
        for pat in frame_patterns:
            if re.search(pat, line, re.IGNORECASE):
                activity_lines += 1
                break

    if activity_lines > 0:
        return HealthCheck(
            name="Frame activity",
            passed=True,
            message=f"Firmware producing output ({activity_lines} activity lines)",
            severity=0,
        )
    else:
        return HealthCheck(
            name="Frame activity",
            passed=False,
            message="No frame/CSI activity detected after fault",
            severity=1,  # Degraded, not fatal
        )


def run_health_checks(
    log_path: Path,
    fault_name: str,
    tail_lines: int = 200,
) -> int:
    """Run all health checks and report results.

    Returns:
        0 = healthy, 1 = degraded, 2 = unhealthy
    """
    if not log_path.exists():
        print(f"  ERROR: Log file not found: {log_path}", file=sys.stderr)
        return 2

    text = log_path.read_text(encoding="utf-8", errors="replace")
    all_lines = text.splitlines()

    # Use last N lines (most recent, after fault injection)
    lines = all_lines[-tail_lines:] if len(all_lines) > tail_lines else all_lines

    if not lines:
        print(f"  WARNING: Log file is empty (fault may have killed output)")
        # Empty log after fault is degraded, not necessarily unhealthy
        return 1

    print(f"  Health check after fault: {fault_name}")
    print(f"  Log lines analyzed: {len(lines)} (of {len(all_lines)} total)")
    print()

    # Run checks
    checks = [
        check_no_crash(lines),
        check_no_heap_errors(lines),
        check_no_stack_overflow(lines),
        check_frame_activity(lines),
    ]

    max_severity = 0
    for check in checks:
        if check.passed:
            icon = green("PASS")
        elif check.severity == 1:
            icon = yellow("WARN")
        else:
            icon = red("FAIL")

        print(f"  [{icon}] {check.name}: {check.message}")
        max_severity = max(max_severity, check.severity)

    print()

    # Summary
    passed = sum(1 for c in checks if c.passed)
    total = len(checks)

    if max_severity == 0:
        print(f"  {green(f'HEALTHY')} — {passed}/{total} checks passed")
    elif max_severity == 1:
        print(f"  {yellow(f'DEGRADED')} — {passed}/{total} checks passed")
    else:
        print(f"  {red(f'UNHEALTHY')} — {passed}/{total} checks passed")

    return max_severity


def main():
    parser = argparse.ArgumentParser(
        description="QEMU Post-Fault Health Checker — ADR-061 Layer 9",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Example output:\n"
            "  [HEALTHY] t=30s frames=150 (5.0 fps) crashes=0 heap_err=0 wdt=0 reboots=0\n"
            "  \n"
            "  VERDICT: Firmware is healthy. No critical issues detected."
        ),
    )
    parser.add_argument(
        "--log", required=True,
        help="Path to the log file (or log segment) to check",
    )
    parser.add_argument(
        "--after-fault", required=True,
        help="Name of the fault that was injected (for reporting)",
    )
    parser.add_argument(
        "--tail", type=int, default=200,
        help="Number of lines from end of log to analyze (default: 200)",
    )
    args = parser.parse_args()

    exit_code = run_health_checks(
        log_path=Path(args.log),
        fault_name=args.after_fault,
        tail_lines=args.tail,
    )
    sys.exit(exit_code)


if __name__ == "__main__":
    main()

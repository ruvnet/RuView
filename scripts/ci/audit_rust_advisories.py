#!/usr/bin/env python3
"""Preserve cargo-audit output and reconcile two reviewed upstream range errors.

This is not an advisory ignore list. Only the exact reviewed RustSec selectors,
registry Wasmtime 36.0.21, matching lockfile checksum and absent WASI packages can
qualify. Corrected/new metadata or other versions remain unresolved. See the
docs/security/wasmtime-audit-20261119.md assessment for authoritative package/range evidence.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tomllib

REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
WASMTIME_ARCHIVE_SHA256 = "c58d2cbba3817454af39ec9f0b39279870a9a98ef4c52bbbcdd9c1ffc1f5f17f"
REVIEWED = {
    "RUSTSEC-2026-0567": "99da727f7424b8b1f70c570e7347f7c2ad1d23ac7f7d1534654324e12297ee4c",
    "RUSTSEC-2026-0573": "1716fc0fdcf2acae357882632f7ee3a3b6551116e6c18ba32f2f79ddf140ad08",
}

PRIMARY = {
    'RUSTSEC-2026-0567': ('GHSA-2674-w29h-wqvj',
        'fe62715ac8a8ae324420af60d2ba36d55be25ede888c045cdfdbf5b18ffbd9ca'),
    'RUSTSEC-2026-0573': ('GHSA-wvgq-g5hf-6gm6',
        '4a1af44d4f625d4efdf6585593ec47fb73e89fc162fe1ba525090ca9d43f5b70'),
}


def primary_scope(document):
    if not isinstance(document, dict) or not isinstance(document.get("vulnerabilities"), list):
        raise ValueError("invalid upstream advisory metadata")
    values = []
    for value in document["vulnerabilities"]:
        package = value["package"]
        values.append({"package": {"name": package["name"], "ecosystem": package["ecosystem"].lower()},
            "vulnerable_version_range": "".join(value["vulnerable_version_range"].split()),
            "patched_versions": "".join(value["patched_versions"].split())})
    values.sort(key=lambda value: json.dumps(value, sort_keys=True))
    return {"ghsa_id": document["ghsa_id"], "vulnerabilities": values}


def primary_digest(document):
    return hashlib.sha256(json.dumps(primary_scope(document), sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def fetch_primary(ghsa):
    # IDs come only from PRIMARY constants, never from an untrusted report URL.
    if ghsa not in {value[0] for value in PRIMARY.values()}:
        raise ValueError("unreviewed upstream advisory identifier")
    completed = subprocess.run(["gh", "api", "--hostname", "github.com",
        "repos/bytecodealliance/wasmtime/security-advisories/" + ghsa],
        capture_output=True, text=True, timeout=45)
    if completed.returncode != 0 or len(completed.stdout) > 512 * 1024:
        raise ValueError(f"live upstream verification failed for {ghsa} (gh exit {completed.returncode}); no offline fallback")
    scope = primary_scope(json.loads(completed.stdout))
    if scope["ghsa_id"] != ghsa:
        raise ValueError("upstream advisory identity changed")
    return scope


def selector_digest(finding):
    advisory = finding.get("advisory", {})
    selectors = {key: finding.get(key) for key in ("affected", "versions")}
    selectors["advisory"] = {key: advisory.get(key) for key in ("id", "package", "aliases", "url")}
    return hashlib.sha256(json.dumps(selectors, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def metadata_mismatch(finding, packages, primary):
    advisory = finding.get("advisory", {})
    expected = REVIEWED.get(advisory.get("id"))
    if expected is None or selector_digest(finding) != expected:
        return False
    ghsa, expected_primary = PRIMARY[advisory["id"]]
    if ghsa not in primary or primary_digest(primary[ghsa]) != expected_primary:
        return False
    package = finding.get("package", {})
    if (package.get("name"), package.get("version"), package.get("source")) != ("wasmtime", "36.0.21", REGISTRY):
        return False
    checksum = package.get("checksum")
    if checksum != WASMTIME_ARCHIVE_SHA256:
        return False
    identity = ("name", "version", "source", "checksum")
    if not any(all(p.get(key) == package.get(key) for key in identity) for p in packages):
        return False
    if any(p.get("name") in {"wasmtime-wasi", "wasmtime-wasi-http"} for p in packages):
        return False
    return True


def classify(report, lock, ignored=(), primary=None):
    primary = primary or {}
    if not isinstance(report, dict) or report.get("error") or not isinstance(lock.get("package"), list):
        raise ValueError("invalid cargo-audit report or lockfile")
    vulnerability = report.get("vulnerabilities", {})
    findings = vulnerability.get("list")
    count = vulnerability.get("count")
    if (not isinstance(findings, list) or type(count) is not int or count < 0
            or count != len(findings) or type(vulnerability.get("found")) is not bool
            or vulnerability["found"] != bool(findings)):
        raise ValueError("cargo-audit omitted or malformed vulnerability findings")
    configured = report.get("settings", {}).get("ignore", [])
    if not isinstance(configured, list) or set(configured) != set(ignored):
        raise ValueError("cargo-audit policy does not match the existing local policy")
    result = {"raw_vulnerability_count": count, "metadata_corrections": [], "existing_policy_ignored": [], "unresolved": []}
    for finding in findings:
        if not isinstance(finding, dict) or not isinstance(finding.get("advisory"), dict):
            raise ValueError("invalid advisory finding")
        advisory_id = finding["advisory"].get("id")
        if advisory_id in ignored:
            result["existing_policy_ignored"].append(advisory_id)
        elif metadata_mismatch(finding, lock["package"], primary):
            result["metadata_corrections"].append({"id": advisory_id, "package": "wasmtime", "version": "36.0.21", "selector_sha256": REVIEWED[advisory_id], "upstream_advisory": PRIMARY[advisory_id][0], "upstream_scope_sha256": PRIMARY[advisory_id][1]})
        else:
            result["unresolved"].append(finding)
    return result


def decision(native_exit, result, policy):
    if native_exit not in (0, 1):
        raise ValueError("cargo-audit execution failed")
    if native_exit == 1 and result["raw_vulnerability_count"] == 0:
        raise ValueError("unexplained native failure without vulnerability findings")
    # Never turn an explicitly denied native warning into success.
    denied_output = bool(policy.get("output", {}).get("deny", []))
    return 1 if result["unresolved"] or (native_exit == 1 and denied_output) else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lockfile", type=Path, default=Path("Cargo.lock"))
    parser.add_argument("--raw-output", type=Path, default=Path("cargo-audit.json"))
    parser.add_argument("--classified-output", type=Path, default=Path("cargo-audit-classification.json"))
    parser.add_argument("--upstream-output", type=Path, default=Path("cargo-audit-upstream.json"))
    args = parser.parse_args()
    lockfile = args.lockfile.resolve()
    if lockfile.name != "Cargo.lock":
        raise ValueError("the workspace audit requires a standard Cargo.lock filename")
    lock = tomllib.loads(lockfile.read_text())
    policy_file = lockfile.parent / ".cargo/audit.toml"
    policy = tomllib.loads(policy_file.read_text()) if policy_file.exists() else {}
    ignored = policy.get("advisories", {}).get("ignore", [])
    # A fixed direct auditor invocation avoids Cargo aliases and option injection.
    completed = subprocess.run(["cargo-audit", "audit", "--file", "Cargo.lock", "--json"],
        cwd=lockfile.parent, capture_output=True, text=True, timeout=300, shell=False)
    args.raw_output.write_text(completed.stdout)
    sys.stdout.write(completed.stdout)
    sys.stderr.write(completed.stderr)
    if completed.returncode not in (0, 1) or len(completed.stdout) > 16 * 1024 * 1024:
        raise ValueError("cargo-audit execution failed or returned an oversized report")
    report = json.loads(completed.stdout)
    primary = {}
    for finding in report.get("vulnerabilities", {}).get("list", []):
        advisory_id = finding.get("advisory", {}).get("id")
        if advisory_id in PRIMARY and advisory_id not in ignored:
            ghsa = PRIMARY[advisory_id][0]
            primary[ghsa] = fetch_primary(ghsa)
    args.upstream_output.write_text(json.dumps({"retrieved_at": datetime.datetime.now(datetime.timezone.utc).isoformat(), "scopes": primary}, indent=2) + "\n")
    result = classify(report, lock, ignored, primary)
    exit_code = decision(completed.returncode, result, policy)
    result["native_exit_code"] = completed.returncode
    args.classified_output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({key: value for key, value in result.items() if key != "unresolved"} | {"unresolved_count": len(result["unresolved"])}))
    return exit_code


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"RUST AUDIT FAILED: {error}", file=sys.stderr)
        sys.exit(2)

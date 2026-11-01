#!/usr/bin/env python3
"""SYNTHETIC cross-repository acceptance, never hardware evidence."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

def run(args, **kw):
    return subprocess.run([str(a) for a in args], check=True, capture_output=True, timeout=30, **kw)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rufield-bin", type=Path, required=True)
    args = parser.parse_args()
    base = Path(__file__).resolve().parent
    model = base.parent.parent / "docs/rf-diagnostics/model.js"
    run(["make", "-C", base, "all"])
    scenarios = ("normal", "gap", "event", "flat", "tag", "badkind")
    with tempfile.TemporaryDirectory() as temp:
        for scenario in scenarios:
            fixture = Path(temp) / "stream.shm"
            run([base / "build/w1700k-fixture", fixture, scenario])
            output = run([base / "build/w1700k-diagnostics", "--input", fixture,
                          "--source-kind", "synthetic", "--once"]).stdout
            records = [json.loads(line) for line in output.splitlines()]
            assert len(records) == 4
            run(["node", "-e", "const m=require(process.argv[1]);m.parse(require('fs').readFileSync(0,'utf8'));", model], input=output)
            events = [json.loads(line) for line in run([args.rufield_bin.resolve()], input=output).stdout.splitlines()]
            assert len(events) == len(records)
            for record, event in zip(records, events):
                assert record["valid"] is False and record["config_verified"] is False
                assert event["sensor"]["modality"] == "rf_spectrum_quality"
                assert event["provenance"]["synthetic"] is True
                assert event["observation"]["confidence"] == 0
                assert event["observation"]["labels"] == []
                assert event["observation"]["attributes"]["trust"] == "unverified_advisory"
            if scenario == "gap":
                assert sum(r["missing_samples"] for r in records) == 2048
            if scenario == "event":
                assert sum(r["event_count"] for r in records) == 1
                assert sum(r["sample_count"] for r in records) == 6144
            if scenario in ("tag", "badkind"):
                assert records[1]["sample_count"] == 0
            if scenario == "flat":
                assert records[1]["rms_dbfs"] > 0 and records[1]["rail_fraction"] == 0
            print(f"PASS {scenario}: C ABI -> JSONL -> browser validator -> RuField")
    print("SYNTHETIC INTEGRATION PASS; hardware validation pending")

if __name__ == "__main__":
    main()

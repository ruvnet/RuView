"""SYNTHETIC ABI and failure-path tests; no physical sensor claims."""
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import time
import unittest

HERE = Path(__file__).resolve().parent

class BridgeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.bin = Path(cls.tmp.name) / "diagnostics"
        cls.fixture_bin = Path(cls.tmp.name) / "fixture"
        for source, target in [("diagnostics.c", cls.bin), ("fixture.c", cls.fixture_bin)]:
            subprocess.run(["cc", "-std=c11", "-Wall", "-Wextra", "-Werror",
                            "-O2", str(HERE/source), "-lm", "-o", str(target)], check=True)
    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()
    def fixture(self, kind="normal"):
        path = Path(self.tmp.name) / (kind + ".shm")
        subprocess.run([str(self.fixture_bin), str(path), kind], check=True)
        return path
    def invoke(self, kind="normal", *args):
        return subprocess.run([str(self.bin), "--input", str(self.fixture(kind)),
                               "--source-kind", "synthetic", *args],
                              capture_output=True, text=True, timeout=4)
    def test_normal(self):
        p = self.invoke()
        self.assertEqual(p.returncode, 0, p.stderr)
        rows = [json.loads(x) for x in p.stdout.splitlines()]
        self.assertEqual(len(rows), 4)
        for x in rows:
            self.assertEqual(x["sample_count"], 2048)
            self.assertAlmostEqual(x["rms_dbfs"], 20*math.log10(.5), places=5)
            self.assertEqual(x["rail_fraction"], 0)
            self.assertFalse(x["valid"])
            self.assertFalse(x["config_verified"])
            self.assertIsNone(x["sample_start"])
            self.assertEqual(x["source_kind"], "synthetic")
    def test_invalid_headers(self):
        for kind in ["magic", "ring", "offset", "overflow", "seqlock", "truncated", "run_overflow"]:
            with self.subTest(kind=kind):
                p = self.invoke(kind)
                self.assertEqual(p.returncode, 2)
                self.assertEqual(p.stdout, "")
    def test_gap_event_and_tag(self):
        for kind, reason in [("gap", "lost"), ("event", "event"), ("tag", "tag_mismatch"), ("badkind", "tag_mismatch")]:
            p = self.invoke(kind)
            rows = list(map(json.loads, p.stdout.splitlines()))
            self.assertIn(reason, rows[1]["invalid_reasons"])
            self.assertEqual(rows[1]["sample_count"], 0)
            self.assertIsNone(rows[1]["rms_dbfs"])
            self.assertEqual(rows[1]["missing_samples"], 2048 if kind == "gap" else 0)
    def test_flat_headroom_not_clipping(self):
        rows = list(map(json.loads, self.invoke("flat").stdout.splitlines()))
        self.assertEqual(rows[1]["rail_fraction"], 0)
        self.assertGreater(rows[1]["rms_dbfs"], 0)
    def test_live_stale(self):
        p = self.invoke("stale", "--source-kind", "live")
        self.assertEqual(p.returncode, 3)
        self.assertIn("stale", json.loads(p.stdout)["invalid_reasons"])
    def test_bounded_tail(self):
        rows = list(map(json.loads, self.invoke("normal", "--max-records", "2").stdout.splitlines()))
        self.assertEqual([r["slot_index"] for r in rows], [2, 3])
    def test_idle_timeout(self):
        start = time.monotonic()
        p = self.invoke("idle", "--follow", "--timeout-seconds", "1")
        self.assertEqual(p.returncode, 0)
        self.assertEqual(p.stdout, "")
        self.assertLess(time.monotonic()-start, 2)
    def test_concurrent_truncate(self):
        path = self.fixture("idle")
        p = subprocess.Popen([str(self.bin), "--input", str(path), "--source-kind", "synthetic",
                              "--follow", "--timeout-seconds", "2"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        time.sleep(.05)
        with path.open("wb"):
            pass
        p.communicate(timeout=3)
        self.assertEqual(p.returncode, 2) # no SIGBUS
    def test_overrun(self):
        path = self.fixture("idle")
        p = subprocess.Popen([str(self.bin), "--input", str(path), "--source-kind", "synthetic",
                              "--follow", "--max-records", "1", "--timeout-seconds", "2"],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        time.sleep(.05)
        with path.open("r+b", buffering=0) as f:
            f.seek(128); f.write(struct.pack("=Q", 1))
            f.seek(144); f.write(struct.pack("=Q", 20))
            f.seek(128); f.write(struct.pack("=Q", 2))
        out, err = p.communicate(timeout=3)
        self.assertEqual(p.returncode, 0, err)
        row = json.loads(out)
        self.assertIn("overrun", row["invalid_reasons"])
        self.assertEqual(row["overrun_slices"], 12)
        self.assertEqual(row["missing_samples"], 0) # event slots have no sample time
    def test_symlink_rejected(self):
        link = Path(self.tmp.name)/"symlink"
        if link.exists(): link.unlink()
        link.symlink_to(self.fixture())
        p = subprocess.run([str(self.bin), "--input", str(link)], capture_output=True)
        self.assertEqual(p.returncode, 2)
    def test_wide_center(self):
        p = self.invoke("normal", "--center-frequency-hz", "7576000000")
        self.assertEqual(p.returncode, 0, p.stderr)
        row = json.loads(p.stdout.splitlines()[0])
        self.assertEqual(row["center_frequency_hz"], 7576000000)
        self.assertFalse(row["config_verified"])
    def test_bad_args(self):
        for args in [["--source-kind","madeup"], ["--max-records","0"], ["--max-records","999999"],
                     ["--timeout-seconds","-1"], ["--center-frequency-hz","nan"]]:
            p = self.invoke("normal", *args)
            self.assertEqual(p.returncode, 2)

if __name__ == "__main__":
    unittest.main(verbosity=2)

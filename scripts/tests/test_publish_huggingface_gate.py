"""Tests for the ADR-298 release-gate wiring in `scripts/publish-huggingface.py`.

Issue #1521: the ADR-298 gates existed and were tested, but nothing ran them
against a real artifact, so a degenerate head could still be published. The
publish script now runs the gate before anything reaches the Hub.

These tests never invoke cargo: the runner is injected, so they assert the
*wiring* — which artifact is gated, what each exit status means, and that a
rejected artifact stops the publish before any upload happens.
"""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "publish-huggingface.py"


def _load_module():
    spec = importlib.util.spec_from_file_location("publish_huggingface", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    # The module has to be in `sys.modules` before it is executed. The script
    # uses `from __future__ import annotations`, so `dataclasses` resolves its
    # field annotations as strings and then looks the defining module up
    # through `sys.modules[cls.__module__]` (Python 3.10-3.13). Without this
    # entry that lookup returns None and the import dies with
    # `AttributeError: 'NoneType' object has no attribute '__dict__'`.
    sys.modules[spec.name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        del sys.modules[spec.name]
        raise
    return module


publish_huggingface = _load_module()


class ModuleLoadingTests(unittest.TestCase):
    def test_the_script_module_is_registered_in_sys_modules(self):
        # Python 3.10-3.13 resolve `from __future__ import annotations` string
        # annotations through `sys.modules[cls.__module__]`. A module loaded
        # without that entry cannot define its dataclasses, so the entry is
        # part of the contract, not a convenience.
        self.assertIn(publish_huggingface.__name__, sys.modules)
        self.assertIs(sys.modules[publish_huggingface.__name__], publish_huggingface)


def _completed(returncode: int, stdout: str = "", stderr: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess(
        args=["cargo", "run"],
        returncode=returncode,
        stdout=stdout,
        stderr=stderr,
    )


class GateResultTests(unittest.TestCase):
    def test_a_passing_gate_is_reported_as_a_pass(self):
        def runner(command, **kwargs):
            self.assertIn("model_release_gate", command)
            self.assertIn("--l2-normalized-input", command)
            return _completed(0, stdout="VERDICT: PASS")

        with tempfile.TemporaryDirectory() as tmp:
            model_dir = Path(tmp)
            (model_dir / "model.safetensors").write_bytes(b"stub")

            result = publish_huggingface.run_model_release_gate(model_dir, runner=runner)

        self.assertTrue(result.passed)
        self.assertEqual(result.status, "pass")

    def test_a_rejected_gate_fails_closed(self):
        def runner(command, **kwargs):
            return _completed(1, stdout="unreachable decision boundary", stderr="VERDICT: FAIL")

        with tempfile.TemporaryDirectory() as tmp:
            model_dir = Path(tmp)
            (model_dir / "model.safetensors").write_bytes(b"stub")

            result = publish_huggingface.run_model_release_gate(model_dir, runner=runner)

        self.assertFalse(result.passed)
        self.assertEqual(result.status, "fail")
        self.assertIn("ADR-298", result.report)
        self.assertIn("unreachable decision boundary", result.report)

    def test_a_gate_that_cannot_reach_a_verdict_fails_closed(self):
        # Exit code 2 means "could not evaluate" — it must not read as "fine".
        def runner(command, **kwargs):
            return _completed(2, stderr="error: cannot read `model.safetensors`")

        with tempfile.TemporaryDirectory() as tmp:
            model_dir = Path(tmp)
            (model_dir / "model.safetensors").write_bytes(b"stub")

            result = publish_huggingface.run_model_release_gate(model_dir, runner=runner)

        self.assertFalse(result.passed)
        self.assertEqual(result.status, "not_evaluated")

    def test_a_missing_cargo_fails_closed(self):
        def runner(command, **kwargs):
            raise FileNotFoundError("cargo")

        with tempfile.TemporaryDirectory() as tmp:
            model_dir = Path(tmp)
            (model_dir / "model.safetensors").write_bytes(b"stub")

            result = publish_huggingface.run_model_release_gate(model_dir, runner=runner)

        self.assertFalse(result.passed)
        self.assertEqual(result.status, "not_evaluated")
        self.assertIn("cargo", result.report)

    def test_a_model_set_without_a_classifier_artifact_is_not_applicable(self):
        def runner(command, **kwargs):
            raise AssertionError("the gate must not run without a classifier artifact")

        with tempfile.TemporaryDirectory() as tmp:
            model_dir = Path(tmp)
            (model_dir / "README.md").write_text("no head here", encoding="utf-8")

            result = publish_huggingface.run_model_release_gate(model_dir, runner=runner)

        self.assertTrue(result.passed)
        self.assertEqual(result.status, "not_applicable")
        self.assertIn("NOT APPLICABLE", result.report)

    def test_the_presence_head_sidecar_is_gated_before_the_container(self):
        seen = {}

        def runner(command, **kwargs):
            seen["command"] = command
            return _completed(0, stdout="VERDICT: PASS")

        with tempfile.TemporaryDirectory() as tmp:
            model_dir = Path(tmp)
            (model_dir / "model.safetensors").write_bytes(b"stub")
            (model_dir / "presence-head.json").write_text("{}", encoding="utf-8")

            publish_huggingface.run_model_release_gate(model_dir, runner=runner)

        artifact = seen["command"][seen["command"].index("--artifact") + 1]
        self.assertTrue(artifact.endswith("presence-head.json"), artifact)


class PublishGuardTests(unittest.TestCase):
    def test_a_rejected_gate_stops_the_publish(self):
        gate = publish_huggingface.GateResult(
            passed=False,
            status="fail",
            report="ADR-298 gate: FAIL — unreachable decision boundary",
        )
        with tempfile.TemporaryDirectory() as tmp:
            model_dir = Path(tmp)
            (model_dir / "model.safetensors").write_bytes(b"stub")

            with self.assertRaises(SystemExit) as raised:
                publish_huggingface.publish(
                    repo_id="ruvnet/wifi-densepose-pretrained",
                    model_dir=model_dir,
                    version="v0.0.1",
                    token="unused",
                    dry_run=True,
                    gate=gate,
                )

        self.assertEqual(raised.exception.code, 1)

    def test_the_publish_runs_the_gate_itself_when_none_is_supplied(self):
        called = {}

        def fake_gate(model_dir):
            called["model_dir"] = model_dir
            return publish_huggingface.GateResult(
                passed=False,
                status="fail",
                report="rejected",
            )

        with tempfile.TemporaryDirectory() as tmp:
            model_dir = Path(tmp)
            (model_dir / "model.safetensors").write_bytes(b"stub")

            original = publish_huggingface.run_model_release_gate
            publish_huggingface.run_model_release_gate = fake_gate
            try:
                with self.assertRaises(SystemExit):
                    publish_huggingface.publish(
                        repo_id="ruvnet/wifi-densepose-pretrained",
                        model_dir=model_dir,
                        version="v0.0.1",
                        token="unused",
                        dry_run=True,
                    )
            finally:
                publish_huggingface.run_model_release_gate = original

        self.assertIn("model_dir", called)

    def test_a_passing_gate_reaches_the_dry_run(self):
        gate = publish_huggingface.GateResult(
            passed=True,
            status="pass",
            report="ADR-298 gate: PASS",
        )
        # The Hub client is stubbed so the test never reaches the network.
        fake_hub = types.ModuleType("huggingface_hub")
        fake_hub.HfApi = object  # type: ignore[attr-defined]
        fake_hub.login = lambda **kwargs: None  # type: ignore[attr-defined]
        sys.modules["huggingface_hub"] = fake_hub
        try:
            with tempfile.TemporaryDirectory() as tmp:
                model_dir = Path(tmp)
                (model_dir / "model.safetensors").write_bytes(b"stub")

                publish_huggingface.publish(
                    repo_id="ruvnet/wifi-densepose-pretrained",
                    model_dir=model_dir,
                    version="v0.0.1",
                    token="unused",
                    dry_run=True,
                    gate=gate,
                )
        finally:
            sys.modules.pop("huggingface_hub", None)


if __name__ == "__main__":
    unittest.main()

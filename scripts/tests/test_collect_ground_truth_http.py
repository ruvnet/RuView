"""Error-path tests for collect-ground-truth's HTTP / token helpers.

Uses a local stub HTTP server; no camera, no MediaPipe model.

Run:  python -m pytest scripts/tests/test_collect_ground_truth_http.py -q
"""

from __future__ import annotations

import importlib.util
import sys
import threading
import types
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "collect-ground-truth.py"


def _load_module():
    try:
        import mediapipe  # noqa: F401
    except Exception:
        mp = types.ModuleType("mediapipe")
        tasks = types.ModuleType("mediapipe.tasks")
        py = types.ModuleType("mediapipe.tasks.python")
        vision = types.ModuleType("mediapipe.tasks.python.vision")
        py.BaseOptions = object
        vision.PoseLandmarker = vision.PoseLandmarkerOptions = vision.RunningMode = object
        sys.modules.update({
            "mediapipe": mp,
            "mediapipe.tasks": tasks,
            "mediapipe.tasks.python": py,
            "mediapipe.tasks.python.vision": vision,
        })
    spec = importlib.util.spec_from_file_location("collect_ground_truth", SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


cgt = _load_module()


@pytest.fixture
def stub_server():
    """Yield a function that serves one fixed (status, body) and returns its URL."""
    servers = []

    def start(body: bytes, status: int = 200) -> str:
        class H(BaseHTTPRequestHandler):
            def do_POST(self):
                self.rfile.read(int(self.headers.get("Content-Length", 0)))
                self.send_response(status)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *a):
                pass

        srv = HTTPServer(("127.0.0.1", 0), H)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        servers.append(srv)
        return f"http://127.0.0.1:{srv.server_port}/rec"

    yield start
    for s in servers:
        s.shutdown()
        s.server_close()


def test_post_json_success(stub_server):
    assert cgt.post_json(stub_server(b'{"success": true}')) is True


def test_post_json_refusal(stub_server, capsys):
    url = stub_server(b'{"success": false, "error": "busy"}')
    assert cgt.post_json(url) is False
    assert "refused" in capsys.readouterr().err


def test_post_json_non_json_200_body(stub_server, capsys):
    assert cgt.post_json(stub_server(b"<html>proxy login</html>")) is False
    assert "non-JSON" in capsys.readouterr().err


def test_post_json_non_object_json(stub_server, capsys):
    assert cgt.post_json(stub_server(b"[1, 2]")) is False
    assert "expected an object" in capsys.readouterr().err


def test_post_json_connection_refused(capsys):
    assert cgt.post_json("http://127.0.0.1:1/rec", timeout=1.0) is False
    assert "failed" in capsys.readouterr().err


def test_missing_token_file_exits_nonzero(tmp_path, capsys):
    with pytest.raises(SystemExit) as exc:
        cgt.load_api_token(str(tmp_path / "nope.token"))
    assert exc.value.code not in (0, None)
    assert "--token-file" in capsys.readouterr().err


def test_unreadable_token_file_exits_nonzero(tmp_path, capsys):
    f = tmp_path / "bad.token"
    f.write_bytes(b"\xff\xfe\x00bad")
    with pytest.raises(SystemExit) as exc:
        cgt.load_api_token(str(f))
    assert exc.value.code not in (0, None)


def test_token_file_ok(tmp_path):
    f = tmp_path / "t"
    f.write_text("  secret \n", encoding="utf-8")
    assert cgt.load_api_token(str(f)) == "secret"

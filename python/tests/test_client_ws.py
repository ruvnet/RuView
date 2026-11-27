"""ADR-117 P4 — End-to-end test for SensingClient against an in-process
WS server.

We spin up a real `websockets.serve()` server in the same event loop,
send the four message types defined in ADR-115 §1, and assert the
client decodes them into the right dataclasses. No mocks — the only
moving part this test does NOT exercise is the actual sensing-server
binary, but the wire protocol is the contract under test here.
"""

from __future__ import annotations

import json
from typing import Any

import pytest
import websockets

from wifi_densepose.client import (
    ConnectionEstablishedMessage,
    EdgeVitalsMessage,
    PoseDataMessage,
    SensingClient,
    SensingMessage,
)

# ─── In-process WS server fixture ────────────────────────────────────


_FIXTURE_MESSAGES = [
    {
        "type": "connection_established",
        "node_id": "test-node-001",
        "version": "0.7.4",
        "capabilities": ["edge_vitals", "pose_data"],
    },
    {
        "type": "edge_vitals",
        "node_id": "test-node-001",
        "presence": True,
        "fall_detected": False,
        "motion": 0.21,
        "breathing_rate_bpm": 14.5,
        "heartrate_bpm": 72.3,
        "n_persons": 1,
        "motion_energy": 0.034,
        "presence_score": 0.91,
        "rssi": -42.0,
    },
    {
        "type": "pose_data",
        "node_id": "test-node-001",
        "timestamp": 1700000000.5,
        "persons": [{"id": 1, "keypoints": []}],
        "confidence": 0.88,
    },
    # Unknown type — should NOT crash the stream; should yield a plain
    # SensingMessage.
    {
        "type": "future_message_type_not_yet_modelled",
        "extra": "data",
    },
]


#: Upgrade-request headers captured by the in-process server, so tests
#: can assert what the client actually put on the handshake.
_CAPTURED_UPGRADE_HEADERS: dict[str, str] = {}


def _upgrade_headers(websocket: Any) -> Any:
    """Read the handshake request headers across websockets versions
    (`websocket.request.headers` >= 14, `websocket.request_headers` <= 13)."""
    req = getattr(websocket, "request", None)
    if req is not None and getattr(req, "headers", None) is not None:
        return req.headers
    return getattr(websocket, "request_headers", {})


async def _handler(websocket: Any) -> None:
    _CAPTURED_UPGRADE_HEADERS.clear()
    try:
        headers = _upgrade_headers(websocket)
        auth = headers.get("Authorization") if hasattr(headers, "get") else None
        if auth is not None:
            _CAPTURED_UPGRADE_HEADERS["Authorization"] = auth
    except Exception:
        pass
    for msg in _FIXTURE_MESSAGES:
        await websocket.send(json.dumps(msg))
    # Send one malformed frame to assert the client logs+drops it
    # rather than crashing the stream.
    await websocket.send("{not valid json")
    # And one final "real" message so the test can confirm the stream
    # survived the malformed one.
    await websocket.send(json.dumps({"type": "edge_vitals", "node_id": "post-bad-frame"}))


@pytest.fixture
async def ws_server() -> Any:
    """Start a websocket server on a random port; yield the bound URL."""
    server = await websockets.serve(_handler, "127.0.0.1", 0)
    # Get the bound port (host="127.0.0.1" returns one socket).
    port = server.sockets[0].getsockname()[1]  # type: ignore[union-attr]
    try:
        yield f"ws://127.0.0.1:{port}/ws/sensing"
    finally:
        server.close()
        await server.wait_closed()


# ─── End-to-end stream test ──────────────────────────────────────────


async def test_sensing_client_decodes_all_message_types(ws_server: str) -> None:
    received: list[SensingMessage] = []
    async with SensingClient(ws_server) as client:
        async for msg in client.stream():
            received.append(msg)
            if len(received) >= len(_FIXTURE_MESSAGES) + 1:  # +1 for post-bad-frame
                break

    # connection_established → typed
    assert isinstance(received[0], ConnectionEstablishedMessage)
    assert received[0].node_id == "test-node-001"
    assert received[0].version == "0.7.4"
    assert "edge_vitals" in received[0].capabilities

    # edge_vitals → typed with full fields
    assert isinstance(received[1], EdgeVitalsMessage)
    assert received[1].presence is True
    assert received[1].fall_detected is False
    assert received[1].breathing_rate_bpm == 14.5
    assert received[1].heartrate_bpm == 72.3
    assert received[1].n_persons == 1
    assert received[1].rssi == -42.0

    # pose_data → typed
    assert isinstance(received[2], PoseDataMessage)
    assert received[2].timestamp == 1700000000.5
    assert len(received[2].persons) == 1
    assert received[2].confidence == 0.88

    # Unknown type → plain SensingMessage (forward-compat)
    assert type(received[3]) is SensingMessage  # exact base class
    assert received[3].type == "future_message_type_not_yet_modelled"
    assert received[3].raw["extra"] == "data"

    # After the malformed frame: the stream should have survived and
    # yielded the post-bad-frame message.
    assert isinstance(received[4], EdgeVitalsMessage)
    assert received[4].node_id == "post-bad-frame"


async def test_sensing_client_recv_one(ws_server: str) -> None:
    async with SensingClient(ws_server) as client:
        msg = await client.recv_one(timeout=2.0)
    assert isinstance(msg, ConnectionEstablishedMessage)


async def test_sensing_client_raises_when_used_without_context() -> None:
    client = SensingClient("ws://127.0.0.1:1/")  # never connects
    with pytest.raises(RuntimeError, match="not connected"):
        await client.recv_one(timeout=0.1)
    with pytest.raises(RuntimeError, match="not connected"):
        async for _ in client.stream():
            pass


async def test_sensing_client_close_is_idempotent(ws_server: str) -> None:
    client = SensingClient(ws_server)
    await client.__aenter__()
    await client.close()
    await client.close()  # second close is a no-op


def test_sensing_client_decoder_directly() -> None:
    """The decoder is pure — exercise it without bringing up a WS
    server, so we have a fast unit test for the type mapping."""
    from wifi_densepose.client.ws import _decode

    msg = _decode(json.dumps({
        "type": "edge_vitals",
        "node_id": "x",
        "presence": True,
        "fall_detected": False,
        "motion": 1.5,
    }))
    assert isinstance(msg, EdgeVitalsMessage)
    assert msg.presence is True
    assert msg.motion == 1.5
    assert msg.breathing_rate_bpm is None  # not present → None, not 0.0
    assert msg.heartrate_bpm is None
    assert msg.rssi is None


# ─── Auth: bearer token on the WS upgrade (issue #1395) ──────────────


def _auth_header_from_kwargs(kwargs: dict) -> Any:
    """Pull the Authorization value out of whichever header kwarg the
    installed `websockets` uses (`additional_headers` >= 14,
    `extra_headers` <= 13). Returns None if no header kwarg was passed."""
    for key in ("additional_headers", "extra_headers"):
        if key in kwargs:
            return dict(kwargs[key]).get("Authorization")
    return None


class _DummyWS:
    async def close(self) -> None:
        pass


class _CapturingConnect:
    """Stand-in for `websockets.connect` that records the kwargs it was
    called with and returns an awaitable yielding a dummy connection."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, dict]] = []

    def __call__(self, url: str, **kwargs: Any) -> Any:
        self.calls.append((url, kwargs))

        async def _coro() -> _DummyWS:
            return _DummyWS()

        return _coro()

    @property
    def last_kwargs(self) -> dict:
        return self.calls[-1][1]


async def test_token_from_constructor_sets_auth_header(monkeypatch: Any) -> None:
    from wifi_densepose.client import ws as ws_mod

    fake = _CapturingConnect()
    monkeypatch.setattr(ws_mod.websockets, "connect", fake)

    async with SensingClient("ws://x/ws/sensing", token="tok-abc"):
        pass

    assert _auth_header_from_kwargs(fake.last_kwargs) == "Bearer tok-abc"


async def test_token_from_env_sets_auth_header(monkeypatch: Any) -> None:
    from wifi_densepose.client import ws as ws_mod

    fake = _CapturingConnect()
    monkeypatch.setattr(ws_mod.websockets, "connect", fake)
    monkeypatch.setenv("RUVIEW_API_TOKEN", "env-tok-123")

    async with SensingClient("ws://x/ws/sensing"):
        pass

    assert _auth_header_from_kwargs(fake.last_kwargs) == "Bearer env-tok-123"


async def test_constructor_token_overrides_env(monkeypatch: Any) -> None:
    from wifi_densepose.client import ws as ws_mod

    fake = _CapturingConnect()
    monkeypatch.setattr(ws_mod.websockets, "connect", fake)
    monkeypatch.setenv("RUVIEW_API_TOKEN", "env-tok")

    async with SensingClient("ws://x/ws/sensing", token="ctor-tok"):
        pass

    assert _auth_header_from_kwargs(fake.last_kwargs) == "Bearer ctor-tok"


async def test_no_token_sends_no_auth_header(monkeypatch: Any) -> None:
    from wifi_densepose.client import ws as ws_mod

    fake = _CapturingConnect()
    monkeypatch.setattr(ws_mod.websockets, "connect", fake)
    monkeypatch.delenv("RUVIEW_API_TOKEN", raising=False)

    async with SensingClient("ws://x/ws/sensing"):
        pass

    assert _auth_header_from_kwargs(fake.last_kwargs) is None
    # Auth-disabled path must not smuggle either header kwarg in.
    assert "additional_headers" not in fake.last_kwargs
    assert "extra_headers" not in fake.last_kwargs


async def test_empty_token_sends_no_auth_header(monkeypatch: Any) -> None:
    """An explicitly empty token (or empty env var) means 'no auth'."""
    from wifi_densepose.client import ws as ws_mod

    fake = _CapturingConnect()
    monkeypatch.setattr(ws_mod.websockets, "connect", fake)
    monkeypatch.setenv("RUVIEW_API_TOKEN", "")

    async with SensingClient("ws://x/ws/sensing"):
        pass

    assert _auth_header_from_kwargs(fake.last_kwargs) is None


@pytest.mark.parametrize(
    "header_param,expected",
    [
        ("additional_headers", "additional_headers"),  # websockets >= 14
        ("extra_headers", "extra_headers"),             # websockets <= 13
    ],
)
def test_select_header_kwarg_across_websockets_versions(
    header_param: str, expected: str
) -> None:
    """Version-compat: the kwarg is chosen by inspecting the installed
    `connect` signature, so both the pre-14 (`extra_headers`) and
    post-14 (`additional_headers`) conventions resolve correctly without
    two websockets installs."""
    from wifi_densepose.client.ws import _select_header_kwarg

    # Build a fake `connect` whose signature carries only the one kwarg
    # the emulated websockets version would expose.
    ns: dict = {}
    exec(
        f"def fake_connect(uri, *, {header_param}=None, ping_interval=None): ...",
        ns,
    )
    assert _select_header_kwarg(ns["fake_connect"]) == expected


def test_select_header_kwarg_matches_installed_websockets() -> None:
    """On whatever `websockets` is actually installed, the chosen kwarg
    must be a real parameter of `websockets.connect`."""
    import inspect

    import websockets

    from wifi_densepose.client.ws import _select_header_kwarg

    chosen = _select_header_kwarg(websockets.connect)
    assert chosen in inspect.signature(websockets.connect).parameters


async def test_auth_header_reaches_server_end_to_end(ws_server: str) -> None:
    """Real in-process server: assert the bearer actually arrives on the
    upgrade request (proves the header is wired to the live handshake,
    not just the connect kwargs)."""
    async with SensingClient(ws_server, token="e2e-token") as client:
        await client.recv_one(timeout=2.0)
    assert _CAPTURED_UPGRADE_HEADERS.get("Authorization") == "Bearer e2e-token"


def test_sensing_client_decoder_handles_None_subfields() -> None:
    """When the sensing-server explicitly emits null for HR/BR (no
    measurement yet), the client should propagate None, not crash."""
    from wifi_densepose.client.ws import _decode

    msg = _decode(json.dumps({
        "type": "edge_vitals",
        "node_id": "x",
        "presence": False,
        "fall_detected": False,
        "motion": 0.0,
        "breathing_rate_bpm": None,
        "heartrate_bpm": None,
        "rssi": None,
    }))
    assert isinstance(msg, EdgeVitalsMessage)
    assert msg.breathing_rate_bpm is None
    assert msg.heartrate_bpm is None
    assert msg.rssi is None

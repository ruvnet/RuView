"""ADR-117 P4 — Asyncio WebSocket client for the sensing-server.

The Rust sensing-server (`v2/crates/wifi-densepose-sensing-server`)
broadcasts three structured message types over `ws://<host>:<port>/ws/sensing`:

| `type` field | Source line in main.rs | Payload shape |
|---|---|---|
| `connection_established` | 2596 | `{node_id, version, capabilities}` |
| `pose_data` | 2655 | `{node_id, timestamp, persons: [...], confidence}` |
| `edge_vitals` | 4548 | `{node_id, presence, fall_detected, motion, breathing_rate_bpm, heartrate_bpm, ...}` |

`SensingClient` is a pure-Python asyncio wrapper around `websockets>=12`
that connects, decodes JSON, and yields typed dataclasses.

Example:

```python
import asyncio
from wifi_densepose.client import SensingClient, EdgeVitalsMessage

async def main():
    async with SensingClient("ws://localhost:8765/ws/sensing") as client:
        async for msg in client.stream():
            if isinstance(msg, EdgeVitalsMessage):
                print(f"BR={msg.breathing_rate_bpm}, HR={msg.heartrate_bpm}")

asyncio.run(main())
```
"""  # noqa: E501 — markdown table rows above must stay on one line

from __future__ import annotations

import asyncio
import inspect
import json
import logging
import os
from collections.abc import AsyncIterator
from dataclasses import dataclass, field
from typing import Any

# Defer import — only fail at construction time, not at module load.
try:
    import websockets  # type: ignore[import-not-found]
    from websockets.exceptions import ConnectionClosed  # type: ignore[import-not-found]
    _WEBSOCKETS_AVAILABLE = True
except ImportError:  # pragma: no cover
    _WEBSOCKETS_AVAILABLE = False


log = logging.getLogger(__name__)


#: Environment variable the sensing-server bearer token is read from by
#: default. Mirrors the TypeScript MCP client (tools/ruview-mcp).
TOKEN_ENV_VAR = "RUVIEW_API_TOKEN"


def _select_header_kwarg(connect_fn: Any) -> str:
    """Return the ``websockets.connect`` keyword for extra request headers.

    The keyword was renamed inside the ``websockets>=12`` range this
    package supports: ``<= 13`` accepts ``extra_headers``, ``>= 14``
    accepts ``additional_headers``. We inspect the actual signature of
    the installed ``connect`` rather than guessing from ``__version__``,
    so a version bump that renames the kwarg again is handled by
    detection instead of raising ``TypeError`` at connect time.
    """
    try:
        params = inspect.signature(connect_fn).parameters
    except (TypeError, ValueError):  # pragma: no cover — no introspectable sig
        return "additional_headers"
    if "additional_headers" in params:
        return "additional_headers"
    if "extra_headers" in params:
        return "extra_headers"
    # Neither present (unexpected) — prefer the newer convention.
    return "additional_headers"


# ─── Typed messages ──────────────────────────────────────────────────


@dataclass(frozen=True)
class SensingMessage:
    """Base class for typed sensing-server messages. The original JSON
    payload is preserved in ``raw`` for forward-compatibility with
    fields not yet modelled here."""
    type: str
    raw: dict[str, Any] = field(default_factory=dict, hash=False, compare=False)


@dataclass(frozen=True)
class ConnectionEstablishedMessage(SensingMessage):
    """First message after a successful WS handshake. Lets the client
    discover the node ID and capability flags without making a separate
    REST call."""
    node_id: str = ""
    version: str = ""
    capabilities: tuple[str, ...] = ()


@dataclass(frozen=True)
class EdgeVitalsMessage(SensingMessage):
    """Vital-sign telemetry fused from the edge-vitals path
    (ADR-021/ADR-110). Optional fields may be ``None`` when the
    upstream channel hasn't produced a measurement yet."""
    node_id: str = ""
    presence: bool = False
    fall_detected: bool = False
    motion: float = 0.0
    breathing_rate_bpm: float | None = None
    heartrate_bpm: float | None = None
    n_persons: int = 0
    motion_energy: float = 0.0
    presence_score: float = 0.0
    rssi: float | None = None


@dataclass(frozen=True)
class PoseDataMessage(SensingMessage):
    """17-keypoint pose data broadcast at the sensing-server's frame
    cadence. Persons are a list of opaque dicts — typed PoseEstimate
    decoding lives in the P2 bindings; the WS client passes through."""
    node_id: str = ""
    timestamp: float = 0.0
    persons: tuple[dict[str, Any], ...] = ()
    confidence: float = 0.0


# ─── Decoder ─────────────────────────────────────────────────────────


def _decode(raw_text: str) -> SensingMessage:
    """Decode a single WS frame into a typed message.

    Unknown ``type`` values yield a plain ``SensingMessage`` rather
    than raising — the sensing-server is on a faster release cadence
    than this client, and unknown types should not break the stream.
    """
    obj = json.loads(raw_text)
    if not isinstance(obj, dict):
        raise ValueError(f"sensing-server emitted non-dict payload: {type(obj).__name__}")
    mtype = obj.get("type", "")
    if mtype == "connection_established":
        return ConnectionEstablishedMessage(
            type=mtype,
            raw=obj,
            node_id=obj.get("node_id", ""),
            version=obj.get("version", ""),
            capabilities=tuple(obj.get("capabilities", ())),
        )
    if mtype == "edge_vitals":
        return EdgeVitalsMessage(
            type=mtype,
            raw=obj,
            node_id=obj.get("node_id", ""),
            presence=bool(obj.get("presence", False)),
            fall_detected=bool(obj.get("fall_detected", False)),
            motion=float(obj.get("motion", 0.0)),
            breathing_rate_bpm=(
                float(obj["breathing_rate_bpm"])
                if obj.get("breathing_rate_bpm") is not None else None
            ),
            heartrate_bpm=(
                float(obj["heartrate_bpm"])
                if obj.get("heartrate_bpm") is not None else None
            ),
            n_persons=int(obj.get("n_persons", 0)),
            motion_energy=float(obj.get("motion_energy", 0.0)),
            presence_score=float(obj.get("presence_score", 0.0)),
            rssi=(float(obj["rssi"]) if obj.get("rssi") is not None else None),
        )
    if mtype == "pose_data":
        persons = obj.get("persons", ())
        return PoseDataMessage(
            type=mtype,
            raw=obj,
            node_id=obj.get("node_id", ""),
            timestamp=float(obj.get("timestamp", 0.0)),
            persons=tuple(persons) if isinstance(persons, list) else (),
            confidence=float(obj.get("confidence", 0.0)),
        )
    return SensingMessage(type=mtype, raw=obj)


# ─── Client ──────────────────────────────────────────────────────────


class SensingClient:
    """Asyncio WebSocket client for the RuView sensing-server.

    Usage as async context manager:

    ```python
    async with SensingClient("ws://localhost:8765/ws/sensing") as c:
        async for msg in c.stream():
            ...
    ```

    The client does NOT auto-reconnect — if you want resilience, wrap
    the ``async with`` in your own retry loop. Auto-reconnect logic is
    application-specific (e.g., "retry forever" for a long-running
    automation vs "fail fast" for a CLI tool that should exit).

    Auth: pass ``token=`` to send ``Authorization: Bearer <token>`` on
    the WS upgrade, for sensing-servers started with ``RUVIEW_API_TOKEN``
    set. If ``token`` is omitted it defaults to the ``RUVIEW_API_TOKEN``
    environment variable; when neither is set, no header is sent.
    """

    def __init__(
        self,
        url: str,
        *,
        token: str | None = None,
        ping_interval: float = 20.0,
        ping_timeout: float = 20.0,
        max_size: int = 16 * 1024 * 1024,
    ) -> None:
        if not _WEBSOCKETS_AVAILABLE:
            raise ImportError(
                "SensingClient requires the `websockets` package. Install with "
                "`pip install \"wifi-densepose[client]\"` to enable the client extras."
            )
        self.url = url
        # Bearer token for auth-enabled sensing-servers. Explicit
        # constructor argument wins; otherwise fall back to the
        # RUVIEW_API_TOKEN environment variable. An empty value (unset
        # env, or "") means "no auth" — no Authorization header is sent.
        self._token = token if token is not None else os.environ.get(TOKEN_ENV_VAR)
        self._ping_interval = ping_interval
        self._ping_timeout = ping_timeout
        self._max_size = max_size
        self._ws: Any = None  # websockets.WebSocketClientProtocol — typed Any to avoid import cost

    async def __aenter__(self) -> SensingClient:
        connect_kwargs: dict[str, Any] = dict(
            ping_interval=self._ping_interval,
            ping_timeout=self._ping_timeout,
            max_size=self._max_size,
        )
        if self._token:
            # Python (unlike the browser UI) can set Authorization
            # directly on the WS upgrade — no ticket workaround needed.
            header_kwarg = _select_header_kwarg(websockets.connect)
            connect_kwargs[header_kwarg] = {"Authorization": f"Bearer {self._token}"}
        self._ws = await websockets.connect(self.url, **connect_kwargs)
        return self

    async def __aexit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        await self.close()

    async def close(self) -> None:
        """Idempotent connection close."""
        if self._ws is not None:
            try:
                await self._ws.close()
            except Exception as e:  # pragma: no cover — best-effort close
                log.debug("ignored WS close error: %r", e)
            self._ws = None

    async def stream(self) -> AsyncIterator[SensingMessage]:
        """Yield typed messages until the server closes the connection
        or the context is exited.

        Decode failures on individual frames are logged at WARN and
        swallowed — a malformed frame should not terminate the stream
        (the next frame may be fine)."""
        if self._ws is None:
            raise RuntimeError("SensingClient not connected. Use `async with` first.")
        try:
            async for frame in self._ws:
                if isinstance(frame, bytes):
                    frame = frame.decode("utf-8", errors="replace")
                try:
                    yield _decode(frame)
                except (ValueError, json.JSONDecodeError) as e:
                    log.warning("dropping malformed sensing-server frame: %r", e)
        except ConnectionClosed:
            # Graceful EOF — exit the iterator normally.
            return

    async def send_ping(self) -> None:
        """Send an application-level ping. The sensing-server replies
        with `{"type": "pong"}` (main.rs:2698)."""
        if self._ws is None:
            raise RuntimeError("SensingClient not connected. Use `async with` first.")
        await self._ws.send(json.dumps({"type": "ping"}))

    async def recv_one(self, *, timeout: float | None = None) -> SensingMessage:
        """Receive a single decoded message. Convenience for short
        scripts and tests that don't need an async generator."""
        if self._ws is None:
            raise RuntimeError("SensingClient not connected. Use `async with` first.")
        if timeout is None:
            frame = await self._ws.recv()
        else:
            frame = await asyncio.wait_for(self._ws.recv(), timeout=timeout)
        if isinstance(frame, bytes):
            frame = frame.decode("utf-8", errors="replace")
        return _decode(frame)

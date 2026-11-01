"""ADR-117 P4 — Typed listener for HA-MIND semantic primitives.

ADR-115 §3.12 defines 10 fused inference outputs that the sensing-server
publishes under the HA-DISCO MQTT namespace. This module gives clients
a typed handle on them so they can write `if event.kind ==
SemanticPrimitive.SomeoneSleeping: ...` instead of pattern-matching
strings.

The 10 v1 primitives (ADR-115 §3.12.1):

| Enum value | Topic suffix | Output kind |
|---|---|---|
| `SomeoneSleeping` | `someone_sleeping` | binary_sensor |
| `PossibleDistress` | `possible_distress` | binary_sensor + event |
| `RoomActive` | `room_active` | binary_sensor |
| `ElderlyInactivityAnomaly` | `elderly_inactivity_anomaly` | binary_sensor + event |
| `MeetingInProgress` | `meeting_in_progress` | binary_sensor |
| `BathroomOccupied` | `bathroom_occupied` | binary_sensor |
| `FallRiskElevated` | `fall_risk_elevated` | sensor (0–100) + event |
| `BedExit` | `bed_exit` | event |
| `NoMovementSafety` | `no_movement` | binary_sensor + event |
| `MultiRoomTransition` | `multi_room_transition` | event |
"""

from __future__ import annotations

import enum
import json
from dataclasses import dataclass, field
from typing import Any, Callable, Optional


# ─── Enum ────────────────────────────────────────────────────────────


class SemanticPrimitive(enum.Enum):
    """One of the 10 HA-MIND fused inference outputs."""
    SomeoneSleeping = "someone_sleeping"
    PossibleDistress = "possible_distress"
    RoomActive = "room_active"
    ElderlyInactivityAnomaly = "elderly_inactivity_anomaly"
    MeetingInProgress = "meeting_in_progress"
    BathroomOccupied = "bathroom_occupied"
    FallRiskElevated = "fall_risk_elevated"
    BedExit = "bed_exit"
    NoMovementSafety = "no_movement"
    MultiRoomTransition = "multi_room_transition"

    @classmethod
    def from_object_id(cls, object_id: str) -> Optional["SemanticPrimitive"]:
        for v in cls:
            if v.value == object_id:
                return v
        return None


# ─── Event payload ───────────────────────────────────────────────────


@dataclass(frozen=True)
class SemanticPrimitiveEvent:
    """A single fired event for one semantic primitive.

    `state` semantics depend on the primitive kind:
    - binary_sensor: "ON" / "OFF"
    - sensor: numeric string (e.g. "73" for fall_risk_elevated 0–100)
    - event: "fired" or an event-class string like "bed_exit_detected"
    """
    kind: SemanticPrimitive
    node_id: str
    state: str
    confidence: float = 0.0
    explanation: tuple[str, ...] = ()
    timestamp: float = 0.0
    raw: dict[str, Any] = field(default_factory=dict, hash=False, compare=False)


# ─── Listener ────────────────────────────────────────────────────────


Callback = Callable[[SemanticPrimitiveEvent], None]


class SemanticPrimitiveListener:
    """Routes raw MQTT state messages to per-primitive callbacks.

    Designed to plug into RuViewMqttClient:

    ```python
    from wifi_densepose.client import (
        RuViewMqttClient, SemanticPrimitive, SemanticPrimitiveListener
    )

    listener = SemanticPrimitiveListener()
    listener.on(SemanticPrimitive.SomeoneSleeping, lambda e: print(e))

    client = RuViewMqttClient()
    client.on_message(
        # MQTT `+` must fill a whole topic level; the listener skips non-RuView topics.
        "homeassistant/+/+/+/state",
        listener.handle_mqtt_message,
    )
    client.start()
    ```

    The listener itself never touches MQTT — it's a pure router. You
    feed it `(topic, payload)` pairs and it figures out which primitive
    the topic refers to and decodes the payload.
    """

    # Matches state topics for any of the 10 primitives.
    # homeassistant/<kind>/wifi_densepose_<node>/<primitive_slug>/state
    _SLUGS = {p.value for p in SemanticPrimitive}

    def __init__(self) -> None:
        self._handlers: dict[Optional[SemanticPrimitive], list[Callback]] = {}

    def on(self, primitive: SemanticPrimitive, cb: Callback) -> None:
        """Register a callback for a specific primitive."""
        self._handlers.setdefault(primitive, []).append(cb)

    def on_any(self, cb: Callback) -> None:
        """Register a callback that fires for ALL primitives. Useful
        for logging or dashboards."""
        self._handlers.setdefault(None, []).append(cb)

    def handle_mqtt_message(self, topic: str, payload: Any) -> Optional[SemanticPrimitiveEvent]:
        """Decode one MQTT message into a SemanticPrimitiveEvent and
        fire the matching callbacks. Returns the event (or None if the
        topic was not a semantic-primitive state topic)."""
        parts = topic.split("/")
        # Shape: homeassistant / <kind> / wifi_densepose_<node> / <slug> / state
        if len(parts) != 5:
            return None
        if parts[0] != "homeassistant" or parts[4] != "state":
            return None
        node_prefix = parts[2]
        if not node_prefix.startswith("wifi_densepose_"):
            return None
        slug = parts[3]
        if slug not in self._SLUGS:
            return None

        primitive = SemanticPrimitive.from_object_id(slug)
        if primitive is None:  # pragma: no cover — guarded above
            return None

        node_id = node_prefix[len("wifi_densepose_"):]
        event = _decode_event(primitive, node_id, payload)

        # Dispatch — primitive-specific first, then "any" handlers.
        for cb in self._handlers.get(primitive, ()):
            cb(event)
        for cb in self._handlers.get(None, ()):
            cb(event)
        return event


def _decode_event(
    primitive: SemanticPrimitive,
    node_id: str,
    payload: Any,
) -> SemanticPrimitiveEvent:
    """Decode a raw state payload into a typed event.

    HA state payloads come in two shapes:
    1. Plain string ("ON", "OFF", "73") — used by binary_sensor/sensor
       with no json_attributes_topic.
    2. JSON object with `state` + `confidence` + `explanation` fields —
       used by HA-MIND semantic primitives per ADR-115 §3.12.4.

    Both are supported transparently.
    """
    if isinstance(payload, bytes):
        try:
            payload = payload.decode("utf-8")
        except UnicodeDecodeError:
            return SemanticPrimitiveEvent(
                kind=primitive, node_id=node_id, state="", raw={}
            )

    if isinstance(payload, dict):
        body = payload
    elif isinstance(payload, str):
        # Try to JSON-decode; if it's not JSON, treat as a plain state string.
        try:
            decoded = json.loads(payload)
        except json.JSONDecodeError:
            return SemanticPrimitiveEvent(
                kind=primitive,
                node_id=node_id,
                state=payload,
                raw={"state": payload},
            )
        if isinstance(decoded, dict):
            body = decoded
        else:
            return SemanticPrimitiveEvent(
                kind=primitive,
                node_id=node_id,
                state=str(decoded),
                raw={"state": decoded},
            )
    else:
        return SemanticPrimitiveEvent(
            kind=primitive, node_id=node_id, state=str(payload), raw={}
        )

    expl = body.get("explanation") or body.get("reason") or ()
    if isinstance(expl, str):
        expl_tuple: tuple[str, ...] = (expl,)
    else:
        expl_tuple = tuple(str(x) for x in expl)

    return SemanticPrimitiveEvent(
        kind=primitive,
        node_id=node_id,
        # Score sensors (fall_risk_elevated) publish `{"score": ..., "ts": ...}`.
        state=str(body.get("state", body.get("score", ""))),
        confidence=float(body.get("confidence", 0.0)),
        explanation=expl_tuple,
        timestamp=float(body.get("timestamp", 0.0)),
        raw=body,
    )

"""ADR-117 P4 — Tests for RuViewMqttClient.

These tests do NOT bring up a broker — they exercise:

1. Topic-wildcard matching (`_topic_matches`)
2. Client construction + handler registration
3. The callback path by directly invoking the paho callback methods
   with synthesized messages

End-to-end broker integration is a P4-followon item (the mosquitto
patterns from memory [[feedback_mqtt_integration_test_patterns]] go
there). This file keeps unit coverage tight without requiring a
broker on every CI run.
"""

from __future__ import annotations

import json
from types import SimpleNamespace
from typing import Any

import pytest

from wifi_densepose.client import (
    RuViewMqttClient,
    SemanticPrimitive,
    SemanticPrimitiveListener,
)
from wifi_densepose.client.mqtt import _topic_matches


# ─── Topic wildcard matcher ──────────────────────────────────────────


@pytest.mark.parametrize("pattern,topic,expected", [
    ("ruview/+/raw/edge_vitals", "ruview/aabb/raw/edge_vitals", True),
    ("ruview/+/raw/edge_vitals", "ruview/aabb/cooked/edge_vitals", False),
    ("ruview/+/raw/+", "ruview/aabb/raw/pose", True),
    ("ruview/+/raw/+", "ruview/aabb/raw/pose/extra", False),
    # Per MQTT v5 §4.7.1.2: `+` is a whole-level wildcard only — mid-
    # segment `+` is a literal `+` character, not a wildcard. The
    # spec-correct way to wildcard the third segment of the HA
    # discovery topic is `homeassistant/+/+/+/config`.
    ("homeassistant/+/+/+/config",
     "homeassistant/binary_sensor/wifi_densepose_aabb/presence/config", True),
    # `wifi_densepose_+` is therefore NOT a wildcard — it matches the
    # literal string only. Asserting that behaviour stays stable.
    ("homeassistant/+/wifi_densepose_+/+/config",
     "homeassistant/binary_sensor/wifi_densepose_aabb/presence/config", False),
    ("ruview/#", "ruview/aabb/raw/edge_vitals", True),
    # Per MQTT v5 §4.7.1.2: `<prefix>/#` ALSO matches the bare
    # `<prefix>` itself (it represents "this topic and all sub-topics").
    ("ruview/#", "ruview", True),
    ("ruview/+/raw/#", "ruview/aabb/raw/pose/extra", True),
    ("exact/topic", "exact/topic", True),
    ("exact/topic", "exact/topic/extra", False),
    ("a/b/c", "a/b", False),
])
def test_topic_matches(pattern: str, topic: str, expected: bool) -> None:
    assert _topic_matches(pattern, topic) is expected


# ─── RuViewMqttClient construction ──────────────────────────────────


def test_client_constructs_with_defaults() -> None:
    c = RuViewMqttClient()
    assert c.broker_host == "localhost"
    assert c.broker_port == 1883
    assert c.connected is False
    assert c.client_id.startswith("wifi-densepose-client-")


def test_client_unique_client_id_per_instance() -> None:
    """Per the rumqttc memory lesson — each instance needs a unique
    client_id so parallel tests don't kick each other off the broker."""
    c1 = RuViewMqttClient()
    c2 = RuViewMqttClient()
    assert c1.client_id != c2.client_id


def test_client_accepts_explicit_client_id() -> None:
    c = RuViewMqttClient(client_id="explicit-id")
    assert c.client_id == "explicit-id"


# ─── Handler registration ────────────────────────────────────────────


def test_handler_registration_stores_callback() -> None:
    c = RuViewMqttClient()
    seen: list[Any] = []
    c.on_message("ruview/+/raw/edge_vitals", lambda t, p: seen.append((t, p)))
    # Internal state — we're allowed to inspect since the handler
    # path needs to be unit-testable without a broker.
    assert "ruview/+/raw/edge_vitals" in c._handlers


def test_handler_unregister_drops_callback() -> None:
    c = RuViewMqttClient()
    c.on_message("ruview/+/raw/edge_vitals", lambda t, p: None)
    c.unsubscribe_handler("ruview/+/raw/edge_vitals")
    assert "ruview/+/raw/edge_vitals" not in c._handlers


# ─── Callback dispatch (synthesized) ─────────────────────────────────


def _fake_message(topic: str, body: Any) -> Any:
    """Synthesize a paho-mqtt MQTTMessage-ish object."""
    if isinstance(body, (dict, list)):
        payload_bytes = json.dumps(body).encode("utf-8")
    elif isinstance(body, bytes):
        payload_bytes = body
    else:
        payload_bytes = str(body).encode("utf-8")
    return SimpleNamespace(topic=topic, payload=payload_bytes)


def test_message_dispatch_to_matching_handler() -> None:
    c = RuViewMqttClient()
    received: list[tuple[str, Any]] = []
    c.on_message("ruview/+/raw/edge_vitals", lambda t, p: received.append((t, p)))

    msg = _fake_message(
        "ruview/aabbccddeeff/raw/edge_vitals",
        {"breathing_rate_bpm": 14.0, "heartrate_bpm": 72.0, "presence": True},
    )
    c._on_message(None, None, msg)

    assert len(received) == 1
    topic, payload = received[0]
    assert topic == "ruview/aabbccddeeff/raw/edge_vitals"
    assert payload["breathing_rate_bpm"] == 14.0


def test_message_dispatch_ignores_non_matching_topic() -> None:
    c = RuViewMqttClient()
    received: list[Any] = []
    c.on_message("ruview/+/raw/edge_vitals", lambda t, p: received.append(p))

    msg = _fake_message("ruview/aabb/raw/pose", {"persons": []})
    c._on_message(None, None, msg)

    assert received == []


def test_message_dispatch_falls_back_to_bytes_on_non_json() -> None:
    c = RuViewMqttClient()
    received: list[Any] = []
    c.on_message("custom/binary/+", lambda t, p: received.append(p))

    msg = _fake_message("custom/binary/data", b"\x00\x01\x02not-json")
    c._on_message(None, None, msg)

    assert received == [b"\x00\x01\x02not-json"]


def test_handler_exception_does_not_propagate() -> None:
    """A misbehaving user callback must not crash the paho network
    loop — exceptions are caught and logged."""
    c = RuViewMqttClient()
    seen_after_crash: list[Any] = []

    def crashing(_topic: str, _p: Any) -> None:
        raise RuntimeError("simulated callback crash")

    c.on_message("crashy/topic", crashing)
    c.on_message("safe/topic", lambda t, p: seen_after_crash.append(p))

    # First, the crashing handler — must NOT raise out of _on_message.
    c._on_message(None, None, _fake_message("crashy/topic", "anything"))
    # Then the safe handler — must still fire on a subsequent message.
    c._on_message(None, None, _fake_message("safe/topic", {"x": 1}))
    assert seen_after_crash == [{"x": 1}]


def test_multiple_handlers_for_overlapping_patterns_all_fire() -> None:
    c = RuViewMqttClient()
    a_received: list[Any] = []
    b_received: list[Any] = []
    c.on_message("ruview/+/raw/+", lambda t, p: a_received.append(p))
    c.on_message("ruview/aabb/raw/edge_vitals", lambda t, p: b_received.append(p))

    msg = _fake_message("ruview/aabb/raw/edge_vitals", {"presence": True})
    c._on_message(None, None, msg)

    assert len(a_received) == 1
    assert len(b_received) == 1


def test_documented_ha_state_pattern_subscribes_and_reaches_listener() -> None:
    """The subscription shown in python/README.md and the
    SemanticPrimitiveListener docstring. paho rejects mid-level `+`
    (e.g. `wifi_densepose_+`) with ValueError on subscribe."""
    listener = SemanticPrimitiveListener()
    received: list[Any] = []
    listener.on(SemanticPrimitive.SomeoneSleeping, received.append)

    c = RuViewMqttClient()
    c.on_message("homeassistant/+/+/+/state", listener.handle_mqtt_message)

    # The real paho client validates the filter before it checks for a
    # connection, so this exercises the same subscribe() the broker path runs.
    c._on_connect(c._client, None, None, 0)

    c._on_message(None, None, _fake_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/someone_sleeping/state",
        {"state": "ON", "confidence": 0.9},
    ))
    assert [(e.kind, e.node_id, e.state) for e in received] == [
        (SemanticPrimitive.SomeoneSleeping, "aabb", "ON"),
    ]


# ─── on_connect path ─────────────────────────────────────────────────


def test_on_connect_sets_event_and_subscribes() -> None:
    c = RuViewMqttClient()
    c.on_message("ruview/+/raw/edge_vitals", lambda t, p: None)

    # Stub the paho client so we can capture subscribe() calls.
    subscribed: list[str] = []
    stub = SimpleNamespace(subscribe=lambda pattern: subscribed.append(pattern))

    c._on_connect(stub, None, None, 0)
    assert c.connected is True
    assert subscribed == ["ruview/+/raw/edge_vitals"]


def test_on_connect_with_nonzero_rc_does_not_set_connected() -> None:
    c = RuViewMqttClient()
    stub = SimpleNamespace(subscribe=lambda pattern: None)
    c._on_connect(stub, None, None, 5)  # CONNACK fail
    assert c.connected is False

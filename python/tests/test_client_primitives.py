"""ADR-117 P4 — Tests for the HA-MIND semantic primitive listener.

Pure routing tests — no MQTT broker needed.
"""

from __future__ import annotations

import json

from wifi_densepose.client import (
    SemanticPrimitive,
    SemanticPrimitiveEvent,
    SemanticPrimitiveListener,
)


# ─── SemanticPrimitive enum ──────────────────────────────────────────


def test_enum_covers_all_10_v1_primitives() -> None:
    expected = {
        "someone_sleeping",
        "possible_distress",
        "room_active",
        "elderly_inactivity_anomaly",
        "meeting_in_progress",
        "bathroom_occupied",
        "fall_risk_elevated",
        "bed_exit",
        "no_movement",
        "multi_room_transition",
    }
    actual = {p.value for p in SemanticPrimitive}
    assert actual == expected


def test_enum_from_object_id_round_trips() -> None:
    for p in SemanticPrimitive:
        assert SemanticPrimitive.from_object_id(p.value) is p


def test_enum_from_object_id_returns_none_for_unknown() -> None:
    assert SemanticPrimitive.from_object_id("garbage") is None


# ─── Listener routing ────────────────────────────────────────────────


def test_listener_dispatches_to_specific_handler() -> None:
    listener = SemanticPrimitiveListener()
    received: list[SemanticPrimitiveEvent] = []
    listener.on(SemanticPrimitive.SomeoneSleeping, received.append)

    evt = listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/someone_sleeping/state",
        json.dumps({"state": "ON", "confidence": 0.92, "explanation": ["motion<5%"]}),
    )
    assert evt is not None
    assert evt.kind is SemanticPrimitive.SomeoneSleeping
    assert evt.node_id == "aabb"
    assert evt.state == "ON"
    assert evt.confidence == 0.92
    assert evt.explanation == ("motion<5%",)
    assert len(received) == 1
    assert received[0] is evt


def test_listener_on_any_fires_for_every_primitive() -> None:
    listener = SemanticPrimitiveListener()
    seen: list[SemanticPrimitiveEvent] = []
    listener.on_any(seen.append)

    listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/room_active/state",
        json.dumps({"state": "ON"}),
    )
    listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/bathroom_occupied/state",
        json.dumps({"state": "OFF"}),
    )
    assert len(seen) == 2
    assert seen[0].kind is SemanticPrimitive.RoomActive
    assert seen[1].kind is SemanticPrimitive.BathroomOccupied


def test_listener_specific_handler_does_not_fire_for_other_primitives() -> None:
    listener = SemanticPrimitiveListener()
    received: list[SemanticPrimitiveEvent] = []
    listener.on(SemanticPrimitive.PossibleDistress, received.append)

    listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/someone_sleeping/state",
        json.dumps({"state": "ON"}),
    )
    assert received == []


def test_listener_decodes_plain_state_string() -> None:
    """HA convention: binary_sensors that don't carry attributes emit
    plain strings ('ON' / 'OFF'). We must accept that too."""
    listener = SemanticPrimitiveListener()
    evt = listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/room_active/state",
        "ON",
    )
    assert evt is not None
    assert evt.state == "ON"
    assert evt.confidence == 0.0  # not provided in plain string
    assert evt.explanation == ()


def test_listener_decodes_numeric_sensor_state() -> None:
    """fall_risk_elevated is a 0–100 sensor — verify numeric string."""
    listener = SemanticPrimitiveListener()
    evt = listener.handle_mqtt_message(
        "homeassistant/sensor/wifi_densepose_aabb/fall_risk_elevated/state",
        "73",
    )
    assert evt is not None
    assert evt.kind is SemanticPrimitive.FallRiskElevated
    assert evt.state == "73"


def test_listener_decodes_server_score_payload() -> None:
    """The sensing-server publishes fall_risk_elevated as a ScorePayload
    (`{"score": ..., "ts": ...}`, mqtt/state.rs) with no `state` key."""
    listener = SemanticPrimitiveListener()
    evt = listener.handle_mqtt_message(
        "homeassistant/sensor/wifi_densepose_aabb/fall_risk_elevated/state",
        json.dumps({"score": 73.0, "ts": "2026-10-11T12:00:00Z"}),
    )
    assert evt is not None
    assert evt.state == "73.0"


def test_listener_routes_server_topic_slugs() -> None:
    """Slugs must match EntityKind::topic_slug() in the sensing-server's
    mqtt/discovery.rs, or the server's state messages are dropped."""
    listener = SemanticPrimitiveListener()
    received: list[SemanticPrimitive] = []
    listener.on_any(lambda e: received.append(e.kind))
    for slug in ("elderly_inactivity_anomaly", "no_movement"):
        listener.handle_mqtt_message(
            f"homeassistant/binary_sensor/wifi_densepose_aabb/{slug}/state", "ON"
        )
    assert received == [
        SemanticPrimitive.ElderlyInactivityAnomaly,
        SemanticPrimitive.NoMovementSafety,
    ]


def test_listener_decodes_bytes_payload() -> None:
    listener = SemanticPrimitiveListener()
    evt = listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/room_active/state",
        b"ON",
    )
    assert evt is not None
    assert evt.state == "ON"


def test_listener_ignores_non_state_topics() -> None:
    listener = SemanticPrimitiveListener()
    assert listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/room_active/config",
        json.dumps({"name": "Room Active"}),
    ) is None


def test_listener_ignores_unknown_slug() -> None:
    listener = SemanticPrimitiveListener()
    assert listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/unknown_primitive/state",
        "ON",
    ) is None


def test_listener_ignores_non_wifi_densepose_node() -> None:
    listener = SemanticPrimitiveListener()
    # third segment doesn't start with wifi_densepose_
    assert listener.handle_mqtt_message(
        "homeassistant/binary_sensor/aqara_fp2/room_active/state",
        "ON",
    ) is None


def test_listener_explanation_string_is_normalised_to_tuple() -> None:
    """Producers may send `explanation` as a single string by mistake;
    accept that and wrap in a 1-tuple so downstream code can iterate
    uniformly."""
    listener = SemanticPrimitiveListener()
    evt = listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aabb/possible_distress/state",
        json.dumps({"state": "ON", "explanation": "HR=120 baseline=80"}),
    )
    assert evt is not None
    assert evt.explanation == ("HR=120 baseline=80",)


def test_event_is_frozen() -> None:
    evt = SemanticPrimitiveEvent(
        kind=SemanticPrimitive.SomeoneSleeping,
        node_id="aabb",
        state="ON",
    )
    import pytest
    with pytest.raises((AttributeError, Exception)):  # FrozenInstanceError subclass
        evt.state = "OFF"  # type: ignore[misc]

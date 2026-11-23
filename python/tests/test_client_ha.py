"""ADR-117 P4 — Tests for HA-DISCO payload parsing.

Pure parsing tests — no MQTT broker needed.
"""

from __future__ import annotations

import json

import pytest

from wifi_densepose.client import (
    HABlueprintHelper,
    HaDiscoveryPayload,
    HaEntity,
)
from wifi_densepose.client.ha import (
    parse_discovery_payload,
    parse_discovery_topic,
)


# Real discovery payloads pulled from ADR-115 §3 (formatted for test
# readability; payloads are otherwise verbatim).
_PRESENCE_TOPIC = "homeassistant/binary_sensor/wifi_densepose_aabbccddeeff/presence/config"
_PRESENCE_BODY = {
    "name": "Presence",
    "unique_id": "wifi_densepose_aabbccddeeff_presence",
    "object_id": "wifi_densepose_aabbccddeeff_presence",
    "state_topic": "homeassistant/binary_sensor/wifi_densepose_aabbccddeeff/presence/state",
    "availability_topic": "homeassistant/binary_sensor/wifi_densepose_aabbccddeeff/presence/availability",
    "device_class": "occupancy",
    "icon": "mdi:motion-sensor",
}

_HEART_RATE_TOPIC = "homeassistant/sensor/wifi_densepose_aabbccddeeff/heart_rate/config"
_HEART_RATE_BODY = {
    "name": "Heart rate",
    "unique_id": "wifi_densepose_aabbccddeeff_heart_rate",
    "state_topic": "homeassistant/sensor/wifi_densepose_aabbccddeeff/heart_rate/state",
    "state_class": "measurement",
    "unit_of_measurement": "bpm",
    "icon": "mdi:heart-pulse",
    "json_attributes_topic": "homeassistant/sensor/wifi_densepose_aabbccddeeff/heart_rate/state",
}


# ─── Topic parsing ───────────────────────────────────────────────────


def test_parse_discovery_topic_binary_sensor() -> None:
    out = parse_discovery_topic(_PRESENCE_TOPIC)
    assert out == ("binary_sensor", "aabbccddeeff", "presence")


def test_parse_discovery_topic_sensor() -> None:
    out = parse_discovery_topic(_HEART_RATE_TOPIC)
    assert out == ("sensor", "aabbccddeeff", "heart_rate")


def test_parse_discovery_topic_event() -> None:
    out = parse_discovery_topic(
        "homeassistant/event/wifi_densepose_aabbccddeeff/fall/config"
    )
    assert out == ("event", "aabbccddeeff", "fall")


def test_parse_discovery_topic_accepts_default_server_node_id() -> None:
    """The sensing-server's node id defaults to its MQTT client id,
    `wifi-densepose-<8 hex>`, with `-node<N>` appended per node."""
    out = parse_discovery_topic(
        "homeassistant/binary_sensor/wifi_densepose_wifi-densepose-1a2b3c4d-node1/presence/config"
    )
    assert out == ("binary_sensor", "wifi-densepose-1a2b3c4d-node1", "presence")


def test_parse_discovery_topic_returns_none_for_non_discovery() -> None:
    assert parse_discovery_topic("homeassistant/binary_sensor/foo/state") is None
    assert parse_discovery_topic("ruview/aabbccddeeff/raw/edge_vitals") is None
    assert parse_discovery_topic("") is None


# ─── Payload parsing ─────────────────────────────────────────────────


def test_parse_discovery_payload_from_dict() -> None:
    out = parse_discovery_payload(_PRESENCE_TOPIC, _PRESENCE_BODY)
    assert out is not None
    assert out.entity_kind == "binary_sensor"
    assert out.node_id == "aabbccddeeff"
    assert out.object_id == "presence"
    assert out.payload["device_class"] == "occupancy"


def test_parse_discovery_payload_from_bytes() -> None:
    raw = json.dumps(_PRESENCE_BODY).encode("utf-8")
    out = parse_discovery_payload(_PRESENCE_TOPIC, raw)
    assert out is not None
    assert out.payload["unique_id"] == "wifi_densepose_aabbccddeeff_presence"


def test_parse_discovery_payload_from_string() -> None:
    raw = json.dumps(_PRESENCE_BODY)
    out = parse_discovery_payload(_PRESENCE_TOPIC, raw)
    assert out is not None
    assert out.entity_kind == "binary_sensor"


def test_parse_discovery_payload_rejects_malformed_json() -> None:
    assert parse_discovery_payload(_PRESENCE_TOPIC, "{ broken: json") is None


def test_parse_discovery_payload_rejects_non_object_root() -> None:
    assert parse_discovery_payload(_PRESENCE_TOPIC, "[1, 2, 3]") is None


def test_parse_discovery_payload_returns_none_for_non_discovery_topic() -> None:
    assert parse_discovery_payload(
        "ruview/aabbccddeeff/raw/edge_vitals",
        _PRESENCE_BODY,
    ) is None


# ─── HaEntity projection ─────────────────────────────────────────────


def test_ha_entity_from_payload_extracts_fields() -> None:
    p = HaDiscoveryPayload(
        entity_kind="sensor",
        node_id="aabbccddeeff",
        object_id="heart_rate",
        payload=_HEART_RATE_BODY,
    )
    e = HaEntity.from_payload(p)
    assert e.entity_kind == "sensor"
    assert e.unique_id == "wifi_densepose_aabbccddeeff_heart_rate"
    assert e.unit_of_measurement == "bpm"
    assert e.icon == "mdi:heart-pulse"
    assert e.json_attributes_topic == _HEART_RATE_BODY["json_attributes_topic"]


def test_ha_entity_handles_missing_optional_fields() -> None:
    p = HaDiscoveryPayload(
        entity_kind="event",
        node_id="aabbccddeeff",
        object_id="bed_exit",
        payload={"unique_id": "wifi_densepose_aabbccddeeff_bed_exit"},
    )
    e = HaEntity.from_payload(p)
    assert e.unique_id == "wifi_densepose_aabbccddeeff_bed_exit"
    assert e.device_class == ""
    assert e.unit_of_measurement == ""


# ─── HABlueprintHelper aggregation ───────────────────────────────────


def _populated_helper() -> HABlueprintHelper:
    h = HABlueprintHelper()
    h.add_payload(_PRESENCE_TOPIC, _PRESENCE_BODY)
    h.add_payload(_HEART_RATE_TOPIC, _HEART_RATE_BODY)
    # Same fields but a different node
    h.add_payload(
        "homeassistant/binary_sensor/wifi_densepose_ff00ff00ff00/presence/config",
        {**_PRESENCE_BODY, "unique_id": "wifi_densepose_ff00ff00ff00_presence"},
    )
    return h


def test_helper_starts_empty() -> None:
    h = HABlueprintHelper()
    assert len(h) == 0
    assert h.nodes() == []
    assert h.all_payloads() == []


def test_helper_aggregates_multiple_payloads() -> None:
    h = _populated_helper()
    assert len(h) == 3
    assert h.nodes() == ["aabbccddeeff", "ff00ff00ff00"]


def test_helper_entities_for_node() -> None:
    h = _populated_helper()
    entities = h.entities_for_node("aabbccddeeff")
    object_ids = sorted(e.object_id for e in entities)
    assert object_ids == ["heart_rate", "presence"]


def test_helper_by_device_class() -> None:
    h = _populated_helper()
    occupancy_entities = h.by_device_class("occupancy")
    assert len(occupancy_entities) == 2  # presence on both nodes
    assert {e.node_id for e in occupancy_entities} == {"aabbccddeeff", "ff00ff00ff00"}


def test_helper_remove() -> None:
    h = _populated_helper()
    assert h.remove("aabbccddeeff", "binary_sensor", "presence") is True
    assert h.remove("aabbccddeeff", "binary_sensor", "presence") is False  # no-op
    assert len(h) == 2


def test_helper_rejects_non_discovery_topics() -> None:
    h = HABlueprintHelper()
    ok = h.add_payload("ruview/aabbccddeeff/raw/edge_vitals", _PRESENCE_BODY)
    assert ok is False
    assert len(h) == 0


def test_helper_in_operator() -> None:
    h = _populated_helper()
    assert ("aabbccddeeff", "binary_sensor", "presence") in h
    assert ("nonexistent", "binary_sensor", "presence") not in h

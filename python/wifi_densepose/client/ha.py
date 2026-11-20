"""ADR-117 P4 — Home Assistant MQTT-discovery payload helpers.

Parses the `homeassistant/<entity_kind>/wifi_densepose_<node>/<id>/config`
discovery payloads described in ADR-115 §3 into typed Python objects so
client code can introspect what a node is publishing without
hand-parsing JSON.

This is **read-only**: we do NOT generate discovery payloads from
Python (that's the sensing-server's job). The helper exists so a
client (HA blueprint author, debugger, dashboard) can ask "what
entities does this node expose?" and get a structured answer.

Example:

```python
from wifi_densepose.client import HaDiscoveryPayload, HABlueprintHelper

helper = HABlueprintHelper()
helper.add_payload(topic, json_bytes)
for entity in helper.entities_for_node("aabbccddeeff"):
    print(entity.entity_kind, entity.object_id, entity.unique_id)
```
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from typing import Any, Iterable


# ─── Topic schema ────────────────────────────────────────────────────


# Matches discovery topics like:
#   homeassistant/binary_sensor/wifi_densepose_aabbccddeeff/presence/config
#   homeassistant/sensor/wifi_densepose_aabbccddeeff/heart_rate/config
#   homeassistant/event/wifi_densepose_aabbccddeeff/fall/config
_DISCOVERY_TOPIC_RE = re.compile(
    r"^homeassistant/"
    r"(?P<entity_kind>[A-Za-z_]+)/"
    # Node ids default to the server's MQTT client id (`wifi-densepose-<hex>`,
    # plus `-node<N>` per node), so accept `-`, `_` and `.` as well.
    r"wifi_densepose_(?P<node_id>[A-Za-z0-9_.\-]+)/"
    r"(?P<object_id>[A-Za-z0-9_\-]+)/"
    r"config$"
)


@dataclass(frozen=True)
class HaDiscoveryPayload:
    """One MQTT discovery payload (config topic + JSON body)."""
    entity_kind: str  # "binary_sensor", "sensor", "event", "switch", ...
    node_id: str      # the node's MAC-ish identifier
    object_id: str    # entity slug (e.g. "presence", "heart_rate")
    payload: dict[str, Any]

    @property
    def topic(self) -> str:
        return (
            f"homeassistant/{self.entity_kind}/"
            f"wifi_densepose_{self.node_id}/{self.object_id}/config"
        )


@dataclass(frozen=True)
class HaEntity:
    """A user-facing view of one HA entity registered by a node."""
    entity_kind: str
    node_id: str
    object_id: str
    unique_id: str = ""
    name: str = ""
    state_topic: str = ""
    device_class: str = ""
    unit_of_measurement: str = ""
    icon: str = ""
    json_attributes_topic: str = ""

    @classmethod
    def from_payload(cls, p: HaDiscoveryPayload) -> "HaEntity":
        body = p.payload
        return cls(
            entity_kind=p.entity_kind,
            node_id=p.node_id,
            object_id=p.object_id,
            unique_id=str(body.get("unique_id", "")),
            name=str(body.get("name", "")),
            state_topic=str(body.get("state_topic", "")),
            device_class=str(body.get("device_class", "")),
            unit_of_measurement=str(body.get("unit_of_measurement", "")),
            icon=str(body.get("icon", "")),
            json_attributes_topic=str(body.get("json_attributes_topic", "")),
        )


def parse_discovery_topic(topic: str) -> tuple[str, str, str] | None:
    """Parse a discovery config topic into (entity_kind, node_id,
    object_id). Returns None for non-discovery topics."""
    m = _DISCOVERY_TOPIC_RE.match(topic)
    if not m:
        return None
    return (m.group("entity_kind"), m.group("node_id"), m.group("object_id"))


def parse_discovery_payload(
    topic: str, payload: bytes | str | dict[str, Any]
) -> HaDiscoveryPayload | None:
    """Decode an HA discovery payload. Returns None for non-discovery
    topics OR malformed JSON; raises only on programmer error."""
    parsed = parse_discovery_topic(topic)
    if parsed is None:
        return None
    entity_kind, node_id, object_id = parsed
    body: dict[str, Any]
    if isinstance(payload, dict):
        body = payload
    else:
        if isinstance(payload, bytes):
            try:
                payload = payload.decode("utf-8")
            except UnicodeDecodeError:
                return None
        try:
            decoded = json.loads(payload)
        except json.JSONDecodeError:
            return None
        if not isinstance(decoded, dict):
            return None
        body = decoded
    return HaDiscoveryPayload(
        entity_kind=entity_kind,
        node_id=node_id,
        object_id=object_id,
        payload=body,
    )


# ─── Helper / aggregator ─────────────────────────────────────────────


class HABlueprintHelper:
    """Aggregates HA discovery payloads observed on the bus and offers
    structured queries against them.

    Intended use: subscribe a RuViewMqttClient to
    `homeassistant/+/+/+/config`, feed every message into
    `add_payload()` (it ignores non-RuView topics), then ask the helper
    "what entities does node X expose?" or "what binary_sensors are
    presence-class?".
    """

    def __init__(self) -> None:
        # (node_id, entity_kind, object_id) → HaDiscoveryPayload
        self._payloads: dict[tuple[str, str, str], HaDiscoveryPayload] = {}

    def add_payload(self, topic: str, payload: bytes | str | dict[str, Any]) -> bool:
        """Returns True if the payload was a valid HA discovery
        message and was stored; False otherwise."""
        parsed = parse_discovery_payload(topic, payload)
        if parsed is None:
            return False
        self._payloads[(parsed.node_id, parsed.entity_kind, parsed.object_id)] = parsed
        return True

    def remove(self, node_id: str, entity_kind: str, object_id: str) -> bool:
        """Drop a stored payload — useful when handling a discovery
        retain-flag clear (HA's convention for removing an entity)."""
        return self._payloads.pop((node_id, entity_kind, object_id), None) is not None

    def __len__(self) -> int:
        return len(self._payloads)

    def __contains__(self, item: tuple[str, str, str]) -> bool:
        return item in self._payloads

    def all_payloads(self) -> list[HaDiscoveryPayload]:
        return list(self._payloads.values())

    def entities_for_node(self, node_id: str) -> list[HaEntity]:
        return [
            HaEntity.from_payload(p)
            for p in self._payloads.values()
            if p.node_id == node_id
        ]

    def nodes(self) -> list[str]:
        return sorted({p.node_id for p in self._payloads.values()})

    def by_device_class(self, device_class: str) -> list[HaEntity]:
        out: list[HaEntity] = []
        for p in self._payloads.values():
            e = HaEntity.from_payload(p)
            if e.device_class == device_class:
                out.append(e)
        return out

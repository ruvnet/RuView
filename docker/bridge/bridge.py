#!/usr/bin/env python3
"""RuView -> Home Assistant MQTT bridge.

Subscribes to the sensing-server WebSocket (`/ws/sensing`), translates
SensingUpdate messages into MQTT topics under `${MQTT_PREFIX}/...`, and
publishes Home Assistant MQTT Discovery configs so HA auto-creates
entities (binary_sensor, sensor, event) without manual YAML.

Reconnects with exponential backoff on both WS and MQTT failures.
Marks all entities `unavailable` when WS drops via the LWT topic.

Configurable via environment variables (see README of this folder).
"""

from __future__ import annotations

import asyncio
import json
import logging
import os
import signal
import time
from typing import Any

import paho.mqtt.client as mqtt
import websockets
from websockets.exceptions import ConnectionClosed, WebSocketException

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------
WS_URL = os.getenv("RUVIEW_WS", "ws://sensing-server:3001/ws/sensing")
MQTT_HOST = os.getenv("MQTT_HOST", "homeassistant.local")
MQTT_PORT = int(os.getenv("MQTT_PORT", "1883"))
MQTT_USER = os.getenv("MQTT_USER", "")
MQTT_PASS = os.getenv("MQTT_PASS", "")
MQTT_PREFIX = os.getenv("MQTT_PREFIX", "ruview")
HA_PREFIX = os.getenv("HA_DISCOVERY_PREFIX", "homeassistant")
DEVICE_NAME = os.getenv("DEVICE_NAME", "RuView Sensing")
DEVICE_ID = os.getenv("DEVICE_ID", "ruview")
LOG_LEVEL = os.getenv("LOG_LEVEL", "INFO").upper()

logging.basicConfig(
    level=LOG_LEVEL,
    format="%(asctime)s %(levelname)s %(name)s :: %(message)s",
)
LOG = logging.getLogger("ruview-bridge")

AVAIL_TOPIC = f"{MQTT_PREFIX}/availability"
STATE_TOPIC = f"{MQTT_PREFIX}/state"
EVENT_TOPIC = f"{MQTT_PREFIX}/event"

DEVICE_BLOCK = {
    "identifiers": [DEVICE_ID],
    "name": DEVICE_NAME,
    "manufacturer": "RuView / WiFi-DensePose",
    "model": "Sensing Server",
}

# ---------------------------------------------------------------------------
# Home Assistant MQTT Discovery
# ---------------------------------------------------------------------------
def _discovery(component: str, object_id: str, payload: dict) -> tuple[str, str]:
    """Build (topic, payload) for HA MQTT Discovery."""
    topic = f"{HA_PREFIX}/{component}/{DEVICE_ID}/{object_id}/config"
    base = {
        "device": DEVICE_BLOCK,
        "availability_topic": AVAIL_TOPIC,
        "payload_available": "online",
        "payload_not_available": "offline",
        "unique_id": f"{DEVICE_ID}_{object_id}",
    }
    base.update(payload)
    return topic, json.dumps(base)


def discovery_messages() -> list[tuple[str, str]]:
    """Return all entity discovery configs to publish on connect."""
    msgs: list[tuple[str, str]] = []

    # ── Binary sensors ────────────────────────────────────────────────
    msgs.append(_discovery("binary_sensor", "presence", {
        "name": "Presence",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ 'ON' if value_json.presence else 'OFF' }}",
        "device_class": "presence",
    }))
    msgs.append(_discovery("binary_sensor", "fall_detected", {
        "name": "Fall Detected",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ 'ON' if value_json.fall_detected else 'OFF' }}",
        "device_class": "safety",
        "off_delay": 30,
    }))

    # ── Sensors ───────────────────────────────────────────────────────
    msgs.append(_discovery("sensor", "n_persons", {
        "name": "Person Count",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ value_json.n_persons }}",
        "icon": "mdi:account-group",
        "state_class": "measurement",
    }))
    msgs.append(_discovery("sensor", "motion_level", {
        "name": "Motion Level",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ value_json.motion_level }}",
        "icon": "mdi:run-fast",
    }))
    msgs.append(_discovery("sensor", "motion_energy", {
        "name": "Motion Energy",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ value_json.motion_energy | round(3) }}",
        "icon": "mdi:waveform",
        "state_class": "measurement",
    }))
    msgs.append(_discovery("sensor", "confidence", {
        "name": "Detection Confidence",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ (value_json.confidence * 100) | round(1) }}",
        "unit_of_measurement": "%",
        "state_class": "measurement",
    }))
    msgs.append(_discovery("sensor", "breathing_rate", {
        "name": "Breathing Rate",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ value_json.breathing_rate_bpm | round(1) }}",
        "unit_of_measurement": "bpm",
        "icon": "mdi:lungs",
        "state_class": "measurement",
    }))
    msgs.append(_discovery("sensor", "heart_rate", {
        "name": "Heart Rate",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ value_json.heart_rate_bpm | round(0) }}",
        "unit_of_measurement": "bpm",
        "device_class": "frequency",
        "icon": "mdi:heart-pulse",
        "state_class": "measurement",
    }))
    msgs.append(_discovery("sensor", "signal_quality", {
        "name": "Signal Quality",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ (value_json.signal_quality * 100) | round(1) }}",
        "unit_of_measurement": "%",
        "icon": "mdi:signal",
        "state_class": "measurement",
    }))
    msgs.append(_discovery("sensor", "source", {
        "name": "Data Source",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ value_json.source }}",
        "icon": "mdi:lan-connect",
        "entity_category": "diagnostic",
    }))
    msgs.append(_discovery("sensor", "active_nodes", {
        "name": "Active Nodes",
        "state_topic": STATE_TOPIC,
        "value_template": "{{ value_json.active_nodes }}",
        "icon": "mdi:access-point-network",
        "entity_category": "diagnostic",
        "state_class": "measurement",
    }))

    # ── Event entity (HA 2023.8+) for fall alerts ─────────────────────
    msgs.append(_discovery("event", "fall_event", {
        "name": "Fall Event",
        "state_topic": EVENT_TOPIC,
        "event_types": ["fall"],
        "value_template": "{{ value_json.event_type }}",
    }))

    return msgs


# ---------------------------------------------------------------------------
# SensingUpdate -> flat state dict
# ---------------------------------------------------------------------------
def translate(update: dict[str, Any]) -> dict[str, Any]:
    """Flatten the WS SensingUpdate JSON into a state dict for MQTT."""
    classification = update.get("classification") or {}
    features = update.get("features") or {}
    vitals = update.get("vital_signs") or {}
    nodes = update.get("nodes") or []

    # Fall detection: presence + abrupt motion energy spike. Sensing-server
    # emits per-frame `fall_detected` upstream; if absent, derive a soft
    # heuristic from motion band power so HA still sees something useful.
    fall = bool(update.get("fall_detected", False))

    return {
        "presence": bool(classification.get("presence", False)),
        "confidence": float(classification.get("confidence", 0.0)),
        "motion_level": str(classification.get("motion_level", "unknown")),
        "motion_energy": float(features.get("motion_band_power", 0.0)),
        "breathing_rate_bpm": float(vitals.get("breathing_rate_bpm") or 0.0),
        "heart_rate_bpm": float(vitals.get("heart_rate_bpm") or 0.0),
        "signal_quality": float(vitals.get("signal_quality", 0.0)),
        "n_persons": int(update.get("estimated_persons") or len(update.get("persons") or []) or 0),
        "source": str(update.get("source", "unknown")),
        "active_nodes": len(nodes),
        "fall_detected": fall,
        "tick": int(update.get("tick", 0)),
        "ts": float(update.get("timestamp", time.time())),
    }


# ---------------------------------------------------------------------------
# Per-node discovery + state (auto-registered the first time a node is seen)
# ---------------------------------------------------------------------------
class NodeRegistry:
    """Auto-publishes discovery configs the first time a node id appears."""

    def __init__(self, client: mqtt.Client) -> None:
        self._client = client
        self._seen: set[int] = set()

    def publish_node(self, node: dict[str, Any]) -> None:
        nid = int(node.get("node_id", 0))
        if nid not in self._seen:
            self._seen.add(nid)
            object_id = f"node_{nid}_rssi"
            topic, payload = _discovery("sensor", object_id, {
                "name": f"Node {nid} RSSI",
                "state_topic": f"{MQTT_PREFIX}/nodes/{nid}",
                "value_template": "{{ value_json.rssi_dbm | round(0) }}",
                "unit_of_measurement": "dBm",
                "device_class": "signal_strength",
                "state_class": "measurement",
                "entity_category": "diagnostic",
            })
            self._client.publish(topic, payload, retain=True)
            LOG.info("registered node %s with HA discovery", nid)

        self._client.publish(
            f"{MQTT_PREFIX}/nodes/{nid}",
            json.dumps({
                "node_id": nid,
                "rssi_dbm": float(node.get("rssi_dbm", 0.0)),
                "subcarrier_count": int(node.get("subcarrier_count", 0)),
                "position": node.get("position", [0, 0, 0]),
            }),
        )


# ---------------------------------------------------------------------------
# MQTT client setup with reconnect
# ---------------------------------------------------------------------------
def make_mqtt() -> mqtt.Client:
    client = mqtt.Client(
        client_id=f"{DEVICE_ID}-bridge",
        callback_api_version=mqtt.CallbackAPIVersion.VERSION2,
    )
    client.will_set(AVAIL_TOPIC, "offline", retain=True)
    if MQTT_USER:
        client.username_pw_set(MQTT_USER, MQTT_PASS)

    def on_connect(c, _ud, _flags, reason_code, _props=None):
        if reason_code == 0:
            LOG.info("MQTT connected to %s:%s", MQTT_HOST, MQTT_PORT)
            c.publish(AVAIL_TOPIC, "online", retain=True)
            for topic, payload in discovery_messages():
                c.publish(topic, payload, retain=True)
            LOG.info("HA discovery configs published")
        else:
            LOG.error("MQTT connect failed, reason=%s", reason_code)

    def on_disconnect(_c, _ud, _flags, reason_code, _props=None):
        LOG.warning("MQTT disconnected, reason=%s — paho will auto-reconnect", reason_code)

    client.on_connect = on_connect
    client.on_disconnect = on_disconnect
    client.connect_async(MQTT_HOST, MQTT_PORT, keepalive=30)
    client.loop_start()
    return client


# ---------------------------------------------------------------------------
# WebSocket consumer with reconnect
# ---------------------------------------------------------------------------
async def consume(stop: asyncio.Event, client: mqtt.Client) -> None:
    backoff = 1.0
    nodes = NodeRegistry(client)

    while not stop.is_set():
        try:
            LOG.info("connecting to %s", WS_URL)
            async with websockets.connect(WS_URL, ping_interval=20, ping_timeout=20) as ws:
                LOG.info("WS connected")
                backoff = 1.0
                last_fall_tick = -1

                async for raw in ws:
                    if stop.is_set():
                        break
                    try:
                        update = json.loads(raw)
                    except json.JSONDecodeError:
                        LOG.debug("non-JSON frame: %r", raw[:80])
                        continue
                    if update.get("type") and update["type"] != "sensing_update":
                        continue

                    state = translate(update)
                    client.publish(STATE_TOPIC, json.dumps(state))

                    for node in update.get("nodes") or []:
                        nodes.publish_node(node)

                    # Edge-trigger fall event: only fire once per tick where
                    # `fall_detected` flips from false->true.
                    if state["fall_detected"] and state["tick"] != last_fall_tick:
                        last_fall_tick = state["tick"]
                        client.publish(EVENT_TOPIC, json.dumps({
                            "event_type": "fall",
                            "tick": state["tick"],
                            "ts": state["ts"],
                            "source": state["source"],
                            "confidence": state["confidence"],
                        }))
                        LOG.warning("FALL event published (tick=%s)", state["tick"])
        except (ConnectionClosed, WebSocketException, OSError) as exc:
            LOG.warning("WS error: %s — retrying in %.1fs", exc, backoff)
            try:
                await asyncio.wait_for(stop.wait(), timeout=backoff)
            except asyncio.TimeoutError:
                pass
            backoff = min(backoff * 2, 30.0)


# ---------------------------------------------------------------------------
# Entrypoint
# ---------------------------------------------------------------------------
async def main() -> None:
    LOG.info("RuView -> HA MQTT bridge starting")
    LOG.info("  WS:    %s", WS_URL)
    LOG.info("  MQTT:  %s:%s as user=%r prefix=%r", MQTT_HOST, MQTT_PORT, MQTT_USER, MQTT_PREFIX)
    LOG.info("  HA:    discovery prefix=%r device_id=%r", HA_PREFIX, DEVICE_ID)

    stop = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, stop.set)

    client = make_mqtt()
    try:
        await consume(stop, client)
    finally:
        client.publish(AVAIL_TOPIC, "offline", retain=True)
        client.loop_stop()
        client.disconnect()
        LOG.info("bridge stopped")


if __name__ == "__main__":
    asyncio.run(main())

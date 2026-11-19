"""ADR-117 P4 — paho-mqtt v2 wrapper for RuView MQTT topics.

Subscribes to the topic namespaces defined in ADR-115:

- `ruview/<node>/raw/edge_vitals` — opt-in firehose of the WS edge_vitals
- `ruview/<node>/raw/pose` — opt-in firehose of pose data
- `ruview/<node>/raw/sensing_update` — opt-in firehose of every sensing update
- `homeassistant/+/wifi_densepose_<node>/+/config` — HA discovery payloads
- `homeassistant/+/wifi_densepose_<node>/+/state` — HA state payloads

The client uses **paho-mqtt v2's `Client(CallbackAPIVersion.VERSION2)`**
API explicitly. v1's deprecated callback signatures will not work.

Example:

```python
from wifi_densepose.client import RuViewMqttClient

def on_edge_vitals(topic, payload):
    print(topic, payload["breathing_rate_bpm"])

client = RuViewMqttClient(broker_host="localhost", broker_port=1883)
client.on_message("ruview/+/raw/edge_vitals", on_edge_vitals)
client.start()
# ... runs in a background thread; call client.stop() to disconnect
```

The constructor never connects; call `.start()` to enter the network
loop and `.stop()` to disconnect cleanly. Both are idempotent.
"""

from __future__ import annotations

import json
import logging
import threading
import uuid
from collections.abc import Callable
from typing import Any

try:
    import paho.mqtt.client as mqtt  # type: ignore[import-not-found]
    from paho.mqtt.enums import CallbackAPIVersion  # type: ignore[import-not-found]
    _PAHO_AVAILABLE = True
except ImportError:  # pragma: no cover
    _PAHO_AVAILABLE = False


log = logging.getLogger(__name__)


MessageHandler = Callable[[str, Any], None]
"""(topic, decoded_payload) → None. The payload is JSON-decoded if the
content is valid JSON, otherwise the raw bytes are passed through."""


class RuViewMqttClient:
    """Wrapper around paho-mqtt v2 with per-topic-pattern callbacks.

    Per the rumqttc lesson [[feedback_mqtt_integration_test_patterns]]:
    - Each instance gets a unique client_id (per-test isolation when
      tests run in parallel against the same broker).
    - Subscription wildcards (`+`, `#`) are supported by paho's
      built-in matcher; we route by exact pattern match against the
      registered handler.
    """

    def __init__(
        self,
        *,
        broker_host: str = "localhost",
        broker_port: int = 1883,
        client_id: str | None = None,
        username: str | None = None,
        password: str | None = None,
        keepalive: int = 60,
        tls: bool = False,
    ) -> None:
        if not _PAHO_AVAILABLE:
            raise ImportError(
                "RuViewMqttClient requires the `paho-mqtt` package. Install with "
                "`pip install \"wifi-densepose[client]\"` to enable the client extras."
            )
        self.broker_host = broker_host
        self.broker_port = broker_port
        self.keepalive = keepalive
        self._client_id = client_id or f"wifi-densepose-client-{uuid.uuid4().hex[:12]}"
        self._handlers: dict[str, MessageHandler] = {}
        self._handlers_lock = threading.Lock()
        self._client = mqtt.Client(
            callback_api_version=CallbackAPIVersion.VERSION2,
            client_id=self._client_id,
            clean_session=True,
        )
        if username is not None:
            self._client.username_pw_set(username, password)
        if tls:
            self._client.tls_set()
        self._client.on_connect = self._on_connect
        self._client.on_message = self._on_message
        self._client.on_disconnect = self._on_disconnect
        self._started = False
        self._connected_event = threading.Event()

    @property
    def client_id(self) -> str:
        return self._client_id

    @property
    def connected(self) -> bool:
        return self._connected_event.is_set()

    # ── handler registration ─────────────────────────────────────────

    def on_message(self, topic_pattern: str, handler: MessageHandler) -> None:
        """Register a handler for a topic pattern. Replaces any
        previous handler for the same pattern."""
        with self._handlers_lock:
            self._handlers[topic_pattern] = handler

    def unsubscribe_handler(self, topic_pattern: str) -> None:
        with self._handlers_lock:
            self._handlers.pop(topic_pattern, None)
        if self._started:
            self._client.unsubscribe(topic_pattern)

    # ── lifecycle ────────────────────────────────────────────────────

    def start(self) -> None:
        """Connect to the broker and enter the network loop in a
        background thread. Idempotent."""
        if self._started:
            return
        self._client.connect(self.broker_host, self.broker_port, self.keepalive)
        self._client.loop_start()
        self._started = True

    def wait_connected(self, timeout: float = 5.0) -> bool:
        """Block until CONNACK has been received. Returns True on
        connect, False on timeout. Mirrors the rumqttc SubAck pump
        pattern but for paho's connect step."""
        return self._connected_event.wait(timeout=timeout)

    def stop(self) -> None:
        """Disconnect and stop the network loop. Idempotent."""
        if not self._started:
            return
        try:
            self._client.disconnect()
        except Exception as e:  # pragma: no cover — best-effort
            log.debug("ignored mqtt disconnect error: %r", e)
        try:
            self._client.loop_stop()
        except Exception as e:  # pragma: no cover
            log.debug("ignored mqtt loop_stop error: %r", e)
        self._started = False
        self._connected_event.clear()

    def publish(
        self,
        topic: str,
        payload: Any,
        *,
        qos: int = 0,
        retain: bool = False,
    ) -> None:
        """Publish a payload. Dicts/lists are JSON-encoded; bytes pass
        through; strings are encoded UTF-8."""
        if isinstance(payload, (dict, list)):
            data: Any = json.dumps(payload, default=str)
        else:
            data = payload
        info = self._client.publish(topic, data, qos=qos, retain=retain)
        # paho v2 returns MQTTMessageInfo; rc != MQTT_ERR_SUCCESS is a
        # broker-side error we should propagate so callers don't think
        # the publish succeeded.
        if info.rc != mqtt.MQTT_ERR_SUCCESS:
            raise RuntimeError(f"mqtt publish failed: topic={topic} rc={info.rc}")

    # ── paho callbacks (v2 signatures) ───────────────────────────────

    def _on_connect(self, client: Any, _userdata: Any, _flags: Any, reason_code: Any, _properties: Any = None) -> None:
        # paho v2 passes ReasonCode; success is 0 ("Success" / Granted_QoS_0)
        rc = int(reason_code) if hasattr(reason_code, "__int__") else reason_code
        if rc == 0:
            self._connected_event.set()
            # Re-subscribe to all known patterns. Important after a
            # reconnect — paho doesn't auto-resubscribe with
            # clean_session=True.
            with self._handlers_lock:
                patterns = list(self._handlers.keys())
            for pattern in patterns:
                client.subscribe(pattern)
            log.debug("mqtt CONNACK ok; subscribed to %d pattern(s)", len(patterns))
        else:
            log.warning("mqtt CONNACK with non-success rc=%r", reason_code)

    def _on_disconnect(self, _client: Any, _userdata: Any, _flags: Any = None, reason_code: Any = None, _properties: Any = None) -> None:
        self._connected_event.clear()
        log.debug("mqtt disconnected rc=%r", reason_code)

    def _on_message(self, _client: Any, _userdata: Any, message: Any) -> None:
        topic = message.topic
        # Best-effort JSON decode — fall back to raw bytes if it's not JSON.
        payload: Any
        try:
            payload = json.loads(message.payload.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            payload = message.payload

        with self._handlers_lock:
            handlers = list(self._handlers.items())

        for pattern, handler in handlers:
            if _topic_matches(pattern, topic):
                try:
                    handler(topic, payload)
                except Exception as e:  # never let a user callback crash the loop
                    log.exception("handler for pattern %r raised: %r", pattern, e)

    # ── re-subscribe on demand ──────────────────────────────────────

    def subscribe_registered(self) -> None:
        """Explicitly issue SUBSCRIBE for every registered handler.
        Useful when you registered handlers AFTER calling start().
        """
        if not self._started:
            return
        with self._handlers_lock:
            patterns = list(self._handlers.keys())
        for pattern in patterns:
            self._client.subscribe(pattern)


# ─── Topic-pattern matching ──────────────────────────────────────────


def _topic_matches(pattern: str, topic: str) -> bool:
    """MQTT topic wildcard matcher.

    - `+` matches exactly one topic level
    - `#` matches one or more remaining levels (must be the final segment)
    """
    p_parts = pattern.split("/")
    t_parts = topic.split("/")
    i = 0
    while i < len(p_parts):
        if p_parts[i] == "#":
            return i == len(p_parts) - 1 and len(t_parts) >= i
        if i >= len(t_parts):
            return False
        if p_parts[i] == "+":
            i += 1
            continue
        if p_parts[i] != t_parts[i]:
            return False
        i += 1
    return len(p_parts) == len(t_parts)

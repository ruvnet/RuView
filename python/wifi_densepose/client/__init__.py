"""ADR-117 P4 — Pure-Python client layer.

This sub-package is the **client-facing** half of `wifi-densepose`:
end users who only want to *consume* live RuView telemetry (rather than
running DSP locally) get a tight, opt-in client extra:

```
pip install "wifi-densepose[client]"
```

The runtime install footprint stays small for users who only need the
compiled PyO3 surface: `websockets` and `paho-mqtt` are declared as the
`[client]` extra in `pyproject.toml` and are NOT pulled in by the
default install.

## Modules

- `ws` — `SensingClient`: asyncio WebSocket client for the
  sensing-server `/ws/sensing` endpoint (ADR-115 §1)
- `mqtt` — `RuViewMqttClient`: paho-mqtt v2 wrapper for
  `ruview/<node>/raw/+` + `homeassistant/+/wifi_densepose_<node>/+/+`
  topics (ADR-115 §3)
- `primitives` — `SemanticPrimitiveListener`: typed view over the
  10 HA-MIND semantic primitives (ADR-115 §3.12)
- `ha` — `HABlueprintHelper`: parses MQTT-discovery payloads, helps
  users introspect what entities a node is publishing

No PyO3 here — this module is pure Python so it loads without the
compiled extension (useful for users who only want the client surface
and not the DSP pipeline).
"""

from __future__ import annotations

# Re-export the user-facing types. Import errors are deferred to the
# moment the user actually instantiates one of these classes — that way
# `from wifi_densepose.client import HABlueprintHelper` still works
# even if the user hasn't installed `[client]` extras yet (HABlueprint
# is pure stdlib).
from wifi_densepose.client.ha import (
    HABlueprintHelper,
    HaDiscoveryPayload,
    HaEntity,
)
from wifi_densepose.client.primitives import (
    SemanticPrimitive,
    SemanticPrimitiveEvent,
    SemanticPrimitiveListener,
)

__all__ = [
    # ws — re-exported lazily; see module docstring
    "SensingClient",
    "SensingMessage",
    "EdgeVitalsMessage",
    "PoseDataMessage",
    "ConnectionEstablishedMessage",
    # mqtt — re-exported lazily; see module docstring
    "RuViewMqttClient",
    # ha — pure stdlib
    "HaDiscoveryPayload",
    "HaEntity",
    "HABlueprintHelper",
    # primitives — pure stdlib
    "SemanticPrimitive",
    "SemanticPrimitiveEvent",
    "SemanticPrimitiveListener",
]


def __getattr__(name: str):
    """Lazy re-exports for the modules that pull in optional extras.

    `SensingClient` needs `websockets`; `RuViewMqttClient` needs
    `paho-mqtt`. Importing those at package init would make
    `wifi_densepose.client` unusable without the extras installed
    — defeating the point of an *optional* extra. We defer the import
    until the attribute is actually looked up.
    """
    if name in {
        "SensingClient",
        "SensingMessage",
        "EdgeVitalsMessage",
        "PoseDataMessage",
        "ConnectionEstablishedMessage",
    }:
        from wifi_densepose.client import ws as _ws
        return getattr(_ws, name)
    if name == "RuViewMqttClient":
        from wifi_densepose.client.mqtt import RuViewMqttClient as _R
        return _R
    raise AttributeError(f"module 'wifi_densepose.client' has no attribute {name!r}")

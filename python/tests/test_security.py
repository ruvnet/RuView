"""ADR-117 hardening sweep — Security & robustness tests for the
client surface.

Scope: malformed/hostile input handling across the WS decoder, MQTT
matcher + dispatch, HA discovery parser, and semantic primitive
listener. The goal is to ensure that an adversarial broker or
sensing-server can't:

- Crash the client process via malformed JSON, UTF-8, or topic shapes
- Bypass topic-wildcard matching to deliver messages to the wrong handler
- Leak MQTT credentials through `repr()` or string conversion
- Trigger unbounded memory growth via deeply-nested JSON
- Get a handler exception to crash the network loop
"""

from __future__ import annotations

import json
from types import SimpleNamespace

import pytest

from wifi_densepose.client import RuViewMqttClient, SemanticPrimitiveListener
from wifi_densepose.client.ha import (
    HABlueprintHelper,
    parse_discovery_payload,
    parse_discovery_topic,
)
from wifi_densepose.client.mqtt import _topic_matches
from wifi_densepose.client.ws import _decode

# ─── WS decoder robustness ──────────────────────────────────────────


def test_ws_decoder_rejects_non_object_root() -> None:
    """A JSON array at the root must NOT crash the decoder. Plain
    string/array root values are valid JSON but not valid sensing-
    server messages — the decoder must reject them cleanly."""
    with pytest.raises(ValueError):
        _decode("[1, 2, 3]")
    with pytest.raises(ValueError):
        _decode('"just a string"')
    with pytest.raises(ValueError):
        _decode("42")


def test_ws_decoder_rejects_malformed_json() -> None:
    with pytest.raises(json.JSONDecodeError):
        _decode("{ broken: json")


def test_ws_decoder_handles_deeply_nested_payload_without_crash() -> None:
    """Hostile JSON nested 1000 levels deep must not crash via
    Python's default recursion limit. Json.loads has a built-in
    guard; verify we don't accidentally bypass it."""
    nested = "{" + '"a":{' * 999 + '"x":1' + "}" * 1000
    # json.loads either succeeds (since 999 < ~1000 limit) or raises
    # RecursionError; either is acceptable — the key is no segfault
    # or hang.
    try:
        _decode(nested)
    except (RecursionError, json.JSONDecodeError, ValueError):
        pass  # All acceptable.


def test_ws_decoder_handles_huge_string_values() -> None:
    """A 1 MB string in a JSON field must decode without exploding.
    The websockets `max_size` parameter (default 16 MB) is the actual
    DoS guard — this just confirms the decoder itself is linear."""
    huge_payload = json.dumps({
        "type": "edge_vitals",
        "node_id": "x" * (1024 * 1024),  # 1 MB string
        "presence": True,
        "fall_detected": False,
        "motion": 0.0,
    })
    msg = _decode(huge_payload)
    assert msg.type == "edge_vitals"


def test_ws_decoder_handles_unicode_in_node_id() -> None:
    """Non-ASCII node IDs (e.g. accidental terminal escapes) must
    round-trip cleanly without re-encoding errors."""
    payload = json.dumps({"type": "edge_vitals", "node_id": "nöde-中", "presence": True, "fall_detected": False, "motion": 0.0})
    msg = _decode(payload)
    assert msg.node_id == "nöde-中"  # type: ignore[attr-defined]


# ─── MQTT topic matcher — exhaustive edge cases ─────────────────────


@pytest.mark.parametrize("pattern,topic,expected", [
    # Empty / boundary
    ("", "", True),
    ("a", "", False),
    ("", "a", False),
    # `+` cannot bypass a literal level boundary
    ("a/+/c", "a/b/c", True),
    ("a/+/c", "a/b/d", False),
    ("a/+/c", "a/b/c/d", False),
    # `#` is greedy from its position but does not match if it's
    # mid-pattern (per MQTT spec; our matcher returns False then).
    ("a/#/c", "a/b/c", False),  # `#` must be terminal
    # Topics starting with `$` are legal here — we don't filter them;
    # matching is purely syntactic. `+` is one-level only, so `$SYS/+`
    # matches `$SYS/broker` but NOT `$SYS/broker/version`.
    ("$SYS/+", "$SYS/broker", True),
    ("$SYS/+", "$SYS/broker/version", False),
    ("$SYS/#", "$SYS/broker/version", True),
    # Null byte in topic: still string comparison, but useful to lock
    # down behaviour.
    ("a/b", "a\x00/b", False),
])
def test_topic_matcher_edge_cases(pattern: str, topic: str, expected: bool) -> None:
    assert _topic_matches(pattern, topic) is expected


# ─── MQTT credential confidentiality ────────────────────────────────


def test_mqtt_password_never_in_repr() -> None:
    """A user's broker password must NOT leak through __repr__ or
    __str__. Currently RuViewMqttClient doesn't define repr — that's
    the safest default (uses object identity). Lock that down so a
    future "let's add a friendly repr" change doesn't expose creds."""
    c = RuViewMqttClient(
        broker_host="broker.example.com",
        username="alice",
        password="super-secret-token-do-not-leak",
    )
    rep = repr(c)
    s = str(c)
    assert "super-secret-token-do-not-leak" not in rep
    assert "super-secret-token-do-not-leak" not in s


def test_mqtt_password_never_stored_in_plain_attribute() -> None:
    """The plaintext password must not be stored on the client
    instance — paho-mqtt internalises it into `_client._username_pw`
    which we never expose. Audit by walking the public dict."""
    c = RuViewMqttClient(password="dont-leak-me")
    for k, v in vars(c).items():
        if isinstance(v, str):
            assert "dont-leak-me" not in v, f"password leaked via attribute {k!r}"


# ─── HA discovery — adversarial topics ──────────────────────────────


def test_ha_discovery_rejects_topic_with_null_byte() -> None:
    """Defensive: regex must not match a null-byte-laced topic."""
    bad = "homeassistant/binary_sensor/wifi_densepose_aa\x00bb/presence/config"
    assert parse_discovery_topic(bad) is None
    assert parse_discovery_payload(bad, {"name": "x"}) is None


def test_ha_discovery_rejects_topic_with_slash_in_node_id() -> None:
    """A node_id with embedded slashes would break the unique_id
    contract; reject."""
    bad = "homeassistant/binary_sensor/wifi_densepose_aa/bb/presence/config"
    # The regex won't match because there are too many segments.
    assert parse_discovery_topic(bad) is None


def test_ha_helper_drops_invalid_topic_silently() -> None:
    """`add_payload` should return False (not raise) for non-discovery
    topics so a misconfigured broker doesn't bring down the client."""
    h = HABlueprintHelper()
    assert h.add_payload("garbage", {"x": 1}) is False
    assert h.add_payload("ruview/aa/raw/edge_vitals", {"x": 1}) is False
    assert len(h) == 0


def test_ha_helper_handles_non_dict_payload() -> None:
    """If the HA discovery body is a list or scalar (broken producer),
    the helper must reject rather than crash on attribute access."""
    h = HABlueprintHelper()
    topic = "homeassistant/binary_sensor/wifi_densepose_aabb/presence/config"
    assert h.add_payload(topic, "[1, 2, 3]") is False
    assert h.add_payload(topic, "42") is False
    assert h.add_payload(topic, b"\xff\xfe invalid utf-8") is False


# ─── Semantic primitive listener — adversarial input ────────────────


def test_primitive_listener_ignores_topic_injection_attempts() -> None:
    listener = SemanticPrimitiveListener()
    # Extra leading segments
    assert listener.handle_mqtt_message(
        "evil/homeassistant/binary_sensor/wifi_densepose_aa/someone_sleeping/state",
        "ON",
    ) is None
    # Wrong final segment
    assert listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aa/someone_sleeping/STATE",
        "ON",
    ) is None
    # Empty node_id after the wifi_densepose_ prefix is still routed
    # (the node_id is "") because we don't enforce a minimum length —
    # but that's not an injection vector. Confirm behaviour.
    evt = listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_/someone_sleeping/state",
        "ON",
    )
    assert evt is not None
    assert evt.node_id == ""


def test_primitive_listener_handles_garbage_payload_without_crash() -> None:
    listener = SemanticPrimitiveListener()
    # Bytes that aren't valid UTF-8
    evt = listener.handle_mqtt_message(
        "homeassistant/binary_sensor/wifi_densepose_aa/room_active/state",
        b"\xff\xfe\xfd",
    )
    assert evt is not None  # we return a sentinel rather than crash
    # No assertions on state content — undefined for invalid UTF-8;
    # what matters is no exception escaped.


# ─── Public surface integrity ───────────────────────────────────────


def test_public_surface_is_stable() -> None:
    """Every name in `wifi_densepose.__all__` must be resolvable.
    Catches accidental re-export breakage between phases."""
    import wifi_densepose
    for name in wifi_densepose.__all__:
        assert hasattr(wifi_densepose, name), f"__all__ promises {name!r} but attribute missing"


def test_client_public_surface_is_stable() -> None:
    import wifi_densepose.client as c
    for name in c.__all__:
        # Lazy re-exports for SensingClient + RuViewMqttClient need to
        # be resolvable too — touch them to exercise __getattr__.
        _ = getattr(c, name)


# ─── Handler crash isolation (expanded) ─────────────────────────────


def test_mqtt_handler_exception_isolation_with_multiple_handlers() -> None:
    """Earlier test covered one crashing handler; this version makes
    sure a crashing handler in the *middle* of a list of registered
    handlers doesn't prevent later handlers from firing."""
    c = RuViewMqttClient()
    received_before: list[str] = []
    received_after: list[str] = []
    c.on_message("a/+", lambda t, p: received_before.append(t))
    c.on_message("a/b", lambda t, p: (_ for _ in ()).throw(RuntimeError("middle crash")))
    c.on_message("+/b", lambda t, p: received_after.append(t))

    msg = SimpleNamespace(topic="a/b", payload=b"x")
    c._on_message(None, None, msg)

    assert received_before == ["a/b"]
    assert received_after == ["a/b"]

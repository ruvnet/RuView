# ADR-049: Cross-Platform WiFi Interface Detection and Graceful Degradation

| Field | Value |
|-------|-------|
| Status | Proposed |
| Date | 2026-03-06 |
| Deciders | ruv |
| Depends on | ADR-013 (Feature-Level Sensing), ADR-025 (macOS CoreWLAN) |
| Issue | [#148](https://github.com/ruvnet/wifi-densepose/issues/148) |

## Context

Users report `RuntimeError: Cannot read /proc/net/wireless` when running WiFi DensePose in environments where the Linux wireless proc filesystem is unavailable:

- **Docker containers** on macOS/Windows (Linux kernel detected, but no wireless subsystem)
- **WSL2** without USB WiFi passthrough
- **Headless Linux servers** without WiFi hardware
- **Embedded Linux** boards without wireless-extensions support

The current architecture has two layers of defense:

1. **`ws_server.py`** (line 345-355) checks `os.path.exists("/proc/net/wireless")` before instantiating `LinuxWifiCollector` and falls back to `SimulatedCollector` if missing.
2. **`rssi_collector.py`** `LinuxWifiCollector._validate_interface()` (line 178-196) raises a hard `RuntimeError` if `/proc/net/wireless` is missing or the interface isn't listed.

However, there are gaps:

- **Direct usage**: Any code that instantiates `LinuxWifiCollector` directly (outside `ws_server.py`) hits the unguarded `RuntimeError` with no fallback.
- **Error message**: The RuntimeError message tells users to "use SimulatedCollector instead" but doesn't explain how.
- **No auto-detection**: The collector selection logic is duplicated between `ws_server.py` and `install.sh` with no shared platform-detection utility.
- **Partial `/proc/net/wireless`**: The file may exist (e.g., kernel module loaded) but contain no interfaces, producing a confusing "interface not found" error instead of a clean fallback.

## Decision

### 1. Platform-Aware Collector Factory

Introduce a `create_collector()` factory function in `rssi_collector.py` that encapsulates the platform detection and fallback chain:

```python
def create_collector(
    preferred: str = "auto",
    interface: str = "wlan0",
    sample_rate_hz: float = 10.0,
) -> BaseCollector:
    """
    Create the best available WiFi collector for the current platform.

    Resolution order (when preferred="auto"):
      1. ESP32 CSI (if UDP port 5005 is receiving frames)
      2. Platform-native WiFi:
         - Linux: LinuxWifiCollector (requires /proc/net/wireless + active interface)
         - Windows: WindowsWifiCollector (netsh wlan)
         - macOS: MacosWifiCollector (CoreWLAN)
      3. SimulatedCollector (always available)

    Raises nothing — always returns a usable collector.
    """
```

### 2. Soft Validation in LinuxWifiCollector

Replace the hard `RuntimeError` in `_validate_interface()` with a class method that returns availability status without raising:

```python
@classmethod
def is_available(cls, interface: str = "wlan0") -> tuple[bool, str]:
    """Check if Linux WiFi collection is possible. Returns (available, reason)."""
    if not os.path.exists("/proc/net/wireless"):
        return False, "/proc/net/wireless not found (Docker, WSL, or no wireless subsystem)"
    with open("/proc/net/wireless") as f:
        content = f.read()
    if interface not in content:
        names = cls._parse_interface_names(content)
        return False, f"Interface '{interface}' not in /proc/net/wireless. Available: {names}"
    return True, "ok"
```

The existing `_validate_interface()` continues to raise `RuntimeError` for direct callers who need fail-fast behavior, but `create_collector()` uses `is_available()` to probe without exceptions.

### 3. Structured Fallback Logging

When auto-detection skips a collector, log at `WARNING` level with actionable context:

```
WiFi collector: LinuxWifiCollector unavailable (/proc/net/wireless not found — likely Docker/WSL).
WiFi collector: Falling back to SimulatedCollector. For real sensing, connect ESP32 nodes via UDP:5005.
```

### 4. Consolidate Platform Detection

Remove duplicated platform-detection logic from `ws_server.py` and `install.sh`. Both should use `create_collector()` (Python) or a shared `detect_wifi_platform()` shell function.

## Consequences

### Positive

- **Zero-crash startup**: `create_collector("auto")` never raises — Docker, WSL, and headless users get `SimulatedCollector` automatically with a clear log message.
- **Single detection path**: Platform logic lives in one place (`rssi_collector.py`), reducing drift between `ws_server.py`, `install.sh`, and future entry points.
- **Better DX**: Error messages explain *why* a collector is unavailable and *what to do* (connect ESP32, install WiFi driver, etc.).

### Negative

- **SimulatedCollector may mask hardware issues**: Users with real WiFi hardware that fails detection might unknowingly run on simulated data. Mitigated by the `WARNING`-level log.
- **Breaking change for direct `LinuxWifiCollector` callers**: Code that catches `RuntimeError` from `_validate_interface()` as a signal needs to migrate to `is_available()` or `create_collector()`. This is a minor change — there are no known external consumers.

### Neutral

- `_validate_interface()` behavior is unchanged for existing direct callers — this is additive.

## Implementation Notes

1. Add `create_collector()` and `BaseCollector.is_available()` to `archive/v1/src/sensing/rssi_collector.py`
2. Refactor `ws_server.py` `_init_collector()` to call `create_collector()`
3. Update `install.sh` `detect_wifi_hardware()` to use shared detection logic
4. Add unit tests for each platform path (mock `/proc/net/wireless` presence/absence)
5. Comment on issue #148 with the fix

## References

- Issue #148: RuntimeError: Cannot read /proc/net/wireless
- ADR-013: Feature-Level Sensing on Commodity Gear
- ADR-025: macOS CoreWLAN WiFi Sensing
- [Linux /proc/net/wireless documentation](https://www.kernel.org/doc/html/latest/networking/statistics.html)

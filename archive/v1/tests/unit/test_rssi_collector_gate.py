"""The Windows collector must refuse to invent a signal.

Windows 11 gates ``netsh wlan show interfaces`` behind Location services. The
command still exits successfully and prints no interface block at all, so a
reader that only looks for a "Signal" line would carry the -80 dBm default for
ever: the classifier would report "absent" with nothing in the log to explain
it, and the dashboard would sit at "absent (100%)" without a word of
diagnostics.

These tests hold that refusal in place.
"""
from __future__ import annotations

import subprocess
from unittest.mock import patch

import pytest

from v1.src.sensing.rssi_collector import WindowsWifiCollector

# What Windows prints when the Location services gate is closed: the command
# succeeds, but no interface block follows.
GATED = """There is 1 interface on the system:

Network shell commands need location permission to access WLAN information.
Turn on Location services on the Location page in Privacy & security settings.

Function WlanQueryInterface returns error 5:
The requested operation requires elevation (Run as administrator).
"""

# netsh succeeded but listed nothing.
EMPTY = "There is 1 interface on the system:\n"

CONNECTED = """There is 1 interface on the system:

    Name                   : Wi-Fi 2
    Description            : Intel(R) Wi-Fi 6E AX210 160MHz
    State                  : connected
    SSID                   : home
    Signal                 : 94%
    Rssi                   : -39 dBm
"""

# Some Windows builds print the Rssi value without its unit.
CONNECTED_NO_UNIT = CONNECTED.replace("-39 dBm", "-39")


def _netsh(stdout: str) -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess(
        args=["netsh"], returncode=0, stdout=stdout, stderr=""
    )


def test_the_location_gate_is_reported() -> None:
    collector = WindowsWifiCollector(interface="Wi-Fi 2")
    with patch(
        "v1.src.sensing.rssi_collector.subprocess.run",
        return_value=_netsh(GATED),
    ):
        with pytest.raises(RuntimeError, match="Location services"):
            collector.collect_once()


def test_a_missing_signal_line_is_reported() -> None:
    collector = WindowsWifiCollector(interface="Wi-Fi 2")
    with patch(
        "v1.src.sensing.rssi_collector.subprocess.run",
        return_value=_netsh(EMPTY),
    ):
        with pytest.raises(RuntimeError, match="no signal data"):
            collector.collect_once()


def test_a_connected_interface_is_sampled() -> None:
    collector = WindowsWifiCollector(interface="Wi-Fi 2")
    with patch(
        "v1.src.sensing.rssi_collector.subprocess.run",
        return_value=_netsh(CONNECTED),
    ):
        sample = collector.collect_once()
    assert sample.rssi_dbm == pytest.approx(-39.0)
    assert sample.link_quality == pytest.approx(0.94)
    assert sample.interface == "Wi-Fi 2"


def test_the_rssi_value_is_read_without_its_unit() -> None:
    # One Windows build prints "Rssi : -72", another "Rssi : -39 dBm". Both
    # must yield the same reading; the unit must not throw the value away.
    for stdout in (CONNECTED, CONNECTED_NO_UNIT):
        collector = WindowsWifiCollector(interface="Wi-Fi 2")
        with patch(
            "v1.src.sensing.rssi_collector.subprocess.run",
            return_value=_netsh(stdout),
        ):
            sample = collector.collect_once()
        assert sample.rssi_dbm == pytest.approx(-39.0)


def test_the_gate_markers_cover_the_windows_message() -> None:
    for marker in WindowsWifiCollector.LOCATION_GATE_MARKERS:
        assert marker in GATED.lower()

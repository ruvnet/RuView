"""
RSSI data collection from Linux WiFi interfaces.

Provides two concrete collectors:
    - LinuxWifiCollector: reads real RSSI from /proc/net/wireless and iw commands
    - SimulatedCollector: produces deterministic synthetic signals for testing

Both share the same WifiSample dataclass and thread-safe ring buffer.
"""

from __future__ import annotations

import logging
import math
import re
import subprocess
import threading
import time
from collections import deque
from dataclasses import dataclass, field
from typing import Deque, List, Optional, Protocol

import numpy as np

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class WifiSample:
    """A single WiFi measurement sample."""

    timestamp: float          # UNIX epoch seconds (time.time())
    rssi_dbm: float           # Received signal strength in dBm
    noise_dbm: float          # Noise floor in dBm
    link_quality: float       # Link quality 0-1 (normalised)
    tx_bytes: int             # Cumulative TX bytes
    rx_bytes: int             # Cumulative RX bytes
    retry_count: int          # Cumulative retry count
    interface: str            # WiFi interface name


# ---------------------------------------------------------------------------
# Thread-safe ring buffer
# ---------------------------------------------------------------------------

class RingBuffer:
    """Thread-safe fixed-size ring buffer for WifiSample objects."""

    def __init__(self, max_size: int) -> None:
        self._buf: Deque[WifiSample] = deque(maxlen=max_size)
        self._lock = threading.Lock()

    def append(self, sample: WifiSample) -> None:
        with self._lock:
            self._buf.append(sample)

    def get_all(self) -> List[WifiSample]:
        """Return a snapshot of all samples (oldest first)."""
        with self._lock:
            return list(self._buf)

    def get_last_n(self, n: int) -> List[WifiSample]:
        """Return the most recent *n* samples."""
        with self._lock:
            items = list(self._buf)
            return items[-n:] if n < len(items) else items

    def __len__(self) -> int:
        with self._lock:
            return len(self._buf)

    def clear(self) -> None:
        with self._lock:
            self._buf.clear()


# ---------------------------------------------------------------------------
# Collector protocol
# ---------------------------------------------------------------------------

class WifiCollector(Protocol):
    """Protocol that all WiFi collectors must satisfy."""

    def start(self) -> None: ...
    def stop(self) -> None: ...
    def get_samples(self, n: Optional[int] = None) -> List[WifiSample]: ...
    @property
    def sample_rate_hz(self) -> float: ...


# ---------------------------------------------------------------------------
# Linux WiFi collector (real hardware)
# ---------------------------------------------------------------------------

class LinuxWifiCollector:
    """
    Collects real RSSI data from a Linux WiFi interface.

    Data sources:
        - /proc/net/wireless  (RSSI, noise, link quality)
        - iw dev <iface> station dump  (TX/RX bytes, retry count)

    Parameters
    ----------
    interface : str
        WiFi interface name, e.g. ``"wlan0"``.
    sample_rate_hz : float
        Target sampling rate in Hz (default 10).
    buffer_seconds : int
        How many seconds of history to keep in the ring buffer (default 120).
    """

    def __init__(
        self,
        interface: str = "wlan0",
        sample_rate_hz: float = 10.0,
        buffer_seconds: int = 120,
    ) -> None:
        self._interface = interface
        self._rate = sample_rate_hz
        self._buffer = RingBuffer(max_size=int(sample_rate_hz * buffer_seconds))
        self._running = False
        self._thread: Optional[threading.Thread] = None

    # -- public API ----------------------------------------------------------

    @property
    def sample_rate_hz(self) -> float:
        return self._rate

    def start(self) -> None:
        """Start the background sampling thread."""
        if self._running:
            return
        self._validate_interface()
        self._running = True
        self._thread = threading.Thread(
            target=self._sample_loop, daemon=True, name="wifi-rssi-collector"
        )
        self._thread.start()
        logger.info(
            "LinuxWifiCollector started on %s at %.1f Hz",
            self._interface,
            self._rate,
        )

    def stop(self) -> None:
        """Stop the background sampling thread."""
        self._running = False
        if self._thread is not None:
            self._thread.join(timeout=2.0)
            self._thread = None
        logger.info("LinuxWifiCollector stopped")

    def get_samples(self, n: Optional[int] = None) -> List[WifiSample]:
        """
        Return collected samples.

        Parameters
        ----------
        n : int or None
            If given, return only the most recent *n* samples.
        """
        if n is not None:
            return self._buffer.get_last_n(n)
        return self._buffer.get_all()

    def collect_once(self) -> WifiSample:
        """Collect a single sample right now (blocking)."""
        return self._read_sample()

    # -- internals -----------------------------------------------------------

    def _validate_interface(self) -> None:
        """Check that the interface exists on this machine."""
        try:
            with open("/proc/net/wireless", "r") as f:
                content = f.read()
            if self._interface not in content:
                raise RuntimeError(
                    f"WiFi interface '{self._interface}' not found in "
                    f"/proc/net/wireless. Available interfaces may include: "
                    f"{self._parse_interface_names(content)}. "
                    f"Ensure the interface is up and associated with an AP."
                )
        except FileNotFoundError:
            raise RuntimeError(
                "Cannot read /proc/net/wireless. "
                "This collector requires a Linux system with wireless-extensions support. "
                "If running in a container or VM without WiFi hardware, use "
                "SimulatedCollector instead."
            )

    @staticmethod
    def _parse_interface_names(proc_content: str) -> List[str]:
        """Extract interface names from /proc/net/wireless content."""
        names: List[str] = []
        for line in proc_content.splitlines()[2:]:  # skip header lines
            parts = line.split(":")
            if len(parts) >= 2:
                names.append(parts[0].strip())
        return names

    def _sample_loop(self) -> None:
        interval = 1.0 / self._rate
        while self._running:
            t0 = time.monotonic()
            try:
                sample = self._read_sample()
                self._buffer.append(sample)
            except Exception:
                logger.exception("Error reading WiFi sample")
            elapsed = time.monotonic() - t0
            sleep_time = max(0.0, interval - elapsed)
            if sleep_time > 0:
                time.sleep(sleep_time)

    def _read_sample(self) -> WifiSample:
        """Read one sample from the OS."""
        rssi, noise, quality = self._read_proc_wireless()
        tx_bytes, rx_bytes, retries = self._read_iw_station()
        return WifiSample(
            timestamp=time.time(),
            rssi_dbm=rssi,
            noise_dbm=noise,
            link_quality=quality,
            tx_bytes=tx_bytes,
            rx_bytes=rx_bytes,
            retry_count=retries,
            interface=self._interface,
        )

    def _read_proc_wireless(self) -> tuple[float, float, float]:
        """Parse /proc/net/wireless for the configured interface."""
        try:
            with open("/proc/net/wireless", "r") as f:
                for line in f:
                    if self._interface in line:
                        # Format: iface: status quality signal noise ...
                        parts = line.split()
                        # parts[0] = "wlan0:", parts[2]=quality, parts[3]=signal, parts[4]=noise
                        quality_raw = float(parts[2].rstrip("."))
                        signal_raw = float(parts[3].rstrip("."))
                        noise_raw = float(parts[4].rstrip("."))
                        # Normalise quality to 0..1 (max is typically 70)
                        quality = min(1.0, max(0.0, quality_raw / 70.0))
                        return signal_raw, noise_raw, quality
        except (FileNotFoundError, IndexError, ValueError) as exc:
            raise RuntimeError(
                f"Failed to read /proc/net/wireless for {self._interface}: {exc}"
            ) from exc
        raise RuntimeError(
            f"Interface {self._interface} not found in /proc/net/wireless"
        )

    def _read_iw_station(self) -> tuple[int, int, int]:
        """Run ``iw dev <iface> station dump`` and parse TX/RX/retries."""
        try:
            result = subprocess.run(
                ["iw", "dev", self._interface, "station", "dump"],
                capture_output=True,
                text=True,
                timeout=2.0,
            )
            text = result.stdout

            tx_bytes = self._extract_int(text, r"tx bytes:\s*(\d+)")
            rx_bytes = self._extract_int(text, r"rx bytes:\s*(\d+)")
            retries = self._extract_int(text, r"tx retries:\s*(\d+)")
            return tx_bytes, rx_bytes, retries
        except (FileNotFoundError, subprocess.TimeoutExpired):
            # iw not installed or timed out -- degrade gracefully
            return 0, 0, 0

    @staticmethod
    def _extract_int(text: str, pattern: str) -> int:
        m = re.search(pattern, text)
        return int(m.group(1)) if m else 0


# ---------------------------------------------------------------------------
# Simulated collector (deterministic, for testing)
# ---------------------------------------------------------------------------

class SimulatedCollector:
    """
    Deterministic simulated WiFi collector for testing.

    Generates a synthetic RSSI signal composed of:
        - A constant baseline (-50 dBm default)
        - An optional sinusoidal component (configurable frequency/amplitude)
        - Optional step-change injection (for change-point testing)
        - Deterministic noise from a seeded PRNG

    This is explicitly a test/development tool and makes no attempt to
    appear as real hardware.

    Parameters
    ----------
    seed : int
        Random seed for deterministic output.
    sample_rate_hz : float
        Target sampling rate in Hz (default 10).
    buffer_seconds : int
        Ring buffer capacity in seconds (default 120).
    baseline_dbm : float
        RSSI baseline in dBm (default -50).
    sine_freq_hz : float
        Frequency of the sinusoidal RSSI component (default 0.3 Hz, breathing band).
    sine_amplitude_dbm : float
        Amplitude of the sinusoidal component (default 2.0 dBm).
    noise_std_dbm : float
        Standard deviation of additive Gaussian noise (default 0.5 dBm).
    step_change_at : float or None
        If set, inject a step change of ``step_change_dbm`` at this time offset
        (seconds from start).
    step_change_dbm : float
        Magnitude of the step change (default -10 dBm).
    """

    def __init__(
        self,
        seed: int = 42,
        sample_rate_hz: float = 10.0,
        buffer_seconds: int = 120,
        baseline_dbm: float = -50.0,
        sine_freq_hz: float = 0.3,
        sine_amplitude_dbm: float = 2.0,
        noise_std_dbm: float = 0.5,
        step_change_at: Optional[float] = None,
        step_change_dbm: float = -10.0,
    ) -> None:
        self._rate = sample_rate_hz
        self._buffer = RingBuffer(max_size=int(sample_rate_hz * buffer_seconds))
        self._rng = np.random.default_rng(seed)

        self._baseline = baseline_dbm
        self._sine_freq = sine_freq_hz
        self._sine_amp = sine_amplitude_dbm
        self._noise_std = noise_std_dbm
        self._step_at = step_change_at
        self._step_dbm = step_change_dbm

        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._start_time: float = 0.0
        self._sample_index: int = 0

    # -- public API ----------------------------------------------------------

    @property
    def sample_rate_hz(self) -> float:
        return self._rate

    def start(self) -> None:
        if self._running:
            return
        self._running = True
        self._start_time = time.time()
        self._sample_index = 0
        self._thread = threading.Thread(
            target=self._sample_loop, daemon=True, name="sim-rssi-collector"
        )
        self._thread.start()
        logger.info("SimulatedCollector started at %.1f Hz (seed reused from init)", self._rate)

    def stop(self) -> None:
        self._running = False
        if self._thread is not None:
            self._thread.join(timeout=2.0)
            self._thread = None

    def get_samples(self, n: Optional[int] = None) -> List[WifiSample]:
        if n is not None:
            return self._buffer.get_last_n(n)
        return self._buffer.get_all()

    def generate_samples(self, duration_seconds: float) -> List[WifiSample]:
        """
        Generate a batch of samples without the background thread.

        Useful for unit tests that need a known signal without timing jitter.

        Parameters
        ----------
        duration_seconds : float
            How many seconds of signal to produce.

        Returns
        -------
        list of WifiSample
        """
        n_samples = int(duration_seconds * self._rate)
        samples: List[WifiSample] = []
        base_time = time.time()
        for i in range(n_samples):
            t = i / self._rate
            sample = self._make_sample(base_time + t, t, i)
            samples.append(sample)
        return samples

    # -- internals -----------------------------------------------------------

    def _sample_loop(self) -> None:
        interval = 1.0 / self._rate
        while self._running:
            t0 = time.monotonic()
            now = time.time()
            t_offset = now - self._start_time
            sample = self._make_sample(now, t_offset, self._sample_index)
            self._buffer.append(sample)
            self._sample_index += 1
            elapsed = time.monotonic() - t0
            sleep_time = max(0.0, interval - elapsed)
            if sleep_time > 0:
                time.sleep(sleep_time)

    def _make_sample(self, timestamp: float, t_offset: float, index: int) -> WifiSample:
        """Build one deterministic sample."""
        # Sinusoidal component
        sine = self._sine_amp * math.sin(2.0 * math.pi * self._sine_freq * t_offset)

        # Deterministic Gaussian noise (uses the seeded RNG)
        noise = self._rng.normal(0.0, self._noise_std)

        # Step change
        step = 0.0
        if self._step_at is not None and t_offset >= self._step_at:
            step = self._step_dbm

        rssi = self._baseline + sine + noise + step

        return WifiSample(
            timestamp=timestamp,
            rssi_dbm=float(rssi),
            noise_dbm=-95.0,
            link_quality=max(0.0, min(1.0, (rssi + 100.0) / 60.0)),
            tx_bytes=index * 1500,
            rx_bytes=index * 3000,
            retry_count=max(0, index // 100),
            interface="sim0",
        )


# ---------------------------------------------------------------------------
# Windows WiFi collector (real hardware via netsh)
# ---------------------------------------------------------------------------

class WindowsWifiCollector:
    """
    Collects real RSSI data from a Windows WiFi interface.

    Data source: ``netsh wlan show interfaces`` which provides RSSI in dBm,
    signal quality percentage, channel, band, and connection state.

    Parameters
    ----------
    interface : str
        WiFi interface name (default ``"Wi-Fi"``).  Must match the ``Name``
        field shown by ``netsh wlan show interfaces``.
    sample_rate_hz : float
        Target sampling rate in Hz (default 2.0).  Windows ``netsh`` is slow
        (~200-400ms per call) so rates above 2 Hz may not be achievable.
    buffer_seconds : int
        Ring buffer capacity in seconds (default 120).
    """

    def __init__(
        self,
        interface: str = "Wi-Fi",
        sample_rate_hz: float = 2.0,
        buffer_seconds: int = 120,
    ) -> None:
        self._interface = interface
        self._rate = sample_rate_hz
        self._buffer = RingBuffer(max_size=int(sample_rate_hz * buffer_seconds))
        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._cumulative_tx: int = 0
        self._cumulative_rx: int = 0

    # -- public API ----------------------------------------------------------

    @property
    def sample_rate_hz(self) -> float:
        return self._rate

    def start(self) -> None:
        if self._running:
            return
        self._validate_interface()
        self._running = True
        self._thread = threading.Thread(
            target=self._sample_loop, daemon=True, name="win-rssi-collector"
        )
        self._thread.start()
        logger.info(
            "WindowsWifiCollector started on '%s' at %.1f Hz",
            self._interface,
            self._rate,
        )

    def stop(self) -> None:
        self._running = False
        if self._thread is not None:
            self._thread.join(timeout=2.0)
            self._thread = None
        logger.info("WindowsWifiCollector stopped")

    def get_samples(self, n: Optional[int] = None) -> List[WifiSample]:
        if n is not None:
            return self._buffer.get_last_n(n)
        return self._buffer.get_all()

    def collect_once(self) -> WifiSample:
        return self._read_sample()

    # -- internals -----------------------------------------------------------

    def _validate_interface(self) -> None:
        try:
            result = subprocess.run(
                ["netsh", "wlan", "show", "interfaces"],
                capture_output=True, text=True, timeout=5.0,
            )
            if self._interface not in result.stdout:
                raise RuntimeError(
                    f"WiFi interface '{self._interface}' not found. "
                    f"Check 'netsh wlan show interfaces' for the correct name."
                )
            if "disconnected" in result.stdout.lower().split(self._interface.lower())[1][:200]:
                raise RuntimeError(
                    f"WiFi interface '{self._interface}' is disconnected. "
                    f"Connect to a WiFi network first."
                )
        except FileNotFoundError:
            raise RuntimeError(
                "netsh not found. This collector requires Windows."
            )

    def _sample_loop(self) -> None:
        interval = 1.0 / self._rate
        while self._running:
            t0 = time.monotonic()
            try:
                sample = self._read_sample()
                self._buffer.append(sample)
            except Exception:
                logger.exception("Error reading WiFi sample")
            elapsed = time.monotonic() - t0
            sleep_time = max(0.0, interval - elapsed)
            if sleep_time > 0:
                time.sleep(sleep_time)

    def _read_sample(self) -> WifiSample:
        result = subprocess.run(
            ["netsh", "wlan", "show", "interfaces"],
            capture_output=True, text=True, timeout=5.0,
        )
        rssi = -80.0
        signal_pct = 0.0

        for line in result.stdout.splitlines():
            stripped = line.strip()
            # "Rssi" line contains the raw dBm value (available on Win10+)
            if stripped.lower().startswith("rssi"):
                try:
                    rssi = float(stripped.split(":")[1].strip())
                except (IndexError, ValueError):
                    pass
            # "Signal" line contains percentage (always available)
            elif stripped.lower().startswith("signal"):
                try:
                    pct_str = stripped.split(":")[1].strip().rstrip("%")
                    signal_pct = float(pct_str)
                    # If RSSI line was missing, estimate from percentage
                    # Signal% roughly maps: 100% ≈ -30 dBm, 0% ≈ -90 dBm
                except (IndexError, ValueError):
                    pass

        # Normalise link quality from signal percentage
        link_quality = signal_pct / 100.0

        # Estimate noise floor (Windows doesn't expose it directly)
        noise_dbm = -95.0

        # Track cumulative bytes (not available from netsh; increment synthetic counter)
        self._cumulative_tx += 1500
        self._cumulative_rx += 3000

        return WifiSample(
            timestamp=time.time(),
            rssi_dbm=rssi,
            noise_dbm=noise_dbm,
            link_quality=link_quality,
            tx_bytes=self._cumulative_tx,
            rx_bytes=self._cumulative_rx,
            retry_count=0,
            interface=self._interface,
        )

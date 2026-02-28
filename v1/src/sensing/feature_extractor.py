"""
Signal feature extraction from RSSI time series.

Extracts both time-domain statistical features and frequency-domain spectral
features using real mathematics (scipy.fft, scipy.stats).  Also implements
CUSUM change-point detection for abrupt RSSI transitions.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import List, Optional, Tuple

import numpy as np
from numpy.typing import NDArray
from scipy import fft as scipy_fft
from scipy import stats as scipy_stats

from v1.src.sensing.rssi_collector import WifiSample

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Feature dataclass
# ---------------------------------------------------------------------------

@dataclass
class RssiFeatures:
    """Container for all extracted RSSI features."""

    # -- time-domain --------------------------------------------------------
    mean: float = 0.0
    variance: float = 0.0
    std: float = 0.0
    skewness: float = 0.0
    kurtosis: float = 0.0
    range: float = 0.0
    iqr: float = 0.0              # inter-quartile range

    # -- frequency-domain ---------------------------------------------------
    dominant_freq_hz: float = 0.0
    breathing_band_power: float = 0.0   # 0.1 - 0.5 Hz
    motion_band_power: float = 0.0      # 0.5 - 3.0 Hz
    total_spectral_power: float = 0.0

    # -- change-point -------------------------------------------------------
    change_points: List[int] = field(default_factory=list)
    n_change_points: int = 0

    # -- metadata -----------------------------------------------------------
    n_samples: int = 0
    duration_seconds: float = 0.0
    sample_rate_hz: float = 0.0


# ---------------------------------------------------------------------------
# Feature extractor
# ---------------------------------------------------------------------------

class RssiFeatureExtractor:
    """
    Extract time-domain and frequency-domain features from an RSSI time series.

    Parameters
    ----------
    window_seconds : float
        Length of the analysis window in seconds (default 30).
    cusum_threshold : float
        CUSUM threshold for change-point detection (default 3.0 standard deviations
        of the signal).
    cusum_drift : float
        CUSUM drift allowance (default 0.5 standard deviations).
    """

    def __init__(
        self,
        window_seconds: float = 30.0,
        cusum_threshold: float = 3.0,
        cusum_drift: float = 0.5,
    ) -> None:
        self._window_seconds = window_seconds
        self._cusum_threshold = cusum_threshold
        self._cusum_drift = cusum_drift

    @property
    def window_seconds(self) -> float:
        return self._window_seconds

    def extract(self, samples: List[WifiSample]) -> RssiFeatures:
        """
        Extract features from a list of WifiSample objects.

        Only the most recent ``window_seconds`` of data are used.
        At least 4 samples are required for meaningful features.
        """
        if len(samples) < 4:
            logger.warning(
                "Not enough samples for feature extraction (%d < 4)", len(samples)
            )
            return RssiFeatures(n_samples=len(samples))

        # Trim to window
        samples = self._trim_to_window(samples)
        rssi = np.array([s.rssi_dbm for s in samples], dtype=np.float64)
        timestamps = np.array([s.timestamp for s in samples], dtype=np.float64)

        # Estimate sample rate from actual timestamps
        dt = np.diff(timestamps)
        if len(dt) == 0 or np.mean(dt) <= 0:
            sample_rate = 10.0  # fallback
        else:
            sample_rate = 1.0 / np.mean(dt)

        duration = timestamps[-1] - timestamps[0] if len(timestamps) > 1 else 0.0

        # Build features
        features = RssiFeatures(
            n_samples=len(rssi),
            duration_seconds=float(duration),
            sample_rate_hz=float(sample_rate),
        )

        self._compute_time_domain(rssi, features)
        self._compute_frequency_domain(rssi, sample_rate, features)
        self._compute_change_points(rssi, features)

        return features

    def extract_from_array(
        self, rssi: NDArray[np.float64], sample_rate_hz: float
    ) -> RssiFeatures:
        """
        Extract features directly from a numpy array (useful for testing).

        Parameters
        ----------
        rssi : ndarray
            1-D array of RSSI values in dBm.
        sample_rate_hz : float
            Sampling rate in Hz.
        """
        if len(rssi) < 4:
            return RssiFeatures(n_samples=len(rssi))

        duration = len(rssi) / sample_rate_hz

        features = RssiFeatures(
            n_samples=len(rssi),
            duration_seconds=float(duration),
            sample_rate_hz=float(sample_rate_hz),
        )

        self._compute_time_domain(rssi, features)
        self._compute_frequency_domain(rssi, sample_rate_hz, features)
        self._compute_change_points(rssi, features)

        return features

    # -- time-domain ---------------------------------------------------------

    @staticmethod
    def _compute_time_domain(rssi: NDArray[np.float64], features: RssiFeatures) -> None:
        features.mean = float(np.mean(rssi))
        features.variance = float(np.var(rssi, ddof=1)) if len(rssi) > 1 else 0.0
        features.std = float(np.std(rssi, ddof=1)) if len(rssi) > 1 else 0.0
        features.skewness = float(scipy_stats.skew(rssi, bias=False)) if len(rssi) > 2 else 0.0
        features.kurtosis = float(scipy_stats.kurtosis(rssi, bias=False)) if len(rssi) > 3 else 0.0
        features.range = float(np.ptp(rssi))

        q75, q25 = np.percentile(rssi, [75, 25])
        features.iqr = float(q75 - q25)

    # -- frequency-domain ----------------------------------------------------

    @staticmethod
    def _compute_frequency_domain(
        rssi: NDArray[np.float64],
        sample_rate: float,
        features: RssiFeatures,
    ) -> None:
        """Compute one-sided FFT power spectrum and extract band powers."""
        n = len(rssi)
        if n < 4:
            return

        # Remove DC (subtract mean)
        signal = rssi - np.mean(rssi)

        # Apply Hann window to reduce spectral leakage
        window = np.hanning(n)
        windowed = signal * window

        # Compute real FFT
        fft_vals = scipy_fft.rfft(windowed)
        freqs = scipy_fft.rfftfreq(n, d=1.0 / sample_rate)

        # Power spectral density (magnitude squared, normalised by N)
        psd = (np.abs(fft_vals) ** 2) / n

        # Skip DC component (index 0)
        if len(freqs) > 1:
            freqs_no_dc = freqs[1:]
            psd_no_dc = psd[1:]
        else:
            return

        # Total spectral power
        features.total_spectral_power = float(np.sum(psd_no_dc))

        # Dominant frequency
        if len(psd_no_dc) > 0:
            peak_idx = int(np.argmax(psd_no_dc))
            features.dominant_freq_hz = float(freqs_no_dc[peak_idx])

        # Band powers
        features.breathing_band_power = float(
            _band_power(freqs_no_dc, psd_no_dc, 0.1, 0.5)
        )
        features.motion_band_power = float(
            _band_power(freqs_no_dc, psd_no_dc, 0.5, 3.0)
        )

    # -- change-point detection (CUSUM) --------------------------------------

    def _compute_change_points(
        self, rssi: NDArray[np.float64], features: RssiFeatures
    ) -> None:
        """
        Detect change points using the CUSUM algorithm.

        The CUSUM statistic tracks cumulative deviations from the mean,
        flagging points where the signal mean shifts abruptly.
        """
        if len(rssi) < 4:
            return

        mean_val = np.mean(rssi)
        std_val = np.std(rssi, ddof=1)
        if std_val < 1e-12:
            features.change_points = []
            features.n_change_points = 0
            return

        threshold = self._cusum_threshold * std_val
        drift = self._cusum_drift * std_val

        change_points = cusum_detect(rssi, mean_val, threshold, drift)
        features.change_points = change_points
        features.n_change_points = len(change_points)


# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------

def _band_power(
    freqs: NDArray[np.float64],
    psd: NDArray[np.float64],
    low_hz: float,
    high_hz: float,
) -> float:
    """Sum PSD within a frequency band [low_hz, high_hz]."""
    mask = (freqs >= low_hz) & (freqs <= high_hz)
    return float(np.sum(psd[mask]))


def cusum_detect(
    signal: NDArray[np.float64],
    target: float,
    threshold: float,
    drift: float,
) -> List[int]:
    """
    CUSUM (cumulative sum) change-point detection.

    Detects both upward and downward shifts in the signal mean.

    Parameters
    ----------
    signal : ndarray
        The 1-D signal to analyse.
    target : float
        Expected mean of the signal.
    threshold : float
        Decision threshold for declaring a change point.
    drift : float
        Allowable drift before accumulating deviation.

    Returns
    -------
    list of int
        Indices where change points were detected.
    """
    n = len(signal)
    s_pos = 0.0
    s_neg = 0.0
    change_points: List[int] = []

    for i in range(n):
        deviation = signal[i] - target
        s_pos = max(0.0, s_pos + deviation - drift)
        s_neg = max(0.0, s_neg - deviation - drift)

        if s_pos > threshold or s_neg > threshold:
            change_points.append(i)
            # Reset after detection to find subsequent changes
            s_pos = 0.0
            s_neg = 0.0

    return change_points

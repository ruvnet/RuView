"""
Presence and motion classification from RSSI features.

Uses rule-based logic with configurable thresholds to classify the current
sensing state into one of three motion levels:
    ABSENT        -- no person detected
    PRESENT_STILL -- person present but stationary
    ACTIVE        -- person present and moving

Confidence is derived from spectral feature strength and optional
cross-receiver agreement.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from enum import Enum
from typing import List, Optional

from v1.src.sensing.feature_extractor import RssiFeatures

logger = logging.getLogger(__name__)


class MotionLevel(Enum):
    """Classified motion state."""

    ABSENT = "absent"
    PRESENT_STILL = "present_still"
    ACTIVE = "active"


@dataclass
class SensingResult:
    """Output of the presence/motion classifier."""

    motion_level: MotionLevel
    confidence: float                 # 0.0 to 1.0
    presence_detected: bool
    rssi_variance: float
    motion_band_energy: float
    breathing_band_energy: float
    n_change_points: int
    details: str = ""


class PresenceClassifier:
    """
    Rule-based presence and motion classifier.

    Classification rules
    --------------------
    1. **Presence**: RSSI variance exceeds ``presence_variance_threshold``.
    2. **Motion level**:
       - ABSENT  if variance < presence threshold
       - ACTIVE  if variance >= presence threshold AND motion band energy
         exceeds ``motion_energy_threshold``
       - PRESENT_STILL otherwise (variance above threshold but low motion energy)

    Confidence model
    ----------------
    Base confidence comes from how far the measured variance / energy exceeds
    the respective thresholds.  Cross-receiver agreement (when multiple
    receivers report results) can boost confidence further.

    Parameters
    ----------
    presence_variance_threshold : float
        Minimum RSSI variance (dBm^2) to declare presence (default 0.5).
    motion_energy_threshold : float
        Minimum motion-band spectral energy to classify as ACTIVE (default 0.1).
    max_receivers : int
        Maximum number of receivers for cross-receiver agreement (default 1).
    """

    def __init__(
        self,
        presence_variance_threshold: float = 0.5,
        motion_energy_threshold: float = 0.1,
        max_receivers: int = 1,
    ) -> None:
        self._var_thresh = presence_variance_threshold
        self._motion_thresh = motion_energy_threshold
        self._max_receivers = max_receivers

    @property
    def presence_variance_threshold(self) -> float:
        return self._var_thresh

    @property
    def motion_energy_threshold(self) -> float:
        return self._motion_thresh

    def classify(
        self,
        features: RssiFeatures,
        other_receiver_results: Optional[List[SensingResult]] = None,
    ) -> SensingResult:
        """
        Classify presence and motion from extracted RSSI features.

        Parameters
        ----------
        features : RssiFeatures
            Features extracted from the RSSI time series of one receiver.
        other_receiver_results : list of SensingResult, optional
            Results from other receivers for cross-receiver agreement.

        Returns
        -------
        SensingResult
        """
        variance = features.variance
        motion_energy = features.motion_band_power
        breathing_energy = features.breathing_band_power

        # -- presence decision ------------------------------------------------
        presence = variance >= self._var_thresh

        # -- motion level -----------------------------------------------------
        if not presence:
            level = MotionLevel.ABSENT
        elif motion_energy >= self._motion_thresh:
            level = MotionLevel.ACTIVE
        else:
            level = MotionLevel.PRESENT_STILL

        # -- confidence -------------------------------------------------------
        confidence = self._compute_confidence(
            variance, motion_energy, breathing_energy, level, other_receiver_results
        )

        # -- detail string ----------------------------------------------------
        details = (
            f"var={variance:.4f} (thresh={self._var_thresh}), "
            f"motion_energy={motion_energy:.4f} (thresh={self._motion_thresh}), "
            f"breathing_energy={breathing_energy:.4f}, "
            f"change_points={features.n_change_points}"
        )

        return SensingResult(
            motion_level=level,
            confidence=confidence,
            presence_detected=presence,
            rssi_variance=variance,
            motion_band_energy=motion_energy,
            breathing_band_energy=breathing_energy,
            n_change_points=features.n_change_points,
            details=details,
        )

    def _compute_confidence(
        self,
        variance: float,
        motion_energy: float,
        breathing_energy: float,
        level: MotionLevel,
        other_results: Optional[List[SensingResult]],
    ) -> float:
        """
        Compute a confidence score in [0, 1].

        The score is composed of:
            - Base (60%): how clearly the variance exceeds (or falls below) the
              presence threshold.
            - Spectral (20%): strength of the relevant spectral band.
            - Agreement (20%): cross-receiver consensus (if available).
        """
        # -- base confidence (0..1) ------------------------------------------
        if level == MotionLevel.ABSENT:
            # Confidence in absence increases as variance shrinks relative to threshold
            if self._var_thresh > 0:
                base = max(0.0, 1.0 - variance / self._var_thresh)
            else:
                base = 1.0
        else:
            # Confidence in presence increases as variance exceeds threshold
            ratio = variance / self._var_thresh if self._var_thresh > 0 else 10.0
            base = min(1.0, ratio)

        # -- spectral confidence (0..1) --------------------------------------
        if level == MotionLevel.ACTIVE:
            spectral = min(1.0, motion_energy / max(self._motion_thresh, 1e-12))
        elif level == MotionLevel.PRESENT_STILL:
            # For still, breathing band energy is more relevant
            spectral = min(1.0, breathing_energy / max(self._motion_thresh, 1e-12))
        else:
            spectral = 1.0  # No spectral requirement for absence

        # -- cross-receiver agreement (0..1) ---------------------------------
        agreement = 1.0  # default: single receiver
        if other_results:
            same_level = sum(
                1 for r in other_results if r.motion_level == level
            )
            agreement = (same_level + 1) / (len(other_results) + 1)

        # Weighted combination
        confidence = 0.6 * base + 0.2 * spectral + 0.2 * agreement
        return max(0.0, min(1.0, confidence))

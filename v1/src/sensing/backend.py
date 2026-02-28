"""
Common sensing backend interface.

Defines the ``SensingBackend`` protocol and the ``CommodityBackend`` concrete
implementation that wires together the RSSI collector, feature extractor, and
classifier into a single coherent pipeline.

The ``Capability`` enum enumerates all possible sensing capabilities.  The
``CommodityBackend`` honestly reports that it supports only PRESENCE and MOTION.
"""

from __future__ import annotations

import logging
from enum import Enum, auto
from typing import List, Optional, Protocol, Set, runtime_checkable

from v1.src.sensing.classifier import MotionLevel, PresenceClassifier, SensingResult
from v1.src.sensing.feature_extractor import RssiFeatureExtractor, RssiFeatures
from v1.src.sensing.rssi_collector import (
    LinuxWifiCollector,
    SimulatedCollector,
    WifiCollector,
    WifiSample,
)

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Capability enum
# ---------------------------------------------------------------------------

class Capability(Enum):
    """All possible sensing capabilities across backend tiers."""

    PRESENCE = auto()
    MOTION = auto()
    RESPIRATION = auto()
    LOCATION = auto()
    POSE = auto()


# ---------------------------------------------------------------------------
# Backend protocol
# ---------------------------------------------------------------------------

@runtime_checkable
class SensingBackend(Protocol):
    """Protocol that all sensing backends must implement."""

    def get_features(self) -> RssiFeatures:
        """Extract current features from the sensing pipeline."""
        ...

    def get_capabilities(self) -> Set[Capability]:
        """Return the set of capabilities this backend supports."""
        ...


# ---------------------------------------------------------------------------
# Commodity backend
# ---------------------------------------------------------------------------

class CommodityBackend:
    """
    RSSI-based commodity sensing backend.

    Wires together:
        - A WiFi collector (real or simulated)
        - An RSSI feature extractor
        - A presence/motion classifier

    Capabilities: PRESENCE and MOTION only.

    Parameters
    ----------
    collector : WifiCollector-compatible object
        The data source (LinuxWifiCollector or SimulatedCollector).
    extractor : RssiFeatureExtractor, optional
        Feature extractor (created with defaults if not provided).
    classifier : PresenceClassifier, optional
        Classifier (created with defaults if not provided).
    """

    SUPPORTED_CAPABILITIES: Set[Capability] = frozenset(
        {Capability.PRESENCE, Capability.MOTION}
    )

    def __init__(
        self,
        collector: LinuxWifiCollector | SimulatedCollector,
        extractor: Optional[RssiFeatureExtractor] = None,
        classifier: Optional[PresenceClassifier] = None,
    ) -> None:
        self._collector = collector
        self._extractor = extractor or RssiFeatureExtractor()
        self._classifier = classifier or PresenceClassifier()

    @property
    def collector(self) -> LinuxWifiCollector | SimulatedCollector:
        return self._collector

    @property
    def extractor(self) -> RssiFeatureExtractor:
        return self._extractor

    @property
    def classifier(self) -> PresenceClassifier:
        return self._classifier

    # -- SensingBackend protocol ---------------------------------------------

    def get_features(self) -> RssiFeatures:
        """
        Get current features from the latest collected samples.

        Uses the extractor's window_seconds to determine how many samples
        to pull from the collector's ring buffer.
        """
        window = self._extractor.window_seconds
        sample_rate = self._collector.sample_rate_hz
        n_needed = int(window * sample_rate)
        samples = self._collector.get_samples(n=n_needed)
        return self._extractor.extract(samples)

    def get_capabilities(self) -> Set[Capability]:
        """CommodityBackend supports PRESENCE and MOTION only."""
        return set(self.SUPPORTED_CAPABILITIES)

    # -- convenience methods -------------------------------------------------

    def get_result(self) -> SensingResult:
        """
        Run the full pipeline: collect -> extract -> classify.

        Returns
        -------
        SensingResult
            Classification result with motion level and confidence.
        """
        features = self.get_features()
        return self._classifier.classify(features)

    def start(self) -> None:
        """Start the underlying collector."""
        self._collector.start()
        logger.info(
            "CommodityBackend started (capabilities: %s)",
            ", ".join(c.name for c in self.SUPPORTED_CAPABILITIES),
        )

    def stop(self) -> None:
        """Stop the underlying collector."""
        self._collector.stop()
        logger.info("CommodityBackend stopped")

    def is_capable(self, capability: Capability) -> bool:
        """Check whether this backend supports a specific capability."""
        return capability in self.SUPPORTED_CAPABILITIES

    def __repr__(self) -> str:
        caps = ", ".join(c.name for c in sorted(self.SUPPORTED_CAPABILITIES, key=lambda c: c.value))
        return f"CommodityBackend(capabilities=[{caps}])"

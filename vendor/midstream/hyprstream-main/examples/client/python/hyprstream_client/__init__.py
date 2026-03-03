"""
Hyprstream Python client library for interacting with the Hyprstream metrics service.
"""

from .client import MetricsClient
from .types import MetricRecord

__version__ = "0.1.0"
__all__ = ["MetricsClient", "MetricRecord"] 
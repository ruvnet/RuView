"""Type definitions for the Hyprstream client."""

from dataclasses import dataclass
from typing import Dict, Any
import time

@dataclass
class MetricRecord:
    """Represents a metric record matching the server's schema."""
    metric_id: str
    timestamp: int
    value_running_window_sum: float
    value_running_window_avg: float
    value_running_window_count: int

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'MetricRecord':
        """Create a MetricRecord from a dictionary, handling field name mappings."""
        return cls(
            metric_id=str(data.get('metric_id')),  # Ensure string type
            timestamp=data.get('timestamp', int(time.time() * 1e9)),
            value_running_window_sum=float(data.get('value_running_window_sum', 0.0)),
            value_running_window_avg=float(data.get('value_running_window_avg', 0.0)),
            value_running_window_count=int(data.get('value_running_window_count', 0))
        )

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for database operations."""
        return {
            'metric_id': self.metric_id,
            'timestamp': self.timestamp,
            'value_running_window_sum': self.value_running_window_sum,
            'value_running_window_avg': self.value_running_window_avg,
            'value_running_window_count': self.value_running_window_count
        } 
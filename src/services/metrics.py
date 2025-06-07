"""
Metrics collection service for WiFi-DensePose API
"""

import asyncio
import logging
import time
import psutil
from typing import Dict, Any, List, Optional
from datetime import datetime, timedelta
from dataclasses import dataclass, field
from collections import defaultdict, deque

from src.config.settings import Settings

logger = logging.getLogger(__name__)


@dataclass
class MetricPoint:
    """Single metric data point."""
    timestamp: datetime
    value: float
    labels: Dict[str, str] = field(default_factory=dict)


@dataclass
class MetricSeries:
    """Time series of metric points."""
    name: str
    description: str
    unit: str
    points: deque = field(default_factory=lambda: deque(maxlen=1000))
    
    def add_point(self, value: float, labels: Optional[Dict[str, str]] = None):
        """Add a metric point."""
        point = MetricPoint(
            timestamp=datetime.utcnow(),
            value=value,
            labels=labels or {}
        )
        self.points.append(point)
    
    def get_latest(self) -> Optional[MetricPoint]:
        """Get the latest metric point."""
        return self.points[-1] if self.points else None
    
    def get_average(self, duration: timedelta) -> Optional[float]:
        """Get average value over a time duration."""
        cutoff = datetime.utcnow() - duration
        relevant_points = [
            point for point in self.points
            if point.timestamp >= cutoff
        ]
        
        if not relevant_points:
            return None
        
        return sum(point.value for point in relevant_points) / len(relevant_points)
    
    def get_max(self, duration: timedelta) -> Optional[float]:
        """Get maximum value over a time duration."""
        cutoff = datetime.utcnow() - duration
        relevant_points = [
            point for point in self.points
            if point.timestamp >= cutoff
        ]
        
        if not relevant_points:
            return None
        
        return max(point.value for point in relevant_points)


class MetricsService:
    """Service for collecting and managing application metrics."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self._metrics: Dict[str, MetricSeries] = {}
        self._counters: Dict[str, float] = defaultdict(float)
        self._gauges: Dict[str, float] = {}
        self._histograms: Dict[str, List[float]] = defaultdict(list)
        self._start_time = time.time()
        self._initialized = False
        self._running = False
        
        # Initialize standard metrics
        self._initialize_standard_metrics()
    
    def _initialize_standard_metrics(self):
        """Initialize standard system and application metrics."""
        self._metrics.update({
            # System metrics
            "system_cpu_usage": MetricSeries(
                "system_cpu_usage", "System CPU usage percentage", "percent"
            ),
            "system_memory_usage": MetricSeries(
                "system_memory_usage", "System memory usage percentage", "percent"
            ),
            "system_disk_usage": MetricSeries(
                "system_disk_usage", "System disk usage percentage", "percent"
            ),
            "system_network_bytes_sent": MetricSeries(
                "system_network_bytes_sent", "Network bytes sent", "bytes"
            ),
            "system_network_bytes_recv": MetricSeries(
                "system_network_bytes_recv", "Network bytes received", "bytes"
            ),
            
            # Application metrics
            "app_requests_total": MetricSeries(
                "app_requests_total", "Total HTTP requests", "count"
            ),
            "app_request_duration": MetricSeries(
                "app_request_duration", "HTTP request duration", "seconds"
            ),
            "app_active_connections": MetricSeries(
                "app_active_connections", "Active WebSocket connections", "count"
            ),
            "app_pose_detections": MetricSeries(
                "app_pose_detections", "Pose detections performed", "count"
            ),
            "app_pose_processing_time": MetricSeries(
                "app_pose_processing_time", "Pose processing time", "seconds"
            ),
            "app_csi_data_points": MetricSeries(
                "app_csi_data_points", "CSI data points processed", "count"
            ),
            "app_stream_fps": MetricSeries(
                "app_stream_fps", "Streaming frames per second", "fps"
            ),
            
            # Error metrics
            "app_errors_total": MetricSeries(
                "app_errors_total", "Total application errors", "count"
            ),
            "app_http_errors": MetricSeries(
                "app_http_errors", "HTTP errors", "count"
            ),
        })
    
    async def initialize(self):
        """Initialize metrics service."""
        if self._initialized:
            return
        
        logger.info("Initializing metrics service")
        self._initialized = True
        logger.info("Metrics service initialized")
    
    async def start(self):
        """Start metrics service."""
        if not self._initialized:
            await self.initialize()
        
        self._running = True
        logger.info("Metrics service started")
    
    async def shutdown(self):
        """Shutdown metrics service."""
        self._running = False
        logger.info("Metrics service shut down")
    
    async def collect_metrics(self):
        """Collect all metrics."""
        if not self._running:
            return
        
        logger.debug("Collecting metrics")
        
        # Collect system metrics
        await self._collect_system_metrics()
        
        # Collect application metrics
        await self._collect_application_metrics()
        
        logger.debug("Metrics collection completed")
    
    async def _collect_system_metrics(self):
        """Collect system-level metrics."""
        try:
            # CPU usage
            cpu_percent = psutil.cpu_percent(interval=1)
            self._metrics["system_cpu_usage"].add_point(cpu_percent)
            
            # Memory usage
            memory = psutil.virtual_memory()
            self._metrics["system_memory_usage"].add_point(memory.percent)
            
            # Disk usage
            disk = psutil.disk_usage('/')
            disk_percent = (disk.used / disk.total) * 100
            self._metrics["system_disk_usage"].add_point(disk_percent)
            
            # Network I/O
            network = psutil.net_io_counters()
            self._metrics["system_network_bytes_sent"].add_point(network.bytes_sent)
            self._metrics["system_network_bytes_recv"].add_point(network.bytes_recv)
            
        except Exception as e:
            logger.error(f"Error collecting system metrics: {e}")
    
    async def _collect_application_metrics(self):
        """Collect application-specific metrics."""
        try:
            # Import here to avoid circular imports
            from src.api.websocket.connection_manager import connection_manager
            
            # Active connections
            connection_stats = await connection_manager.get_connection_stats()
            active_connections = connection_stats.get("active_connections", 0)
            self._metrics["app_active_connections"].add_point(active_connections)
            
            # Update counters as metrics
            for name, value in self._counters.items():
                if name in self._metrics:
                    self._metrics[name].add_point(value)
            
            # Update gauges as metrics
            for name, value in self._gauges.items():
                if name in self._metrics:
                    self._metrics[name].add_point(value)
            
        except Exception as e:
            logger.error(f"Error collecting application metrics: {e}")
    
    def increment_counter(self, name: str, value: float = 1.0, labels: Optional[Dict[str, str]] = None):
        """Increment a counter metric."""
        self._counters[name] += value
        
        if name in self._metrics:
            self._metrics[name].add_point(self._counters[name], labels)
    
    def set_gauge(self, name: str, value: float, labels: Optional[Dict[str, str]] = None):
        """Set a gauge metric value."""
        self._gauges[name] = value
        
        if name in self._metrics:
            self._metrics[name].add_point(value, labels)
    
    def record_histogram(self, name: str, value: float, labels: Optional[Dict[str, str]] = None):
        """Record a histogram value."""
        self._histograms[name].append(value)
        
        # Keep only last 1000 values
        if len(self._histograms[name]) > 1000:
            self._histograms[name] = self._histograms[name][-1000:]
        
        if name in self._metrics:
            self._metrics[name].add_point(value, labels)
    
    def time_function(self, metric_name: str):
        """Decorator to time function execution."""
        def decorator(func):
            import functools
            
            @functools.wraps(func)
            async def async_wrapper(*args, **kwargs):
                start_time = time.time()
                try:
                    result = await func(*args, **kwargs)
                    return result
                finally:
                    duration = time.time() - start_time
                    self.record_histogram(metric_name, duration)
            
            @functools.wraps(func)
            def sync_wrapper(*args, **kwargs):
                start_time = time.time()
                try:
                    result = func(*args, **kwargs)
                    return result
                finally:
                    duration = time.time() - start_time
                    self.record_histogram(metric_name, duration)
            
            return async_wrapper if asyncio.iscoroutinefunction(func) else sync_wrapper
        
        return decorator
    
    def get_metric(self, name: str) -> Optional[MetricSeries]:
        """Get a metric series by name."""
        return self._metrics.get(name)
    
    def get_metric_value(self, name: str) -> Optional[float]:
        """Get the latest value of a metric."""
        metric = self._metrics.get(name)
        if metric:
            latest = metric.get_latest()
            return latest.value if latest else None
        return None
    
    def get_counter_value(self, name: str) -> float:
        """Get current counter value."""
        return self._counters.get(name, 0.0)
    
    def get_gauge_value(self, name: str) -> Optional[float]:
        """Get current gauge value."""
        return self._gauges.get(name)
    
    def get_histogram_stats(self, name: str) -> Dict[str, float]:
        """Get histogram statistics."""
        values = self._histograms.get(name, [])
        if not values:
            return {}
        
        sorted_values = sorted(values)
        count = len(sorted_values)
        
        return {
            "count": count,
            "sum": sum(sorted_values),
            "min": sorted_values[0],
            "max": sorted_values[-1],
            "mean": sum(sorted_values) / count,
            "p50": sorted_values[int(count * 0.5)],
            "p90": sorted_values[int(count * 0.9)],
            "p95": sorted_values[int(count * 0.95)],
            "p99": sorted_values[int(count * 0.99)],
        }
    
    async def get_all_metrics(self) -> Dict[str, Any]:
        """Get all current metrics."""
        metrics = {}
        
        # Current metric values
        for name, metric_series in self._metrics.items():
            latest = metric_series.get_latest()
            if latest:
                metrics[name] = {
                    "value": latest.value,
                    "timestamp": latest.timestamp.isoformat(),
                    "description": metric_series.description,
                    "unit": metric_series.unit,
                    "labels": latest.labels
                }
        
        # Counter values
        metrics.update({
            f"counter_{name}": value
            for name, value in self._counters.items()
        })
        
        # Gauge values
        metrics.update({
            f"gauge_{name}": value
            for name, value in self._gauges.items()
        })
        
        # Histogram statistics
        for name, values in self._histograms.items():
            if values:
                stats = self.get_histogram_stats(name)
                metrics[f"histogram_{name}"] = stats
        
        return metrics
    
    async def get_system_metrics(self) -> Dict[str, Any]:
        """Get system metrics summary."""
        return {
            "cpu_usage": self.get_metric_value("system_cpu_usage"),
            "memory_usage": self.get_metric_value("system_memory_usage"),
            "disk_usage": self.get_metric_value("system_disk_usage"),
            "network_bytes_sent": self.get_metric_value("system_network_bytes_sent"),
            "network_bytes_recv": self.get_metric_value("system_network_bytes_recv"),
        }
    
    async def get_application_metrics(self) -> Dict[str, Any]:
        """Get application metrics summary."""
        return {
            "requests_total": self.get_counter_value("app_requests_total"),
            "active_connections": self.get_metric_value("app_active_connections"),
            "pose_detections": self.get_counter_value("app_pose_detections"),
            "csi_data_points": self.get_counter_value("app_csi_data_points"),
            "errors_total": self.get_counter_value("app_errors_total"),
            "uptime_seconds": time.time() - self._start_time,
            "request_duration_stats": self.get_histogram_stats("app_request_duration"),
            "pose_processing_time_stats": self.get_histogram_stats("app_pose_processing_time"),
        }
    
    async def get_performance_summary(self) -> Dict[str, Any]:
        """Get performance metrics summary."""
        one_hour = timedelta(hours=1)
        
        return {
            "system": {
                "cpu_avg_1h": self._metrics["system_cpu_usage"].get_average(one_hour),
                "memory_avg_1h": self._metrics["system_memory_usage"].get_average(one_hour),
                "cpu_max_1h": self._metrics["system_cpu_usage"].get_max(one_hour),
                "memory_max_1h": self._metrics["system_memory_usage"].get_max(one_hour),
            },
            "application": {
                "avg_request_duration": self.get_histogram_stats("app_request_duration").get("mean"),
                "avg_pose_processing_time": self.get_histogram_stats("app_pose_processing_time").get("mean"),
                "total_requests": self.get_counter_value("app_requests_total"),
                "total_errors": self.get_counter_value("app_errors_total"),
                "error_rate": (
                    self.get_counter_value("app_errors_total") / 
                    max(self.get_counter_value("app_requests_total"), 1)
                ) * 100,
            }
        }
    
    async def get_status(self) -> Dict[str, Any]:
        """Get metrics service status."""
        return {
            "status": "healthy" if self._running else "stopped",
            "initialized": self._initialized,
            "running": self._running,
            "metrics_count": len(self._metrics),
            "counters_count": len(self._counters),
            "gauges_count": len(self._gauges),
            "histograms_count": len(self._histograms),
            "uptime": time.time() - self._start_time
        }
    
    def reset_metrics(self):
        """Reset all metrics."""
        logger.info("Resetting all metrics")
        
        # Clear metric points but keep series definitions
        for metric_series in self._metrics.values():
            metric_series.points.clear()
        
        # Reset counters, gauges, and histograms
        self._counters.clear()
        self._gauges.clear()
        self._histograms.clear()
        
        logger.info("All metrics reset")
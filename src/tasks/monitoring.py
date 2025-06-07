"""
Monitoring tasks for WiFi-DensePose API
"""

import asyncio
import logging
import psutil
import time
from datetime import datetime, timedelta
from typing import Dict, Any, Optional, List
from contextlib import asynccontextmanager

from sqlalchemy import select, func, and_, or_
from sqlalchemy.ext.asyncio import AsyncSession

from src.config.settings import Settings
from src.database.connection import get_database_manager
from src.database.models import SystemMetric, Device, Session, CSIData, PoseDetection
from src.logger import get_logger

logger = get_logger(__name__)


class MonitoringTask:
    """Base class for monitoring tasks."""
    
    def __init__(self, name: str, settings: Settings):
        self.name = name
        self.settings = settings
        self.enabled = True
        self.last_run = None
        self.run_count = 0
        self.error_count = 0
        self.interval_seconds = 60  # Default interval
    
    async def collect_metrics(self, session: AsyncSession) -> List[Dict[str, Any]]:
        """Collect metrics for this task."""
        raise NotImplementedError
    
    async def run(self, session: AsyncSession) -> Dict[str, Any]:
        """Run the monitoring task with error handling."""
        start_time = datetime.utcnow()
        
        try:
            logger.debug(f"Starting monitoring task: {self.name}")
            
            metrics = await self.collect_metrics(session)
            
            # Store metrics in database
            for metric_data in metrics:
                metric = SystemMetric(
                    metric_name=metric_data["name"],
                    metric_type=metric_data["type"],
                    value=metric_data["value"],
                    unit=metric_data.get("unit"),
                    labels=metric_data.get("labels"),
                    tags=metric_data.get("tags"),
                    source=metric_data.get("source", self.name),
                    component=metric_data.get("component"),
                    description=metric_data.get("description"),
                    meta_data=metric_data.get("metadata"),
                )
                session.add(metric)
            
            await session.commit()
            
            self.last_run = start_time
            self.run_count += 1
            
            logger.debug(f"Monitoring task {self.name} completed: collected {len(metrics)} metrics")
            
            return {
                "task": self.name,
                "status": "success",
                "start_time": start_time.isoformat(),
                "duration_ms": (datetime.utcnow() - start_time).total_seconds() * 1000,
                "metrics_collected": len(metrics),
            }
            
        except Exception as e:
            self.error_count += 1
            logger.error(f"Monitoring task {self.name} failed: {e}", exc_info=True)
            
            return {
                "task": self.name,
                "status": "error",
                "start_time": start_time.isoformat(),
                "duration_ms": (datetime.utcnow() - start_time).total_seconds() * 1000,
                "error": str(e),
                "metrics_collected": 0,
            }
    
    def get_stats(self) -> Dict[str, Any]:
        """Get task statistics."""
        return {
            "name": self.name,
            "enabled": self.enabled,
            "interval_seconds": self.interval_seconds,
            "last_run": self.last_run.isoformat() if self.last_run else None,
            "run_count": self.run_count,
            "error_count": self.error_count,
        }


class SystemResourceMonitoring(MonitoringTask):
    """Monitor system resources (CPU, memory, disk, network)."""
    
    def __init__(self, settings: Settings):
        super().__init__("system_resources", settings)
        self.interval_seconds = settings.system_monitoring_interval
    
    async def collect_metrics(self, session: AsyncSession) -> List[Dict[str, Any]]:
        """Collect system resource metrics."""
        metrics = []
        timestamp = datetime.utcnow()
        
        # CPU metrics
        cpu_percent = psutil.cpu_percent(interval=1)
        cpu_count = psutil.cpu_count()
        cpu_freq = psutil.cpu_freq()
        
        metrics.extend([
            {
                "name": "system_cpu_usage_percent",
                "type": "gauge",
                "value": cpu_percent,
                "unit": "percent",
                "component": "cpu",
                "description": "CPU usage percentage",
                "metadata": {"timestamp": timestamp.isoformat()}
            },
            {
                "name": "system_cpu_count",
                "type": "gauge",
                "value": cpu_count,
                "unit": "count",
                "component": "cpu",
                "description": "Number of CPU cores",
                "metadata": {"timestamp": timestamp.isoformat()}
            }
        ])
        
        if cpu_freq:
            metrics.append({
                "name": "system_cpu_frequency_mhz",
                "type": "gauge",
                "value": cpu_freq.current,
                "unit": "mhz",
                "component": "cpu",
                "description": "Current CPU frequency",
                "metadata": {"timestamp": timestamp.isoformat()}
            })
        
        # Memory metrics
        memory = psutil.virtual_memory()
        swap = psutil.swap_memory()
        
        metrics.extend([
            {
                "name": "system_memory_total_bytes",
                "type": "gauge",
                "value": memory.total,
                "unit": "bytes",
                "component": "memory",
                "description": "Total system memory",
                "metadata": {"timestamp": timestamp.isoformat()}
            },
            {
                "name": "system_memory_used_bytes",
                "type": "gauge",
                "value": memory.used,
                "unit": "bytes",
                "component": "memory",
                "description": "Used system memory",
                "metadata": {"timestamp": timestamp.isoformat()}
            },
            {
                "name": "system_memory_available_bytes",
                "type": "gauge",
                "value": memory.available,
                "unit": "bytes",
                "component": "memory",
                "description": "Available system memory",
                "metadata": {"timestamp": timestamp.isoformat()}
            },
            {
                "name": "system_memory_usage_percent",
                "type": "gauge",
                "value": memory.percent,
                "unit": "percent",
                "component": "memory",
                "description": "Memory usage percentage",
                "metadata": {"timestamp": timestamp.isoformat()}
            },
            {
                "name": "system_swap_total_bytes",
                "type": "gauge",
                "value": swap.total,
                "unit": "bytes",
                "component": "memory",
                "description": "Total swap memory",
                "metadata": {"timestamp": timestamp.isoformat()}
            },
            {
                "name": "system_swap_used_bytes",
                "type": "gauge",
                "value": swap.used,
                "unit": "bytes",
                "component": "memory",
                "description": "Used swap memory",
                "metadata": {"timestamp": timestamp.isoformat()}
            }
        ])
        
        # Disk metrics
        disk_usage = psutil.disk_usage('/')
        disk_io = psutil.disk_io_counters()
        
        metrics.extend([
            {
                "name": "system_disk_total_bytes",
                "type": "gauge",
                "value": disk_usage.total,
                "unit": "bytes",
                "component": "disk",
                "description": "Total disk space",
                "metadata": {"timestamp": timestamp.isoformat()}
            },
            {
                "name": "system_disk_used_bytes",
                "type": "gauge",
                "value": disk_usage.used,
                "unit": "bytes",
                "component": "disk",
                "description": "Used disk space",
                "metadata": {"timestamp": timestamp.isoformat()}
            },
            {
                "name": "system_disk_free_bytes",
                "type": "gauge",
                "value": disk_usage.free,
                "unit": "bytes",
                "component": "disk",
                "description": "Free disk space",
                "metadata": {"timestamp": timestamp.isoformat()}
            },
            {
                "name": "system_disk_usage_percent",
                "type": "gauge",
                "value": (disk_usage.used / disk_usage.total) * 100,
                "unit": "percent",
                "component": "disk",
                "description": "Disk usage percentage",
                "metadata": {"timestamp": timestamp.isoformat()}
            }
        ])
        
        if disk_io:
            metrics.extend([
                {
                    "name": "system_disk_read_bytes_total",
                    "type": "counter",
                    "value": disk_io.read_bytes,
                    "unit": "bytes",
                    "component": "disk",
                    "description": "Total bytes read from disk",
                    "metadata": {"timestamp": timestamp.isoformat()}
                },
                {
                    "name": "system_disk_write_bytes_total",
                    "type": "counter",
                    "value": disk_io.write_bytes,
                    "unit": "bytes",
                    "component": "disk",
                    "description": "Total bytes written to disk",
                    "metadata": {"timestamp": timestamp.isoformat()}
                }
            ])
        
        # Network metrics
        network_io = psutil.net_io_counters()
        
        if network_io:
            metrics.extend([
                {
                    "name": "system_network_bytes_sent_total",
                    "type": "counter",
                    "value": network_io.bytes_sent,
                    "unit": "bytes",
                    "component": "network",
                    "description": "Total bytes sent over network",
                    "metadata": {"timestamp": timestamp.isoformat()}
                },
                {
                    "name": "system_network_bytes_recv_total",
                    "type": "counter",
                    "value": network_io.bytes_recv,
                    "unit": "bytes",
                    "component": "network",
                    "description": "Total bytes received over network",
                    "metadata": {"timestamp": timestamp.isoformat()}
                },
                {
                    "name": "system_network_packets_sent_total",
                    "type": "counter",
                    "value": network_io.packets_sent,
                    "unit": "count",
                    "component": "network",
                    "description": "Total packets sent over network",
                    "metadata": {"timestamp": timestamp.isoformat()}
                },
                {
                    "name": "system_network_packets_recv_total",
                    "type": "counter",
                    "value": network_io.packets_recv,
                    "unit": "count",
                    "component": "network",
                    "description": "Total packets received over network",
                    "metadata": {"timestamp": timestamp.isoformat()}
                }
            ])
        
        return metrics


class DatabaseMonitoring(MonitoringTask):
    """Monitor database performance and statistics."""
    
    def __init__(self, settings: Settings):
        super().__init__("database", settings)
        self.interval_seconds = settings.database_monitoring_interval
    
    async def collect_metrics(self, session: AsyncSession) -> List[Dict[str, Any]]:
        """Collect database metrics."""
        metrics = []
        timestamp = datetime.utcnow()
        
        # Get database connection stats
        db_manager = get_database_manager(self.settings)
        connection_stats = await db_manager.get_connection_stats()
        
        # PostgreSQL connection metrics
        if "postgresql" in connection_stats:
            pg_stats = connection_stats["postgresql"]
            metrics.extend([
                {
                    "name": "database_connections_total",
                    "type": "gauge",
                    "value": pg_stats.get("total_connections", 0),
                    "unit": "count",
                    "component": "postgresql",
                    "description": "Total database connections",
                    "metadata": {"timestamp": timestamp.isoformat()}
                },
                {
                    "name": "database_connections_active",
                    "type": "gauge",
                    "value": pg_stats.get("checked_out", 0),
                    "unit": "count",
                    "component": "postgresql",
                    "description": "Active database connections",
                    "metadata": {"timestamp": timestamp.isoformat()}
                },
                {
                    "name": "database_connections_available",
                    "type": "gauge",
                    "value": pg_stats.get("available_connections", 0),
                    "unit": "count",
                    "component": "postgresql",
                    "description": "Available database connections",
                    "metadata": {"timestamp": timestamp.isoformat()}
                }
            ])
        
        # Redis connection metrics
        if "redis" in connection_stats and not connection_stats["redis"].get("error"):
            redis_stats = connection_stats["redis"]
            metrics.extend([
                {
                    "name": "redis_connections_active",
                    "type": "gauge",
                    "value": redis_stats.get("connected_clients", 0),
                    "unit": "count",
                    "component": "redis",
                    "description": "Active Redis connections",
                    "metadata": {"timestamp": timestamp.isoformat()}
                },
                {
                    "name": "redis_connections_blocked",
                    "type": "gauge",
                    "value": redis_stats.get("blocked_clients", 0),
                    "unit": "count",
                    "component": "redis",
                    "description": "Blocked Redis connections",
                    "metadata": {"timestamp": timestamp.isoformat()}
                }
            ])
        
        # Table row counts
        table_counts = await self._get_table_counts(session)
        for table_name, count in table_counts.items():
            metrics.append({
                "name": f"database_table_rows_{table_name}",
                "type": "gauge",
                "value": count,
                "unit": "count",
                "component": "postgresql",
                "description": f"Number of rows in {table_name} table",
                "metadata": {"timestamp": timestamp.isoformat(), "table": table_name}
            })
        
        return metrics
    
    async def _get_table_counts(self, session: AsyncSession) -> Dict[str, int]:
        """Get row counts for all tables."""
        counts = {}
        
        # Count devices
        result = await session.execute(select(func.count(Device.id)))
        counts["devices"] = result.scalar() or 0
        
        # Count sessions
        result = await session.execute(select(func.count(Session.id)))
        counts["sessions"] = result.scalar() or 0
        
        # Count CSI data
        result = await session.execute(select(func.count(CSIData.id)))
        counts["csi_data"] = result.scalar() or 0
        
        # Count pose detections
        result = await session.execute(select(func.count(PoseDetection.id)))
        counts["pose_detections"] = result.scalar() or 0
        
        # Count system metrics
        result = await session.execute(select(func.count(SystemMetric.id)))
        counts["system_metrics"] = result.scalar() or 0
        
        return counts


class ApplicationMonitoring(MonitoringTask):
    """Monitor application-specific metrics."""
    
    def __init__(self, settings: Settings):
        super().__init__("application", settings)
        self.interval_seconds = settings.application_monitoring_interval
        self.start_time = datetime.utcnow()
    
    async def collect_metrics(self, session: AsyncSession) -> List[Dict[str, Any]]:
        """Collect application metrics."""
        metrics = []
        timestamp = datetime.utcnow()
        
        # Application uptime
        uptime_seconds = (timestamp - self.start_time).total_seconds()
        metrics.append({
            "name": "application_uptime_seconds",
            "type": "gauge",
            "value": uptime_seconds,
            "unit": "seconds",
            "component": "application",
            "description": "Application uptime in seconds",
            "metadata": {"timestamp": timestamp.isoformat()}
        })
        
        # Active sessions count
        active_sessions_query = select(func.count(Session.id)).where(
            Session.status == "active"
        )
        result = await session.execute(active_sessions_query)
        active_sessions = result.scalar() or 0
        
        metrics.append({
            "name": "application_active_sessions",
            "type": "gauge",
            "value": active_sessions,
            "unit": "count",
            "component": "application",
            "description": "Number of active sessions",
            "metadata": {"timestamp": timestamp.isoformat()}
        })
        
        # Active devices count
        active_devices_query = select(func.count(Device.id)).where(
            Device.status == "active"
        )
        result = await session.execute(active_devices_query)
        active_devices = result.scalar() or 0
        
        metrics.append({
            "name": "application_active_devices",
            "type": "gauge",
            "value": active_devices,
            "unit": "count",
            "component": "application",
            "description": "Number of active devices",
            "metadata": {"timestamp": timestamp.isoformat()}
        })
        
        # Recent data processing metrics (last hour)
        one_hour_ago = timestamp - timedelta(hours=1)
        
        # Recent CSI data count
        recent_csi_query = select(func.count(CSIData.id)).where(
            CSIData.created_at >= one_hour_ago
        )
        result = await session.execute(recent_csi_query)
        recent_csi_count = result.scalar() or 0
        
        metrics.append({
            "name": "application_csi_data_hourly",
            "type": "gauge",
            "value": recent_csi_count,
            "unit": "count",
            "component": "application",
            "description": "CSI data records created in the last hour",
            "metadata": {"timestamp": timestamp.isoformat()}
        })
        
        # Recent pose detections count
        recent_pose_query = select(func.count(PoseDetection.id)).where(
            PoseDetection.created_at >= one_hour_ago
        )
        result = await session.execute(recent_pose_query)
        recent_pose_count = result.scalar() or 0
        
        metrics.append({
            "name": "application_pose_detections_hourly",
            "type": "gauge",
            "value": recent_pose_count,
            "unit": "count",
            "component": "application",
            "description": "Pose detections created in the last hour",
            "metadata": {"timestamp": timestamp.isoformat()}
        })
        
        # Processing status metrics
        processing_statuses = ["pending", "processing", "completed", "failed"]
        for status in processing_statuses:
            status_query = select(func.count(CSIData.id)).where(
                CSIData.processing_status == status
            )
            result = await session.execute(status_query)
            status_count = result.scalar() or 0
            
            metrics.append({
                "name": f"application_csi_processing_{status}",
                "type": "gauge",
                "value": status_count,
                "unit": "count",
                "component": "application",
                "description": f"CSI data records with {status} processing status",
                "metadata": {"timestamp": timestamp.isoformat(), "status": status}
            })
        
        return metrics


class PerformanceMonitoring(MonitoringTask):
    """Monitor performance metrics and response times."""
    
    def __init__(self, settings: Settings):
        super().__init__("performance", settings)
        self.interval_seconds = settings.performance_monitoring_interval
        self.response_times = []
        self.error_counts = {}
    
    async def collect_metrics(self, session: AsyncSession) -> List[Dict[str, Any]]:
        """Collect performance metrics."""
        metrics = []
        timestamp = datetime.utcnow()
        
        # Database query performance test
        start_time = time.time()
        test_query = select(func.count(Device.id))
        await session.execute(test_query)
        db_response_time = (time.time() - start_time) * 1000  # Convert to milliseconds
        
        metrics.append({
            "name": "performance_database_query_time_ms",
            "type": "gauge",
            "value": db_response_time,
            "unit": "milliseconds",
            "component": "database",
            "description": "Database query response time",
            "metadata": {"timestamp": timestamp.isoformat()}
        })
        
        # Average response time (if we have data)
        if self.response_times:
            avg_response_time = sum(self.response_times) / len(self.response_times)
            metrics.append({
                "name": "performance_avg_response_time_ms",
                "type": "gauge",
                "value": avg_response_time,
                "unit": "milliseconds",
                "component": "api",
                "description": "Average API response time",
                "metadata": {"timestamp": timestamp.isoformat()}
            })
            
            # Clear old response times (keep only recent ones)
            self.response_times = self.response_times[-100:]  # Keep last 100
        
        # Error rates
        for error_type, count in self.error_counts.items():
            metrics.append({
                "name": f"performance_errors_{error_type}_total",
                "type": "counter",
                "value": count,
                "unit": "count",
                "component": "api",
                "description": f"Total {error_type} errors",
                "metadata": {"timestamp": timestamp.isoformat(), "error_type": error_type}
            })
        
        return metrics
    
    def record_response_time(self, response_time_ms: float):
        """Record an API response time."""
        self.response_times.append(response_time_ms)
    
    def record_error(self, error_type: str):
        """Record an error occurrence."""
        self.error_counts[error_type] = self.error_counts.get(error_type, 0) + 1


class MonitoringManager:
    """Manager for all monitoring tasks."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self.db_manager = get_database_manager(settings)
        self.tasks = self._initialize_tasks()
        self.running = False
        self.last_run = None
        self.run_count = 0
    
    def _initialize_tasks(self) -> List[MonitoringTask]:
        """Initialize all monitoring tasks."""
        tasks = [
            SystemResourceMonitoring(self.settings),
            DatabaseMonitoring(self.settings),
            ApplicationMonitoring(self.settings),
            PerformanceMonitoring(self.settings),
        ]
        
        # Filter enabled tasks
        enabled_tasks = [task for task in tasks if task.enabled]
        
        logger.info(f"Initialized {len(enabled_tasks)} monitoring tasks")
        return enabled_tasks
    
    async def run_all_tasks(self) -> Dict[str, Any]:
        """Run all monitoring tasks."""
        if self.running:
            return {"status": "already_running", "message": "Monitoring already in progress"}
        
        self.running = True
        start_time = datetime.utcnow()
        
        try:
            logger.debug("Starting monitoring tasks")
            
            results = []
            total_metrics = 0
            
            async with self.db_manager.get_async_session() as session:
                for task in self.tasks:
                    if not task.enabled:
                        continue
                    
                    result = await task.run(session)
                    results.append(result)
                    total_metrics += result.get("metrics_collected", 0)
            
            self.last_run = start_time
            self.run_count += 1
            
            duration = (datetime.utcnow() - start_time).total_seconds()
            
            logger.debug(
                f"Monitoring tasks completed: collected {total_metrics} metrics "
                f"in {duration:.2f} seconds"
            )
            
            return {
                "status": "completed",
                "start_time": start_time.isoformat(),
                "duration_seconds": duration,
                "total_metrics": total_metrics,
                "task_results": results,
            }
            
        except Exception as e:
            logger.error(f"Monitoring tasks failed: {e}", exc_info=True)
            return {
                "status": "error",
                "start_time": start_time.isoformat(),
                "duration_seconds": (datetime.utcnow() - start_time).total_seconds(),
                "error": str(e),
                "total_metrics": 0,
            }
        
        finally:
            self.running = False
    
    async def run_task(self, task_name: str) -> Dict[str, Any]:
        """Run a specific monitoring task."""
        task = next((t for t in self.tasks if t.name == task_name), None)
        
        if not task:
            return {
                "status": "error",
                "error": f"Task '{task_name}' not found",
                "available_tasks": [t.name for t in self.tasks]
            }
        
        if not task.enabled:
            return {
                "status": "error",
                "error": f"Task '{task_name}' is disabled"
            }
        
        async with self.db_manager.get_async_session() as session:
            return await task.run(session)
    
    def get_stats(self) -> Dict[str, Any]:
        """Get monitoring manager statistics."""
        return {
            "manager": {
                "running": self.running,
                "last_run": self.last_run.isoformat() if self.last_run else None,
                "run_count": self.run_count,
            },
            "tasks": [task.get_stats() for task in self.tasks],
        }
    
    def get_performance_task(self) -> Optional[PerformanceMonitoring]:
        """Get the performance monitoring task for recording metrics."""
        return next((t for t in self.tasks if isinstance(t, PerformanceMonitoring)), None)


# Global monitoring manager instance
_monitoring_manager: Optional[MonitoringManager] = None


def get_monitoring_manager(settings: Settings) -> MonitoringManager:
    """Get monitoring manager instance."""
    global _monitoring_manager
    if _monitoring_manager is None:
        _monitoring_manager = MonitoringManager(settings)
    return _monitoring_manager


async def run_periodic_monitoring(settings: Settings):
    """Run periodic monitoring tasks."""
    monitoring_manager = get_monitoring_manager(settings)
    
    while True:
        try:
            await monitoring_manager.run_all_tasks()
            
            # Wait for next monitoring interval
            await asyncio.sleep(settings.monitoring_interval_seconds)
            
        except asyncio.CancelledError:
            logger.info("Periodic monitoring cancelled")
            break
        except Exception as e:
            logger.error(f"Periodic monitoring error: {e}", exc_info=True)
            # Wait before retrying
            await asyncio.sleep(30)
"""
Health check API endpoints
"""

import logging
import psutil
from typing import Dict, Any, Optional
from datetime import datetime, timedelta

from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel, Field

from src.api.dependencies import (
    get_hardware_service,
    get_pose_service,
    get_stream_service,
    get_current_user
)
from src.services.hardware_service import HardwareService
from src.services.pose_service import PoseService
from src.services.stream_service import StreamService
from src.config.settings import get_settings

logger = logging.getLogger(__name__)
router = APIRouter()


# Response models
class ComponentHealth(BaseModel):
    """Health status for a system component."""
    
    name: str = Field(..., description="Component name")
    status: str = Field(..., description="Health status (healthy, degraded, unhealthy)")
    message: Optional[str] = Field(default=None, description="Status message")
    last_check: datetime = Field(..., description="Last health check timestamp")
    uptime_seconds: Optional[float] = Field(default=None, description="Component uptime")
    metrics: Optional[Dict[str, Any]] = Field(default=None, description="Component metrics")


class SystemHealth(BaseModel):
    """Overall system health status."""
    
    status: str = Field(..., description="Overall system status")
    timestamp: datetime = Field(..., description="Health check timestamp")
    uptime_seconds: float = Field(..., description="System uptime")
    components: Dict[str, ComponentHealth] = Field(..., description="Component health status")
    system_metrics: Dict[str, Any] = Field(..., description="System-level metrics")


class ReadinessCheck(BaseModel):
    """System readiness check result."""
    
    ready: bool = Field(..., description="Whether system is ready to serve requests")
    timestamp: datetime = Field(..., description="Readiness check timestamp")
    checks: Dict[str, bool] = Field(..., description="Individual readiness checks")
    message: str = Field(..., description="Readiness status message")


# Health check endpoints
@router.get("/health", response_model=SystemHealth)
async def health_check(
    hardware_service: HardwareService = Depends(get_hardware_service),
    pose_service: PoseService = Depends(get_pose_service),
    stream_service: StreamService = Depends(get_stream_service)
):
    """Comprehensive system health check."""
    try:
        timestamp = datetime.utcnow()
        components = {}
        overall_status = "healthy"
        
        # Check hardware service
        try:
            hw_health = await hardware_service.health_check()
            components["hardware"] = ComponentHealth(
                name="Hardware Service",
                status=hw_health["status"],
                message=hw_health.get("message"),
                last_check=timestamp,
                uptime_seconds=hw_health.get("uptime_seconds"),
                metrics=hw_health.get("metrics")
            )
            
            if hw_health["status"] != "healthy":
                overall_status = "degraded" if overall_status == "healthy" else "unhealthy"
                
        except Exception as e:
            logger.error(f"Hardware service health check failed: {e}")
            components["hardware"] = ComponentHealth(
                name="Hardware Service",
                status="unhealthy",
                message=f"Health check failed: {str(e)}",
                last_check=timestamp
            )
            overall_status = "unhealthy"
        
        # Check pose service
        try:
            pose_health = await pose_service.health_check()
            components["pose"] = ComponentHealth(
                name="Pose Service",
                status=pose_health["status"],
                message=pose_health.get("message"),
                last_check=timestamp,
                uptime_seconds=pose_health.get("uptime_seconds"),
                metrics=pose_health.get("metrics")
            )
            
            if pose_health["status"] != "healthy":
                overall_status = "degraded" if overall_status == "healthy" else "unhealthy"
                
        except Exception as e:
            logger.error(f"Pose service health check failed: {e}")
            components["pose"] = ComponentHealth(
                name="Pose Service",
                status="unhealthy",
                message=f"Health check failed: {str(e)}",
                last_check=timestamp
            )
            overall_status = "unhealthy"
        
        # Check stream service
        try:
            stream_health = await stream_service.health_check()
            components["stream"] = ComponentHealth(
                name="Stream Service",
                status=stream_health["status"],
                message=stream_health.get("message"),
                last_check=timestamp,
                uptime_seconds=stream_health.get("uptime_seconds"),
                metrics=stream_health.get("metrics")
            )
            
            if stream_health["status"] != "healthy":
                overall_status = "degraded" if overall_status == "healthy" else "unhealthy"
                
        except Exception as e:
            logger.error(f"Stream service health check failed: {e}")
            components["stream"] = ComponentHealth(
                name="Stream Service",
                status="unhealthy",
                message=f"Health check failed: {str(e)}",
                last_check=timestamp
            )
            overall_status = "unhealthy"
        
        # Get system metrics
        system_metrics = get_system_metrics()
        
        # Calculate system uptime (placeholder - would need actual startup time)
        uptime_seconds = 0.0  # TODO: Implement actual uptime tracking
        
        return SystemHealth(
            status=overall_status,
            timestamp=timestamp,
            uptime_seconds=uptime_seconds,
            components=components,
            system_metrics=system_metrics
        )
        
    except Exception as e:
        logger.error(f"Health check failed: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Health check failed: {str(e)}"
        )


@router.get("/ready", response_model=ReadinessCheck)
async def readiness_check(
    hardware_service: HardwareService = Depends(get_hardware_service),
    pose_service: PoseService = Depends(get_pose_service),
    stream_service: StreamService = Depends(get_stream_service)
):
    """Check if system is ready to serve requests."""
    try:
        timestamp = datetime.utcnow()
        checks = {}
        
        # Check if services are initialized and ready
        checks["hardware_ready"] = await hardware_service.is_ready()
        checks["pose_ready"] = await pose_service.is_ready()
        checks["stream_ready"] = await stream_service.is_ready()
        
        # Check system resources
        checks["memory_available"] = check_memory_availability()
        checks["disk_space_available"] = check_disk_space()
        
        # Overall readiness
        ready = all(checks.values())
        
        message = "System is ready" if ready else "System is not ready"
        if not ready:
            failed_checks = [name for name, status in checks.items() if not status]
            message += f". Failed checks: {', '.join(failed_checks)}"
        
        return ReadinessCheck(
            ready=ready,
            timestamp=timestamp,
            checks=checks,
            message=message
        )
        
    except Exception as e:
        logger.error(f"Readiness check failed: {e}")
        return ReadinessCheck(
            ready=False,
            timestamp=datetime.utcnow(),
            checks={},
            message=f"Readiness check failed: {str(e)}"
        )


@router.get("/live")
async def liveness_check():
    """Simple liveness check for load balancers."""
    return {
        "status": "alive",
        "timestamp": datetime.utcnow().isoformat()
    }


@router.get("/metrics")
async def get_system_metrics(
    current_user: Optional[Dict] = Depends(get_current_user)
):
    """Get detailed system metrics."""
    try:
        metrics = get_system_metrics()
        
        # Add additional metrics if authenticated
        if current_user:
            metrics.update(get_detailed_metrics())
        
        return {
            "timestamp": datetime.utcnow().isoformat(),
            "metrics": metrics
        }
        
    except Exception as e:
        logger.error(f"Error getting system metrics: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Failed to get system metrics: {str(e)}"
        )


@router.get("/version")
async def get_version_info():
    """Get application version information."""
    settings = get_settings()
    
    return {
        "name": settings.app_name,
        "version": settings.version,
        "environment": settings.environment,
        "debug": settings.debug,
        "timestamp": datetime.utcnow().isoformat()
    }


def get_system_metrics() -> Dict[str, Any]:
    """Get basic system metrics."""
    try:
        # CPU metrics
        cpu_percent = psutil.cpu_percent(interval=1)
        cpu_count = psutil.cpu_count()
        
        # Memory metrics
        memory = psutil.virtual_memory()
        memory_metrics = {
            "total_gb": round(memory.total / (1024**3), 2),
            "available_gb": round(memory.available / (1024**3), 2),
            "used_gb": round(memory.used / (1024**3), 2),
            "percent": memory.percent
        }
        
        # Disk metrics
        disk = psutil.disk_usage('/')
        disk_metrics = {
            "total_gb": round(disk.total / (1024**3), 2),
            "free_gb": round(disk.free / (1024**3), 2),
            "used_gb": round(disk.used / (1024**3), 2),
            "percent": round((disk.used / disk.total) * 100, 2)
        }
        
        # Network metrics (basic)
        network = psutil.net_io_counters()
        network_metrics = {
            "bytes_sent": network.bytes_sent,
            "bytes_recv": network.bytes_recv,
            "packets_sent": network.packets_sent,
            "packets_recv": network.packets_recv
        }
        
        return {
            "cpu": {
                "percent": cpu_percent,
                "count": cpu_count
            },
            "memory": memory_metrics,
            "disk": disk_metrics,
            "network": network_metrics
        }
        
    except Exception as e:
        logger.error(f"Error getting system metrics: {e}")
        return {}


def get_detailed_metrics() -> Dict[str, Any]:
    """Get detailed system metrics (requires authentication)."""
    try:
        # Process metrics
        process = psutil.Process()
        process_metrics = {
            "pid": process.pid,
            "cpu_percent": process.cpu_percent(),
            "memory_mb": round(process.memory_info().rss / (1024**2), 2),
            "num_threads": process.num_threads(),
            "create_time": datetime.fromtimestamp(process.create_time()).isoformat()
        }
        
        # Load average (Unix-like systems)
        load_avg = None
        try:
            load_avg = psutil.getloadavg()
        except AttributeError:
            # Windows doesn't have load average
            pass
        
        # Temperature sensors (if available)
        temperatures = {}
        try:
            temps = psutil.sensors_temperatures()
            for name, entries in temps.items():
                temperatures[name] = [
                    {"label": entry.label, "current": entry.current}
                    for entry in entries
                ]
        except AttributeError:
            # Not available on all systems
            pass
        
        detailed = {
            "process": process_metrics
        }
        
        if load_avg:
            detailed["load_average"] = {
                "1min": load_avg[0],
                "5min": load_avg[1],
                "15min": load_avg[2]
            }
        
        if temperatures:
            detailed["temperatures"] = temperatures
        
        return detailed
        
    except Exception as e:
        logger.error(f"Error getting detailed metrics: {e}")
        return {}


def check_memory_availability() -> bool:
    """Check if sufficient memory is available."""
    try:
        memory = psutil.virtual_memory()
        # Consider system ready if less than 90% memory is used
        return memory.percent < 90.0
    except Exception:
        return False


def check_disk_space() -> bool:
    """Check if sufficient disk space is available."""
    try:
        disk = psutil.disk_usage('/')
        # Consider system ready if more than 1GB free space
        free_gb = disk.free / (1024**3)
        return free_gb > 1.0
    except Exception:
        return False
"""
Health check service for WiFi-DensePose API
"""

import asyncio
import logging
import time
from typing import Dict, Any, List, Optional
from datetime import datetime, timedelta
from dataclasses import dataclass, field
from enum import Enum

from src.config.settings import Settings

logger = logging.getLogger(__name__)


class HealthStatus(Enum):
    """Health status enumeration."""
    HEALTHY = "healthy"
    DEGRADED = "degraded"
    UNHEALTHY = "unhealthy"
    UNKNOWN = "unknown"


@dataclass
class HealthCheck:
    """Health check result."""
    name: str
    status: HealthStatus
    message: str
    timestamp: datetime = field(default_factory=datetime.utcnow)
    duration_ms: float = 0.0
    details: Dict[str, Any] = field(default_factory=dict)


@dataclass
class ServiceHealth:
    """Service health information."""
    name: str
    status: HealthStatus
    last_check: Optional[datetime] = None
    checks: List[HealthCheck] = field(default_factory=list)
    uptime: float = 0.0
    error_count: int = 0
    last_error: Optional[str] = None


class HealthCheckService:
    """Service for monitoring application health."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self._services: Dict[str, ServiceHealth] = {}
        self._start_time = time.time()
        self._initialized = False
        self._running = False
    
    async def initialize(self):
        """Initialize health check service."""
        if self._initialized:
            return
        
        logger.info("Initializing health check service")
        
        # Initialize service health tracking
        self._services = {
            "api": ServiceHealth("api", HealthStatus.UNKNOWN),
            "database": ServiceHealth("database", HealthStatus.UNKNOWN),
            "redis": ServiceHealth("redis", HealthStatus.UNKNOWN),
            "hardware": ServiceHealth("hardware", HealthStatus.UNKNOWN),
            "pose": ServiceHealth("pose", HealthStatus.UNKNOWN),
            "stream": ServiceHealth("stream", HealthStatus.UNKNOWN),
        }
        
        self._initialized = True
        logger.info("Health check service initialized")
    
    async def start(self):
        """Start health check service."""
        if not self._initialized:
            await self.initialize()
        
        self._running = True
        logger.info("Health check service started")
    
    async def shutdown(self):
        """Shutdown health check service."""
        self._running = False
        logger.info("Health check service shut down")
    
    async def perform_health_checks(self) -> Dict[str, HealthCheck]:
        """Perform all health checks."""
        if not self._running:
            return {}
        
        logger.debug("Performing health checks")
        results = {}
        
        # Perform individual health checks
        checks = [
            self._check_api_health(),
            self._check_database_health(),
            self._check_redis_health(),
            self._check_hardware_health(),
            self._check_pose_health(),
            self._check_stream_health(),
        ]
        
        # Run checks concurrently
        check_results = await asyncio.gather(*checks, return_exceptions=True)
        
        # Process results
        for i, result in enumerate(check_results):
            check_name = ["api", "database", "redis", "hardware", "pose", "stream"][i]
            
            if isinstance(result, Exception):
                health_check = HealthCheck(
                    name=check_name,
                    status=HealthStatus.UNHEALTHY,
                    message=f"Health check failed: {result}"
                )
            else:
                health_check = result
            
            results[check_name] = health_check
            self._update_service_health(check_name, health_check)
        
        logger.debug(f"Completed {len(results)} health checks")
        return results
    
    async def _check_api_health(self) -> HealthCheck:
        """Check API health."""
        start_time = time.time()
        
        try:
            # Basic API health check
            uptime = time.time() - self._start_time
            
            status = HealthStatus.HEALTHY
            message = "API is running normally"
            details = {
                "uptime_seconds": uptime,
                "uptime_formatted": str(timedelta(seconds=int(uptime)))
            }
            
        except Exception as e:
            status = HealthStatus.UNHEALTHY
            message = f"API health check failed: {e}"
            details = {"error": str(e)}
        
        duration_ms = (time.time() - start_time) * 1000
        
        return HealthCheck(
            name="api",
            status=status,
            message=message,
            duration_ms=duration_ms,
            details=details
        )
    
    async def _check_database_health(self) -> HealthCheck:
        """Check database health."""
        start_time = time.time()
        
        try:
            # Import here to avoid circular imports
            from src.database.connection import get_database_manager
            
            db_manager = get_database_manager()
            
            if not db_manager.is_connected():
                status = HealthStatus.UNHEALTHY
                message = "Database is not connected"
                details = {"connected": False}
            else:
                # Test database connection
                await db_manager.test_connection()
                
                status = HealthStatus.HEALTHY
                message = "Database is connected and responsive"
                details = {
                    "connected": True,
                    "pool_size": db_manager.get_pool_size(),
                    "active_connections": db_manager.get_active_connections()
                }
        
        except Exception as e:
            status = HealthStatus.UNHEALTHY
            message = f"Database health check failed: {e}"
            details = {"error": str(e)}
        
        duration_ms = (time.time() - start_time) * 1000
        
        return HealthCheck(
            name="database",
            status=status,
            message=message,
            duration_ms=duration_ms,
            details=details
        )
    
    async def _check_redis_health(self) -> HealthCheck:
        """Check Redis health."""
        start_time = time.time()
        
        try:
            redis_config = self.settings.get_redis_url()
            
            if not redis_config:
                status = HealthStatus.UNKNOWN
                message = "Redis is not configured"
                details = {"configured": False}
            else:
                # Test Redis connection
                import redis.asyncio as redis
                
                redis_client = redis.from_url(redis_config)
                await redis_client.ping()
                await redis_client.close()
                
                status = HealthStatus.HEALTHY
                message = "Redis is connected and responsive"
                details = {"connected": True}
        
        except Exception as e:
            status = HealthStatus.UNHEALTHY
            message = f"Redis health check failed: {e}"
            details = {"error": str(e)}
        
        duration_ms = (time.time() - start_time) * 1000
        
        return HealthCheck(
            name="redis",
            status=status,
            message=message,
            duration_ms=duration_ms,
            details=details
        )
    
    async def _check_hardware_health(self) -> HealthCheck:
        """Check hardware service health."""
        start_time = time.time()
        
        try:
            # Import here to avoid circular imports
            from src.api.dependencies import get_hardware_service
            
            hardware_service = get_hardware_service()
            
            if hasattr(hardware_service, 'get_status'):
                status_info = await hardware_service.get_status()
                
                if status_info.get("status") == "healthy":
                    status = HealthStatus.HEALTHY
                    message = "Hardware service is operational"
                else:
                    status = HealthStatus.DEGRADED
                    message = f"Hardware service status: {status_info.get('status', 'unknown')}"
                
                details = status_info
            else:
                status = HealthStatus.UNKNOWN
                message = "Hardware service status unavailable"
                details = {}
        
        except Exception as e:
            status = HealthStatus.UNHEALTHY
            message = f"Hardware health check failed: {e}"
            details = {"error": str(e)}
        
        duration_ms = (time.time() - start_time) * 1000
        
        return HealthCheck(
            name="hardware",
            status=status,
            message=message,
            duration_ms=duration_ms,
            details=details
        )
    
    async def _check_pose_health(self) -> HealthCheck:
        """Check pose service health."""
        start_time = time.time()
        
        try:
            # Import here to avoid circular imports
            from src.api.dependencies import get_pose_service
            
            pose_service = get_pose_service()
            
            if hasattr(pose_service, 'get_status'):
                status_info = await pose_service.get_status()
                
                if status_info.get("status") == "healthy":
                    status = HealthStatus.HEALTHY
                    message = "Pose service is operational"
                else:
                    status = HealthStatus.DEGRADED
                    message = f"Pose service status: {status_info.get('status', 'unknown')}"
                
                details = status_info
            else:
                status = HealthStatus.UNKNOWN
                message = "Pose service status unavailable"
                details = {}
        
        except Exception as e:
            status = HealthStatus.UNHEALTHY
            message = f"Pose health check failed: {e}"
            details = {"error": str(e)}
        
        duration_ms = (time.time() - start_time) * 1000
        
        return HealthCheck(
            name="pose",
            status=status,
            message=message,
            duration_ms=duration_ms,
            details=details
        )
    
    async def _check_stream_health(self) -> HealthCheck:
        """Check stream service health."""
        start_time = time.time()
        
        try:
            # Import here to avoid circular imports
            from src.api.dependencies import get_stream_service
            
            stream_service = get_stream_service()
            
            if hasattr(stream_service, 'get_status'):
                status_info = await stream_service.get_status()
                
                if status_info.get("status") == "healthy":
                    status = HealthStatus.HEALTHY
                    message = "Stream service is operational"
                else:
                    status = HealthStatus.DEGRADED
                    message = f"Stream service status: {status_info.get('status', 'unknown')}"
                
                details = status_info
            else:
                status = HealthStatus.UNKNOWN
                message = "Stream service status unavailable"
                details = {}
        
        except Exception as e:
            status = HealthStatus.UNHEALTHY
            message = f"Stream health check failed: {e}"
            details = {"error": str(e)}
        
        duration_ms = (time.time() - start_time) * 1000
        
        return HealthCheck(
            name="stream",
            status=status,
            message=message,
            duration_ms=duration_ms,
            details=details
        )
    
    def _update_service_health(self, service_name: str, health_check: HealthCheck):
        """Update service health information."""
        if service_name not in self._services:
            self._services[service_name] = ServiceHealth(service_name, HealthStatus.UNKNOWN)
        
        service_health = self._services[service_name]
        service_health.status = health_check.status
        service_health.last_check = health_check.timestamp
        service_health.uptime = time.time() - self._start_time
        
        # Keep last 10 checks
        service_health.checks.append(health_check)
        if len(service_health.checks) > 10:
            service_health.checks.pop(0)
        
        # Update error tracking
        if health_check.status == HealthStatus.UNHEALTHY:
            service_health.error_count += 1
            service_health.last_error = health_check.message
    
    async def get_overall_health(self) -> Dict[str, Any]:
        """Get overall system health."""
        if not self._services:
            return {
                "status": HealthStatus.UNKNOWN.value,
                "message": "Health checks not initialized"
            }
        
        # Determine overall status
        statuses = [service.status for service in self._services.values()]
        
        if all(status == HealthStatus.HEALTHY for status in statuses):
            overall_status = HealthStatus.HEALTHY
            message = "All services are healthy"
        elif any(status == HealthStatus.UNHEALTHY for status in statuses):
            overall_status = HealthStatus.UNHEALTHY
            unhealthy_services = [
                name for name, service in self._services.items()
                if service.status == HealthStatus.UNHEALTHY
            ]
            message = f"Unhealthy services: {', '.join(unhealthy_services)}"
        elif any(status == HealthStatus.DEGRADED for status in statuses):
            overall_status = HealthStatus.DEGRADED
            degraded_services = [
                name for name, service in self._services.items()
                if service.status == HealthStatus.DEGRADED
            ]
            message = f"Degraded services: {', '.join(degraded_services)}"
        else:
            overall_status = HealthStatus.UNKNOWN
            message = "System health status unknown"
        
        return {
            "status": overall_status.value,
            "message": message,
            "timestamp": datetime.utcnow().isoformat(),
            "uptime": time.time() - self._start_time,
            "services": {
                name: {
                    "status": service.status.value,
                    "last_check": service.last_check.isoformat() if service.last_check else None,
                    "error_count": service.error_count,
                    "last_error": service.last_error
                }
                for name, service in self._services.items()
            }
        }
    
    async def get_service_health(self, service_name: str) -> Optional[Dict[str, Any]]:
        """Get health information for a specific service."""
        service = self._services.get(service_name)
        if not service:
            return None
        
        return {
            "name": service.name,
            "status": service.status.value,
            "last_check": service.last_check.isoformat() if service.last_check else None,
            "uptime": service.uptime,
            "error_count": service.error_count,
            "last_error": service.last_error,
            "recent_checks": [
                {
                    "timestamp": check.timestamp.isoformat(),
                    "status": check.status.value,
                    "message": check.message,
                    "duration_ms": check.duration_ms,
                    "details": check.details
                }
                for check in service.checks[-5:]  # Last 5 checks
            ]
        }
    
    async def get_status(self) -> Dict[str, Any]:
        """Get health check service status."""
        return {
            "status": "healthy" if self._running else "stopped",
            "initialized": self._initialized,
            "running": self._running,
            "services_monitored": len(self._services),
            "uptime": time.time() - self._start_time
        }
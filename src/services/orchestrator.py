"""
Main service orchestrator for WiFi-DensePose API
"""

import asyncio
import logging
from typing import Dict, Any, List, Optional
from contextlib import asynccontextmanager

from src.config.settings import Settings
from src.services.health_check import HealthCheckService
from src.services.metrics import MetricsService
from src.api.dependencies import (
    get_hardware_service,
    get_pose_service,
    get_stream_service
)
from src.api.websocket.connection_manager import connection_manager
from src.api.websocket.pose_stream import PoseStreamHandler

logger = logging.getLogger(__name__)


class ServiceOrchestrator:
    """Main service orchestrator that manages all application services."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self._services: Dict[str, Any] = {}
        self._background_tasks: List[asyncio.Task] = []
        self._initialized = False
        self._started = False
        
        # Core services
        self.health_service = HealthCheckService(settings)
        self.metrics_service = MetricsService(settings)
        
        # Application services (will be initialized later)
        self.hardware_service = None
        self.pose_service = None
        self.stream_service = None
        self.pose_stream_handler = None
    
    async def initialize(self):
        """Initialize all services."""
        if self._initialized:
            logger.warning("Services already initialized")
            return
        
        logger.info("Initializing services...")
        
        try:
            # Initialize core services
            await self.health_service.initialize()
            await self.metrics_service.initialize()
            
            # Initialize application services
            await self._initialize_application_services()
            
            # Store services in registry
            self._services = {
                'health': self.health_service,
                'metrics': self.metrics_service,
                'hardware': self.hardware_service,
                'pose': self.pose_service,
                'stream': self.stream_service,
                'pose_stream_handler': self.pose_stream_handler,
                'connection_manager': connection_manager
            }
            
            self._initialized = True
            logger.info("All services initialized successfully")
            
        except Exception as e:
            logger.error(f"Failed to initialize services: {e}")
            await self.shutdown()
            raise
    
    async def _initialize_application_services(self):
        """Initialize application-specific services."""
        try:
            # Initialize hardware service
            self.hardware_service = get_hardware_service()
            await self.hardware_service.initialize()
            logger.info("Hardware service initialized")
            
            # Initialize pose service
            self.pose_service = get_pose_service()
            await self.pose_service.initialize()
            logger.info("Pose service initialized")
            
            # Initialize stream service
            self.stream_service = get_stream_service()
            await self.stream_service.initialize()
            logger.info("Stream service initialized")
            
            # Initialize pose stream handler
            self.pose_stream_handler = PoseStreamHandler(
                connection_manager=connection_manager,
                pose_service=self.pose_service,
                stream_service=self.stream_service
            )
            logger.info("Pose stream handler initialized")
            
        except Exception as e:
            logger.error(f"Failed to initialize application services: {e}")
            raise
    
    async def start(self):
        """Start all services and background tasks."""
        if not self._initialized:
            await self.initialize()
        
        if self._started:
            logger.warning("Services already started")
            return
        
        logger.info("Starting services...")
        
        try:
            # Start core services
            await self.health_service.start()
            await self.metrics_service.start()
            
            # Start application services
            await self._start_application_services()
            
            # Start background tasks
            await self._start_background_tasks()
            
            self._started = True
            logger.info("All services started successfully")
            
        except Exception as e:
            logger.error(f"Failed to start services: {e}")
            await self.shutdown()
            raise
    
    async def _start_application_services(self):
        """Start application-specific services."""
        try:
            # Start hardware service
            if hasattr(self.hardware_service, 'start'):
                await self.hardware_service.start()
            
            # Start pose service
            if hasattr(self.pose_service, 'start'):
                await self.pose_service.start()
            
            # Start stream service
            if hasattr(self.stream_service, 'start'):
                await self.stream_service.start()
            
            logger.info("Application services started")
            
        except Exception as e:
            logger.error(f"Failed to start application services: {e}")
            raise
    
    async def _start_background_tasks(self):
        """Start background tasks."""
        try:
            # Start health check monitoring
            if self.settings.health_check_interval > 0:
                task = asyncio.create_task(self._health_check_loop())
                self._background_tasks.append(task)
            
            # Start metrics collection
            if self.settings.metrics_enabled:
                task = asyncio.create_task(self._metrics_collection_loop())
                self._background_tasks.append(task)
            
            # Start pose streaming if enabled
            if self.settings.enable_real_time_processing:
                await self.pose_stream_handler.start_streaming()
            
            logger.info(f"Started {len(self._background_tasks)} background tasks")
            
        except Exception as e:
            logger.error(f"Failed to start background tasks: {e}")
            raise
    
    async def _health_check_loop(self):
        """Background health check loop."""
        logger.info("Starting health check loop")
        
        while True:
            try:
                await self.health_service.perform_health_checks()
                await asyncio.sleep(self.settings.health_check_interval)
            except asyncio.CancelledError:
                logger.info("Health check loop cancelled")
                break
            except Exception as e:
                logger.error(f"Error in health check loop: {e}")
                await asyncio.sleep(self.settings.health_check_interval)
    
    async def _metrics_collection_loop(self):
        """Background metrics collection loop."""
        logger.info("Starting metrics collection loop")
        
        while True:
            try:
                await self.metrics_service.collect_metrics()
                await asyncio.sleep(60)  # Collect metrics every minute
            except asyncio.CancelledError:
                logger.info("Metrics collection loop cancelled")
                break
            except Exception as e:
                logger.error(f"Error in metrics collection loop: {e}")
                await asyncio.sleep(60)
    
    async def shutdown(self):
        """Shutdown all services and cleanup resources."""
        logger.info("Shutting down services...")
        
        try:
            # Cancel background tasks
            for task in self._background_tasks:
                if not task.done():
                    task.cancel()
            
            if self._background_tasks:
                await asyncio.gather(*self._background_tasks, return_exceptions=True)
                self._background_tasks.clear()
            
            # Stop pose streaming
            if self.pose_stream_handler:
                await self.pose_stream_handler.shutdown()
            
            # Shutdown connection manager
            await connection_manager.shutdown()
            
            # Shutdown application services
            await self._shutdown_application_services()
            
            # Shutdown core services
            await self.health_service.shutdown()
            await self.metrics_service.shutdown()
            
            self._started = False
            self._initialized = False
            
            logger.info("All services shut down successfully")
            
        except Exception as e:
            logger.error(f"Error during shutdown: {e}")
    
    async def _shutdown_application_services(self):
        """Shutdown application-specific services."""
        try:
            # Shutdown services in reverse order
            if self.stream_service and hasattr(self.stream_service, 'shutdown'):
                await self.stream_service.shutdown()
            
            if self.pose_service and hasattr(self.pose_service, 'shutdown'):
                await self.pose_service.shutdown()
            
            if self.hardware_service and hasattr(self.hardware_service, 'shutdown'):
                await self.hardware_service.shutdown()
            
            logger.info("Application services shut down")
            
        except Exception as e:
            logger.error(f"Error shutting down application services: {e}")
    
    async def restart_service(self, service_name: str):
        """Restart a specific service."""
        logger.info(f"Restarting service: {service_name}")
        
        service = self._services.get(service_name)
        if not service:
            raise ValueError(f"Service not found: {service_name}")
        
        try:
            # Stop service
            if hasattr(service, 'stop'):
                await service.stop()
            elif hasattr(service, 'shutdown'):
                await service.shutdown()
            
            # Reinitialize service
            if hasattr(service, 'initialize'):
                await service.initialize()
            
            # Start service
            if hasattr(service, 'start'):
                await service.start()
            
            logger.info(f"Service restarted successfully: {service_name}")
            
        except Exception as e:
            logger.error(f"Failed to restart service {service_name}: {e}")
            raise
    
    async def reset_services(self):
        """Reset all services to initial state."""
        logger.info("Resetting all services")
        
        try:
            # Reset application services
            if self.hardware_service and hasattr(self.hardware_service, 'reset'):
                await self.hardware_service.reset()
            
            if self.pose_service and hasattr(self.pose_service, 'reset'):
                await self.pose_service.reset()
            
            if self.stream_service and hasattr(self.stream_service, 'reset'):
                await self.stream_service.reset()
            
            # Reset connection manager
            await connection_manager.reset()
            
            logger.info("All services reset successfully")
            
        except Exception as e:
            logger.error(f"Failed to reset services: {e}")
            raise
    
    async def get_service_status(self) -> Dict[str, Any]:
        """Get status of all services."""
        status = {}
        
        for name, service in self._services.items():
            try:
                if hasattr(service, 'get_status'):
                    status[name] = await service.get_status()
                else:
                    status[name] = {"status": "unknown"}
            except Exception as e:
                status[name] = {"status": "error", "error": str(e)}
        
        return status
    
    async def get_service_metrics(self) -> Dict[str, Any]:
        """Get metrics from all services."""
        metrics = {}
        
        for name, service in self._services.items():
            try:
                if hasattr(service, 'get_metrics'):
                    metrics[name] = await service.get_metrics()
                elif hasattr(service, 'get_performance_metrics'):
                    metrics[name] = await service.get_performance_metrics()
            except Exception as e:
                logger.error(f"Failed to get metrics from {name}: {e}")
                metrics[name] = {"error": str(e)}
        
        return metrics
    
    async def get_service_info(self) -> Dict[str, Any]:
        """Get information about all services."""
        info = {
            "total_services": len(self._services),
            "initialized": self._initialized,
            "started": self._started,
            "background_tasks": len(self._background_tasks),
            "services": {}
        }
        
        for name, service in self._services.items():
            service_info = {
                "type": type(service).__name__,
                "module": type(service).__module__
            }
            
            # Add service-specific info if available
            if hasattr(service, 'get_info'):
                try:
                    service_info.update(await service.get_info())
                except Exception as e:
                    service_info["error"] = str(e)
            
            info["services"][name] = service_info
        
        return info
    
    def get_service(self, name: str) -> Optional[Any]:
        """Get a specific service by name."""
        return self._services.get(name)
    
    @property
    def is_healthy(self) -> bool:
        """Check if all services are healthy."""
        return self._initialized and self._started
    
    @asynccontextmanager
    async def service_context(self):
        """Context manager for service lifecycle."""
        try:
            await self.initialize()
            await self.start()
            yield self
        finally:
            await self.shutdown()
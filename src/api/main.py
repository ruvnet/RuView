"""
FastAPI application for WiFi-DensePose API
"""

import asyncio
import logging
import logging.config
from contextlib import asynccontextmanager
from typing import Dict, Any

from fastapi import FastAPI, Request, Response
from fastapi.middleware.cors import CORSMiddleware
from fastapi.middleware.trustedhost import TrustedHostMiddleware
from fastapi.responses import JSONResponse
from fastapi.exceptions import RequestValidationError
from starlette.exceptions import HTTPException as StarletteHTTPException

from src.config.settings import get_settings
from src.config.domains import get_domain_config
from src.api.routers import pose, stream, health
from src.api.middleware.auth import AuthMiddleware
from src.api.middleware.rate_limit import RateLimitMiddleware
from src.api.dependencies import get_pose_service, get_stream_service, get_hardware_service
from src.api.websocket.connection_manager import connection_manager
from src.api.websocket.pose_stream import PoseStreamHandler

# Configure logging
settings = get_settings()
logging.config.dictConfig(settings.get_logging_config())
logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan manager."""
    logger.info("Starting WiFi-DensePose API...")
    
    try:
        # Initialize services
        await initialize_services(app)
        
        # Start background tasks
        await start_background_tasks(app)
        
        logger.info("WiFi-DensePose API started successfully")
        
        yield
        
    except Exception as e:
        logger.error(f"Failed to start application: {e}")
        raise
    finally:
        # Cleanup on shutdown
        logger.info("Shutting down WiFi-DensePose API...")
        await cleanup_services(app)
        logger.info("WiFi-DensePose API shutdown complete")


async def initialize_services(app: FastAPI):
    """Initialize application services."""
    try:
        # Initialize hardware service
        hardware_service = get_hardware_service()
        await hardware_service.initialize()
        
        # Initialize pose service
        pose_service = get_pose_service()
        await pose_service.initialize()
        
        # Initialize stream service
        stream_service = get_stream_service()
        await stream_service.initialize()
        
        # Initialize pose stream handler
        pose_stream_handler = PoseStreamHandler(
            connection_manager=connection_manager,
            pose_service=pose_service,
            stream_service=stream_service
        )
        
        # Store in app state for access in routes
        app.state.hardware_service = hardware_service
        app.state.pose_service = pose_service
        app.state.stream_service = stream_service
        app.state.pose_stream_handler = pose_stream_handler
        
        logger.info("Services initialized successfully")
        
    except Exception as e:
        logger.error(f"Failed to initialize services: {e}")
        raise


async def start_background_tasks(app: FastAPI):
    """Start background tasks."""
    try:
        # Start pose service
        pose_service = app.state.pose_service
        await pose_service.start()
        logger.info("Pose service started")
        
        # Start pose streaming if enabled
        if settings.enable_real_time_processing:
            pose_stream_handler = app.state.pose_stream_handler
            await pose_stream_handler.start_streaming()
        
        logger.info("Background tasks started")
        
    except Exception as e:
        logger.error(f"Failed to start background tasks: {e}")
        raise


async def cleanup_services(app: FastAPI):
    """Cleanup services on shutdown."""
    try:
        # Stop pose streaming
        if hasattr(app.state, 'pose_stream_handler'):
            await app.state.pose_stream_handler.shutdown()
        
        # Shutdown connection manager
        await connection_manager.shutdown()
        
        # Cleanup services
        if hasattr(app.state, 'stream_service'):
            await app.state.stream_service.shutdown()
        
        if hasattr(app.state, 'pose_service'):
            await app.state.pose_service.stop()
        
        if hasattr(app.state, 'hardware_service'):
            await app.state.hardware_service.shutdown()
        
        logger.info("Services cleaned up successfully")
        
    except Exception as e:
        logger.error(f"Error during cleanup: {e}")


# Create FastAPI application
app = FastAPI(
    title=settings.app_name,
    version=settings.version,
    description="WiFi-based human pose estimation and activity recognition API",
    docs_url=settings.docs_url if not settings.is_production else None,
    redoc_url=settings.redoc_url if not settings.is_production else None,
    openapi_url=settings.openapi_url if not settings.is_production else None,
    lifespan=lifespan
)

# Add middleware
if settings.enable_rate_limiting:
    app.add_middleware(RateLimitMiddleware)

if settings.enable_authentication:
    app.add_middleware(AuthMiddleware)

# Add CORS middleware
cors_config = settings.get_cors_config()
app.add_middleware(
    CORSMiddleware,
    **cors_config
)

# Add trusted host middleware for production
if settings.is_production:
    app.add_middleware(
        TrustedHostMiddleware,
        allowed_hosts=settings.allowed_hosts
    )


# Exception handlers
@app.exception_handler(StarletteHTTPException)
async def http_exception_handler(request: Request, exc: StarletteHTTPException):
    """Handle HTTP exceptions."""
    return JSONResponse(
        status_code=exc.status_code,
        content={
            "error": {
                "code": exc.status_code,
                "message": exc.detail,
                "type": "http_error"
            }
        }
    )


@app.exception_handler(RequestValidationError)
async def validation_exception_handler(request: Request, exc: RequestValidationError):
    """Handle request validation errors."""
    return JSONResponse(
        status_code=422,
        content={
            "error": {
                "code": 422,
                "message": "Validation error",
                "type": "validation_error",
                "details": exc.errors()
            }
        }
    )


@app.exception_handler(Exception)
async def general_exception_handler(request: Request, exc: Exception):
    """Handle general exceptions."""
    logger.error(f"Unhandled exception: {exc}", exc_info=True)
    
    return JSONResponse(
        status_code=500,
        content={
            "error": {
                "code": 500,
                "message": "Internal server error",
                "type": "internal_error"
            }
        }
    )


# Middleware for request logging
@app.middleware("http")
async def log_requests(request: Request, call_next):
    """Log all requests."""
    start_time = asyncio.get_event_loop().time()
    
    # Process request
    response = await call_next(request)
    
    # Calculate processing time
    process_time = asyncio.get_event_loop().time() - start_time
    
    # Log request
    logger.info(
        f"{request.method} {request.url.path} - "
        f"Status: {response.status_code} - "
        f"Time: {process_time:.3f}s"
    )
    
    # Add processing time header
    response.headers["X-Process-Time"] = str(process_time)
    
    return response


# Include routers
app.include_router(
    health.router,
    prefix="/health",
    tags=["Health"]
)

app.include_router(
    pose.router,
    prefix=f"{settings.api_prefix}/pose",
    tags=["Pose Estimation"]
)

app.include_router(
    stream.router,
    prefix=f"{settings.api_prefix}/stream",
    tags=["Streaming"]
)


# Root endpoint
@app.get("/")
async def root():
    """Root endpoint with API information."""
    return {
        "name": settings.app_name,
        "version": settings.version,
        "environment": settings.environment,
        "docs_url": settings.docs_url,
        "api_prefix": settings.api_prefix,
        "features": {
            "authentication": settings.enable_authentication,
            "rate_limiting": settings.enable_rate_limiting,
            "websockets": settings.enable_websockets,
            "real_time_processing": settings.enable_real_time_processing
        }
    }


# API information endpoint
@app.get(f"{settings.api_prefix}/info")
async def api_info():
    """Get detailed API information."""
    domain_config = get_domain_config()
    
    return {
        "api": {
            "name": settings.app_name,
            "version": settings.version,
            "environment": settings.environment,
            "prefix": settings.api_prefix
        },
        "configuration": {
            "zones": len(domain_config.zones),
            "routers": len(domain_config.routers),
            "pose_models": len(domain_config.pose_models)
        },
        "features": {
            "authentication": settings.enable_authentication,
            "rate_limiting": settings.enable_rate_limiting,
            "websockets": settings.enable_websockets,
            "real_time_processing": settings.enable_real_time_processing,
            "historical_data": settings.enable_historical_data
        },
        "limits": {
            "rate_limit_requests": settings.rate_limit_requests,
            "rate_limit_window": settings.rate_limit_window,
            "max_websocket_connections": domain_config.streaming.max_connections
        }
    }


# Status endpoint
@app.get(f"{settings.api_prefix}/status")
async def api_status(request: Request):
    """Get current API status."""
    try:
        # Get services from app state
        hardware_service = getattr(request.app.state, 'hardware_service', None)
        pose_service = getattr(request.app.state, 'pose_service', None)
        stream_service = getattr(request.app.state, 'stream_service', None)
        pose_stream_handler = getattr(request.app.state, 'pose_stream_handler', None)
        
        # Get service statuses
        status = {
            "api": {
                "status": "healthy",
                "uptime": "unknown",
                "version": settings.version
            },
            "services": {
                "hardware": await hardware_service.get_status() if hardware_service else {"status": "unavailable"},
                "pose": await pose_service.get_status() if pose_service else {"status": "unavailable"},
                "stream": await stream_service.get_status() if stream_service else {"status": "unavailable"}
            },
            "streaming": pose_stream_handler.get_stream_status() if pose_stream_handler else {"is_streaming": False},
            "connections": await connection_manager.get_connection_stats()
        }
        
        return status
        
    except Exception as e:
        logger.error(f"Error getting API status: {e}")
        return {
            "api": {
                "status": "error",
                "error": str(e)
            }
        }


# Metrics endpoint (if enabled)
if settings.metrics_enabled:
    @app.get(f"{settings.api_prefix}/metrics")
    async def api_metrics(request: Request):
        """Get API metrics."""
        try:
            # Get services from app state
            pose_stream_handler = getattr(request.app.state, 'pose_stream_handler', None)
            
            metrics = {
                "connections": await connection_manager.get_metrics(),
                "streaming": await pose_stream_handler.get_performance_metrics() if pose_stream_handler else {}
            }
            
            return metrics
            
        except Exception as e:
            logger.error(f"Error getting metrics: {e}")
            return {"error": str(e)}


# Development endpoints (only in development)
if settings.is_development and settings.enable_test_endpoints:
    @app.get(f"{settings.api_prefix}/dev/config")
    async def dev_config():
        """Get current configuration (development only)."""
        domain_config = get_domain_config()
        return {
            "settings": settings.dict(),
            "domain_config": domain_config.to_dict()
        }
    
    @app.post(f"{settings.api_prefix}/dev/reset")
    async def dev_reset(request: Request):
        """Reset services (development only)."""
        try:
            # Reset services
            hardware_service = getattr(request.app.state, 'hardware_service', None)
            pose_service = getattr(request.app.state, 'pose_service', None)
            
            if hardware_service:
                await hardware_service.reset()
            
            if pose_service:
                await pose_service.reset()
            
            return {"message": "Services reset successfully"}
            
        except Exception as e:
            logger.error(f"Error resetting services: {e}")
            return {"error": str(e)}


if __name__ == "__main__":
    import uvicorn
    
    uvicorn.run(
        "src.api.main:app",
        host=settings.host,
        port=settings.port,
        reload=settings.reload,
        workers=settings.workers if not settings.reload else 1,
        log_level=settings.log_level.lower()
    )
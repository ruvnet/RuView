"""
FastAPI application factory and configuration
"""

import logging
import os
from contextlib import asynccontextmanager
from typing import Optional

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.middleware.trustedhost import TrustedHostMiddleware
from fastapi.responses import JSONResponse
from fastapi.exceptions import RequestValidationError
from starlette.exceptions import HTTPException as StarletteHTTPException

from src.config.settings import Settings
from src.services.orchestrator import ServiceOrchestrator
from src.middleware.auth import AuthenticationMiddleware
from fastapi.middleware.cors import CORSMiddleware
from src.middleware.rate_limit import RateLimitMiddleware
from src.middleware.error_handler import ErrorHandlingMiddleware
from src.api.routers import pose, stream, health
from src.api.websocket.connection_manager import connection_manager

logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan manager."""
    logger.info("Starting WiFi-DensePose API...")
    
    try:
        # Get orchestrator from app state
        orchestrator: ServiceOrchestrator = app.state.orchestrator
        
        # Start connection manager
        await connection_manager.start()
        
        # Start all services
        await orchestrator.start()
        
        logger.info("WiFi-DensePose API started successfully")
        
        yield
        
    except Exception as e:
        logger.error(f"Failed to start application: {e}")
        raise
    finally:
        # Cleanup on shutdown
        logger.info("Shutting down WiFi-DensePose API...")
        
        # Shutdown connection manager
        await connection_manager.shutdown()
        
        if hasattr(app.state, 'orchestrator'):
            await app.state.orchestrator.shutdown()
        logger.info("WiFi-DensePose API shutdown complete")


def create_app(settings: Settings, orchestrator: ServiceOrchestrator) -> FastAPI:
    """Create and configure FastAPI application."""
    
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
    
    # Store orchestrator in app state
    app.state.orchestrator = orchestrator
    app.state.settings = settings
    
    # Add middleware in reverse order (last added = first executed)
    setup_middleware(app, settings)
    
    # Add exception handlers
    setup_exception_handlers(app)
    
    # Include routers
    setup_routers(app, settings)
    
    # Add root endpoints
    setup_root_endpoints(app, settings)
    
    return app


def setup_middleware(app: FastAPI, settings: Settings):
    """Setup application middleware."""
    
    # Rate limiting middleware
    if settings.enable_rate_limiting:
        app.add_middleware(RateLimitMiddleware, settings=settings)
    
    # Authentication middleware
    if settings.enable_authentication:
        app.add_middleware(AuthenticationMiddleware, settings=settings)
    
    # CORS middleware
    if settings.cors_enabled:
        app.add_middleware(
            CORSMiddleware,
            allow_origins=settings.cors_origins,
            allow_credentials=settings.cors_allow_credentials,
            allow_methods=["GET", "POST", "PUT", "DELETE", "OPTIONS", "PATCH"],
            allow_headers=["*"],
        )
    
    # Trusted host middleware for production
    if settings.is_production:
        app.add_middleware(
            TrustedHostMiddleware,
            allowed_hosts=settings.allowed_hosts
        )


def setup_exception_handlers(app: FastAPI):
    """Setup global exception handlers."""
    
    @app.exception_handler(StarletteHTTPException)
    async def http_exception_handler(request: Request, exc: StarletteHTTPException):
        """Handle HTTP exceptions."""
        return JSONResponse(
            status_code=exc.status_code,
            content={
                "error": {
                    "code": exc.status_code,
                    "message": exc.detail,
                    "type": "http_error",
                    "path": str(request.url.path)
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
                    "path": str(request.url.path),
                    "details": exc.errors()
                }
            }
        )
    
    @app.exception_handler(Exception)
    async def general_exception_handler(request: Request, exc: Exception):
        """Handle general exceptions."""
        logger.error(f"Unhandled exception on {request.url.path}: {exc}", exc_info=True)
        
        return JSONResponse(
            status_code=500,
            content={
                "error": {
                    "code": 500,
                    "message": "Internal server error",
                    "type": "internal_error",
                    "path": str(request.url.path)
                }
            }
        )


def setup_routers(app: FastAPI, settings: Settings):
    """Setup API routers."""
    
    # Health check router (no prefix)
    app.include_router(
        health.router,
        prefix="/health",
        tags=["Health"]
    )
    
    # API routers with prefix
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


def setup_root_endpoints(app: FastAPI, settings: Settings):
    """Setup root application endpoints."""
    
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
    
    @app.get(f"{settings.api_prefix}/info")
    async def api_info(request: Request):
        """Get detailed API information."""
        orchestrator: ServiceOrchestrator = request.app.state.orchestrator
        
        return {
            "api": {
                "name": settings.app_name,
                "version": settings.version,
                "environment": settings.environment,
                "prefix": settings.api_prefix
            },
            "services": await orchestrator.get_service_info(),
            "features": {
                "authentication": settings.enable_authentication,
                "rate_limiting": settings.enable_rate_limiting,
                "websockets": settings.enable_websockets,
                "real_time_processing": settings.enable_real_time_processing,
                "historical_data": settings.enable_historical_data
            },
            "limits": {
                "rate_limit_requests": settings.rate_limit_requests,
                "rate_limit_window": settings.rate_limit_window
            }
        }
    
    @app.get(f"{settings.api_prefix}/status")
    async def api_status(request: Request):
        """Get current API status."""
        try:
            orchestrator: ServiceOrchestrator = request.app.state.orchestrator
            
            status = {
                "api": {
                    "status": "healthy",
                    "version": settings.version,
                    "environment": settings.environment
                },
                "services": await orchestrator.get_service_status(),
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
                orchestrator: ServiceOrchestrator = request.app.state.orchestrator
                
                metrics = {
                    "connections": await connection_manager.get_metrics(),
                    "services": await orchestrator.get_service_metrics()
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
            return {
                "settings": settings.dict(),
                "environment_variables": dict(os.environ)
            }
        
        @app.post(f"{settings.api_prefix}/dev/reset")
        async def dev_reset(request: Request):
            """Reset services (development only)."""
            try:
                orchestrator: ServiceOrchestrator = request.app.state.orchestrator
                await orchestrator.reset_services()
                return {"message": "Services reset successfully"}
                
            except Exception as e:
                logger.error(f"Error resetting services: {e}")
                return {"error": str(e)}


# Create default app instance for uvicorn
def get_app() -> FastAPI:
    """Get the default application instance."""
    from src.config.settings import get_settings
    from src.services.orchestrator import ServiceOrchestrator
    
    settings = get_settings()
    orchestrator = ServiceOrchestrator(settings)
    return create_app(settings, orchestrator)


# Default app instance for uvicorn
app = get_app()
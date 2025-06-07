"""
CORS middleware for WiFi-DensePose API
"""

import logging
from typing import List, Optional, Union, Callable
from urllib.parse import urlparse

from fastapi import Request, Response
from fastapi.middleware.cors import CORSMiddleware as FastAPICORSMiddleware
from starlette.types import ASGIApp

from src.config.settings import Settings

logger = logging.getLogger(__name__)


class CORSMiddleware:
    """Enhanced CORS middleware with additional security features."""
    
    def __init__(
        self,
        app: ASGIApp,
        settings: Settings,
        allow_origins: Optional[List[str]] = None,
        allow_methods: Optional[List[str]] = None,
        allow_headers: Optional[List[str]] = None,
        allow_credentials: bool = False,
        expose_headers: Optional[List[str]] = None,
        max_age: int = 600,
    ):
        self.app = app
        self.settings = settings
        self.allow_origins = allow_origins or settings.cors_origins
        self.allow_methods = allow_methods or ["GET", "POST", "PUT", "DELETE", "OPTIONS", "PATCH"]
        self.allow_headers = allow_headers or [
            "Accept",
            "Accept-Language",
            "Content-Language",
            "Content-Type",
            "Authorization",
            "X-Requested-With",
            "X-Request-ID",
            "X-User-Agent",
        ]
        self.allow_credentials = allow_credentials or settings.cors_allow_credentials
        self.expose_headers = expose_headers or [
            "X-Request-ID",
            "X-Response-Time",
            "X-Rate-Limit-Remaining",
            "X-Rate-Limit-Reset",
        ]
        self.max_age = max_age
        
        # Security settings
        self.strict_origin_check = settings.is_production
        self.log_cors_violations = True
    
    async def __call__(self, scope, receive, send):
        """ASGI middleware implementation."""
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return
        
        request = Request(scope, receive)
        
        # Check if this is a CORS preflight request
        if request.method == "OPTIONS" and "access-control-request-method" in request.headers:
            response = await self._handle_preflight(request)
            await response(scope, receive, send)
            return
        
        # Handle actual request
        async def send_wrapper(message):
            if message["type"] == "http.response.start":
                # Add CORS headers to response
                headers = dict(message.get("headers", []))
                cors_headers = self._get_cors_headers(request)
                
                for key, value in cors_headers.items():
                    headers[key.encode()] = value.encode()
                
                message["headers"] = list(headers.items())
            
            await send(message)
        
        await self.app(scope, receive, send_wrapper)
    
    async def _handle_preflight(self, request: Request) -> Response:
        """Handle CORS preflight request."""
        origin = request.headers.get("origin")
        requested_method = request.headers.get("access-control-request-method")
        requested_headers = request.headers.get("access-control-request-headers", "")
        
        # Validate origin
        if not self._is_origin_allowed(origin):
            if self.log_cors_violations:
                logger.warning(f"CORS preflight rejected for origin: {origin}")
            
            return Response(
                status_code=403,
                content="CORS preflight request rejected",
                headers={"Content-Type": "text/plain"}
            )
        
        # Validate method
        if requested_method not in self.allow_methods:
            if self.log_cors_violations:
                logger.warning(f"CORS preflight rejected for method: {requested_method}")
            
            return Response(
                status_code=405,
                content="Method not allowed",
                headers={"Content-Type": "text/plain"}
            )
        
        # Validate headers
        if requested_headers:
            requested_header_list = [h.strip().lower() for h in requested_headers.split(",")]
            allowed_headers_lower = [h.lower() for h in self.allow_headers]
            
            for header in requested_header_list:
                if header not in allowed_headers_lower:
                    if self.log_cors_violations:
                        logger.warning(f"CORS preflight rejected for header: {header}")
                    
                    return Response(
                        status_code=400,
                        content="Header not allowed",
                        headers={"Content-Type": "text/plain"}
                    )
        
        # Build preflight response headers
        headers = {
            "Access-Control-Allow-Origin": origin,
            "Access-Control-Allow-Methods": ", ".join(self.allow_methods),
            "Access-Control-Allow-Headers": ", ".join(self.allow_headers),
            "Access-Control-Max-Age": str(self.max_age),
        }
        
        if self.allow_credentials:
            headers["Access-Control-Allow-Credentials"] = "true"
        
        if self.expose_headers:
            headers["Access-Control-Expose-Headers"] = ", ".join(self.expose_headers)
        
        logger.debug(f"CORS preflight approved for origin: {origin}")
        
        return Response(
            status_code=200,
            headers=headers
        )
    
    def _get_cors_headers(self, request: Request) -> dict:
        """Get CORS headers for actual request."""
        origin = request.headers.get("origin")
        headers = {}
        
        if self._is_origin_allowed(origin):
            headers["Access-Control-Allow-Origin"] = origin
            
            if self.allow_credentials:
                headers["Access-Control-Allow-Credentials"] = "true"
            
            if self.expose_headers:
                headers["Access-Control-Expose-Headers"] = ", ".join(self.expose_headers)
        
        return headers
    
    def _is_origin_allowed(self, origin: Optional[str]) -> bool:
        """Check if origin is allowed."""
        if not origin:
            return not self.strict_origin_check
        
        # Allow all origins in development
        if not self.settings.is_production and "*" in self.allow_origins:
            return True
        
        # Check exact matches
        if origin in self.allow_origins:
            return True
        
        # Check wildcard patterns
        for allowed_origin in self.allow_origins:
            if allowed_origin == "*":
                return not self.strict_origin_check
            
            if self._match_origin_pattern(origin, allowed_origin):
                return True
        
        return False
    
    def _match_origin_pattern(self, origin: str, pattern: str) -> bool:
        """Match origin against pattern with wildcard support."""
        if "*" not in pattern:
            return origin == pattern
        
        # Simple wildcard matching
        if pattern.startswith("*."):
            domain = pattern[2:]
            parsed_origin = urlparse(origin)
            origin_host = parsed_origin.netloc
            
            # Check if origin ends with the domain
            return origin_host.endswith(domain) or origin_host == domain[1:] if domain.startswith('.') else origin_host == domain
        
        return False


def setup_cors_middleware(app: ASGIApp, settings: Settings) -> ASGIApp:
    """Setup CORS middleware for the application."""
    
    if settings.cors_enabled:
        logger.info("Setting up CORS middleware")
        
        # Use FastAPI's built-in CORS middleware for basic functionality
        app = FastAPICORSMiddleware(
            app,
            allow_origins=settings.cors_origins,
            allow_credentials=settings.cors_allow_credentials,
            allow_methods=["GET", "POST", "PUT", "DELETE", "OPTIONS", "PATCH"],
            allow_headers=[
                "Accept",
                "Accept-Language",
                "Content-Language",
                "Content-Type",
                "Authorization",
                "X-Requested-With",
                "X-Request-ID",
                "X-User-Agent",
            ],
            expose_headers=[
                "X-Request-ID",
                "X-Response-Time",
                "X-Rate-Limit-Remaining",
                "X-Rate-Limit-Reset",
            ],
            max_age=600,
        )
        
        logger.info(f"CORS enabled for origins: {settings.cors_origins}")
    else:
        logger.info("CORS middleware disabled")
    
    return app


class CORSConfig:
    """CORS configuration helper."""
    
    @staticmethod
    def development_config() -> dict:
        """Get CORS configuration for development."""
        return {
            "allow_origins": ["*"],
            "allow_credentials": True,
            "allow_methods": ["*"],
            "allow_headers": ["*"],
            "expose_headers": [
                "X-Request-ID",
                "X-Response-Time",
                "X-Rate-Limit-Remaining",
                "X-Rate-Limit-Reset",
            ],
            "max_age": 600,
        }
    
    @staticmethod
    def production_config(allowed_origins: List[str]) -> dict:
        """Get CORS configuration for production."""
        return {
            "allow_origins": allowed_origins,
            "allow_credentials": True,
            "allow_methods": ["GET", "POST", "PUT", "DELETE", "OPTIONS", "PATCH"],
            "allow_headers": [
                "Accept",
                "Accept-Language",
                "Content-Language",
                "Content-Type",
                "Authorization",
                "X-Requested-With",
                "X-Request-ID",
                "X-User-Agent",
            ],
            "expose_headers": [
                "X-Request-ID",
                "X-Response-Time",
                "X-Rate-Limit-Remaining",
                "X-Rate-Limit-Reset",
            ],
            "max_age": 3600,  # 1 hour for production
        }
    
    @staticmethod
    def api_only_config(allowed_origins: List[str]) -> dict:
        """Get CORS configuration for API-only access."""
        return {
            "allow_origins": allowed_origins,
            "allow_credentials": False,
            "allow_methods": ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
            "allow_headers": [
                "Accept",
                "Content-Type",
                "Authorization",
                "X-Request-ID",
            ],
            "expose_headers": [
                "X-Request-ID",
                "X-Rate-Limit-Remaining",
                "X-Rate-Limit-Reset",
            ],
            "max_age": 3600,
        }
    
    @staticmethod
    def websocket_config(allowed_origins: List[str]) -> dict:
        """Get CORS configuration for WebSocket connections."""
        return {
            "allow_origins": allowed_origins,
            "allow_credentials": True,
            "allow_methods": ["GET", "OPTIONS"],
            "allow_headers": [
                "Accept",
                "Authorization",
                "Sec-WebSocket-Protocol",
                "Sec-WebSocket-Extensions",
            ],
            "expose_headers": [],
            "max_age": 86400,  # 24 hours for WebSocket
        }


def validate_cors_config(settings: Settings) -> List[str]:
    """Validate CORS configuration and return issues."""
    issues = []
    
    if not settings.cors_enabled:
        return issues
    
    # Check origins
    if not settings.cors_origins:
        issues.append("CORS is enabled but no origins are configured")
    
    # Check for wildcard in production
    if settings.is_production and "*" in settings.cors_origins:
        issues.append("Wildcard origin (*) should not be used in production")
    
    # Validate origin formats
    for origin in settings.cors_origins:
        if origin != "*" and not origin.startswith(("http://", "https://")):
            issues.append(f"Invalid origin format: {origin}")
    
    # Check credentials with wildcard
    if settings.cors_allow_credentials and "*" in settings.cors_origins:
        issues.append("Cannot use credentials with wildcard origin")
    
    return issues


def get_cors_headers_for_origin(origin: str, settings: Settings) -> dict:
    """Get appropriate CORS headers for a specific origin."""
    headers = {}
    
    if not settings.cors_enabled:
        return headers
    
    # Check if origin is allowed
    cors_middleware = CORSMiddleware(None, settings)
    if cors_middleware._is_origin_allowed(origin):
        headers["Access-Control-Allow-Origin"] = origin
        
        if settings.cors_allow_credentials:
            headers["Access-Control-Allow-Credentials"] = "true"
    
    return headers
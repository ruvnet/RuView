"""
JWT Authentication middleware for WiFi-DensePose API
"""

import logging
from typing import Optional, Dict, Any
from datetime import datetime

from fastapi import Request, Response
from fastapi.responses import JSONResponse
from starlette.middleware.base import BaseHTTPMiddleware
from jose import JWTError, jwt

from src.config.settings import get_settings

logger = logging.getLogger(__name__)


class AuthMiddleware(BaseHTTPMiddleware):
    """JWT Authentication middleware."""
    
    def __init__(self, app):
        super().__init__(app)
        self.settings = get_settings()
        
        # Paths that don't require authentication
        self.public_paths = {
            "/",
            "/docs",
            "/redoc",
            "/openapi.json",
            "/health",
            "/ready",
            "/live",
            "/version",
            "/metrics"
        }
        
        # Paths that require authentication
        self.protected_paths = {
            "/api/v1/pose/analyze",
            "/api/v1/pose/calibrate",
            "/api/v1/pose/historical",
            "/api/v1/stream/start",
            "/api/v1/stream/stop",
            "/api/v1/stream/clients",
            "/api/v1/stream/broadcast"
        }
    
    async def dispatch(self, request: Request, call_next):
        """Process request through authentication middleware."""
        
        # Skip authentication for public paths
        if self._is_public_path(request.url.path):
            return await call_next(request)
        
        # Extract and validate token
        token = self._extract_token(request)
        
        if token:
            try:
                # Verify token and add user info to request state
                user_data = await self._verify_token(token)
                request.state.user = user_data
                request.state.authenticated = True
                
                logger.debug(f"Authenticated user: {user_data.get('id')}")
                
            except Exception as e:
                logger.warning(f"Token validation failed: {e}")
                
                # For protected paths, return 401
                if self._is_protected_path(request.url.path):
                    return JSONResponse(
                        status_code=401,
                        content={
                            "error": {
                                "code": 401,
                                "message": "Invalid or expired token",
                                "type": "authentication_error"
                            }
                        }
                    )
                
                # For other paths, continue without authentication
                request.state.user = None
                request.state.authenticated = False
        else:
            # No token provided
            if self._is_protected_path(request.url.path):
                return JSONResponse(
                    status_code=401,
                    content={
                        "error": {
                            "code": 401,
                            "message": "Authentication required",
                            "type": "authentication_error"
                        }
                    },
                    headers={"WWW-Authenticate": "Bearer"}
                )
            
            request.state.user = None
            request.state.authenticated = False
        
        # Continue with request processing
        response = await call_next(request)
        
        # Add authentication headers to response
        if hasattr(request.state, 'user') and request.state.user:
            response.headers["X-User-ID"] = request.state.user.get("id", "")
            response.headers["X-Authenticated"] = "true"
        else:
            response.headers["X-Authenticated"] = "false"
        
        return response
    
    def _is_public_path(self, path: str) -> bool:
        """Check if path is public (doesn't require authentication)."""
        # Exact match
        if path in self.public_paths:
            return True
        
        # Pattern matching for public paths
        public_patterns = [
            "/health",
            "/metrics",
            "/api/v1/pose/current",  # Allow anonymous access to current pose data
            "/api/v1/pose/zones/",   # Allow anonymous access to zone data
            "/api/v1/pose/activities",  # Allow anonymous access to activities
            "/api/v1/pose/stats",    # Allow anonymous access to stats
            "/api/v1/stream/status"  # Allow anonymous access to stream status
        ]
        
        for pattern in public_patterns:
            if path.startswith(pattern):
                return True
        
        return False
    
    def _is_protected_path(self, path: str) -> bool:
        """Check if path requires authentication."""
        # Exact match
        if path in self.protected_paths:
            return True
        
        # Pattern matching for protected paths
        protected_patterns = [
            "/api/v1/pose/analyze",
            "/api/v1/pose/calibrate",
            "/api/v1/pose/historical",
            "/api/v1/stream/start",
            "/api/v1/stream/stop",
            "/api/v1/stream/clients",
            "/api/v1/stream/broadcast"
        ]
        
        for pattern in protected_patterns:
            if path.startswith(pattern):
                return True
        
        return False
    
    def _extract_token(self, request: Request) -> Optional[str]:
        """Extract JWT token from request."""
        # Check Authorization header
        auth_header = request.headers.get("authorization")
        if auth_header and auth_header.startswith("Bearer "):
            return auth_header.split(" ")[1]
        
        # Check query parameter (for WebSocket connections)
        token = request.query_params.get("token")
        if token:
            return token
        
        # Check cookie
        token = request.cookies.get("access_token")
        if token:
            return token
        
        return None
    
    async def _verify_token(self, token: str) -> Dict[str, Any]:
        """Verify JWT token and return user data."""
        try:
            # Decode JWT token
            payload = jwt.decode(
                token,
                self.settings.secret_key,
                algorithms=[self.settings.jwt_algorithm]
            )
            
            # Extract user information
            user_id = payload.get("sub")
            if not user_id:
                raise ValueError("Token missing user ID")
            
            # Check token expiration
            exp = payload.get("exp")
            if exp and datetime.utcnow() > datetime.fromtimestamp(exp):
                raise ValueError("Token expired")
            
            # Build user object
            user_data = {
                "id": user_id,
                "username": payload.get("username"),
                "email": payload.get("email"),
                "is_admin": payload.get("is_admin", False),
                "permissions": payload.get("permissions", []),
                "accessible_zones": payload.get("accessible_zones", []),
                "token_issued_at": payload.get("iat"),
                "token_expires_at": payload.get("exp"),
                "session_id": payload.get("session_id")
            }
            
            return user_data
            
        except JWTError as e:
            raise ValueError(f"JWT validation failed: {e}")
        except Exception as e:
            raise ValueError(f"Token verification error: {e}")
    
    def _log_authentication_event(self, request: Request, event_type: str, details: Dict[str, Any] = None):
        """Log authentication events for security monitoring."""
        client_ip = request.client.host if request.client else "unknown"
        user_agent = request.headers.get("user-agent", "unknown")
        
        log_data = {
            "event_type": event_type,
            "timestamp": datetime.utcnow().isoformat(),
            "client_ip": client_ip,
            "user_agent": user_agent,
            "path": request.url.path,
            "method": request.method
        }
        
        if details:
            log_data.update(details)
        
        if event_type in ["authentication_failed", "token_expired", "invalid_token"]:
            logger.warning(f"Auth event: {log_data}")
        else:
            logger.info(f"Auth event: {log_data}")


class TokenBlacklist:
    """Simple in-memory token blacklist for logout functionality."""
    
    def __init__(self):
        self._blacklisted_tokens = set()
        self._cleanup_interval = 3600  # 1 hour
        self._last_cleanup = datetime.utcnow()
    
    def add_token(self, token: str):
        """Add token to blacklist."""
        self._blacklisted_tokens.add(token)
        self._cleanup_if_needed()
    
    def is_blacklisted(self, token: str) -> bool:
        """Check if token is blacklisted."""
        self._cleanup_if_needed()
        return token in self._blacklisted_tokens
    
    def _cleanup_if_needed(self):
        """Clean up expired tokens from blacklist."""
        now = datetime.utcnow()
        if (now - self._last_cleanup).total_seconds() > self._cleanup_interval:
            # In a real implementation, you would check token expiration
            # For now, we'll just clear old tokens periodically
            self._blacklisted_tokens.clear()
            self._last_cleanup = now


# Global token blacklist instance
token_blacklist = TokenBlacklist()


class SecurityHeaders:
    """Security headers for API responses."""
    
    @staticmethod
    def add_security_headers(response: Response) -> Response:
        """Add security headers to response."""
        response.headers["X-Content-Type-Options"] = "nosniff"
        response.headers["X-Frame-Options"] = "DENY"
        response.headers["X-XSS-Protection"] = "1; mode=block"
        response.headers["Referrer-Policy"] = "strict-origin-when-cross-origin"
        response.headers["Content-Security-Policy"] = (
            "default-src 'self'; "
            "script-src 'self' 'unsafe-inline'; "
            "style-src 'self' 'unsafe-inline'; "
            "img-src 'self' data:; "
            "connect-src 'self' ws: wss:;"
        )
        
        return response


class APIKeyAuth:
    """Alternative API key authentication for service-to-service communication."""
    
    def __init__(self, api_keys: Dict[str, Dict[str, Any]] = None):
        self.api_keys = api_keys or {}
    
    def verify_api_key(self, api_key: str) -> Optional[Dict[str, Any]]:
        """Verify API key and return associated service info."""
        if api_key in self.api_keys:
            return self.api_keys[api_key]
        return None
    
    def add_api_key(self, api_key: str, service_info: Dict[str, Any]):
        """Add new API key."""
        self.api_keys[api_key] = service_info
    
    def revoke_api_key(self, api_key: str):
        """Revoke API key."""
        if api_key in self.api_keys:
            del self.api_keys[api_key]


# Global API key auth instance
api_key_auth = APIKeyAuth()
"""
Dependency injection for WiFi-DensePose API
"""

import logging
from typing import Optional, Dict, Any
from functools import lru_cache

from fastapi import Depends, HTTPException, status, Request
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials

from src.config.settings import get_settings
from src.config.domains import get_domain_config
from src.services.pose_service import PoseService
from src.services.stream_service import StreamService
from src.services.hardware_service import HardwareService

logger = logging.getLogger(__name__)

# Security scheme for JWT authentication
security = HTTPBearer(auto_error=False)


# Service dependencies
@lru_cache()
def get_pose_service() -> PoseService:
    """Get pose service instance."""
    settings = get_settings()
    domain_config = get_domain_config()
    
    return PoseService(
        settings=settings,
        domain_config=domain_config
    )


@lru_cache()
def get_stream_service() -> StreamService:
    """Get stream service instance."""
    settings = get_settings()
    domain_config = get_domain_config()
    
    return StreamService(
        settings=settings,
        domain_config=domain_config
    )


@lru_cache()
def get_hardware_service() -> HardwareService:
    """Get hardware service instance."""
    settings = get_settings()
    domain_config = get_domain_config()
    
    return HardwareService(
        settings=settings,
        domain_config=domain_config
    )


# Authentication dependencies
async def get_current_user(
    request: Request,
    credentials: Optional[HTTPAuthorizationCredentials] = Depends(security)
) -> Optional[Dict[str, Any]]:
    """Get current authenticated user."""
    settings = get_settings()
    
    # Skip authentication if disabled
    if not settings.enable_authentication:
        return None
    
    # Check if user is already set by middleware
    if hasattr(request.state, 'user') and request.state.user:
        return request.state.user
    
    # No credentials provided
    if not credentials:
        return None
    
    # This would normally validate the JWT token
    # For now, return a mock user for development
    if settings.is_development:
        return {
            "id": "dev-user",
            "username": "developer",
            "email": "dev@example.com",
            "is_admin": True,
            "permissions": ["read", "write", "admin"]
        }
    
    # In production, implement proper JWT validation
    raise HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="Authentication not implemented",
        headers={"WWW-Authenticate": "Bearer"},
    )


async def get_current_active_user(
    current_user: Optional[Dict[str, Any]] = Depends(get_current_user)
) -> Dict[str, Any]:
    """Get current active user (required authentication)."""
    if not current_user:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Authentication required",
            headers={"WWW-Authenticate": "Bearer"},
        )
    
    # Check if user is active
    if not current_user.get("is_active", True):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Inactive user"
        )
    
    return current_user


async def get_admin_user(
    current_user: Dict[str, Any] = Depends(get_current_active_user)
) -> Dict[str, Any]:
    """Get current admin user (admin privileges required)."""
    if not current_user.get("is_admin", False):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Admin privileges required"
        )
    
    return current_user


# Permission dependencies
def require_permission(permission: str):
    """Dependency factory for permission checking."""
    
    async def check_permission(
        current_user: Dict[str, Any] = Depends(get_current_active_user)
    ) -> Dict[str, Any]:
        """Check if user has required permission."""
        user_permissions = current_user.get("permissions", [])
        
        # Admin users have all permissions
        if current_user.get("is_admin", False):
            return current_user
        
        # Check specific permission
        if permission not in user_permissions:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Permission '{permission}' required"
            )
        
        return current_user
    
    return check_permission


# Zone access dependencies
async def validate_zone_access(
    zone_id: str,
    current_user: Optional[Dict[str, Any]] = Depends(get_current_user)
) -> str:
    """Validate user access to a specific zone."""
    domain_config = get_domain_config()
    
    # Check if zone exists
    zone = domain_config.get_zone(zone_id)
    if not zone:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Zone '{zone_id}' not found"
        )
    
    # Check if zone is enabled
    if not zone.enabled:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Zone '{zone_id}' is disabled"
        )
    
    # If authentication is enabled, check user access
    if current_user:
        # Admin users have access to all zones
        if current_user.get("is_admin", False):
            return zone_id
        
        # Check user's zone permissions
        user_zones = current_user.get("zones", [])
        if user_zones and zone_id not in user_zones:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Access denied to zone '{zone_id}'"
            )
    
    return zone_id


# Router access dependencies
async def validate_router_access(
    router_id: str,
    current_user: Optional[Dict[str, Any]] = Depends(get_current_user)
) -> str:
    """Validate user access to a specific router."""
    domain_config = get_domain_config()
    
    # Check if router exists
    router = domain_config.get_router(router_id)
    if not router:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Router '{router_id}' not found"
        )
    
    # Check if router is enabled
    if not router.enabled:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Router '{router_id}' is disabled"
        )
    
    # If authentication is enabled, check user access
    if current_user:
        # Admin users have access to all routers
        if current_user.get("is_admin", False):
            return router_id
        
        # Check user's router permissions
        user_routers = current_user.get("routers", [])
        if user_routers and router_id not in user_routers:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Access denied to router '{router_id}'"
            )
    
    return router_id


# Service health dependencies
async def check_service_health(
    request: Request,
    service_name: str
) -> bool:
    """Check if a service is healthy."""
    try:
        if service_name == "pose":
            service = getattr(request.app.state, 'pose_service', None)
        elif service_name == "stream":
            service = getattr(request.app.state, 'stream_service', None)
        elif service_name == "hardware":
            service = getattr(request.app.state, 'hardware_service', None)
        else:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"Unknown service: {service_name}"
            )
        
        if not service:
            raise HTTPException(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                detail=f"Service '{service_name}' not available"
            )
        
        # Check service health
        status_info = await service.get_status()
        if status_info.get("status") != "healthy":
            raise HTTPException(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                detail=f"Service '{service_name}' is unhealthy: {status_info.get('error', 'Unknown error')}"
            )
        
        return True
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error checking service health for {service_name}: {e}")
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail=f"Service '{service_name}' health check failed"
        )


# Rate limiting dependencies
async def check_rate_limit(
    request: Request,
    current_user: Optional[Dict[str, Any]] = Depends(get_current_user)
) -> bool:
    """Check rate limiting status."""
    settings = get_settings()
    
    # Skip if rate limiting is disabled
    if not settings.enable_rate_limiting:
        return True
    
    # Rate limiting is handled by middleware
    # This dependency can be used for additional checks
    return True


# Configuration dependencies
def get_zone_config(zone_id: str = Depends(validate_zone_access)):
    """Get zone configuration."""
    domain_config = get_domain_config()
    return domain_config.get_zone(zone_id)


def get_router_config(router_id: str = Depends(validate_router_access)):
    """Get router configuration."""
    domain_config = get_domain_config()
    return domain_config.get_router(router_id)


# Pagination dependencies
class PaginationParams:
    """Pagination parameters."""
    
    def __init__(
        self,
        page: int = 1,
        size: int = 20,
        max_size: int = 100
    ):
        if page < 1:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Page must be >= 1"
            )
        
        if size < 1:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Size must be >= 1"
            )
        
        if size > max_size:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"Size must be <= {max_size}"
            )
        
        self.page = page
        self.size = size
        self.offset = (page - 1) * size
        self.limit = size


def get_pagination_params(
    page: int = 1,
    size: int = 20
) -> PaginationParams:
    """Get pagination parameters."""
    return PaginationParams(page=page, size=size)


# Query filter dependencies
class QueryFilters:
    """Common query filters."""
    
    def __init__(
        self,
        start_time: Optional[str] = None,
        end_time: Optional[str] = None,
        min_confidence: Optional[float] = None,
        activity: Optional[str] = None
    ):
        self.start_time = start_time
        self.end_time = end_time
        self.min_confidence = min_confidence
        self.activity = activity
        
        # Validate confidence
        if min_confidence is not None:
            if not 0.0 <= min_confidence <= 1.0:
                raise HTTPException(
                    status_code=status.HTTP_400_BAD_REQUEST,
                    detail="min_confidence must be between 0.0 and 1.0"
                )


def get_query_filters(
    start_time: Optional[str] = None,
    end_time: Optional[str] = None,
    min_confidence: Optional[float] = None,
    activity: Optional[str] = None
) -> QueryFilters:
    """Get query filters."""
    return QueryFilters(
        start_time=start_time,
        end_time=end_time,
        min_confidence=min_confidence,
        activity=activity
    )


# WebSocket dependencies
async def get_websocket_user(
    websocket_token: Optional[str] = None
) -> Optional[Dict[str, Any]]:
    """Get user from WebSocket token."""
    settings = get_settings()
    
    # Skip authentication if disabled
    if not settings.enable_authentication:
        return None
    
    # For development, return mock user
    if settings.is_development:
        return {
            "id": "ws-user",
            "username": "websocket_user",
            "is_admin": False,
            "permissions": ["read"]
        }
    
    # In production, implement proper token validation
    return None


async def get_current_user_ws(
    websocket_token: Optional[str] = None
) -> Optional[Dict[str, Any]]:
    """Get current user for WebSocket connections."""
    return await get_websocket_user(websocket_token)


# Authentication requirement dependencies
async def require_auth(
    current_user: Dict[str, Any] = Depends(get_current_active_user)
) -> Dict[str, Any]:
    """Require authentication for endpoint access."""
    return current_user


# Development dependencies
async def development_only():
    """Dependency that only allows access in development."""
    settings = get_settings()
    
    if not settings.is_development:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Endpoint not available in production"
        )
    
    return True
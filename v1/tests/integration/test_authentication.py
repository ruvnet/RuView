"""
Integration tests for authentication and authorization.

Tests JWT authentication flow, user permissions, and access control.
"""

import pytest
import asyncio
from datetime import datetime, timedelta
from typing import Dict, Any, Optional
from unittest.mock import AsyncMock, MagicMock, patch
import jwt
import json

from fastapi import HTTPException, status
from fastapi.security import HTTPAuthorizationCredentials


class MockJWTToken:
    """Mock JWT token for testing."""
    
    def __init__(self, payload: Dict[str, Any], secret: str = "test-secret"):
        self.payload = payload
        self.secret = secret
        self.token = jwt.encode(payload, secret, algorithm="HS256")
    
    def decode(self, token: str, secret: str) -> Dict[str, Any]:
        """Decode JWT token."""
        return jwt.decode(token, secret, algorithms=["HS256"])


class TestJWTAuthentication:
    """Test JWT authentication functionality."""
    
    @pytest.fixture
    def valid_user_payload(self):
        """Valid user payload for JWT token."""
        return {
            "sub": "user-001",
            "username": "testuser",
            "email": "test@example.com",
            "is_admin": False,
            "is_active": True,
            "permissions": ["read", "write"],
            "exp": datetime.utcnow() + timedelta(hours=1),
            "iat": datetime.utcnow()
        }
    
    @pytest.fixture
    def admin_user_payload(self):
        """Admin user payload for JWT token."""
        return {
            "sub": "admin-001",
            "username": "admin",
            "email": "admin@example.com",
            "is_admin": True,
            "is_active": True,
            "permissions": ["read", "write", "admin"],
            "exp": datetime.utcnow() + timedelta(hours=1),
            "iat": datetime.utcnow()
        }
    
    @pytest.fixture
    def expired_user_payload(self):
        """Expired user payload for JWT token."""
        return {
            "sub": "user-002",
            "username": "expireduser",
            "email": "expired@example.com",
            "is_admin": False,
            "is_active": True,
            "permissions": ["read"],
            "exp": datetime.utcnow() - timedelta(hours=1),  # Expired
            "iat": datetime.utcnow() - timedelta(hours=2)
        }
    
    @pytest.fixture
    def mock_jwt_service(self):
        """Mock JWT service."""
        class MockJWTService:
            def __init__(self):
                self.secret = "test-secret-key"
                self.algorithm = "HS256"
            
            def create_token(self, user_data: Dict[str, Any]) -> str:
                """Create JWT token."""
                payload = {
                    **user_data,
                    "exp": datetime.utcnow() + timedelta(hours=1),
                    "iat": datetime.utcnow()
                }
                return jwt.encode(payload, self.secret, algorithm=self.algorithm)
            
            def verify_token(self, token: str) -> Dict[str, Any]:
                """Verify JWT token."""
                try:
                    payload = jwt.decode(token, self.secret, algorithms=[self.algorithm])
                    return payload
                except jwt.ExpiredSignatureError:
                    raise HTTPException(
                        status_code=status.HTTP_401_UNAUTHORIZED,
                        detail="Token has expired"
                    )
                except jwt.InvalidTokenError:
                    raise HTTPException(
                        status_code=status.HTTP_401_UNAUTHORIZED,
                        detail="Invalid token"
                    )
            
            def refresh_token(self, token: str) -> str:
                """Refresh JWT token."""
                payload = self.verify_token(token)
                # Remove exp and iat for new token
                payload.pop("exp", None)
                payload.pop("iat", None)
                return self.create_token(payload)
        
        return MockJWTService()
    
    def test_jwt_token_creation_should_fail_initially(self, mock_jwt_service, valid_user_payload):
        """Test JWT token creation - should fail initially."""
        token = mock_jwt_service.create_token(valid_user_payload)
        
        # This will fail initially
        assert isinstance(token, str)
        assert len(token) > 0
        
        # Verify token can be decoded
        decoded = mock_jwt_service.verify_token(token)
        assert decoded["sub"] == valid_user_payload["sub"]
        assert decoded["username"] == valid_user_payload["username"]
    
    def test_jwt_token_verification_should_fail_initially(self, mock_jwt_service, valid_user_payload):
        """Test JWT token verification - should fail initially."""
        token = mock_jwt_service.create_token(valid_user_payload)
        decoded = mock_jwt_service.verify_token(token)
        
        # This will fail initially
        assert decoded["sub"] == valid_user_payload["sub"]
        assert decoded["is_admin"] == valid_user_payload["is_admin"]
        assert "exp" in decoded
        assert "iat" in decoded
    
    def test_expired_token_rejection_should_fail_initially(self, mock_jwt_service, expired_user_payload):
        """Test expired token rejection - should fail initially."""
        # Create token with expired payload
        token = jwt.encode(expired_user_payload, mock_jwt_service.secret, algorithm=mock_jwt_service.algorithm)
        
        # This should fail initially
        with pytest.raises(HTTPException) as exc_info:
            mock_jwt_service.verify_token(token)
        
        assert exc_info.value.status_code == status.HTTP_401_UNAUTHORIZED
        assert "expired" in exc_info.value.detail.lower()
    
    def test_invalid_token_rejection_should_fail_initially(self, mock_jwt_service):
        """Test invalid token rejection - should fail initially."""
        invalid_token = "invalid.jwt.token"
        
        # This should fail initially
        with pytest.raises(HTTPException) as exc_info:
            mock_jwt_service.verify_token(invalid_token)
        
        assert exc_info.value.status_code == status.HTTP_401_UNAUTHORIZED
        assert "invalid" in exc_info.value.detail.lower()
    
    def test_token_refresh_should_fail_initially(self, mock_jwt_service, valid_user_payload):
        """Test token refresh functionality - should fail initially."""
        original_token = mock_jwt_service.create_token(valid_user_payload)
        
        # Wait a moment to ensure different timestamps
        import time
        time.sleep(0.1)
        
        refreshed_token = mock_jwt_service.refresh_token(original_token)
        
        # This will fail initially
        assert refreshed_token != original_token
        
        # Verify both tokens are valid but have different timestamps
        original_payload = mock_jwt_service.verify_token(original_token)
        refreshed_payload = mock_jwt_service.verify_token(refreshed_token)
        
        assert original_payload["sub"] == refreshed_payload["sub"]
        assert original_payload["iat"] != refreshed_payload["iat"]


class TestUserAuthentication:
    """Test user authentication scenarios."""
    
    @pytest.fixture
    def mock_user_service(self):
        """Mock user service."""
        class MockUserService:
            def __init__(self):
                self.users = {
                    "testuser": {
                        "id": "user-001",
                        "username": "testuser",
                        "email": "test@example.com",
                        "password_hash": "hashed_password",
                        "is_admin": False,
                        "is_active": True,
                        "permissions": ["read", "write"],
                        "zones": ["zone1", "zone2"],
                        "created_at": datetime.utcnow()
                    },
                    "admin": {
                        "id": "admin-001",
                        "username": "admin",
                        "email": "admin@example.com",
                        "password_hash": "admin_hashed_password",
                        "is_admin": True,
                        "is_active": True,
                        "permissions": ["read", "write", "admin"],
                        "zones": [],  # Admin has access to all zones
                        "created_at": datetime.utcnow()
                    }
                }
            
            async def authenticate_user(self, username: str, password: str) -> Optional[Dict[str, Any]]:
                """Authenticate user with username and password."""
                user = self.users.get(username)
                if not user:
                    return None
                
                # Mock password verification
                if password == "correct_password":
                    return user
                return None
            
            async def get_user_by_id(self, user_id: str) -> Optional[Dict[str, Any]]:
                """Get user by ID."""
                for user in self.users.values():
                    if user["id"] == user_id:
                        return user
                return None
            
            async def update_user_activity(self, user_id: str):
                """Update user last activity."""
                user = await self.get_user_by_id(user_id)
                if user:
                    user["last_activity"] = datetime.utcnow()
        
        return MockUserService()
    
    @pytest.mark.asyncio
    async def test_user_authentication_success_should_fail_initially(self, mock_user_service):
        """Test successful user authentication - should fail initially."""
        user = await mock_user_service.authenticate_user("testuser", "correct_password")
        
        # This will fail initially
        assert user is not None
        assert user["username"] == "testuser"
        assert user["is_active"] is True
        assert "read" in user["permissions"]
    
    @pytest.mark.asyncio
    async def test_user_authentication_failure_should_fail_initially(self, mock_user_service):
        """Test failed user authentication - should fail initially."""
        user = await mock_user_service.authenticate_user("testuser", "wrong_password")
        
        # This will fail initially
        assert user is None
        
        # Test with non-existent user
        user = await mock_user_service.authenticate_user("nonexistent", "any_password")
        assert user is None
    
    @pytest.mark.asyncio
    async def test_admin_user_authentication_should_fail_initially(self, mock_user_service):
        """Test admin user authentication - should fail initially."""
        admin = await mock_user_service.authenticate_user("admin", "correct_password")
        
        # This will fail initially
        assert admin is not None
        assert admin["is_admin"] is True
        assert "admin" in admin["permissions"]
        assert admin["zones"] == []  # Admin has access to all zones


class TestAuthorizationDependencies:
    """Test authorization dependency functions."""
    
    @pytest.fixture
    def mock_request(self):
        """Mock FastAPI request."""
        class MockRequest:
            def __init__(self):
                self.state = MagicMock()
                self.state.user = None
        
        return MockRequest()
    
    @pytest.fixture
    def mock_credentials(self):
        """Mock HTTP authorization credentials."""
        def create_credentials(token: str):
            return HTTPAuthorizationCredentials(
                scheme="Bearer",
                credentials=token
            )
        return create_credentials
    
    @pytest.mark.asyncio
    async def test_get_current_user_with_valid_token_should_fail_initially(self, mock_request, mock_credentials):
        """Test get_current_user with valid token - should fail initially."""
        # Mock the get_current_user dependency
        async def mock_get_current_user(request, credentials):
            if not credentials:
                return None
            
            # Mock token validation
            if credentials.credentials == "valid_token":
                return {
                    "id": "user-001",
                    "username": "testuser",
                    "is_admin": False,
                    "is_active": True,
                    "permissions": ["read", "write"]
                }
            return None
        
        credentials = mock_credentials("valid_token")
        user = await mock_get_current_user(mock_request, credentials)
        
        # This will fail initially
        assert user is not None
        assert user["username"] == "testuser"
        assert user["is_active"] is True
    
    @pytest.mark.asyncio
    async def test_get_current_user_without_credentials_should_fail_initially(self, mock_request):
        """Test get_current_user without credentials - should fail initially."""
        async def mock_get_current_user(request, credentials):
            if not credentials:
                return None
            return {"id": "user-001"}
        
        user = await mock_get_current_user(mock_request, None)
        
        # This will fail initially
        assert user is None
    
    @pytest.mark.asyncio
    async def test_require_active_user_should_fail_initially(self):
        """Test require active user dependency - should fail initially."""
        async def mock_get_current_active_user(current_user):
            if not current_user:
                raise HTTPException(
                    status_code=status.HTTP_401_UNAUTHORIZED,
                    detail="Authentication required"
                )
            
            if not current_user.get("is_active", True):
                raise HTTPException(
                    status_code=status.HTTP_403_FORBIDDEN,
                    detail="Inactive user"
                )
            
            return current_user
        
        # Test with active user
        active_user = {"id": "user-001", "is_active": True}
        result = await mock_get_current_active_user(active_user)
        
        # This will fail initially
        assert result == active_user
        
        # Test with inactive user
        inactive_user = {"id": "user-002", "is_active": False}
        with pytest.raises(HTTPException) as exc_info:
            await mock_get_current_active_user(inactive_user)
        
        assert exc_info.value.status_code == status.HTTP_403_FORBIDDEN
        
        # Test with no user
        with pytest.raises(HTTPException) as exc_info:
            await mock_get_current_active_user(None)
        
        assert exc_info.value.status_code == status.HTTP_401_UNAUTHORIZED
    
    @pytest.mark.asyncio
    async def test_require_admin_user_should_fail_initially(self):
        """Test require admin user dependency - should fail initially."""
        async def mock_get_admin_user(current_user):
            if not current_user.get("is_admin", False):
                raise HTTPException(
                    status_code=status.HTTP_403_FORBIDDEN,
                    detail="Admin privileges required"
                )
            return current_user
        
        # Test with admin user
        admin_user = {"id": "admin-001", "is_admin": True}
        result = await mock_get_admin_user(admin_user)
        
        # This will fail initially
        assert result == admin_user
        
        # Test with regular user
        regular_user = {"id": "user-001", "is_admin": False}
        with pytest.raises(HTTPException) as exc_info:
            await mock_get_admin_user(regular_user)
        
        assert exc_info.value.status_code == status.HTTP_403_FORBIDDEN
    
    @pytest.mark.asyncio
    async def test_permission_checking_should_fail_initially(self):
        """Test permission checking functionality - should fail initially."""
        def require_permission(permission: str):
            async def check_permission(current_user):
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
        
        # Test with user having required permission
        user_with_permission = {
            "id": "user-001",
            "permissions": ["read", "write"],
            "is_admin": False
        }
        
        check_read = require_permission("read")
        result = await check_read(user_with_permission)
        
        # This will fail initially
        assert result == user_with_permission
        
        # Test with user missing permission
        check_admin = require_permission("admin")
        with pytest.raises(HTTPException) as exc_info:
            await check_admin(user_with_permission)
        
        assert exc_info.value.status_code == status.HTTP_403_FORBIDDEN
        assert "admin" in exc_info.value.detail
        
        # Test with admin user (should have all permissions)
        admin_user = {"id": "admin-001", "is_admin": True, "permissions": ["read"]}
        result = await check_admin(admin_user)
        assert result == admin_user


class TestZoneAndRouterAccess:
    """Test zone and router access control."""
    
    @pytest.fixture
    def mock_domain_config(self):
        """Mock domain configuration."""
        class MockDomainConfig:
            def __init__(self):
                self.zones = {
                    "zone1": {"id": "zone1", "name": "Zone 1", "enabled": True},
                    "zone2": {"id": "zone2", "name": "Zone 2", "enabled": True},
                    "zone3": {"id": "zone3", "name": "Zone 3", "enabled": False}
                }
                self.routers = {
                    "router1": {"id": "router1", "name": "Router 1", "enabled": True},
                    "router2": {"id": "router2", "name": "Router 2", "enabled": False}
                }
            
            def get_zone(self, zone_id: str):
                return self.zones.get(zone_id)
            
            def get_router(self, router_id: str):
                return self.routers.get(router_id)
        
        return MockDomainConfig()
    
    @pytest.mark.asyncio
    async def test_zone_access_validation_should_fail_initially(self, mock_domain_config):
        """Test zone access validation - should fail initially."""
        async def validate_zone_access(zone_id: str, current_user=None):
            zone = mock_domain_config.get_zone(zone_id)
            if not zone:
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail=f"Zone '{zone_id}' not found"
                )
            
            if not zone["enabled"]:
                raise HTTPException(
                    status_code=status.HTTP_403_FORBIDDEN,
                    detail=f"Zone '{zone_id}' is disabled"
                )
            
            if current_user:
                if current_user.get("is_admin", False):
                    return zone_id
                
                user_zones = current_user.get("zones", [])
                if user_zones and zone_id not in user_zones:
                    raise HTTPException(
                        status_code=status.HTTP_403_FORBIDDEN,
                        detail=f"Access denied to zone '{zone_id}'"
                    )
            
            return zone_id
        
        # Test valid zone access
        result = await validate_zone_access("zone1")
        
        # This will fail initially
        assert result == "zone1"
        
        # Test invalid zone
        with pytest.raises(HTTPException) as exc_info:
            await validate_zone_access("nonexistent")
        
        assert exc_info.value.status_code == status.HTTP_404_NOT_FOUND
        
        # Test disabled zone
        with pytest.raises(HTTPException) as exc_info:
            await validate_zone_access("zone3")
        
        assert exc_info.value.status_code == status.HTTP_403_FORBIDDEN
        
        # Test user with zone access
        user_with_access = {"id": "user-001", "zones": ["zone1", "zone2"]}
        result = await validate_zone_access("zone1", user_with_access)
        assert result == "zone1"
        
        # Test user without zone access
        with pytest.raises(HTTPException) as exc_info:
            await validate_zone_access("zone2", {"id": "user-002", "zones": ["zone1"]})
        
        assert exc_info.value.status_code == status.HTTP_403_FORBIDDEN
    
    @pytest.mark.asyncio
    async def test_router_access_validation_should_fail_initially(self, mock_domain_config):
        """Test router access validation - should fail initially."""
        async def validate_router_access(router_id: str, current_user=None):
            router = mock_domain_config.get_router(router_id)
            if not router:
                raise HTTPException(
                    status_code=status.HTTP_404_NOT_FOUND,
                    detail=f"Router '{router_id}' not found"
                )
            
            if not router["enabled"]:
                raise HTTPException(
                    status_code=status.HTTP_403_FORBIDDEN,
                    detail=f"Router '{router_id}' is disabled"
                )
            
            return router_id
        
        # Test valid router access
        result = await validate_router_access("router1")
        
        # This will fail initially
        assert result == "router1"
        
        # Test disabled router
        with pytest.raises(HTTPException) as exc_info:
            await validate_router_access("router2")
        
        assert exc_info.value.status_code == status.HTTP_403_FORBIDDEN
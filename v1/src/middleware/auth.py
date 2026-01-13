"""
Authentication middleware for WiFi-DensePose API
"""

import logging
import time
from typing import Optional, Dict, Any, Callable
from datetime import datetime, timedelta

from fastapi import Request, Response, HTTPException, status
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from jose import JWTError, jwt
from passlib.context import CryptContext

from src.config.settings import Settings
from src.logger import set_request_context

logger = logging.getLogger(__name__)

# Password hashing
pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")

# JWT token handler
security = HTTPBearer(auto_error=False)


class AuthenticationError(Exception):
    """Authentication error."""
    pass


class AuthorizationError(Exception):
    """Authorization error."""
    pass


class TokenManager:
    """JWT token management."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self.secret_key = settings.secret_key
        self.algorithm = settings.jwt_algorithm
        self.expire_hours = settings.jwt_expire_hours
    
    def create_access_token(self, data: Dict[str, Any]) -> str:
        """Create JWT access token."""
        to_encode = data.copy()
        expire = datetime.utcnow() + timedelta(hours=self.expire_hours)
        to_encode.update({"exp": expire, "iat": datetime.utcnow()})
        
        encoded_jwt = jwt.encode(to_encode, self.secret_key, algorithm=self.algorithm)
        return encoded_jwt
    
    def verify_token(self, token: str) -> Dict[str, Any]:
        """Verify and decode JWT token."""
        try:
            payload = jwt.decode(token, self.secret_key, algorithms=[self.algorithm])
            return payload
        except JWTError as e:
            logger.warning(f"JWT verification failed: {e}")
            raise AuthenticationError("Invalid token")
    
    def decode_token(self, token: str) -> Optional[Dict[str, Any]]:
        """Decode token without verification (for debugging)."""
        try:
            return jwt.decode(token, options={"verify_signature": False})
        except JWTError:
            return None


class UserManager:
    """User management for authentication."""
    
    def __init__(self):
        # In a real application, this would connect to a database
        # For now, we'll use a simple in-memory store
        self._users: Dict[str, Dict[str, Any]] = {
            "admin": {
                "username": "admin",
                "email": "admin@example.com",
                "hashed_password": self.hash_password("admin123"),
                "roles": ["admin"],
                "is_active": True,
                "created_at": datetime.utcnow(),
            },
            "user": {
                "username": "user",
                "email": "user@example.com",
                "hashed_password": self.hash_password("user123"),
                "roles": ["user"],
                "is_active": True,
                "created_at": datetime.utcnow(),
            }
        }
    
    @staticmethod
    def hash_password(password: str) -> str:
        """Hash a password."""
        return pwd_context.hash(password)
    
    @staticmethod
    def verify_password(plain_password: str, hashed_password: str) -> bool:
        """Verify a password against its hash."""
        return pwd_context.verify(plain_password, hashed_password)
    
    def get_user(self, username: str) -> Optional[Dict[str, Any]]:
        """Get user by username."""
        return self._users.get(username)
    
    def authenticate_user(self, username: str, password: str) -> Optional[Dict[str, Any]]:
        """Authenticate user with username and password."""
        user = self.get_user(username)
        if not user:
            return None
        
        if not self.verify_password(password, user["hashed_password"]):
            return None
        
        if not user.get("is_active", False):
            return None
        
        return user
    
    def create_user(self, username: str, email: str, password: str, roles: list = None) -> Dict[str, Any]:
        """Create a new user."""
        if username in self._users:
            raise ValueError("User already exists")
        
        user = {
            "username": username,
            "email": email,
            "hashed_password": self.hash_password(password),
            "roles": roles or ["user"],
            "is_active": True,
            "created_at": datetime.utcnow(),
        }
        
        self._users[username] = user
        return user
    
    def update_user(self, username: str, updates: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """Update user information."""
        user = self._users.get(username)
        if not user:
            return None
        
        # Don't allow updating certain fields
        protected_fields = {"username", "created_at", "hashed_password"}
        updates = {k: v for k, v in updates.items() if k not in protected_fields}
        
        user.update(updates)
        return user
    
    def deactivate_user(self, username: str) -> bool:
        """Deactivate a user."""
        user = self._users.get(username)
        if user:
            user["is_active"] = False
            return True
        return False


class AuthenticationMiddleware:
    """Authentication middleware for FastAPI."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self.token_manager = TokenManager(settings)
        self.user_manager = UserManager()
        self.enabled = settings.enable_authentication
    
    async def __call__(self, request: Request, call_next: Callable) -> Response:
        """Process request through authentication middleware."""
        start_time = time.time()
        
        try:
            # Skip authentication for certain paths
            if self._should_skip_auth(request):
                response = await call_next(request)
                return response
            
            # Skip if authentication is disabled
            if not self.enabled:
                response = await call_next(request)
                return response
            
            # Extract and verify token
            user_info = await self._authenticate_request(request)
            
            # Set user context
            if user_info:
                request.state.user = user_info
                set_request_context(user_id=user_info.get("username"))
            
            # Process request
            response = await call_next(request)
            
            # Add authentication headers
            self._add_auth_headers(response, user_info)
            
            return response
            
        except AuthenticationError as e:
            logger.warning(f"Authentication failed: {e}")
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail=str(e),
                headers={"WWW-Authenticate": "Bearer"},
            )
        except AuthorizationError as e:
            logger.warning(f"Authorization failed: {e}")
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=str(e),
            )
        except Exception as e:
            logger.error(f"Authentication middleware error: {e}")
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Authentication service error",
            )
        finally:
            # Log request processing time
            processing_time = time.time() - start_time
            logger.debug(f"Auth middleware processing time: {processing_time:.3f}s")
    
    def _should_skip_auth(self, request: Request) -> bool:
        """Check if authentication should be skipped for this request."""
        path = request.url.path
        
        # Skip authentication for these paths
        skip_paths = [
            "/health",
            "/metrics",
            "/docs",
            "/redoc",
            "/openapi.json",
            "/auth/login",
            "/auth/register",
            "/static",
        ]
        
        return any(path.startswith(skip_path) for skip_path in skip_paths)
    
    async def _authenticate_request(self, request: Request) -> Optional[Dict[str, Any]]:
        """Authenticate the request and return user info."""
        # Try to get token from Authorization header
        authorization = request.headers.get("Authorization")
        if not authorization:
            # For WebSocket connections, try to get token from query parameters
            if request.url.path.startswith("/ws"):
                token = request.query_params.get("token")
                if token:
                    authorization = f"Bearer {token}"
        
        if not authorization:
            if self._requires_auth(request):
                raise AuthenticationError("Missing authorization header")
            return None
        
        # Extract token
        try:
            scheme, token = authorization.split()
            if scheme.lower() != "bearer":
                raise AuthenticationError("Invalid authentication scheme")
        except ValueError:
            raise AuthenticationError("Invalid authorization header format")
        
        # Verify token
        try:
            payload = self.token_manager.verify_token(token)
            username = payload.get("sub")
            if not username:
                raise AuthenticationError("Invalid token payload")
            
            # Get user info
            user = self.user_manager.get_user(username)
            if not user:
                raise AuthenticationError("User not found")
            
            if not user.get("is_active", False):
                raise AuthenticationError("User account is disabled")
            
            # Return user info without sensitive data
            return {
                "username": user["username"],
                "email": user["email"],
                "roles": user["roles"],
                "is_active": user["is_active"],
            }
            
        except AuthenticationError:
            raise
        except Exception as e:
            logger.error(f"Token verification error: {e}")
            raise AuthenticationError("Token verification failed")
    
    def _requires_auth(self, request: Request) -> bool:
        """Check if the request requires authentication."""
        # All API endpoints require authentication by default
        path = request.url.path
        return path.startswith("/api/") or path.startswith("/ws/")
    
    def _add_auth_headers(self, response: Response, user_info: Optional[Dict[str, Any]]):
        """Add authentication-related headers to response."""
        if user_info:
            response.headers["X-User"] = user_info["username"]
            response.headers["X-User-Roles"] = ",".join(user_info["roles"])
    
    async def login(self, username: str, password: str) -> Dict[str, Any]:
        """Authenticate user and return token."""
        user = self.user_manager.authenticate_user(username, password)
        if not user:
            raise AuthenticationError("Invalid username or password")
        
        # Create token
        token_data = {
            "sub": user["username"],
            "email": user["email"],
            "roles": user["roles"],
        }
        
        access_token = self.token_manager.create_access_token(token_data)
        
        return {
            "access_token": access_token,
            "token_type": "bearer",
            "expires_in": self.settings.jwt_expire_hours * 3600,
            "user": {
                "username": user["username"],
                "email": user["email"],
                "roles": user["roles"],
            }
        }
    
    async def register(self, username: str, email: str, password: str) -> Dict[str, Any]:
        """Register a new user."""
        try:
            user = self.user_manager.create_user(username, email, password)
            
            # Create token for new user
            token_data = {
                "sub": user["username"],
                "email": user["email"],
                "roles": user["roles"],
            }
            
            access_token = self.token_manager.create_access_token(token_data)
            
            return {
                "access_token": access_token,
                "token_type": "bearer",
                "expires_in": self.settings.jwt_expire_hours * 3600,
                "user": {
                    "username": user["username"],
                    "email": user["email"],
                    "roles": user["roles"],
                }
            }
            
        except ValueError as e:
            raise AuthenticationError(str(e))
    
    async def refresh_token(self, token: str) -> Dict[str, Any]:
        """Refresh an access token."""
        try:
            payload = self.token_manager.verify_token(token)
            username = payload.get("sub")
            
            user = self.user_manager.get_user(username)
            if not user or not user.get("is_active", False):
                raise AuthenticationError("User not found or inactive")
            
            # Create new token
            token_data = {
                "sub": user["username"],
                "email": user["email"],
                "roles": user["roles"],
            }
            
            new_token = self.token_manager.create_access_token(token_data)
            
            return {
                "access_token": new_token,
                "token_type": "bearer",
                "expires_in": self.settings.jwt_expire_hours * 3600,
            }
            
        except Exception as e:
            raise AuthenticationError("Token refresh failed")
    
    def check_permission(self, user_info: Dict[str, Any], required_role: str) -> bool:
        """Check if user has required role/permission."""
        user_roles = user_info.get("roles", [])
        
        # Admin role has all permissions
        if "admin" in user_roles:
            return True
        
        # Check specific role
        return required_role in user_roles
    
    def require_role(self, required_role: str):
        """Decorator to require specific role."""
        def decorator(func):
            import functools
            
            @functools.wraps(func)
            async def wrapper(request: Request, *args, **kwargs):
                user_info = getattr(request.state, "user", None)
                if not user_info:
                    raise AuthorizationError("Authentication required")
                
                if not self.check_permission(user_info, required_role):
                    raise AuthorizationError(f"Role '{required_role}' required")
                
                return await func(request, *args, **kwargs)
            
            return wrapper
        return decorator


# Global authentication middleware instance
_auth_middleware: Optional[AuthenticationMiddleware] = None


def get_auth_middleware(settings: Settings) -> AuthenticationMiddleware:
    """Get authentication middleware instance."""
    global _auth_middleware
    if _auth_middleware is None:
        _auth_middleware = AuthenticationMiddleware(settings)
    return _auth_middleware


def get_current_user(request: Request) -> Optional[Dict[str, Any]]:
    """Get current authenticated user from request."""
    return getattr(request.state, "user", None)


def require_authentication(request: Request) -> Dict[str, Any]:
    """Require authentication and return user info."""
    user = get_current_user(request)
    if not user:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Authentication required",
            headers={"WWW-Authenticate": "Bearer"},
        )
    return user


def require_role(role: str):
    """Dependency to require specific role."""
    def dependency(request: Request) -> Dict[str, Any]:
        user = require_authentication(request)
        
        auth_middleware = get_auth_middleware(request.app.state.settings)
        if not auth_middleware.check_permission(user, role):
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Role '{role}' required",
            )
        
        return user
    
    return dependency
"""
Rate limiting middleware for WiFi-DensePose API
"""

import asyncio
import logging
import time
from typing import Dict, Any, Optional, Callable, Tuple
from datetime import datetime, timedelta
from collections import defaultdict, deque
from dataclasses import dataclass

from fastapi import Request, Response, HTTPException, status
from starlette.types import ASGIApp

from src.config.settings import Settings

logger = logging.getLogger(__name__)


@dataclass
class RateLimitInfo:
    """Rate limit information."""
    requests: int
    window_start: float
    window_size: int
    limit: int
    
    @property
    def remaining(self) -> int:
        """Get remaining requests in current window."""
        return max(0, self.limit - self.requests)
    
    @property
    def reset_time(self) -> float:
        """Get time when window resets."""
        return self.window_start + self.window_size
    
    @property
    def is_exceeded(self) -> bool:
        """Check if rate limit is exceeded."""
        return self.requests >= self.limit


class TokenBucket:
    """Token bucket algorithm for rate limiting."""
    
    def __init__(self, capacity: int, refill_rate: float):
        self.capacity = capacity
        self.tokens = capacity
        self.refill_rate = refill_rate
        self.last_refill = time.time()
        self._lock = asyncio.Lock()
    
    async def consume(self, tokens: int = 1) -> bool:
        """Try to consume tokens from bucket."""
        async with self._lock:
            now = time.time()
            
            # Refill tokens based on time elapsed
            time_passed = now - self.last_refill
            tokens_to_add = time_passed * self.refill_rate
            self.tokens = min(self.capacity, self.tokens + tokens_to_add)
            self.last_refill = now
            
            # Check if we have enough tokens
            if self.tokens >= tokens:
                self.tokens -= tokens
                return True
            
            return False
    
    def get_info(self) -> Dict[str, Any]:
        """Get bucket information."""
        return {
            "capacity": self.capacity,
            "tokens": self.tokens,
            "refill_rate": self.refill_rate,
            "last_refill": self.last_refill
        }


class SlidingWindowCounter:
    """Sliding window counter for rate limiting."""
    
    def __init__(self, window_size: int, limit: int):
        self.window_size = window_size
        self.limit = limit
        self.requests = deque()
        self._lock = asyncio.Lock()
    
    async def is_allowed(self) -> Tuple[bool, RateLimitInfo]:
        """Check if request is allowed."""
        async with self._lock:
            now = time.time()
            window_start = now - self.window_size
            
            # Remove old requests outside the window
            while self.requests and self.requests[0] < window_start:
                self.requests.popleft()
            
            # Check if limit is exceeded
            current_requests = len(self.requests)
            allowed = current_requests < self.limit
            
            if allowed:
                self.requests.append(now)
            
            rate_limit_info = RateLimitInfo(
                requests=current_requests + (1 if allowed else 0),
                window_start=window_start,
                window_size=self.window_size,
                limit=self.limit
            )
            
            return allowed, rate_limit_info


class RateLimiter:
    """Rate limiter with multiple algorithms."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self.enabled = settings.enable_rate_limiting
        
        # Rate limit configurations
        self.default_limit = settings.rate_limit_requests
        self.authenticated_limit = settings.rate_limit_authenticated_requests
        self.window_size = settings.rate_limit_window
        
        # Storage for rate limit data
        self._sliding_windows: Dict[str, SlidingWindowCounter] = {}
        self._token_buckets: Dict[str, TokenBucket] = {}
        
        # Cleanup task
        self._cleanup_task: Optional[asyncio.Task] = None
        self._cleanup_interval = 300  # 5 minutes
    
    async def start(self):
        """Start rate limiter background tasks."""
        if self.enabled:
            self._cleanup_task = asyncio.create_task(self._cleanup_loop())
            logger.info("Rate limiter started")
    
    async def stop(self):
        """Stop rate limiter background tasks."""
        if self._cleanup_task:
            self._cleanup_task.cancel()
            try:
                await self._cleanup_task
            except asyncio.CancelledError:
                pass
            logger.info("Rate limiter stopped")
    
    async def _cleanup_loop(self):
        """Background task to cleanup old rate limit data."""
        while True:
            try:
                await asyncio.sleep(self._cleanup_interval)
                await self._cleanup_old_data()
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in rate limiter cleanup: {e}")
    
    async def _cleanup_old_data(self):
        """Remove old rate limit data."""
        now = time.time()
        cutoff = now - (self.window_size * 2)  # Keep data for 2 windows
        
        # Cleanup sliding windows
        keys_to_remove = []
        for key, window in self._sliding_windows.items():
            # Remove old requests
            while window.requests and window.requests[0] < cutoff:
                window.requests.popleft()
            
            # Remove empty windows
            if not window.requests:
                keys_to_remove.append(key)
        
        for key in keys_to_remove:
            del self._sliding_windows[key]
        
        logger.debug(f"Cleaned up {len(keys_to_remove)} old rate limit windows")
    
    def _get_client_identifier(self, request: Request) -> str:
        """Get client identifier for rate limiting."""
        # Try to get user ID from authenticated request
        user = getattr(request.state, "user", None)
        if user:
            return f"user:{user.get('username', 'unknown')}"
        
        # Fall back to IP address
        client_ip = self._get_client_ip(request)
        return f"ip:{client_ip}"
    
    def _get_client_ip(self, request: Request) -> str:
        """Get client IP address."""
        # Check for forwarded headers
        forwarded_for = request.headers.get("X-Forwarded-For")
        if forwarded_for:
            return forwarded_for.split(",")[0].strip()
        
        real_ip = request.headers.get("X-Real-IP")
        if real_ip:
            return real_ip
        
        # Fall back to direct connection
        return request.client.host if request.client else "unknown"
    
    def _get_rate_limit(self, request: Request) -> int:
        """Get rate limit for request."""
        # Check if user is authenticated
        user = getattr(request.state, "user", None)
        if user:
            return self.authenticated_limit
        
        return self.default_limit
    
    def _get_rate_limit_key(self, request: Request) -> str:
        """Get rate limit key for request."""
        client_id = self._get_client_identifier(request)
        endpoint = f"{request.method}:{request.url.path}"
        return f"{client_id}:{endpoint}"
    
    async def check_rate_limit(self, request: Request) -> Tuple[bool, RateLimitInfo]:
        """Check if request is within rate limits."""
        if not self.enabled:
            # Return dummy info when rate limiting is disabled
            return True, RateLimitInfo(
                requests=0,
                window_start=time.time(),
                window_size=self.window_size,
                limit=float('inf')
            )
        
        key = self._get_rate_limit_key(request)
        limit = self._get_rate_limit(request)
        
        # Get or create sliding window counter
        if key not in self._sliding_windows:
            self._sliding_windows[key] = SlidingWindowCounter(self.window_size, limit)
        
        window = self._sliding_windows[key]
        
        # Update limit if it changed (e.g., user authenticated)
        window.limit = limit
        
        return await window.is_allowed()
    
    async def check_token_bucket(self, request: Request, tokens: int = 1) -> bool:
        """Check rate limit using token bucket algorithm."""
        if not self.enabled:
            return True
        
        key = self._get_client_identifier(request)
        limit = self._get_rate_limit(request)
        
        # Get or create token bucket
        if key not in self._token_buckets:
            # Refill rate: limit per window size
            refill_rate = limit / self.window_size
            self._token_buckets[key] = TokenBucket(limit, refill_rate)
        
        bucket = self._token_buckets[key]
        return await bucket.consume(tokens)
    
    def get_rate_limit_headers(self, rate_limit_info: RateLimitInfo) -> Dict[str, str]:
        """Get rate limit headers for response."""
        return {
            "X-RateLimit-Limit": str(rate_limit_info.limit),
            "X-RateLimit-Remaining": str(rate_limit_info.remaining),
            "X-RateLimit-Reset": str(int(rate_limit_info.reset_time)),
            "X-RateLimit-Window": str(rate_limit_info.window_size),
        }
    
    async def get_stats(self) -> Dict[str, Any]:
        """Get rate limiter statistics."""
        return {
            "enabled": self.enabled,
            "default_limit": self.default_limit,
            "authenticated_limit": self.authenticated_limit,
            "window_size": self.window_size,
            "active_windows": len(self._sliding_windows),
            "active_buckets": len(self._token_buckets),
        }


class RateLimitMiddleware:
    """Rate limiting middleware for FastAPI."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self.rate_limiter = RateLimiter(settings)
        self.enabled = settings.enable_rate_limiting
    
    async def __call__(self, request: Request, call_next: Callable) -> Response:
        """Process request through rate limiting middleware."""
        if not self.enabled:
            return await call_next(request)
        
        # Skip rate limiting for certain paths
        if self._should_skip_rate_limit(request):
            return await call_next(request)
        
        try:
            # Check rate limit
            allowed, rate_limit_info = await self.rate_limiter.check_rate_limit(request)
            
            if not allowed:
                # Rate limit exceeded
                logger.warning(
                    f"Rate limit exceeded for {self.rate_limiter._get_client_identifier(request)} "
                    f"on {request.method} {request.url.path}"
                )
                
                headers = self.rate_limiter.get_rate_limit_headers(rate_limit_info)
                headers["Retry-After"] = str(int(rate_limit_info.reset_time - time.time()))
                
                raise HTTPException(
                    status_code=status.HTTP_429_TOO_MANY_REQUESTS,
                    detail="Rate limit exceeded",
                    headers=headers
                )
            
            # Process request
            response = await call_next(request)
            
            # Add rate limit headers to response
            headers = self.rate_limiter.get_rate_limit_headers(rate_limit_info)
            for key, value in headers.items():
                response.headers[key] = value
            
            return response
            
        except HTTPException:
            raise
        except Exception as e:
            logger.error(f"Rate limiting middleware error: {e}")
            # Continue without rate limiting on error
            return await call_next(request)
    
    def _should_skip_rate_limit(self, request: Request) -> bool:
        """Check if rate limiting should be skipped for this request."""
        path = request.url.path
        
        # Skip rate limiting for these paths
        skip_paths = [
            "/health",
            "/metrics",
            "/docs",
            "/redoc",
            "/openapi.json",
            "/static",
        ]
        
        return any(path.startswith(skip_path) for skip_path in skip_paths)
    
    async def start(self):
        """Start rate limiting middleware."""
        await self.rate_limiter.start()
    
    async def stop(self):
        """Stop rate limiting middleware."""
        await self.rate_limiter.stop()


# Global rate limit middleware instance
_rate_limit_middleware: Optional[RateLimitMiddleware] = None


def get_rate_limit_middleware(settings: Settings) -> RateLimitMiddleware:
    """Get rate limit middleware instance."""
    global _rate_limit_middleware
    if _rate_limit_middleware is None:
        _rate_limit_middleware = RateLimitMiddleware(settings)
    return _rate_limit_middleware


def setup_rate_limiting(app: ASGIApp, settings: Settings) -> ASGIApp:
    """Setup rate limiting middleware for the application."""
    if settings.enable_rate_limiting:
        logger.info("Setting up rate limiting middleware")
        
        middleware = get_rate_limit_middleware(settings)
        
        # Add middleware to app
        @app.middleware("http")
        async def rate_limit_middleware(request: Request, call_next):
            return await middleware(request, call_next)
        
        logger.info(
            f"Rate limiting enabled - Default: {settings.rate_limit_requests}/"
            f"{settings.rate_limit_window}s, Authenticated: "
            f"{settings.rate_limit_authenticated_requests}/{settings.rate_limit_window}s"
        )
    else:
        logger.info("Rate limiting disabled")
    
    return app


class RateLimitConfig:
    """Rate limiting configuration helper."""
    
    @staticmethod
    def development_config() -> dict:
        """Get rate limiting configuration for development."""
        return {
            "enable_rate_limiting": False,  # Disabled in development
            "rate_limit_requests": 1000,
            "rate_limit_authenticated_requests": 5000,
            "rate_limit_window": 3600,  # 1 hour
        }
    
    @staticmethod
    def production_config() -> dict:
        """Get rate limiting configuration for production."""
        return {
            "enable_rate_limiting": True,
            "rate_limit_requests": 100,  # 100 requests per hour for unauthenticated
            "rate_limit_authenticated_requests": 1000,  # 1000 requests per hour for authenticated
            "rate_limit_window": 3600,  # 1 hour
        }
    
    @staticmethod
    def api_config() -> dict:
        """Get rate limiting configuration for API access."""
        return {
            "enable_rate_limiting": True,
            "rate_limit_requests": 60,  # 60 requests per minute
            "rate_limit_authenticated_requests": 300,  # 300 requests per minute
            "rate_limit_window": 60,  # 1 minute
        }
    
    @staticmethod
    def strict_config() -> dict:
        """Get strict rate limiting configuration."""
        return {
            "enable_rate_limiting": True,
            "rate_limit_requests": 10,  # 10 requests per minute
            "rate_limit_authenticated_requests": 100,  # 100 requests per minute
            "rate_limit_window": 60,  # 1 minute
        }


def validate_rate_limit_config(settings: Settings) -> list:
    """Validate rate limiting configuration."""
    issues = []
    
    if settings.enable_rate_limiting:
        if settings.rate_limit_requests <= 0:
            issues.append("Rate limit requests must be positive")
        
        if settings.rate_limit_authenticated_requests <= 0:
            issues.append("Authenticated rate limit requests must be positive")
        
        if settings.rate_limit_window <= 0:
            issues.append("Rate limit window must be positive")
        
        if settings.rate_limit_authenticated_requests < settings.rate_limit_requests:
            issues.append("Authenticated rate limit should be higher than default rate limit")
    
    return issues
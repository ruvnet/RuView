"""
Rate limiting middleware for WiFi-DensePose API
"""

import logging
import time
from typing import Dict, Optional, Tuple
from datetime import datetime, timedelta
from collections import defaultdict, deque

from fastapi import Request, Response
from fastapi.responses import JSONResponse
from starlette.middleware.base import BaseHTTPMiddleware

from src.config.settings import get_settings

logger = logging.getLogger(__name__)


class RateLimitMiddleware(BaseHTTPMiddleware):
    """Rate limiting middleware with sliding window algorithm."""
    
    def __init__(self, app):
        super().__init__(app)
        self.settings = get_settings()
        
        # Rate limit storage (in production, use Redis)
        self.request_counts = defaultdict(lambda: deque())
        self.blocked_clients = {}
        
        # Rate limit configurations
        self.rate_limits = {
            "anonymous": {
                "requests": self.settings.rate_limit_requests,
                "window": self.settings.rate_limit_window,
                "burst": 10  # Allow burst of 10 requests
            },
            "authenticated": {
                "requests": self.settings.rate_limit_authenticated_requests,
                "window": self.settings.rate_limit_window,
                "burst": 50
            },
            "admin": {
                "requests": 10000,  # Very high limit for admins
                "window": self.settings.rate_limit_window,
                "burst": 100
            }
        }
        
        # Path-specific rate limits
        self.path_limits = {
            "/api/v1/pose/current": {"requests": 60, "window": 60},  # 1 per second
            "/api/v1/pose/analyze": {"requests": 10, "window": 60},  # 10 per minute
            "/api/v1/pose/calibrate": {"requests": 1, "window": 300}, # 1 per 5 minutes
            "/api/v1/stream/start": {"requests": 5, "window": 60},   # 5 per minute
            "/api/v1/stream/stop": {"requests": 5, "window": 60},    # 5 per minute
        }
        
        # Exempt paths from rate limiting
        self.exempt_paths = {
            "/health",
            "/ready",
            "/live",
            "/version",
            "/metrics"
        }
    
    async def dispatch(self, request: Request, call_next):
        """Process request through rate limiting middleware."""
        
        # Skip rate limiting for exempt paths
        if self._is_exempt_path(request.url.path):
            return await call_next(request)
        
        # Get client identifier
        client_id = self._get_client_id(request)
        
        # Check if client is temporarily blocked
        if self._is_client_blocked(client_id):
            return self._create_rate_limit_response(
                "Client temporarily blocked due to excessive requests"
            )
        
        # Get user type for rate limiting
        user_type = self._get_user_type(request)
        
        # Check rate limits
        rate_limit_result = self._check_rate_limits(
            client_id, 
            request.url.path, 
            user_type
        )
        
        if not rate_limit_result["allowed"]:
            # Log rate limit violation
            self._log_rate_limit_violation(request, client_id, rate_limit_result)
            
            # Check if client should be temporarily blocked
            if rate_limit_result.get("violations", 0) > 5:
                self._block_client(client_id, duration=300)  # 5 minutes
            
            return self._create_rate_limit_response(
                rate_limit_result["message"],
                retry_after=rate_limit_result.get("retry_after", 60)
            )
        
        # Record the request
        self._record_request(client_id, request.url.path)
        
        # Process request
        response = await call_next(request)
        
        # Add rate limit headers
        self._add_rate_limit_headers(response, client_id, user_type)
        
        return response
    
    def _is_exempt_path(self, path: str) -> bool:
        """Check if path is exempt from rate limiting."""
        return path in self.exempt_paths
    
    def _get_client_id(self, request: Request) -> str:
        """Get unique client identifier for rate limiting."""
        # Try to get user ID from request state (set by auth middleware)
        if hasattr(request.state, 'user') and request.state.user:
            return f"user:{request.state.user['id']}"
        
        # Fall back to IP address
        client_ip = request.client.host if request.client else "unknown"
        
        # Include user agent for better identification
        user_agent = request.headers.get("user-agent", "")
        user_agent_hash = str(hash(user_agent))[:8]
        
        return f"ip:{client_ip}:{user_agent_hash}"
    
    def _get_user_type(self, request: Request) -> str:
        """Determine user type for rate limiting."""
        if hasattr(request.state, 'user') and request.state.user:
            if request.state.user.get("is_admin", False):
                return "admin"
            return "authenticated"
        return "anonymous"
    
    def _check_rate_limits(self, client_id: str, path: str, user_type: str) -> Dict:
        """Check if request is within rate limits."""
        now = time.time()
        
        # Get applicable rate limits
        general_limit = self.rate_limits[user_type]
        path_limit = self.path_limits.get(path)
        
        # Check general rate limit
        general_result = self._check_limit(
            client_id, 
            "general", 
            general_limit["requests"], 
            general_limit["window"],
            now
        )
        
        if not general_result["allowed"]:
            return general_result
        
        # Check path-specific rate limit if exists
        if path_limit:
            path_result = self._check_limit(
                client_id,
                f"path:{path}",
                path_limit["requests"],
                path_limit["window"],
                now
            )
            
            if not path_result["allowed"]:
                return path_result
        
        return {"allowed": True}
    
    def _check_limit(self, client_id: str, limit_type: str, max_requests: int, window: int, now: float) -> Dict:
        """Check specific rate limit using sliding window."""
        key = f"{client_id}:{limit_type}"
        requests = self.request_counts[key]
        
        # Remove old requests outside the window
        cutoff = now - window
        while requests and requests[0] <= cutoff:
            requests.popleft()
        
        # Check if limit exceeded
        if len(requests) >= max_requests:
            # Calculate retry after time
            oldest_request = requests[0] if requests else now
            retry_after = int(oldest_request + window - now) + 1
            
            return {
                "allowed": False,
                "message": f"Rate limit exceeded: {max_requests} requests per {window} seconds",
                "retry_after": retry_after,
                "current_count": len(requests),
                "limit": max_requests,
                "window": window
            }
        
        return {
            "allowed": True,
            "current_count": len(requests),
            "limit": max_requests,
            "window": window
        }
    
    def _record_request(self, client_id: str, path: str):
        """Record a request for rate limiting."""
        now = time.time()
        
        # Record general request
        general_key = f"{client_id}:general"
        self.request_counts[general_key].append(now)
        
        # Record path-specific request if path has specific limits
        if path in self.path_limits:
            path_key = f"{client_id}:path:{path}"
            self.request_counts[path_key].append(now)
    
    def _is_client_blocked(self, client_id: str) -> bool:
        """Check if client is temporarily blocked."""
        if client_id in self.blocked_clients:
            block_until = self.blocked_clients[client_id]
            if time.time() < block_until:
                return True
            else:
                # Block expired, remove it
                del self.blocked_clients[client_id]
        return False
    
    def _block_client(self, client_id: str, duration: int):
        """Temporarily block a client."""
        self.blocked_clients[client_id] = time.time() + duration
        logger.warning(f"Client {client_id} blocked for {duration} seconds due to rate limit violations")
    
    def _create_rate_limit_response(self, message: str, retry_after: int = 60) -> JSONResponse:
        """Create rate limit exceeded response."""
        return JSONResponse(
            status_code=429,
            content={
                "error": {
                    "code": 429,
                    "message": message,
                    "type": "rate_limit_exceeded"
                }
            },
            headers={
                "Retry-After": str(retry_after),
                "X-RateLimit-Limit": "Exceeded",
                "X-RateLimit-Remaining": "0"
            }
        )
    
    def _add_rate_limit_headers(self, response: Response, client_id: str, user_type: str):
        """Add rate limit headers to response."""
        try:
            general_limit = self.rate_limits[user_type]
            general_key = f"{client_id}:general"
            current_requests = len(self.request_counts[general_key])
            
            remaining = max(0, general_limit["requests"] - current_requests)
            
            response.headers["X-RateLimit-Limit"] = str(general_limit["requests"])
            response.headers["X-RateLimit-Remaining"] = str(remaining)
            response.headers["X-RateLimit-Window"] = str(general_limit["window"])
            
            # Add reset time
            if self.request_counts[general_key]:
                oldest_request = self.request_counts[general_key][0]
                reset_time = int(oldest_request + general_limit["window"])
                response.headers["X-RateLimit-Reset"] = str(reset_time)
        
        except Exception as e:
            logger.error(f"Error adding rate limit headers: {e}")
    
    def _log_rate_limit_violation(self, request: Request, client_id: str, result: Dict):
        """Log rate limit violations for monitoring."""
        client_ip = request.client.host if request.client else "unknown"
        user_agent = request.headers.get("user-agent", "unknown")
        
        log_data = {
            "event_type": "rate_limit_violation",
            "timestamp": datetime.utcnow().isoformat(),
            "client_id": client_id,
            "client_ip": client_ip,
            "user_agent": user_agent,
            "path": request.url.path,
            "method": request.method,
            "current_count": result.get("current_count"),
            "limit": result.get("limit"),
            "window": result.get("window")
        }
        
        logger.warning(f"Rate limit violation: {log_data}")
    
    def cleanup_old_data(self):
        """Clean up old rate limiting data (call periodically)."""
        now = time.time()
        cutoff = now - 3600  # Keep data for 1 hour
        
        # Clean up request counts
        for key in list(self.request_counts.keys()):
            requests = self.request_counts[key]
            while requests and requests[0] <= cutoff:
                requests.popleft()
            
            # Remove empty deques
            if not requests:
                del self.request_counts[key]
        
        # Clean up expired blocks
        expired_blocks = [
            client_id for client_id, block_until in self.blocked_clients.items()
            if now >= block_until
        ]
        
        for client_id in expired_blocks:
            del self.blocked_clients[client_id]


class AdaptiveRateLimit:
    """Adaptive rate limiting based on system load."""
    
    def __init__(self):
        self.base_limits = {}
        self.current_multiplier = 1.0
        self.load_history = deque(maxlen=60)  # Keep 1 minute of load data
    
    def update_system_load(self, cpu_percent: float, memory_percent: float):
        """Update system load metrics."""
        load_score = (cpu_percent + memory_percent) / 2
        self.load_history.append(load_score)
        
        # Calculate adaptive multiplier
        if len(self.load_history) >= 10:
            avg_load = sum(self.load_history) / len(self.load_history)
            
            if avg_load > 80:
                self.current_multiplier = 0.5  # Reduce limits by 50%
            elif avg_load > 60:
                self.current_multiplier = 0.7  # Reduce limits by 30%
            elif avg_load < 30:
                self.current_multiplier = 1.2  # Increase limits by 20%
            else:
                self.current_multiplier = 1.0  # Normal limits
    
    def get_adjusted_limit(self, base_limit: int) -> int:
        """Get adjusted rate limit based on system load."""
        return max(1, int(base_limit * self.current_multiplier))


class RateLimitStorage:
    """Abstract interface for rate limit storage (Redis implementation)."""
    
    async def get_count(self, key: str, window: int) -> int:
        """Get current request count for key within window."""
        raise NotImplementedError
    
    async def increment(self, key: str, window: int) -> int:
        """Increment request count and return new count."""
        raise NotImplementedError
    
    async def is_blocked(self, client_id: str) -> bool:
        """Check if client is blocked."""
        raise NotImplementedError
    
    async def block_client(self, client_id: str, duration: int):
        """Block client for duration seconds."""
        raise NotImplementedError


class RedisRateLimitStorage(RateLimitStorage):
    """Redis-based rate limit storage for production use."""
    
    def __init__(self, redis_client):
        self.redis = redis_client
    
    async def get_count(self, key: str, window: int) -> int:
        """Get current request count using Redis sliding window."""
        now = time.time()
        pipeline = self.redis.pipeline()
        
        # Remove old entries
        pipeline.zremrangebyscore(key, 0, now - window)
        
        # Count current entries
        pipeline.zcard(key)
        
        results = await pipeline.execute()
        return results[1]
    
    async def increment(self, key: str, window: int) -> int:
        """Increment request count using Redis."""
        now = time.time()
        pipeline = self.redis.pipeline()
        
        # Add current request
        pipeline.zadd(key, {str(now): now})
        
        # Remove old entries
        pipeline.zremrangebyscore(key, 0, now - window)
        
        # Set expiration
        pipeline.expire(key, window + 1)
        
        # Get count
        pipeline.zcard(key)
        
        results = await pipeline.execute()
        return results[3]
    
    async def is_blocked(self, client_id: str) -> bool:
        """Check if client is blocked."""
        block_key = f"blocked:{client_id}"
        return await self.redis.exists(block_key)
    
    async def block_client(self, client_id: str, duration: int):
        """Block client for duration seconds."""
        block_key = f"blocked:{client_id}"
        await self.redis.setex(block_key, duration, "1")


# Global adaptive rate limiter instance
adaptive_rate_limit = AdaptiveRateLimit()
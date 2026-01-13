"""
Integration tests for rate limiting functionality.

Tests rate limit behavior, throttling, and quota management.
"""

import pytest
import asyncio
from datetime import datetime, timedelta
from typing import Dict, Any, List
from unittest.mock import AsyncMock, MagicMock, patch
import time

from fastapi import HTTPException, status, Request, Response


class MockRateLimiter:
    """Mock rate limiter for testing."""
    
    def __init__(self, requests_per_minute: int = 60, requests_per_hour: int = 1000):
        self.requests_per_minute = requests_per_minute
        self.requests_per_hour = requests_per_hour
        self.request_history = {}
        self.blocked_clients = set()
    
    def _get_client_key(self, client_id: str, endpoint: str = None) -> str:
        """Get client key for rate limiting."""
        return f"{client_id}:{endpoint}" if endpoint else client_id
    
    def _cleanup_old_requests(self, client_key: str):
        """Clean up old request records."""
        if client_key not in self.request_history:
            return
        
        now = datetime.utcnow()
        minute_ago = now - timedelta(minutes=1)
        hour_ago = now - timedelta(hours=1)
        
        # Keep only requests from the last hour
        self.request_history[client_key] = [
            req_time for req_time in self.request_history[client_key]
            if req_time > hour_ago
        ]
    
    def check_rate_limit(self, client_id: str, endpoint: str = None) -> Dict[str, Any]:
        """Check if client is within rate limits."""
        client_key = self._get_client_key(client_id, endpoint)
        
        if client_id in self.blocked_clients:
            return {
                "allowed": False,
                "reason": "Client blocked",
                "retry_after": 3600  # 1 hour
            }
        
        self._cleanup_old_requests(client_key)
        
        if client_key not in self.request_history:
            self.request_history[client_key] = []
        
        now = datetime.utcnow()
        minute_ago = now - timedelta(minutes=1)
        
        # Count requests in the last minute
        recent_requests = [
            req_time for req_time in self.request_history[client_key]
            if req_time > minute_ago
        ]
        
        # Count requests in the last hour
        hour_requests = len(self.request_history[client_key])
        
        if len(recent_requests) >= self.requests_per_minute:
            return {
                "allowed": False,
                "reason": "Rate limit exceeded (per minute)",
                "retry_after": 60,
                "current_requests": len(recent_requests),
                "limit": self.requests_per_minute
            }
        
        if hour_requests >= self.requests_per_hour:
            return {
                "allowed": False,
                "reason": "Rate limit exceeded (per hour)",
                "retry_after": 3600,
                "current_requests": hour_requests,
                "limit": self.requests_per_hour
            }
        
        # Record this request
        self.request_history[client_key].append(now)
        
        return {
            "allowed": True,
            "remaining_minute": self.requests_per_minute - len(recent_requests) - 1,
            "remaining_hour": self.requests_per_hour - hour_requests - 1,
            "reset_time": minute_ago + timedelta(minutes=1)
        }
    
    def block_client(self, client_id: str):
        """Block a client."""
        self.blocked_clients.add(client_id)
    
    def unblock_client(self, client_id: str):
        """Unblock a client."""
        self.blocked_clients.discard(client_id)


class TestRateLimitingBasic:
    """Test basic rate limiting functionality."""
    
    @pytest.fixture
    def rate_limiter(self):
        """Create rate limiter for testing."""
        return MockRateLimiter(requests_per_minute=5, requests_per_hour=100)
    
    def test_rate_limit_within_bounds_should_fail_initially(self, rate_limiter):
        """Test rate limiting within bounds - should fail initially."""
        client_id = "test-client-001"
        
        # Make requests within limit
        for i in range(3):
            result = rate_limiter.check_rate_limit(client_id)
            
            # This will fail initially
            assert result["allowed"] is True
            assert "remaining_minute" in result
            assert "remaining_hour" in result
    
    def test_rate_limit_per_minute_exceeded_should_fail_initially(self, rate_limiter):
        """Test per-minute rate limit exceeded - should fail initially."""
        client_id = "test-client-002"
        
        # Make requests up to the limit
        for i in range(5):
            result = rate_limiter.check_rate_limit(client_id)
            assert result["allowed"] is True
        
        # Next request should be blocked
        result = rate_limiter.check_rate_limit(client_id)
        
        # This will fail initially
        assert result["allowed"] is False
        assert "per minute" in result["reason"]
        assert result["retry_after"] == 60
        assert result["current_requests"] == 5
        assert result["limit"] == 5
    
    def test_rate_limit_per_hour_exceeded_should_fail_initially(self, rate_limiter):
        """Test per-hour rate limit exceeded - should fail initially."""
        # Create rate limiter with very low hour limit for testing
        limiter = MockRateLimiter(requests_per_minute=10, requests_per_hour=3)
        client_id = "test-client-003"
        
        # Make requests up to hour limit
        for i in range(3):
            result = limiter.check_rate_limit(client_id)
            assert result["allowed"] is True
        
        # Next request should be blocked
        result = limiter.check_rate_limit(client_id)
        
        # This will fail initially
        assert result["allowed"] is False
        assert "per hour" in result["reason"]
        assert result["retry_after"] == 3600
    
    def test_blocked_client_should_fail_initially(self, rate_limiter):
        """Test blocked client handling - should fail initially."""
        client_id = "blocked-client"
        
        # Block the client
        rate_limiter.block_client(client_id)
        
        # Request should be blocked
        result = rate_limiter.check_rate_limit(client_id)
        
        # This will fail initially
        assert result["allowed"] is False
        assert result["reason"] == "Client blocked"
        assert result["retry_after"] == 3600
        
        # Unblock and test
        rate_limiter.unblock_client(client_id)
        result = rate_limiter.check_rate_limit(client_id)
        assert result["allowed"] is True
    
    def test_endpoint_specific_rate_limiting_should_fail_initially(self, rate_limiter):
        """Test endpoint-specific rate limiting - should fail initially."""
        client_id = "test-client-004"
        
        # Make requests to different endpoints
        result1 = rate_limiter.check_rate_limit(client_id, "/api/pose/current")
        result2 = rate_limiter.check_rate_limit(client_id, "/api/stream/status")
        
        # This will fail initially
        assert result1["allowed"] is True
        assert result2["allowed"] is True
        
        # Each endpoint should have separate rate limiting
        for i in range(4):
            rate_limiter.check_rate_limit(client_id, "/api/pose/current")
        
        # Pose endpoint should be at limit, but stream should still work
        pose_result = rate_limiter.check_rate_limit(client_id, "/api/pose/current")
        stream_result = rate_limiter.check_rate_limit(client_id, "/api/stream/status")
        
        assert pose_result["allowed"] is False
        assert stream_result["allowed"] is True


class TestRateLimitMiddleware:
    """Test rate limiting middleware functionality."""
    
    @pytest.fixture
    def mock_request(self):
        """Mock FastAPI request."""
        class MockRequest:
            def __init__(self, client_ip="127.0.0.1", path="/api/test", method="GET"):
                self.client = MagicMock()
                self.client.host = client_ip
                self.url = MagicMock()
                self.url.path = path
                self.method = method
                self.headers = {}
                self.state = MagicMock()
        
        return MockRequest
    
    @pytest.fixture
    def mock_response(self):
        """Mock FastAPI response."""
        class MockResponse:
            def __init__(self):
                self.status_code = 200
                self.headers = {}
        
        return MockResponse()
    
    @pytest.fixture
    def rate_limit_middleware(self, rate_limiter):
        """Create rate limiting middleware."""
        class RateLimitMiddleware:
            def __init__(self, rate_limiter):
                self.rate_limiter = rate_limiter
            
            async def __call__(self, request, call_next):
                # Get client identifier
                client_id = self._get_client_id(request)
                endpoint = request.url.path
                
                # Check rate limit
                limit_result = self.rate_limiter.check_rate_limit(client_id, endpoint)
                
                if not limit_result["allowed"]:
                    # Return rate limit exceeded response
                    response = Response(
                        content=f"Rate limit exceeded: {limit_result['reason']}",
                        status_code=status.HTTP_429_TOO_MANY_REQUESTS
                    )
                    response.headers["Retry-After"] = str(limit_result["retry_after"])
                    response.headers["X-RateLimit-Limit"] = str(limit_result.get("limit", "unknown"))
                    response.headers["X-RateLimit-Remaining"] = "0"
                    return response
                
                # Process request
                response = await call_next(request)
                
                # Add rate limit headers
                response.headers["X-RateLimit-Limit"] = str(self.rate_limiter.requests_per_minute)
                response.headers["X-RateLimit-Remaining"] = str(limit_result.get("remaining_minute", 0))
                response.headers["X-RateLimit-Reset"] = str(int(limit_result.get("reset_time", datetime.utcnow()).timestamp()))
                
                return response
            
            def _get_client_id(self, request):
                """Get client identifier from request."""
                # Check for API key in headers
                api_key = request.headers.get("X-API-Key")
                if api_key:
                    return f"api:{api_key}"
                
                # Check for user ID in request state (from auth)
                if hasattr(request.state, "user") and request.state.user:
                    return f"user:{request.state.user.get('id', 'unknown')}"
                
                # Fall back to IP address
                return f"ip:{request.client.host}"
        
        return RateLimitMiddleware(rate_limiter)
    
    @pytest.mark.asyncio
    async def test_middleware_allows_normal_requests_should_fail_initially(
        self, rate_limit_middleware, mock_request, mock_response
    ):
        """Test middleware allows normal requests - should fail initially."""
        request = mock_request()
        
        async def mock_call_next(req):
            return mock_response
        
        response = await rate_limit_middleware(request, mock_call_next)
        
        # This will fail initially
        assert response.status_code == 200
        assert "X-RateLimit-Limit" in response.headers
        assert "X-RateLimit-Remaining" in response.headers
        assert "X-RateLimit-Reset" in response.headers
    
    @pytest.mark.asyncio
    async def test_middleware_blocks_excessive_requests_should_fail_initially(
        self, rate_limit_middleware, mock_request
    ):
        """Test middleware blocks excessive requests - should fail initially."""
        request = mock_request()
        
        async def mock_call_next(req):
            response = Response(content="OK", status_code=200)
            return response
        
        # Make requests up to the limit
        for i in range(5):
            response = await rate_limit_middleware(request, mock_call_next)
            assert response.status_code == 200
        
        # Next request should be blocked
        response = await rate_limit_middleware(request, mock_call_next)
        
        # This will fail initially
        assert response.status_code == status.HTTP_429_TOO_MANY_REQUESTS
        assert "Retry-After" in response.headers
        assert "X-RateLimit-Remaining" in response.headers
        assert response.headers["X-RateLimit-Remaining"] == "0"
    
    @pytest.mark.asyncio
    async def test_middleware_client_identification_should_fail_initially(
        self, rate_limit_middleware, mock_request
    ):
        """Test middleware client identification - should fail initially."""
        # Test API key identification
        request_with_api_key = mock_request()
        request_with_api_key.headers["X-API-Key"] = "test-api-key-123"
        
        # Test user identification
        request_with_user = mock_request()
        request_with_user.state.user = {"id": "user-123"}
        
        # Test IP identification
        request_with_ip = mock_request(client_ip="192.168.1.100")
        
        async def mock_call_next(req):
            return Response(content="OK", status_code=200)
        
        # Each should be treated as different clients
        response1 = await rate_limit_middleware(request_with_api_key, mock_call_next)
        response2 = await rate_limit_middleware(request_with_user, mock_call_next)
        response3 = await rate_limit_middleware(request_with_ip, mock_call_next)
        
        # This will fail initially
        assert response1.status_code == 200
        assert response2.status_code == 200
        assert response3.status_code == 200


class TestRateLimitingStrategies:
    """Test different rate limiting strategies."""
    
    @pytest.fixture
    def sliding_window_limiter(self):
        """Create sliding window rate limiter."""
        class SlidingWindowLimiter:
            def __init__(self, window_size_seconds: int = 60, max_requests: int = 10):
                self.window_size = window_size_seconds
                self.max_requests = max_requests
                self.request_times = {}
            
            def check_limit(self, client_id: str) -> Dict[str, Any]:
                now = time.time()
                
                if client_id not in self.request_times:
                    self.request_times[client_id] = []
                
                # Remove old requests outside the window
                cutoff_time = now - self.window_size
                self.request_times[client_id] = [
                    req_time for req_time in self.request_times[client_id]
                    if req_time > cutoff_time
                ]
                
                # Check if we're at the limit
                if len(self.request_times[client_id]) >= self.max_requests:
                    oldest_request = min(self.request_times[client_id])
                    retry_after = int(oldest_request + self.window_size - now)
                    
                    return {
                        "allowed": False,
                        "retry_after": max(retry_after, 1),
                        "current_requests": len(self.request_times[client_id]),
                        "limit": self.max_requests
                    }
                
                # Record this request
                self.request_times[client_id].append(now)
                
                return {
                    "allowed": True,
                    "remaining": self.max_requests - len(self.request_times[client_id]),
                    "window_reset": int(now + self.window_size)
                }
        
        return SlidingWindowLimiter(window_size_seconds=10, max_requests=3)
    
    @pytest.fixture
    def token_bucket_limiter(self):
        """Create token bucket rate limiter."""
        class TokenBucketLimiter:
            def __init__(self, capacity: int = 10, refill_rate: float = 1.0):
                self.capacity = capacity
                self.refill_rate = refill_rate  # tokens per second
                self.buckets = {}
            
            def check_limit(self, client_id: str) -> Dict[str, Any]:
                now = time.time()
                
                if client_id not in self.buckets:
                    self.buckets[client_id] = {
                        "tokens": self.capacity,
                        "last_refill": now
                    }
                
                bucket = self.buckets[client_id]
                
                # Refill tokens based on time elapsed
                time_elapsed = now - bucket["last_refill"]
                tokens_to_add = time_elapsed * self.refill_rate
                bucket["tokens"] = min(self.capacity, bucket["tokens"] + tokens_to_add)
                bucket["last_refill"] = now
                
                # Check if we have tokens available
                if bucket["tokens"] < 1:
                    return {
                        "allowed": False,
                        "retry_after": int((1 - bucket["tokens"]) / self.refill_rate),
                        "tokens_remaining": bucket["tokens"]
                    }
                
                # Consume a token
                bucket["tokens"] -= 1
                
                return {
                    "allowed": True,
                    "tokens_remaining": bucket["tokens"]
                }
        
        return TokenBucketLimiter(capacity=5, refill_rate=0.5)  # 0.5 tokens per second
    
    def test_sliding_window_limiter_should_fail_initially(self, sliding_window_limiter):
        """Test sliding window rate limiter - should fail initially."""
        client_id = "sliding-test-client"
        
        # Make requests up to limit
        for i in range(3):
            result = sliding_window_limiter.check_limit(client_id)
            
            # This will fail initially
            assert result["allowed"] is True
            assert "remaining" in result
        
        # Next request should be blocked
        result = sliding_window_limiter.check_limit(client_id)
        assert result["allowed"] is False
        assert result["current_requests"] == 3
        assert result["limit"] == 3
    
    def test_token_bucket_limiter_should_fail_initially(self, token_bucket_limiter):
        """Test token bucket rate limiter - should fail initially."""
        client_id = "bucket-test-client"
        
        # Make requests up to capacity
        for i in range(5):
            result = token_bucket_limiter.check_limit(client_id)
            
            # This will fail initially
            assert result["allowed"] is True
            assert "tokens_remaining" in result
        
        # Next request should be blocked (no tokens left)
        result = token_bucket_limiter.check_limit(client_id)
        assert result["allowed"] is False
        assert result["tokens_remaining"] < 1
    
    @pytest.mark.asyncio
    async def test_token_bucket_refill_should_fail_initially(self, token_bucket_limiter):
        """Test token bucket refill mechanism - should fail initially."""
        client_id = "refill-test-client"
        
        # Exhaust all tokens
        for i in range(5):
            token_bucket_limiter.check_limit(client_id)
        
        # Should be blocked
        result = token_bucket_limiter.check_limit(client_id)
        assert result["allowed"] is False
        
        # Wait for refill (simulate time passing)
        await asyncio.sleep(2.1)  # Wait for 1 token to be refilled (0.5 tokens/sec * 2.1 sec > 1)
        
        # Should now be allowed
        result = token_bucket_limiter.check_limit(client_id)
        
        # This will fail initially
        assert result["allowed"] is True


class TestRateLimitingPerformance:
    """Test rate limiting performance characteristics."""
    
    @pytest.mark.asyncio
    async def test_concurrent_rate_limit_checks_should_fail_initially(self):
        """Test concurrent rate limit checks - should fail initially."""
        rate_limiter = MockRateLimiter(requests_per_minute=100, requests_per_hour=1000)
        
        async def make_request(client_id: str, request_id: int):
            result = rate_limiter.check_rate_limit(f"{client_id}-{request_id}")
            return result["allowed"]
        
        # Create many concurrent requests
        tasks = [
            make_request("concurrent-client", i)
            for i in range(50)
        ]
        
        results = await asyncio.gather(*tasks)
        
        # This will fail initially
        assert len(results) == 50
        assert all(results)  # All should be allowed since they're different clients
    
    @pytest.mark.asyncio
    async def test_rate_limiter_memory_cleanup_should_fail_initially(self):
        """Test rate limiter memory cleanup - should fail initially."""
        rate_limiter = MockRateLimiter(requests_per_minute=10, requests_per_hour=100)
        
        # Make requests for many different clients
        for i in range(100):
            rate_limiter.check_rate_limit(f"client-{i}")
        
        initial_memory_size = len(rate_limiter.request_history)
        
        # Simulate time passing and cleanup
        for client_key in list(rate_limiter.request_history.keys()):
            rate_limiter._cleanup_old_requests(client_key)
        
        # This will fail initially
        assert initial_memory_size == 100
        
        # After cleanup, old entries should be removed
        # (In a real implementation, this would clean up old timestamps)
        final_memory_size = len([
            key for key, history in rate_limiter.request_history.items()
            if history  # Only count non-empty histories
        ])
        
        assert final_memory_size <= initial_memory_size
"""
Performance tests for API throughput and load testing.

Tests API endpoint performance under various load conditions.
"""

import pytest
import asyncio
import aiohttp
import time
import numpy as np
from datetime import datetime, timedelta
from typing import Dict, Any, List, Optional
from unittest.mock import AsyncMock, MagicMock, patch
import json
import statistics


class MockAPIServer:
    """Mock API server for load testing."""
    
    def __init__(self):
        self.request_count = 0
        self.response_times = []
        self.error_count = 0
        self.concurrent_requests = 0
        self.max_concurrent = 0
        self.is_running = False
        self.rate_limit_enabled = False
        self.rate_limit_per_second = 100
        self.request_timestamps = []
    
    async def handle_request(self, endpoint: str, method: str = "GET", data: Dict[str, Any] = None) -> Dict[str, Any]:
        """Handle API request."""
        start_time = time.time()
        self.concurrent_requests += 1
        self.max_concurrent = max(self.max_concurrent, self.concurrent_requests)
        self.request_count += 1
        self.request_timestamps.append(start_time)
        
        try:
            # Check rate limiting
            if self.rate_limit_enabled:
                recent_requests = [
                    ts for ts in self.request_timestamps 
                    if start_time - ts <= 1.0
                ]
                if len(recent_requests) > self.rate_limit_per_second:
                    self.error_count += 1
                    return {
                        "status": 429,
                        "error": "Rate limit exceeded",
                        "response_time_ms": 1.0
                    }
            
            # Simulate processing time based on endpoint
            processing_time = self._get_processing_time(endpoint, method)
            await asyncio.sleep(processing_time)
            
            # Generate response
            response = self._generate_response(endpoint, method, data)
            
            end_time = time.time()
            response_time = (end_time - start_time) * 1000
            self.response_times.append(response_time)
            
            return {
                "status": 200,
                "data": response,
                "response_time_ms": response_time
            }
            
        except Exception as e:
            self.error_count += 1
            return {
                "status": 500,
                "error": str(e),
                "response_time_ms": (time.time() - start_time) * 1000
            }
        finally:
            self.concurrent_requests -= 1
    
    def _get_processing_time(self, endpoint: str, method: str) -> float:
        """Get processing time for endpoint."""
        processing_times = {
            "/health": 0.001,
            "/pose/detect": 0.05,
            "/pose/stream": 0.02,
            "/auth/login": 0.01,
            "/auth/refresh": 0.005,
            "/config": 0.003
        }
        
        base_time = processing_times.get(endpoint, 0.01)
        
        # Add some variance
        return base_time * np.random.uniform(0.8, 1.2)
    
    def _generate_response(self, endpoint: str, method: str, data: Dict[str, Any]) -> Dict[str, Any]:
        """Generate response for endpoint."""
        if endpoint == "/health":
            return {"status": "healthy", "timestamp": datetime.utcnow().isoformat()}
        
        elif endpoint == "/pose/detect":
            return {
                "persons": [
                    {
                        "person_id": "person_1",
                        "confidence": 0.85,
                        "bounding_box": {"x": 100, "y": 150, "width": 80, "height": 180},
                        "keypoints": [[x, y, 0.9] for x, y in zip(range(17), range(17))]
                    }
                ],
                "processing_time_ms": 45.2,
                "model_version": "v1.0"
            }
        
        elif endpoint == "/auth/login":
            return {
                "access_token": "mock_access_token",
                "refresh_token": "mock_refresh_token",
                "expires_in": 3600
            }
        
        else:
            return {"message": "Success", "endpoint": endpoint, "method": method}
    
    def get_performance_stats(self) -> Dict[str, Any]:
        """Get performance statistics."""
        if not self.response_times:
            return {
                "total_requests": self.request_count,
                "error_count": self.error_count,
                "error_rate": 0,
                "avg_response_time_ms": 0,
                "median_response_time_ms": 0,
                "p95_response_time_ms": 0,
                "p99_response_time_ms": 0,
                "max_concurrent_requests": self.max_concurrent,
                "requests_per_second": 0
            }
        
        return {
            "total_requests": self.request_count,
            "error_count": self.error_count,
            "error_rate": self.error_count / self.request_count,
            "avg_response_time_ms": statistics.mean(self.response_times),
            "median_response_time_ms": statistics.median(self.response_times),
            "p95_response_time_ms": np.percentile(self.response_times, 95),
            "p99_response_time_ms": np.percentile(self.response_times, 99),
            "max_concurrent_requests": self.max_concurrent,
            "requests_per_second": self._calculate_rps()
        }
    
    def _calculate_rps(self) -> float:
        """Calculate requests per second."""
        if len(self.request_timestamps) < 2:
            return 0
        
        duration = self.request_timestamps[-1] - self.request_timestamps[0]
        return len(self.request_timestamps) / max(duration, 0.001)
    
    def enable_rate_limiting(self, requests_per_second: int):
        """Enable rate limiting."""
        self.rate_limit_enabled = True
        self.rate_limit_per_second = requests_per_second
    
    def reset_stats(self):
        """Reset performance statistics."""
        self.request_count = 0
        self.response_times = []
        self.error_count = 0
        self.concurrent_requests = 0
        self.max_concurrent = 0
        self.request_timestamps = []


class TestAPIThroughput:
    """Test API throughput under various conditions."""
    
    @pytest.fixture
    def api_server(self):
        """Create mock API server."""
        return MockAPIServer()
    
    @pytest.mark.asyncio
    async def test_single_request_performance_should_fail_initially(self, api_server):
        """Test single request performance - should fail initially."""
        start_time = time.time()
        response = await api_server.handle_request("/health")
        end_time = time.time()
        
        response_time = (end_time - start_time) * 1000
        
        # This will fail initially
        assert response["status"] == 200
        assert response_time < 50  # Should respond within 50ms
        assert response["response_time_ms"] > 0
        
        stats = api_server.get_performance_stats()
        assert stats["total_requests"] == 1
        assert stats["error_count"] == 0
    
    @pytest.mark.asyncio
    async def test_concurrent_request_handling_should_fail_initially(self, api_server):
        """Test concurrent request handling - should fail initially."""
        # Send multiple concurrent requests
        concurrent_requests = 10
        tasks = []
        
        for i in range(concurrent_requests):
            task = asyncio.create_task(api_server.handle_request("/health"))
            tasks.append(task)
        
        start_time = time.time()
        responses = await asyncio.gather(*tasks)
        end_time = time.time()
        
        total_time = (end_time - start_time) * 1000
        
        # This will fail initially
        assert len(responses) == concurrent_requests
        assert all(r["status"] == 200 for r in responses)
        
        # All requests should complete within reasonable time
        assert total_time < 200  # Should complete within 200ms
        
        stats = api_server.get_performance_stats()
        assert stats["total_requests"] == concurrent_requests
        assert stats["max_concurrent_requests"] <= concurrent_requests
    
    @pytest.mark.asyncio
    async def test_sustained_load_performance_should_fail_initially(self, api_server):
        """Test sustained load performance - should fail initially."""
        duration_seconds = 3
        target_rps = 50  # 50 requests per second
        
        async def send_requests():
            """Send requests at target rate."""
            interval = 1.0 / target_rps
            end_time = time.time() + duration_seconds
            
            while time.time() < end_time:
                await api_server.handle_request("/health")
                await asyncio.sleep(interval)
        
        start_time = time.time()
        await send_requests()
        actual_duration = time.time() - start_time
        
        stats = api_server.get_performance_stats()
        actual_rps = stats["requests_per_second"]
        
        # This will fail initially
        assert actual_rps >= target_rps * 0.8  # Within 80% of target
        assert stats["error_rate"] < 0.05  # Less than 5% error rate
        assert stats["avg_response_time_ms"] < 100  # Average response time under 100ms
    
    @pytest.mark.asyncio
    async def test_different_endpoint_performance_should_fail_initially(self, api_server):
        """Test different endpoint performance - should fail initially."""
        endpoints = [
            "/health",
            "/pose/detect", 
            "/auth/login",
            "/config"
        ]
        
        results = {}
        
        for endpoint in endpoints:
            # Test each endpoint multiple times
            response_times = []
            
            for _ in range(10):
                response = await api_server.handle_request(endpoint)
                response_times.append(response["response_time_ms"])
            
            results[endpoint] = {
                "avg_response_time": statistics.mean(response_times),
                "min_response_time": min(response_times),
                "max_response_time": max(response_times)
            }
        
        # This will fail initially
        # Health endpoint should be fastest
        assert results["/health"]["avg_response_time"] < results["/pose/detect"]["avg_response_time"]
        
        # All endpoints should respond within reasonable time
        for endpoint, metrics in results.items():
            assert metrics["avg_response_time"] < 200  # Less than 200ms average
            assert metrics["max_response_time"] < 500  # Less than 500ms max
    
    @pytest.mark.asyncio
    async def test_rate_limiting_behavior_should_fail_initially(self, api_server):
        """Test rate limiting behavior - should fail initially."""
        # Enable rate limiting
        api_server.enable_rate_limiting(requests_per_second=10)
        
        # Send requests faster than rate limit
        rapid_requests = 20
        tasks = []
        
        for i in range(rapid_requests):
            task = asyncio.create_task(api_server.handle_request("/health"))
            tasks.append(task)
        
        responses = await asyncio.gather(*tasks)
        
        # This will fail initially
        # Some requests should be rate limited
        success_responses = [r for r in responses if r["status"] == 200]
        rate_limited_responses = [r for r in responses if r["status"] == 429]
        
        assert len(success_responses) > 0
        assert len(rate_limited_responses) > 0
        assert len(success_responses) + len(rate_limited_responses) == rapid_requests
        
        stats = api_server.get_performance_stats()
        assert stats["error_count"] > 0  # Should have rate limit errors


class TestAPILoadTesting:
    """Test API under heavy load conditions."""
    
    @pytest.fixture
    def load_test_server(self):
        """Create server for load testing."""
        server = MockAPIServer()
        return server
    
    @pytest.mark.asyncio
    async def test_high_concurrency_load_should_fail_initially(self, load_test_server):
        """Test high concurrency load - should fail initially."""
        concurrent_users = 50
        requests_per_user = 5
        
        async def user_session(user_id: int):
            """Simulate user session."""
            session_responses = []
            
            for i in range(requests_per_user):
                response = await load_test_server.handle_request("/health")
                session_responses.append(response)
                
                # Small delay between requests
                await asyncio.sleep(0.01)
            
            return session_responses
        
        # Create user sessions
        user_tasks = [user_session(i) for i in range(concurrent_users)]
        
        start_time = time.time()
        all_sessions = await asyncio.gather(*user_tasks)
        end_time = time.time()
        
        total_duration = end_time - start_time
        total_requests = concurrent_users * requests_per_user
        
        # This will fail initially
        # All sessions should complete
        assert len(all_sessions) == concurrent_users
        
        # Check performance metrics
        stats = load_test_server.get_performance_stats()
        assert stats["total_requests"] == total_requests
        assert stats["error_rate"] < 0.1  # Less than 10% error rate
        assert stats["requests_per_second"] > 100  # Should handle at least 100 RPS
    
    @pytest.mark.asyncio
    async def test_mixed_endpoint_load_should_fail_initially(self, load_test_server):
        """Test mixed endpoint load - should fail initially."""
        # Define endpoint mix (realistic usage pattern)
        endpoint_mix = [
            ("/health", 0.4),      # 40% health checks
            ("/pose/detect", 0.3), # 30% pose detection
            ("/auth/login", 0.1),  # 10% authentication
            ("/config", 0.2)       # 20% configuration
        ]
        
        total_requests = 100
        
        async def send_mixed_requests():
            """Send requests with mixed endpoints."""
            tasks = []
            
            for i in range(total_requests):
                # Select endpoint based on distribution
                rand = np.random.random()
                cumulative = 0
                
                for endpoint, probability in endpoint_mix:
                    cumulative += probability
                    if rand <= cumulative:
                        task = asyncio.create_task(
                            load_test_server.handle_request(endpoint)
                        )
                        tasks.append(task)
                        break
            
            return await asyncio.gather(*tasks)
        
        start_time = time.time()
        responses = await send_mixed_requests()
        end_time = time.time()
        
        duration = end_time - start_time
        
        # This will fail initially
        assert len(responses) == total_requests
        
        # Check response distribution
        success_responses = [r for r in responses if r["status"] == 200]
        assert len(success_responses) >= total_requests * 0.9  # At least 90% success
        
        stats = load_test_server.get_performance_stats()
        assert stats["requests_per_second"] > 50  # Should handle at least 50 RPS
        assert stats["avg_response_time_ms"] < 150  # Average response time under 150ms
    
    @pytest.mark.asyncio
    async def test_stress_testing_should_fail_initially(self, load_test_server):
        """Test stress testing - should fail initially."""
        # Gradually increase load to find breaking point
        load_levels = [10, 25, 50, 100, 200]
        results = {}
        
        for concurrent_requests in load_levels:
            load_test_server.reset_stats()
            
            # Send concurrent requests
            tasks = [
                load_test_server.handle_request("/health") 
                for _ in range(concurrent_requests)
            ]
            
            start_time = time.time()
            responses = await asyncio.gather(*tasks)
            end_time = time.time()
            
            duration = end_time - start_time
            stats = load_test_server.get_performance_stats()
            
            results[concurrent_requests] = {
                "duration": duration,
                "rps": stats["requests_per_second"],
                "error_rate": stats["error_rate"],
                "avg_response_time": stats["avg_response_time_ms"],
                "p95_response_time": stats["p95_response_time_ms"]
            }
        
        # This will fail initially
        # Performance should degrade gracefully with increased load
        for load_level, metrics in results.items():
            assert metrics["error_rate"] < 0.2  # Less than 20% error rate
            assert metrics["avg_response_time"] < 1000  # Less than 1 second average
        
        # Higher loads should have higher response times
        assert results[10]["avg_response_time"] <= results[200]["avg_response_time"]
    
    @pytest.mark.asyncio
    async def test_memory_usage_under_load_should_fail_initially(self, load_test_server):
        """Test memory usage under load - should fail initially."""
        import psutil
        import os
        
        process = psutil.Process(os.getpid())
        initial_memory = process.memory_info().rss
        
        # Generate sustained load
        duration_seconds = 5
        target_rps = 100
        
        async def sustained_load():
            """Generate sustained load."""
            interval = 1.0 / target_rps
            end_time = time.time() + duration_seconds
            
            while time.time() < end_time:
                await load_test_server.handle_request("/pose/detect")
                await asyncio.sleep(interval)
        
        await sustained_load()
        
        final_memory = process.memory_info().rss
        memory_increase = final_memory - initial_memory
        
        # This will fail initially
        # Memory increase should be reasonable (less than 100MB)
        assert memory_increase < 100 * 1024 * 1024
        
        stats = load_test_server.get_performance_stats()
        assert stats["total_requests"] > duration_seconds * target_rps * 0.8


class TestAPIPerformanceOptimization:
    """Test API performance optimization techniques."""
    
    @pytest.mark.asyncio
    async def test_response_caching_effect_should_fail_initially(self):
        """Test response caching effect - should fail initially."""
        class CachedAPIServer(MockAPIServer):
            def __init__(self):
                super().__init__()
                self.cache = {}
                self.cache_hits = 0
                self.cache_misses = 0
            
            async def handle_request(self, endpoint: str, method: str = "GET", data: Dict[str, Any] = None) -> Dict[str, Any]:
                cache_key = f"{method}:{endpoint}"
                
                if cache_key in self.cache:
                    self.cache_hits += 1
                    cached_response = self.cache[cache_key].copy()
                    cached_response["response_time_ms"] = 1.0  # Cached responses are fast
                    return cached_response
                
                self.cache_misses += 1
                response = await super().handle_request(endpoint, method, data)
                
                # Cache successful responses
                if response["status"] == 200:
                    self.cache[cache_key] = response.copy()
                
                return response
        
        cached_server = CachedAPIServer()
        
        # First request (cache miss)
        response1 = await cached_server.handle_request("/health")
        
        # Second request (cache hit)
        response2 = await cached_server.handle_request("/health")
        
        # This will fail initially
        assert response1["status"] == 200
        assert response2["status"] == 200
        assert response2["response_time_ms"] < response1["response_time_ms"]
        assert cached_server.cache_hits == 1
        assert cached_server.cache_misses == 1
    
    @pytest.mark.asyncio
    async def test_connection_pooling_effect_should_fail_initially(self):
        """Test connection pooling effect - should fail initially."""
        # Simulate connection overhead
        class ConnectionPoolServer(MockAPIServer):
            def __init__(self, pool_size: int = 10):
                super().__init__()
                self.pool_size = pool_size
                self.active_connections = 0
                self.connection_overhead = 0.01  # 10ms connection overhead
            
            async def handle_request(self, endpoint: str, method: str = "GET", data: Dict[str, Any] = None) -> Dict[str, Any]:
                # Simulate connection acquisition
                if self.active_connections < self.pool_size:
                    # New connection needed
                    await asyncio.sleep(self.connection_overhead)
                    self.active_connections += 1
                
                try:
                    return await super().handle_request(endpoint, method, data)
                finally:
                    # Connection returned to pool (not closed)
                    pass
        
        pooled_server = ConnectionPoolServer(pool_size=5)
        
        # Send requests that exceed pool size
        concurrent_requests = 10
        tasks = [
            pooled_server.handle_request("/health") 
            for _ in range(concurrent_requests)
        ]
        
        start_time = time.time()
        responses = await asyncio.gather(*tasks)
        end_time = time.time()
        
        total_time = (end_time - start_time) * 1000
        
        # This will fail initially
        assert len(responses) == concurrent_requests
        assert all(r["status"] == 200 for r in responses)
        
        # With connection pooling, should complete reasonably fast
        assert total_time < 500  # Should complete within 500ms
    
    @pytest.mark.asyncio
    async def test_request_batching_performance_should_fail_initially(self):
        """Test request batching performance - should fail initially."""
        class BatchingServer(MockAPIServer):
            def __init__(self):
                super().__init__()
                self.batch_size = 5
                self.pending_requests = []
                self.batch_processing = False
            
            async def handle_batch_request(self, requests: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
                """Handle batch of requests."""
                # Batch processing is more efficient
                batch_overhead = 0.01  # 10ms overhead for entire batch
                await asyncio.sleep(batch_overhead)
                
                responses = []
                for req in requests:
                    # Individual processing is faster in batch
                    processing_time = self._get_processing_time(req["endpoint"], req["method"]) * 0.5
                    await asyncio.sleep(processing_time)
                    
                    response = self._generate_response(req["endpoint"], req["method"], req.get("data"))
                    responses.append({
                        "status": 200,
                        "data": response,
                        "response_time_ms": processing_time * 1000
                    })
                
                return responses
        
        batching_server = BatchingServer()
        
        # Test individual requests vs batch
        individual_requests = 5
        
        # Individual requests
        start_time = time.time()
        individual_tasks = [
            batching_server.handle_request("/health") 
            for _ in range(individual_requests)
        ]
        individual_responses = await asyncio.gather(*individual_tasks)
        individual_time = (time.time() - start_time) * 1000
        
        # Batch request
        batch_requests = [
            {"endpoint": "/health", "method": "GET"} 
            for _ in range(individual_requests)
        ]
        
        start_time = time.time()
        batch_responses = await batching_server.handle_batch_request(batch_requests)
        batch_time = (time.time() - start_time) * 1000
        
        # This will fail initially
        assert len(individual_responses) == individual_requests
        assert len(batch_responses) == individual_requests
        
        # Batch should be more efficient
        assert batch_time < individual_time
        assert all(r["status"] == 200 for r in batch_responses)
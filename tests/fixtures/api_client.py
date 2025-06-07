"""
Test client utilities for API testing.

Provides mock and real API clients for comprehensive testing.
"""

import asyncio
import aiohttp
import json
import time
from datetime import datetime, timedelta
from typing import Dict, Any, List, Optional, Union, AsyncGenerator
from unittest.mock import AsyncMock, MagicMock
import websockets
import jwt
from dataclasses import dataclass, asdict
from enum import Enum


class AuthenticationError(Exception):
    """Authentication related errors."""
    pass


class APIError(Exception):
    """General API errors."""
    pass


class RateLimitError(Exception):
    """Rate limiting errors."""
    pass


@dataclass
class APIResponse:
    """API response wrapper."""
    status_code: int
    data: Dict[str, Any]
    headers: Dict[str, str]
    response_time_ms: float
    timestamp: datetime


class MockAPIClient:
    """Mock API client for testing."""
    
    def __init__(self, base_url: str = "http://localhost:8000"):
        self.base_url = base_url
        self.session = None
        self.auth_token = None
        self.refresh_token = None
        self.token_expires_at = None
        self.request_history = []
        self.response_delays = {}
        self.error_simulation = {}
        self.rate_limit_config = {
            "enabled": False,
            "requests_per_minute": 60,
            "current_count": 0,
            "window_start": time.time()
        }
    
    async def __aenter__(self):
        """Async context manager entry."""
        await self.connect()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.disconnect()
    
    async def connect(self):
        """Initialize connection."""
        self.session = aiohttp.ClientSession()
    
    async def disconnect(self):
        """Close connection."""
        if self.session:
            await self.session.close()
    
    def set_response_delay(self, endpoint: str, delay_ms: float):
        """Set artificial delay for endpoint."""
        self.response_delays[endpoint] = delay_ms
    
    def simulate_error(self, endpoint: str, error_type: str, probability: float = 1.0):
        """Simulate errors for endpoint."""
        self.error_simulation[endpoint] = {
            "type": error_type,
            "probability": probability
        }
    
    def enable_rate_limiting(self, requests_per_minute: int = 60):
        """Enable rate limiting simulation."""
        self.rate_limit_config.update({
            "enabled": True,
            "requests_per_minute": requests_per_minute,
            "current_count": 0,
            "window_start": time.time()
        })
    
    async def _check_rate_limit(self):
        """Check rate limiting."""
        if not self.rate_limit_config["enabled"]:
            return
        
        current_time = time.time()
        window_duration = 60  # 1 minute
        
        # Reset window if needed
        if current_time - self.rate_limit_config["window_start"] > window_duration:
            self.rate_limit_config["current_count"] = 0
            self.rate_limit_config["window_start"] = current_time
        
        # Check limit
        if self.rate_limit_config["current_count"] >= self.rate_limit_config["requests_per_minute"]:
            raise RateLimitError("Rate limit exceeded")
        
        self.rate_limit_config["current_count"] += 1
    
    async def _simulate_network_delay(self, endpoint: str):
        """Simulate network delay."""
        delay = self.response_delays.get(endpoint, 0)
        if delay > 0:
            await asyncio.sleep(delay / 1000)  # Convert ms to seconds
    
    async def _check_error_simulation(self, endpoint: str):
        """Check if error should be simulated."""
        if endpoint in self.error_simulation:
            config = self.error_simulation[endpoint]
            if random.random() < config["probability"]:
                error_type = config["type"]
                if error_type == "timeout":
                    raise asyncio.TimeoutError("Simulated timeout")
                elif error_type == "connection":
                    raise aiohttp.ClientConnectionError("Simulated connection error")
                elif error_type == "server_error":
                    raise APIError("Simulated server error")
    
    async def _make_request(self, method: str, endpoint: str, **kwargs) -> APIResponse:
        """Make HTTP request with simulation."""
        start_time = time.time()
        
        # Check rate limiting
        await self._check_rate_limit()
        
        # Simulate network delay
        await self._simulate_network_delay(endpoint)
        
        # Check error simulation
        await self._check_error_simulation(endpoint)
        
        # Record request
        request_record = {
            "method": method,
            "endpoint": endpoint,
            "timestamp": datetime.utcnow(),
            "kwargs": kwargs
        }
        self.request_history.append(request_record)
        
        # Generate mock response
        response_data = await self._generate_mock_response(method, endpoint, kwargs)
        
        end_time = time.time()
        response_time = (end_time - start_time) * 1000
        
        return APIResponse(
            status_code=response_data["status_code"],
            data=response_data["data"],
            headers=response_data.get("headers", {}),
            response_time_ms=response_time,
            timestamp=datetime.utcnow()
        )
    
    async def _generate_mock_response(self, method: str, endpoint: str, kwargs: Dict[str, Any]) -> Dict[str, Any]:
        """Generate mock response based on endpoint."""
        if endpoint == "/health":
            return {
                "status_code": 200,
                "data": {
                    "status": "healthy",
                    "timestamp": datetime.utcnow().isoformat(),
                    "version": "1.0.0"
                }
            }
        
        elif endpoint == "/auth/login":
            if method == "POST":
                # Generate mock JWT tokens
                payload = {
                    "user_id": "test_user",
                    "exp": datetime.utcnow() + timedelta(hours=1)
                }
                access_token = jwt.encode(payload, "secret", algorithm="HS256")
                refresh_token = jwt.encode({"user_id": "test_user"}, "secret", algorithm="HS256")
                
                self.auth_token = access_token
                self.refresh_token = refresh_token
                self.token_expires_at = payload["exp"]
                
                return {
                    "status_code": 200,
                    "data": {
                        "access_token": access_token,
                        "refresh_token": refresh_token,
                        "token_type": "bearer",
                        "expires_in": 3600
                    }
                }
        
        elif endpoint == "/auth/refresh":
            if method == "POST" and self.refresh_token:
                # Generate new access token
                payload = {
                    "user_id": "test_user",
                    "exp": datetime.utcnow() + timedelta(hours=1)
                }
                access_token = jwt.encode(payload, "secret", algorithm="HS256")
                
                self.auth_token = access_token
                self.token_expires_at = payload["exp"]
                
                return {
                    "status_code": 200,
                    "data": {
                        "access_token": access_token,
                        "token_type": "bearer",
                        "expires_in": 3600
                    }
                }
        
        elif endpoint == "/pose/detect":
            if method == "POST":
                return {
                    "status_code": 200,
                    "data": {
                        "persons": [
                            {
                                "person_id": "person_1",
                                "confidence": 0.85,
                                "bounding_box": {"x": 100, "y": 150, "width": 80, "height": 180},
                                "keypoints": [[x, y, 0.9] for x, y in zip(range(17), range(17))],
                                "activity": "standing"
                            }
                        ],
                        "processing_time_ms": 45.2,
                        "model_version": "v1.0",
                        "timestamp": datetime.utcnow().isoformat()
                    }
                }
        
        elif endpoint == "/config":
            if method == "GET":
                return {
                    "status_code": 200,
                    "data": {
                        "model_config": {
                            "confidence_threshold": 0.7,
                            "nms_threshold": 0.5,
                            "max_persons": 10
                        },
                        "processing_config": {
                            "batch_size": 1,
                            "use_gpu": True,
                            "preprocessing": "standard"
                        }
                    }
                }
        
        # Default response
        return {
            "status_code": 404,
            "data": {"error": "Endpoint not found"}
        }
    
    async def get(self, endpoint: str, **kwargs) -> APIResponse:
        """Make GET request."""
        return await self._make_request("GET", endpoint, **kwargs)
    
    async def post(self, endpoint: str, **kwargs) -> APIResponse:
        """Make POST request."""
        return await self._make_request("POST", endpoint, **kwargs)
    
    async def put(self, endpoint: str, **kwargs) -> APIResponse:
        """Make PUT request."""
        return await self._make_request("PUT", endpoint, **kwargs)
    
    async def delete(self, endpoint: str, **kwargs) -> APIResponse:
        """Make DELETE request."""
        return await self._make_request("DELETE", endpoint, **kwargs)
    
    async def login(self, username: str, password: str) -> bool:
        """Authenticate with API."""
        response = await self.post("/auth/login", json={
            "username": username,
            "password": password
        })
        
        if response.status_code == 200:
            return True
        else:
            raise AuthenticationError("Login failed")
    
    async def refresh_auth_token(self) -> bool:
        """Refresh authentication token."""
        if not self.refresh_token:
            raise AuthenticationError("No refresh token available")
        
        response = await self.post("/auth/refresh", json={
            "refresh_token": self.refresh_token
        })
        
        if response.status_code == 200:
            return True
        else:
            raise AuthenticationError("Token refresh failed")
    
    def is_authenticated(self) -> bool:
        """Check if client is authenticated."""
        if not self.auth_token or not self.token_expires_at:
            return False
        
        return datetime.utcnow() < self.token_expires_at
    
    def get_request_history(self) -> List[Dict[str, Any]]:
        """Get request history."""
        return self.request_history.copy()
    
    def clear_request_history(self):
        """Clear request history."""
        self.request_history.clear()


class MockWebSocketClient:
    """Mock WebSocket client for testing."""
    
    def __init__(self, uri: str = "ws://localhost:8000/ws"):
        self.uri = uri
        self.websocket = None
        self.is_connected = False
        self.messages_received = []
        self.messages_sent = []
        self.connection_errors = []
        self.auto_respond = True
        self.response_delay = 0.01  # 10ms default delay
    
    async def connect(self) -> bool:
        """Connect to WebSocket."""
        try:
            # Simulate connection
            await asyncio.sleep(0.01)
            self.is_connected = True
            return True
        except Exception as e:
            self.connection_errors.append(str(e))
            return False
    
    async def disconnect(self):
        """Disconnect from WebSocket."""
        self.is_connected = False
        self.websocket = None
    
    async def send_message(self, message: Dict[str, Any]) -> bool:
        """Send message to WebSocket."""
        if not self.is_connected:
            raise ConnectionError("WebSocket not connected")
        
        # Record sent message
        self.messages_sent.append({
            "message": message,
            "timestamp": datetime.utcnow()
        })
        
        # Auto-respond if enabled
        if self.auto_respond:
            await asyncio.sleep(self.response_delay)
            response = await self._generate_auto_response(message)
            if response:
                self.messages_received.append({
                    "message": response,
                    "timestamp": datetime.utcnow()
                })
        
        return True
    
    async def receive_message(self, timeout: float = 1.0) -> Optional[Dict[str, Any]]:
        """Receive message from WebSocket."""
        if not self.is_connected:
            raise ConnectionError("WebSocket not connected")
        
        # Wait for message or timeout
        start_time = time.time()
        while time.time() - start_time < timeout:
            if self.messages_received:
                return self.messages_received.pop(0)["message"]
            await asyncio.sleep(0.01)
        
        return None
    
    async def _generate_auto_response(self, message: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """Generate automatic response to message."""
        message_type = message.get("type")
        
        if message_type == "subscribe":
            return {
                "type": "subscription_confirmed",
                "channel": message.get("channel"),
                "timestamp": datetime.utcnow().isoformat()
            }
        
        elif message_type == "pose_request":
            return {
                "type": "pose_data",
                "data": {
                    "persons": [
                        {
                            "person_id": "person_1",
                            "confidence": 0.88,
                            "bounding_box": {"x": 150, "y": 200, "width": 80, "height": 180},
                            "keypoints": [[x, y, 0.9] for x, y in zip(range(17), range(17))]
                        }
                    ],
                    "timestamp": datetime.utcnow().isoformat()
                },
                "request_id": message.get("request_id")
            }
        
        elif message_type == "ping":
            return {
                "type": "pong",
                "timestamp": datetime.utcnow().isoformat()
            }
        
        return None
    
    def set_auto_respond(self, enabled: bool, delay_ms: float = 10):
        """Configure auto-response behavior."""
        self.auto_respond = enabled
        self.response_delay = delay_ms / 1000
    
    def inject_message(self, message: Dict[str, Any]):
        """Inject message as if received from server."""
        self.messages_received.append({
            "message": message,
            "timestamp": datetime.utcnow()
        })
    
    def get_sent_messages(self) -> List[Dict[str, Any]]:
        """Get all sent messages."""
        return self.messages_sent.copy()
    
    def get_received_messages(self) -> List[Dict[str, Any]]:
        """Get all received messages."""
        return self.messages_received.copy()
    
    def clear_message_history(self):
        """Clear message history."""
        self.messages_sent.clear()
        self.messages_received.clear()


class APITestClient:
    """High-level test client combining HTTP and WebSocket."""
    
    def __init__(self, base_url: str = "http://localhost:8000"):
        self.base_url = base_url
        self.ws_url = base_url.replace("http", "ws") + "/ws"
        self.http_client = MockAPIClient(base_url)
        self.ws_client = MockWebSocketClient(self.ws_url)
        self.test_session_id = None
    
    async def __aenter__(self):
        """Async context manager entry."""
        await self.setup()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.teardown()
    
    async def setup(self):
        """Setup test client."""
        await self.http_client.connect()
        await self.ws_client.connect()
        self.test_session_id = f"test_session_{int(time.time())}"
    
    async def teardown(self):
        """Teardown test client."""
        await self.ws_client.disconnect()
        await self.http_client.disconnect()
    
    async def authenticate(self, username: str = "test_user", password: str = "test_pass") -> bool:
        """Authenticate with API."""
        return await self.http_client.login(username, password)
    
    async def test_health_endpoint(self) -> APIResponse:
        """Test health endpoint."""
        return await self.http_client.get("/health")
    
    async def test_pose_detection(self, csi_data: Dict[str, Any]) -> APIResponse:
        """Test pose detection endpoint."""
        return await self.http_client.post("/pose/detect", json=csi_data)
    
    async def test_websocket_streaming(self, duration_seconds: int = 5) -> List[Dict[str, Any]]:
        """Test WebSocket streaming."""
        # Subscribe to pose stream
        await self.ws_client.send_message({
            "type": "subscribe",
            "channel": "pose_stream",
            "session_id": self.test_session_id
        })
        
        # Collect messages for specified duration
        messages = []
        end_time = time.time() + duration_seconds
        
        while time.time() < end_time:
            message = await self.ws_client.receive_message(timeout=0.1)
            if message:
                messages.append(message)
        
        return messages
    
    async def simulate_concurrent_requests(self, num_requests: int = 10) -> List[APIResponse]:
        """Simulate concurrent HTTP requests."""
        tasks = []
        
        for i in range(num_requests):
            task = asyncio.create_task(self.http_client.get("/health"))
            tasks.append(task)
        
        responses = await asyncio.gather(*tasks, return_exceptions=True)
        return responses
    
    async def simulate_websocket_load(self, num_connections: int = 5, duration_seconds: int = 3) -> Dict[str, Any]:
        """Simulate WebSocket load testing."""
        # Create multiple WebSocket clients
        ws_clients = []
        for i in range(num_connections):
            client = MockWebSocketClient(self.ws_url)
            await client.connect()
            ws_clients.append(client)
        
        # Send messages from all clients
        message_counts = []
        
        try:
            tasks = []
            for i, client in enumerate(ws_clients):
                task = asyncio.create_task(self._send_messages_for_duration(client, duration_seconds, i))
                tasks.append(task)
            
            results = await asyncio.gather(*tasks)
            message_counts = results
            
        finally:
            # Cleanup
            for client in ws_clients:
                await client.disconnect()
        
        return {
            "num_connections": num_connections,
            "duration_seconds": duration_seconds,
            "messages_per_connection": message_counts,
            "total_messages": sum(message_counts)
        }
    
    async def _send_messages_for_duration(self, client: MockWebSocketClient, duration: int, client_id: int) -> int:
        """Send messages for specified duration."""
        message_count = 0
        end_time = time.time() + duration
        
        while time.time() < end_time:
            await client.send_message({
                "type": "ping",
                "client_id": client_id,
                "message_id": message_count
            })
            message_count += 1
            await asyncio.sleep(0.1)  # 10 messages per second
        
        return message_count
    
    def configure_error_simulation(self, endpoint: str, error_type: str, probability: float = 0.1):
        """Configure error simulation for testing."""
        self.http_client.simulate_error(endpoint, error_type, probability)
    
    def configure_rate_limiting(self, requests_per_minute: int = 60):
        """Configure rate limiting for testing."""
        self.http_client.enable_rate_limiting(requests_per_minute)
    
    def get_performance_metrics(self) -> Dict[str, Any]:
        """Get performance metrics from test session."""
        http_history = self.http_client.get_request_history()
        ws_sent = self.ws_client.get_sent_messages()
        ws_received = self.ws_client.get_received_messages()
        
        # Calculate HTTP metrics
        if http_history:
            response_times = [r.get("response_time_ms", 0) for r in http_history]
            http_metrics = {
                "total_requests": len(http_history),
                "avg_response_time_ms": sum(response_times) / len(response_times),
                "min_response_time_ms": min(response_times),
                "max_response_time_ms": max(response_times)
            }
        else:
            http_metrics = {"total_requests": 0}
        
        # Calculate WebSocket metrics
        ws_metrics = {
            "messages_sent": len(ws_sent),
            "messages_received": len(ws_received),
            "connection_active": self.ws_client.is_connected
        }
        
        return {
            "session_id": self.test_session_id,
            "http_metrics": http_metrics,
            "websocket_metrics": ws_metrics,
            "timestamp": datetime.utcnow().isoformat()
        }


# Utility functions for test data generation
def generate_test_csi_data() -> Dict[str, Any]:
    """Generate test CSI data for API testing."""
    import numpy as np
    
    return {
        "timestamp": datetime.utcnow().isoformat(),
        "router_id": "test_router_001",
        "amplitude": np.random.uniform(0, 1, (4, 64)).tolist(),
        "phase": np.random.uniform(-np.pi, np.pi, (4, 64)).tolist(),
        "frequency": 5.8e9,
        "bandwidth": 80e6,
        "num_antennas": 4,
        "num_subcarriers": 64
    }


def create_test_user_credentials() -> Dict[str, str]:
    """Create test user credentials."""
    return {
        "username": "test_user",
        "password": "test_password_123",
        "email": "test@example.com"
    }


async def wait_for_condition(condition_func, timeout: float = 5.0, interval: float = 0.1) -> bool:
    """Wait for condition to become true."""
    end_time = time.time() + timeout
    
    while time.time() < end_time:
        if await condition_func() if asyncio.iscoroutinefunction(condition_func) else condition_func():
            return True
        await asyncio.sleep(interval)
    
    return False
#!/usr/bin/env python3
"""
Test script for authentication and rate limiting functionality
"""

import asyncio
import time
import json
import sys
from typing import Dict, List, Any
import httpx
import jwt
from datetime import datetime, timedelta

# Configuration
BASE_URL = "http://localhost:8000"
API_PREFIX = "/api/v1"

# Test credentials
TEST_USERS = {
    "admin": {"username": "admin", "password": "admin123"},
    "user": {"username": "user", "password": "user123"}
}

# JWT settings for testing
SECRET_KEY = "your-secret-key-here"  # This should match your settings
JWT_ALGORITHM = "HS256"


class AuthRateLimitTester:
    def __init__(self, base_url: str = BASE_URL):
        self.base_url = base_url
        self.client = httpx.Client(timeout=30.0)
        self.async_client = httpx.AsyncClient(timeout=30.0)
        self.results = []
        
    def log_result(self, test_name: str, success: bool, message: str, details: Dict = None):
        """Log test result"""
        result = {
            "test": test_name,
            "success": success,
            "message": message,
            "timestamp": datetime.now().isoformat(),
            "details": details or {}
        }
        self.results.append(result)
        
        # Print result
        status = "✓" if success else "✗"
        print(f"{status} {test_name}: {message}")
        if details and not success:
            print(f"  Details: {json.dumps(details, indent=2)}")
    
    def generate_test_token(self, username: str, expired: bool = False) -> str:
        """Generate a test JWT token"""
        payload = {
            "sub": username,
            "username": username,
            "email": f"{username}@example.com",
            "roles": ["admin"] if username == "admin" else ["user"],
            "iat": datetime.utcnow(),
            "exp": datetime.utcnow() + (timedelta(hours=-1) if expired else timedelta(hours=24))
        }
        return jwt.encode(payload, SECRET_KEY, algorithm=JWT_ALGORITHM)
    
    def test_public_endpoints(self):
        """Test access to public endpoints without authentication"""
        print("\n=== Testing Public Endpoints ===")
        
        public_endpoints = [
            "/",
            "/health",
            f"{API_PREFIX}/info",
            f"{API_PREFIX}/status",
            f"{API_PREFIX}/pose/current"
        ]
        
        for endpoint in public_endpoints:
            try:
                response = self.client.get(f"{self.base_url}{endpoint}")
                self.log_result(
                    f"Public endpoint {endpoint}",
                    response.status_code in [200, 204],
                    f"Status: {response.status_code}",
                    {"response": response.json() if response.content else None}
                )
            except Exception as e:
                self.log_result(
                    f"Public endpoint {endpoint}",
                    False,
                    str(e)
                )
    
    def test_protected_endpoints(self):
        """Test protected endpoints without authentication"""
        print("\n=== Testing Protected Endpoints (No Auth) ===")
        
        protected_endpoints = [
            (f"{API_PREFIX}/pose/analyze", "POST"),
            (f"{API_PREFIX}/pose/calibrate", "POST"),
            (f"{API_PREFIX}/stream/start", "POST"),
            (f"{API_PREFIX}/stream/stop", "POST")
        ]
        
        for endpoint, method in protected_endpoints:
            try:
                if method == "GET":
                    response = self.client.get(f"{self.base_url}{endpoint}")
                else:
                    response = self.client.post(f"{self.base_url}{endpoint}", json={})
                
                # Should return 401 Unauthorized
                expected_status = 401
                self.log_result(
                    f"Protected endpoint {endpoint} without auth",
                    response.status_code == expected_status,
                    f"Status: {response.status_code} (expected {expected_status})",
                    {"response": response.json() if response.content else None}
                )
            except Exception as e:
                self.log_result(
                    f"Protected endpoint {endpoint}",
                    False,
                    str(e)
                )
    
    def test_authentication_headers(self):
        """Test different authentication header formats"""
        print("\n=== Testing Authentication Headers ===")
        
        endpoint = f"{self.base_url}{API_PREFIX}/pose/analyze"
        test_cases = [
            ("No header", {}),
            ("Invalid format", {"Authorization": "InvalidFormat"}),
            ("Wrong scheme", {"Authorization": "Basic dGVzdDp0ZXN0"}),
            ("Invalid token", {"Authorization": "Bearer invalid.token.here"}),
            ("Expired token", {"Authorization": f"Bearer {self.generate_test_token('user', expired=True)}"}),
            ("Valid token", {"Authorization": f"Bearer {self.generate_test_token('user')}"})
        ]
        
        for test_name, headers in test_cases:
            try:
                response = self.client.post(endpoint, headers=headers, json={})
                
                # Only valid token should succeed (or get validation error)
                if test_name == "Valid token":
                    expected = response.status_code in [200, 422]  # 422 for validation errors
                else:
                    expected = response.status_code == 401
                
                self.log_result(
                    f"Auth header test: {test_name}",
                    expected,
                    f"Status: {response.status_code}",
                    {"headers": headers}
                )
            except Exception as e:
                self.log_result(
                    f"Auth header test: {test_name}",
                    False,
                    str(e)
                )
    
    async def test_rate_limiting(self):
        """Test rate limiting functionality"""
        print("\n=== Testing Rate Limiting ===")
        
        # Test endpoints with different rate limits
        test_configs = [
            {
                "endpoint": f"{API_PREFIX}/pose/current",
                "method": "GET",
                "requests": 70,  # More than 60/min limit
                "window": 60,
                "description": "Current pose endpoint (60/min)"
            },
            {
                "endpoint": f"{API_PREFIX}/pose/analyze",
                "method": "POST",
                "requests": 15,  # More than 10/min limit
                "window": 60,
                "description": "Analyze endpoint (10/min)",
                "auth": True
            }
        ]
        
        for config in test_configs:
            print(f"\nTesting: {config['description']}")
            
            # Prepare headers
            headers = {}
            if config.get("auth"):
                headers["Authorization"] = f"Bearer {self.generate_test_token('user')}"
            
            # Send requests
            responses = []
            start_time = time.time()
            
            for i in range(config["requests"]):
                try:
                    if config["method"] == "GET":
                        response = await self.async_client.get(
                            f"{self.base_url}{config['endpoint']}",
                            headers=headers
                        )
                    else:
                        response = await self.async_client.post(
                            f"{self.base_url}{config['endpoint']}",
                            headers=headers,
                            json={}
                        )
                    
                    responses.append({
                        "request": i + 1,
                        "status": response.status_code,
                        "headers": dict(response.headers)
                    })
                    
                    # Check rate limit headers
                    if "X-RateLimit-Limit" in response.headers:
                        remaining = response.headers.get("X-RateLimit-Remaining", "N/A")
                        if i % 10 == 0:  # Print every 10th request
                            print(f"  Request {i+1}: Status {response.status_code}, Remaining: {remaining}")
                    
                    # Small delay to avoid overwhelming
                    await asyncio.sleep(0.1)
                    
                except Exception as e:
                    responses.append({
                        "request": i + 1,
                        "error": str(e)
                    })
            
            elapsed = time.time() - start_time
            
            # Analyze results
            rate_limited = sum(1 for r in responses if r.get("status") == 429)
            successful = sum(1 for r in responses if r.get("status") in [200, 204])
            
            self.log_result(
                f"Rate limit test: {config['description']}",
                rate_limited > 0,  # Should have some rate limited requests
                f"Sent {config['requests']} requests in {elapsed:.1f}s. "
                f"Successful: {successful}, Rate limited: {rate_limited}",
                {
                    "total_requests": config["requests"],
                    "successful": successful,
                    "rate_limited": rate_limited,
                    "elapsed_time": f"{elapsed:.1f}s"
                }
            )
    
    def test_rate_limit_headers(self):
        """Test rate limit response headers"""
        print("\n=== Testing Rate Limit Headers ===")
        
        endpoint = f"{self.base_url}{API_PREFIX}/pose/current"
        
        try:
            response = self.client.get(endpoint)
            
            # Check for rate limit headers
            expected_headers = [
                "X-RateLimit-Limit",
                "X-RateLimit-Remaining",
                "X-RateLimit-Window"
            ]
            
            found_headers = {h: response.headers.get(h) for h in expected_headers if h in response.headers}
            
            self.log_result(
                "Rate limit headers",
                len(found_headers) > 0,
                f"Found {len(found_headers)} rate limit headers",
                {"headers": found_headers}
            )
            
            # Test 429 response
            if len(found_headers) > 0:
                # Send many requests to trigger rate limit
                for _ in range(100):
                    r = self.client.get(endpoint)
                    if r.status_code == 429:
                        retry_after = r.headers.get("Retry-After")
                        self.log_result(
                            "Rate limit 429 response",
                            retry_after is not None,
                            f"Got 429 with Retry-After: {retry_after}",
                            {"headers": dict(r.headers)}
                        )
                        break
                        
        except Exception as e:
            self.log_result(
                "Rate limit headers",
                False,
                str(e)
            )
    
    def test_cors_headers(self):
        """Test CORS headers"""
        print("\n=== Testing CORS Headers ===")
        
        test_origins = [
            "http://localhost:3000",
            "https://example.com",
            "http://malicious.site"
        ]
        
        endpoint = f"{self.base_url}/health"
        
        for origin in test_origins:
            try:
                # Regular request with Origin header
                response = self.client.get(
                    endpoint,
                    headers={"Origin": origin}
                )
                
                cors_headers = {
                    k: v for k, v in response.headers.items()
                    if k.lower().startswith("access-control-")
                }
                
                self.log_result(
                    f"CORS headers for origin {origin}",
                    len(cors_headers) > 0,
                    f"Found {len(cors_headers)} CORS headers",
                    {"headers": cors_headers}
                )
                
                # Preflight request
                preflight_response = self.client.options(
                    endpoint,
                    headers={
                        "Origin": origin,
                        "Access-Control-Request-Method": "POST",
                        "Access-Control-Request-Headers": "Content-Type,Authorization"
                    }
                )
                
                self.log_result(
                    f"CORS preflight for origin {origin}",
                    preflight_response.status_code in [200, 204],
                    f"Status: {preflight_response.status_code}",
                    {"headers": dict(preflight_response.headers)}
                )
                
            except Exception as e:
                self.log_result(
                    f"CORS test for origin {origin}",
                    False,
                    str(e)
                )
    
    def test_security_headers(self):
        """Test security headers"""
        print("\n=== Testing Security Headers ===")
        
        endpoint = f"{self.base_url}/health"
        
        try:
            response = self.client.get(endpoint)
            
            security_headers = [
                "X-Content-Type-Options",
                "X-Frame-Options",
                "X-XSS-Protection",
                "Referrer-Policy",
                "Content-Security-Policy"
            ]
            
            found_headers = {h: response.headers.get(h) for h in security_headers if h in response.headers}
            
            self.log_result(
                "Security headers",
                len(found_headers) >= 3,  # At least 3 security headers
                f"Found {len(found_headers)}/{len(security_headers)} security headers",
                {"headers": found_headers}
            )
            
        except Exception as e:
            self.log_result(
                "Security headers",
                False,
                str(e)
            )
    
    def test_authentication_states(self):
        """Test authentication enable/disable states"""
        print("\n=== Testing Authentication States ===")
        
        # Check if authentication is enabled
        try:
            info_response = self.client.get(f"{self.base_url}{API_PREFIX}/info")
            if info_response.status_code == 200:
                info = info_response.json()
                auth_enabled = info.get("features", {}).get("authentication", False)
                rate_limit_enabled = info.get("features", {}).get("rate_limiting", False)
                
                self.log_result(
                    "Feature flags",
                    True,
                    f"Authentication: {auth_enabled}, Rate Limiting: {rate_limit_enabled}",
                    {
                        "authentication": auth_enabled,
                        "rate_limiting": rate_limit_enabled
                    }
                )
            
        except Exception as e:
            self.log_result(
                "Feature flags",
                False,
                str(e)
            )
    
    async def run_all_tests(self):
        """Run all tests"""
        print("=" * 60)
        print("WiFi-DensePose Authentication & Rate Limiting Test Suite")
        print("=" * 60)
        
        # Run synchronous tests
        self.test_public_endpoints()
        self.test_protected_endpoints()
        self.test_authentication_headers()
        self.test_rate_limit_headers()
        self.test_cors_headers()
        self.test_security_headers()
        self.test_authentication_states()
        
        # Run async tests
        await self.test_rate_limiting()
        
        # Summary
        print("\n" + "=" * 60)
        print("Test Summary")
        print("=" * 60)
        
        total = len(self.results)
        passed = sum(1 for r in self.results if r["success"])
        failed = total - passed
        
        print(f"Total tests: {total}")
        print(f"Passed: {passed}")
        print(f"Failed: {failed}")
        print(f"Success rate: {(passed/total*100):.1f}%" if total > 0 else "N/A")
        
        # Save results
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"auth_rate_limit_test_results_{timestamp}.json"
        
        with open(filename, "w") as f:
            json.dump({
                "test_run": {
                    "timestamp": datetime.now().isoformat(),
                    "base_url": self.base_url,
                    "total_tests": total,
                    "passed": passed,
                    "failed": failed
                },
                "results": self.results
            }, f, indent=2)
        
        print(f"\nResults saved to: {filename}")
        
        # Cleanup
        await self.async_client.aclose()
        self.client.close()
        
        return passed == total


async def main():
    """Main function"""
    tester = AuthRateLimitTester()
    success = await tester.run_all_tests()
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    asyncio.run(main())
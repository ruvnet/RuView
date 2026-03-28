#!/usr/bin/env python3
"""
API Endpoint Testing Script
Tests all WiFi-DensePose API endpoints and provides debugging information.
"""

import asyncio
import json
import sys
import time
import traceback
from datetime import datetime, timedelta
from typing import Dict, List, Any, Optional

import aiohttp
import websockets
from colorama import Fore, Style, init

# Initialize colorama for colored output
init(autoreset=True)

class APITester:
    """Comprehensive API endpoint tester."""
    
    def __init__(self, base_url: str = "http://localhost:8000"):
        self.base_url = base_url
        self.session = None
        self.results = {
            "total_tests": 0,
            "passed": 0,
            "failed": 0,
            "errors": [],
            "test_details": []
        }
    
    async def __aenter__(self):
        """Async context manager entry."""
        self.session = aiohttp.ClientSession()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        if self.session:
            await self.session.close()
    
    def log_success(self, message: str):
        """Log success message."""
        print(f"{Fore.GREEN}✓ {message}{Style.RESET_ALL}")
    
    def log_error(self, message: str):
        """Log error message."""
        print(f"{Fore.RED}✗ {message}{Style.RESET_ALL}")
    
    def log_info(self, message: str):
        """Log info message."""
        print(f"{Fore.BLUE}ℹ {message}{Style.RESET_ALL}")
    
    def log_warning(self, message: str):
        """Log warning message."""
        print(f"{Fore.YELLOW}⚠ {message}{Style.RESET_ALL}")
    
    async def test_endpoint(
        self,
        method: str,
        endpoint: str,
        expected_status: int = 200,
        data: Optional[Dict] = None,
        params: Optional[Dict] = None,
        headers: Optional[Dict] = None,
        description: str = ""
    ) -> Dict[str, Any]:
        """Test a single API endpoint."""
        self.results["total_tests"] += 1
        test_name = f"{method.upper()} {endpoint}"
        
        try:
            url = f"{self.base_url}{endpoint}"
            
            # Prepare request
            kwargs = {}
            if data:
                kwargs["json"] = data
            if params:
                kwargs["params"] = params
            if headers:
                kwargs["headers"] = headers
            
            # Make request
            start_time = time.time()
            async with self.session.request(method, url, **kwargs) as response:
                response_time = (time.time() - start_time) * 1000
                response_text = await response.text()
                
                # Try to parse JSON response
                try:
                    response_data = json.loads(response_text) if response_text else {}
                except json.JSONDecodeError:
                    response_data = {"raw_response": response_text}
                
                # Check status code
                status_ok = response.status == expected_status
                
                test_result = {
                    "test_name": test_name,
                    "description": description,
                    "url": url,
                    "method": method.upper(),
                    "expected_status": expected_status,
                    "actual_status": response.status,
                    "response_time_ms": round(response_time, 2),
                    "response_data": response_data,
                    "success": status_ok,
                    "timestamp": datetime.now().isoformat()
                }
                
                if status_ok:
                    self.results["passed"] += 1
                    self.log_success(f"{test_name} - {response.status} ({response_time:.1f}ms)")
                    if description:
                        print(f"    {description}")
                else:
                    self.results["failed"] += 1
                    self.log_error(f"{test_name} - Expected {expected_status}, got {response.status}")
                    if description:
                        print(f"    {description}")
                    print(f"    Response: {response_text[:200]}...")
                
                self.results["test_details"].append(test_result)
                return test_result
                
        except Exception as e:
            self.results["failed"] += 1
            error_msg = f"{test_name} - Exception: {str(e)}"
            self.log_error(error_msg)
            
            test_result = {
                "test_name": test_name,
                "description": description,
                "url": f"{self.base_url}{endpoint}",
                "method": method.upper(),
                "expected_status": expected_status,
                "actual_status": None,
                "response_time_ms": None,
                "response_data": None,
                "success": False,
                "error": str(e),
                "traceback": traceback.format_exc(),
                "timestamp": datetime.now().isoformat()
            }
            
            self.results["errors"].append(error_msg)
            self.results["test_details"].append(test_result)
            return test_result
    
    async def test_websocket_endpoint(self, endpoint: str, description: str = "") -> Dict[str, Any]:
        """Test WebSocket endpoint."""
        self.results["total_tests"] += 1
        test_name = f"WebSocket {endpoint}"
        
        try:
            ws_url = f"ws://localhost:8000{endpoint}"
            
            start_time = time.time()
            async with websockets.connect(ws_url) as websocket:
                # Send a test message
                test_message = {"type": "subscribe", "zone_ids": ["zone_1"]}
                await websocket.send(json.dumps(test_message))
                
                # Wait for response
                response = await asyncio.wait_for(websocket.recv(), timeout=3)
                response_time = (time.time() - start_time) * 1000
                
                try:
                    response_data = json.loads(response)
                except json.JSONDecodeError:
                    response_data = {"raw_response": response}
                
                test_result = {
                    "test_name": test_name,
                    "description": description,
                    "url": ws_url,
                    "method": "WebSocket",
                    "response_time_ms": round(response_time, 2),
                    "response_data": response_data,
                    "success": True,
                    "timestamp": datetime.now().isoformat()
                }
                
                self.results["passed"] += 1
                self.log_success(f"{test_name} - Connected ({response_time:.1f}ms)")
                if description:
                    print(f"    {description}")
                
                self.results["test_details"].append(test_result)
                return test_result
                
        except Exception as e:
            self.results["failed"] += 1
            error_msg = f"{test_name} - Exception: {str(e)}"
            self.log_error(error_msg)
            
            test_result = {
                "test_name": test_name,
                "description": description,
                "url": f"ws://localhost:8000{endpoint}",
                "method": "WebSocket",
                "response_time_ms": None,
                "response_data": None,
                "success": False,
                "error": str(e),
                "traceback": traceback.format_exc(),
                "timestamp": datetime.now().isoformat()
            }
            
            self.results["errors"].append(error_msg)
            self.results["test_details"].append(test_result)
            return test_result
    
    async def run_all_tests(self):
        """Run all API endpoint tests."""
        print(f"{Fore.CYAN}{'='*60}")
        print(f"{Fore.CYAN}WiFi-DensePose API Endpoint Testing")
        print(f"{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
        print()
        
        # Test Health Endpoints
        print(f"{Fore.MAGENTA}Testing Health Endpoints:{Style.RESET_ALL}")
        await self.test_endpoint("GET", "/health/health", description="System health check")
        await self.test_endpoint("GET", "/health/ready", description="Readiness check")
        print()
        
        # Test Pose Estimation Endpoints
        print(f"{Fore.MAGENTA}Testing Pose Estimation Endpoints:{Style.RESET_ALL}")
        await self.test_endpoint("GET", "/api/v1/pose/current", description="Current pose estimation")
        await self.test_endpoint("GET", "/api/v1/pose/current",
                                params={"zone_ids": ["zone_1"], "confidence_threshold": 0.7},
                                description="Current pose estimation with parameters")
        await self.test_endpoint("POST", "/api/v1/pose/analyze", description="Pose analysis (requires auth)")
        await self.test_endpoint("GET", "/api/v1/pose/zones/zone_1/occupancy", description="Zone occupancy")
        await self.test_endpoint("GET", "/api/v1/pose/zones/summary", description="All zones summary")
        print()
        
        # Test Historical Data Endpoints
        print(f"{Fore.MAGENTA}Testing Historical Data Endpoints:{Style.RESET_ALL}")
        end_time = datetime.now()
        start_time = end_time - timedelta(hours=1)
        historical_data = {
            "start_time": start_time.isoformat(),
            "end_time": end_time.isoformat(),
            "zone_ids": ["zone_1"],
            "aggregation_interval": 300
        }
        await self.test_endpoint("POST", "/api/v1/pose/historical",
                                data=historical_data,
                                description="Historical pose data (requires auth)")
        await self.test_endpoint("GET", "/api/v1/pose/activities", description="Recent activities")
        await self.test_endpoint("GET", "/api/v1/pose/activities",
                                params={"zone_id": "zone_1", "limit": 5},
                                description="Activities for specific zone")
        print()
        
        # Test Calibration Endpoints
        print(f"{Fore.MAGENTA}Testing Calibration Endpoints:{Style.RESET_ALL}")
        await self.test_endpoint("GET", "/api/v1/pose/calibration/status", description="Calibration status (requires auth)")
        await self.test_endpoint("POST", "/api/v1/pose/calibrate", description="Start calibration (requires auth)")
        print()
        
        # Test Statistics Endpoints
        print(f"{Fore.MAGENTA}Testing Statistics Endpoints:{Style.RESET_ALL}")
        await self.test_endpoint("GET", "/api/v1/pose/stats", description="Pose statistics")
        await self.test_endpoint("GET", "/api/v1/pose/stats",
                                params={"hours": 12}, description="Pose statistics (12 hours)")
        print()
        
        # Test Stream Endpoints
        print(f"{Fore.MAGENTA}Testing Stream Endpoints:{Style.RESET_ALL}")
        await self.test_endpoint("GET", "/api/v1/stream/status", description="Stream status")
        await self.test_endpoint("POST", "/api/v1/stream/start", description="Start streaming (requires auth)")
        await self.test_endpoint("POST", "/api/v1/stream/stop", description="Stop streaming (requires auth)")
        print()
        
        # Test WebSocket Endpoints
        print(f"{Fore.MAGENTA}Testing WebSocket Endpoints:{Style.RESET_ALL}")
        await self.test_websocket_endpoint("/api/v1/stream/pose", description="Pose WebSocket")
        await self.test_websocket_endpoint("/api/v1/stream/events", description="Events WebSocket")
        print()
        
        # Test Documentation Endpoints
        print(f"{Fore.MAGENTA}Testing Documentation Endpoints:{Style.RESET_ALL}")
        await self.test_endpoint("GET", "/docs", description="API documentation")
        await self.test_endpoint("GET", "/openapi.json", description="OpenAPI schema")
        print()
        
        # Test API Info Endpoints
        print(f"{Fore.MAGENTA}Testing API Info Endpoints:{Style.RESET_ALL}")
        await self.test_endpoint("GET", "/", description="Root endpoint")
        await self.test_endpoint("GET", "/api/v1/info", description="API information")
        await self.test_endpoint("GET", "/api/v1/status", description="API status")
        print()
        
        # Test Error Cases
        print(f"{Fore.MAGENTA}Testing Error Cases:{Style.RESET_ALL}")
        await self.test_endpoint("GET", "/nonexistent", expected_status=404,
                                description="Non-existent endpoint")
        await self.test_endpoint("POST", "/api/v1/pose/analyze",
                                data={"invalid": "data"}, expected_status=401,
                                description="Unauthorized request (no auth)")
        print()
    
    def print_summary(self):
        """Print test summary."""
        print(f"{Fore.CYAN}{'='*60}")
        print(f"{Fore.CYAN}Test Summary")
        print(f"{Fore.CYAN}{'='*60}{Style.RESET_ALL}")
        
        total = self.results["total_tests"]
        passed = self.results["passed"]
        failed = self.results["failed"]
        success_rate = (passed / total * 100) if total > 0 else 0
        
        print(f"Total Tests: {total}")
        print(f"{Fore.GREEN}Passed: {passed}{Style.RESET_ALL}")
        print(f"{Fore.RED}Failed: {failed}{Style.RESET_ALL}")
        print(f"Success Rate: {success_rate:.1f}%")
        print()
        
        if self.results["errors"]:
            print(f"{Fore.RED}Errors:{Style.RESET_ALL}")
            for error in self.results["errors"]:
                print(f"  - {error}")
            print()
        
        # Save detailed results to file
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        results_file = f"scripts/api_test_results_{timestamp}.json"
        
        try:
            with open(results_file, 'w') as f:
                json.dump(self.results, f, indent=2, default=str)
            print(f"Detailed results saved to: {results_file}")
        except Exception as e:
            self.log_warning(f"Could not save results file: {e}")
        
        return failed == 0

async def main():
    """Main test function."""
    try:
        async with APITester() as tester:
            await tester.run_all_tests()
            success = tester.print_summary()
            
            # Exit with appropriate code
            sys.exit(0 if success else 1)
            
    except KeyboardInterrupt:
        print(f"\n{Fore.YELLOW}Tests interrupted by user{Style.RESET_ALL}")
        sys.exit(1)
    except Exception as e:
        print(f"\n{Fore.RED}Fatal error: {e}{Style.RESET_ALL}")
        traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    # Check if required packages are available
    try:
        import aiohttp
        import websockets
        import colorama
    except ImportError as e:
        print(f"Missing required package: {e}")
        print("Install with: pip install aiohttp websockets colorama")
        sys.exit(1)
    
    # Run tests
    asyncio.run(main())
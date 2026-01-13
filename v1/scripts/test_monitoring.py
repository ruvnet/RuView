#!/usr/bin/env python3
"""
Test script for WiFi-DensePose monitoring functionality
"""

import asyncio
import aiohttp
import json
import sys
from datetime import datetime
from typing import Dict, Any, List
import time


class MonitoringTester:
    """Test monitoring endpoints and metrics collection."""
    
    def __init__(self, base_url: str = "http://localhost:8000"):
        self.base_url = base_url
        self.session = None
        self.results = []
    
    async def setup(self):
        """Setup test session."""
        self.session = aiohttp.ClientSession()
    
    async def teardown(self):
        """Cleanup test session."""
        if self.session:
            await self.session.close()
    
    async def test_health_endpoint(self):
        """Test the /health endpoint."""
        print("\n[TEST] Health Endpoint")
        try:
            async with self.session.get(f"{self.base_url}/health") as response:
                status = response.status
                data = await response.json()
                
                print(f"Status: {status}")
                print(f"Response: {json.dumps(data, indent=2)}")
                
                self.results.append({
                    "test": "health_endpoint",
                    "status": "passed" if status == 200 else "failed",
                    "response_code": status,
                    "data": data
                })
                
                # Verify structure
                assert "status" in data
                assert "timestamp" in data
                assert "components" in data
                assert "system_metrics" in data
                
                print("✅ Health endpoint test passed")
                
        except Exception as e:
            print(f"❌ Health endpoint test failed: {e}")
            self.results.append({
                "test": "health_endpoint",
                "status": "failed",
                "error": str(e)
            })
    
    async def test_ready_endpoint(self):
        """Test the /ready endpoint."""
        print("\n[TEST] Readiness Endpoint")
        try:
            async with self.session.get(f"{self.base_url}/ready") as response:
                status = response.status
                data = await response.json()
                
                print(f"Status: {status}")
                print(f"Response: {json.dumps(data, indent=2)}")
                
                self.results.append({
                    "test": "ready_endpoint",
                    "status": "passed" if status == 200 else "failed",
                    "response_code": status,
                    "data": data
                })
                
                # Verify structure
                assert "ready" in data
                assert "timestamp" in data
                assert "checks" in data
                assert "message" in data
                
                print("✅ Readiness endpoint test passed")
                
        except Exception as e:
            print(f"❌ Readiness endpoint test failed: {e}")
            self.results.append({
                "test": "ready_endpoint",
                "status": "failed",
                "error": str(e)
            })
    
    async def test_liveness_endpoint(self):
        """Test the /live endpoint."""
        print("\n[TEST] Liveness Endpoint")
        try:
            async with self.session.get(f"{self.base_url}/live") as response:
                status = response.status
                data = await response.json()
                
                print(f"Status: {status}")
                print(f"Response: {json.dumps(data, indent=2)}")
                
                self.results.append({
                    "test": "liveness_endpoint",
                    "status": "passed" if status == 200 else "failed",
                    "response_code": status,
                    "data": data
                })
                
                # Verify structure
                assert "status" in data
                assert "timestamp" in data
                
                print("✅ Liveness endpoint test passed")
                
        except Exception as e:
            print(f"❌ Liveness endpoint test failed: {e}")
            self.results.append({
                "test": "liveness_endpoint",
                "status": "failed",
                "error": str(e)
            })
    
    async def test_metrics_endpoint(self):
        """Test the /metrics endpoint."""
        print("\n[TEST] Metrics Endpoint")
        try:
            async with self.session.get(f"{self.base_url}/metrics") as response:
                status = response.status
                data = await response.json()
                
                print(f"Status: {status}")
                print(f"Response: {json.dumps(data, indent=2)}")
                
                self.results.append({
                    "test": "metrics_endpoint",
                    "status": "passed" if status == 200 else "failed",
                    "response_code": status,
                    "data": data
                })
                
                # Verify structure
                assert "timestamp" in data
                assert "metrics" in data
                
                # Check for system metrics
                metrics = data.get("metrics", {})
                assert "cpu" in metrics
                assert "memory" in metrics
                assert "disk" in metrics
                assert "network" in metrics
                
                print("✅ Metrics endpoint test passed")
                
        except Exception as e:
            print(f"❌ Metrics endpoint test failed: {e}")
            self.results.append({
                "test": "metrics_endpoint",
                "status": "failed",
                "error": str(e)
            })
    
    async def test_version_endpoint(self):
        """Test the /version endpoint."""
        print("\n[TEST] Version Endpoint")
        try:
            async with self.session.get(f"{self.base_url}/version") as response:
                status = response.status
                data = await response.json()
                
                print(f"Status: {status}")
                print(f"Response: {json.dumps(data, indent=2)}")
                
                self.results.append({
                    "test": "version_endpoint",
                    "status": "passed" if status == 200 else "failed",
                    "response_code": status,
                    "data": data
                })
                
                # Verify structure
                assert "name" in data
                assert "version" in data
                assert "environment" in data
                assert "timestamp" in data
                
                print("✅ Version endpoint test passed")
                
        except Exception as e:
            print(f"❌ Version endpoint test failed: {e}")
            self.results.append({
                "test": "version_endpoint",
                "status": "failed",
                "error": str(e)
            })
    
    async def test_metrics_collection(self):
        """Test metrics collection over time."""
        print("\n[TEST] Metrics Collection Over Time")
        try:
            # Collect metrics 3 times with 2-second intervals
            metrics_snapshots = []
            
            for i in range(3):
                async with self.session.get(f"{self.base_url}/metrics") as response:
                    data = await response.json()
                    metrics_snapshots.append({
                        "timestamp": time.time(),
                        "metrics": data.get("metrics", {})
                    })
                
                if i < 2:
                    await asyncio.sleep(2)
            
            # Verify metrics are changing
            cpu_values = [
                snapshot["metrics"].get("cpu", {}).get("percent", 0)
                for snapshot in metrics_snapshots
            ]
            
            print(f"CPU usage over time: {cpu_values}")
            
            # Check if at least some metrics are non-zero
            all_zeros = all(v == 0 for v in cpu_values)
            assert not all_zeros, "All CPU metrics are zero"
            
            self.results.append({
                "test": "metrics_collection",
                "status": "passed",
                "snapshots": len(metrics_snapshots),
                "cpu_values": cpu_values
            })
            
            print("✅ Metrics collection test passed")
            
        except Exception as e:
            print(f"❌ Metrics collection test failed: {e}")
            self.results.append({
                "test": "metrics_collection",
                "status": "failed",
                "error": str(e)
            })
    
    async def test_system_load(self):
        """Test system under load to verify monitoring."""
        print("\n[TEST] System Load Monitoring")
        try:
            # Generate some load by making multiple concurrent requests
            print("Generating load with 20 concurrent requests...")
            
            tasks = []
            for i in range(20):
                tasks.append(self.session.get(f"{self.base_url}/health"))
            
            start_time = time.time()
            responses = await asyncio.gather(*tasks, return_exceptions=True)
            duration = time.time() - start_time
            
            success_count = sum(
                1 for r in responses 
                if not isinstance(r, Exception) and r.status == 200
            )
            
            print(f"Completed {len(responses)} requests in {duration:.2f}s")
            print(f"Success rate: {success_count}/{len(responses)}")
            
            # Check metrics after load
            async with self.session.get(f"{self.base_url}/metrics") as response:
                data = await response.json()
                metrics = data.get("metrics", {})
                
                print(f"CPU after load: {metrics.get('cpu', {}).get('percent', 0)}%")
                print(f"Memory usage: {metrics.get('memory', {}).get('percent', 0)}%")
            
            self.results.append({
                "test": "system_load",
                "status": "passed",
                "requests": len(responses),
                "success_rate": f"{success_count}/{len(responses)}",
                "duration": duration
            })
            
            print("✅ System load monitoring test passed")
            
        except Exception as e:
            print(f"❌ System load monitoring test failed: {e}")
            self.results.append({
                "test": "system_load",
                "status": "failed",
                "error": str(e)
            })
    
    async def run_all_tests(self):
        """Run all monitoring tests."""
        print("=== WiFi-DensePose Monitoring Tests ===")
        print(f"Base URL: {self.base_url}")
        print(f"Started at: {datetime.now().isoformat()}")
        
        await self.setup()
        
        try:
            # Run all tests
            await self.test_health_endpoint()
            await self.test_ready_endpoint()
            await self.test_liveness_endpoint()
            await self.test_metrics_endpoint()
            await self.test_version_endpoint()
            await self.test_metrics_collection()
            await self.test_system_load()
            
        finally:
            await self.teardown()
        
        # Print summary
        print("\n=== Test Summary ===")
        passed = sum(1 for r in self.results if r["status"] == "passed")
        failed = sum(1 for r in self.results if r["status"] == "failed")
        
        print(f"Total tests: {len(self.results)}")
        print(f"Passed: {passed}")
        print(f"Failed: {failed}")
        
        if failed > 0:
            print("\nFailed tests:")
            for result in self.results:
                if result["status"] == "failed":
                    print(f"  - {result['test']}: {result.get('error', 'Unknown error')}")
        
        # Save results
        with open("monitoring_test_results.json", "w") as f:
            json.dump({
                "timestamp": datetime.now().isoformat(),
                "base_url": self.base_url,
                "summary": {
                    "total": len(self.results),
                    "passed": passed,
                    "failed": failed
                },
                "results": self.results
            }, f, indent=2)
        
        print("\nResults saved to monitoring_test_results.json")
        
        return failed == 0


async def main():
    """Main entry point."""
    base_url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8000"
    
    tester = MonitoringTester(base_url)
    success = await tester.run_all_tests()
    
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    asyncio.run(main())
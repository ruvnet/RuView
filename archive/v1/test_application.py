#!/usr/bin/env python3
"""
Test script to verify WiFi-DensePose API functionality
"""

import asyncio
import aiohttp
import json
import websockets
import sys
from typing import Dict, Any

BASE_URL = "http://localhost:8000"
WS_URL = "ws://localhost:8000"

async def test_health_endpoints():
    """Test health check endpoints."""
    print("🔍 Testing health endpoints...")
    
    async with aiohttp.ClientSession() as session:
        # Test basic health
        async with session.get(f"{BASE_URL}/health/health") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ Health check: {data['status']}")
            else:
                print(f"❌ Health check failed: {response.status}")
        
        # Test readiness
        async with session.get(f"{BASE_URL}/health/ready") as response:
            if response.status == 200:
                data = await response.json()
                status = "ready" if data['ready'] else "not ready"
                print(f"✅ Readiness check: {status}")
            else:
                print(f"❌ Readiness check failed: {response.status}")
        
        # Test liveness
        async with session.get(f"{BASE_URL}/health/live") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ Liveness check: {data['status']}")
            else:
                print(f"❌ Liveness check failed: {response.status}")

async def test_api_endpoints():
    """Test main API endpoints."""
    print("\n🔍 Testing API endpoints...")
    
    async with aiohttp.ClientSession() as session:
        # Test root endpoint
        async with session.get(f"{BASE_URL}/") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ Root endpoint: {data['name']} v{data['version']}")
            else:
                print(f"❌ Root endpoint failed: {response.status}")
        
        # Test API info
        async with session.get(f"{BASE_URL}/api/v1/info") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ API info: {len(data['services'])} services configured")
            else:
                print(f"❌ API info failed: {response.status}")
        
        # Test API status
        async with session.get(f"{BASE_URL}/api/v1/status") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ API status: {data['api']['status']}")
            else:
                print(f"❌ API status failed: {response.status}")

async def test_pose_endpoints():
    """Test pose estimation endpoints."""
    print("\n🔍 Testing pose endpoints...")
    
    async with aiohttp.ClientSession() as session:
        # Test current pose data
        async with session.get(f"{BASE_URL}/api/v1/pose/current") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ Current pose data: {len(data.get('poses', []))} poses detected")
            else:
                print(f"❌ Current pose data failed: {response.status}")
        
        # Test zones summary
        async with session.get(f"{BASE_URL}/api/v1/pose/zones/summary") as response:
            if response.status == 200:
                data = await response.json()
                zones = data.get('zones', {})
                print(f"✅ Zones summary: {len(zones)} zones")
                for zone_id, zone_data in list(zones.items())[:3]:  # Show first 3 zones
                    print(f"   - {zone_id}: {zone_data.get('occupancy', 0)} people")
            else:
                print(f"❌ Zones summary failed: {response.status}")
        
        # Test pose stats
        async with session.get(f"{BASE_URL}/api/v1/pose/stats") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ Pose stats: {data.get('total_detections', 0)} total detections")
            else:
                print(f"❌ Pose stats failed: {response.status}")

async def test_stream_endpoints():
    """Test streaming endpoints."""
    print("\n🔍 Testing stream endpoints...")
    
    async with aiohttp.ClientSession() as session:
        # Test stream status
        async with session.get(f"{BASE_URL}/api/v1/stream/status") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ Stream status: {'Active' if data['is_active'] else 'Inactive'}")
                print(f"   - Connected clients: {data['connected_clients']}")
            else:
                print(f"❌ Stream status failed: {response.status}")
        
        # Test stream metrics
        async with session.get(f"{BASE_URL}/api/v1/stream/metrics") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ Stream metrics available")
            else:
                print(f"❌ Stream metrics failed: {response.status}")

async def test_websocket_connection():
    """Test WebSocket connection."""
    print("\n🔍 Testing WebSocket connection...")
    
    try:
        uri = f"{WS_URL}/api/v1/stream/pose"
        async with websockets.connect(uri) as websocket:
            print("✅ WebSocket connected successfully")
            
            # Wait for connection confirmation
            message = await asyncio.wait_for(websocket.recv(), timeout=5.0)
            data = json.loads(message)
            
            if data.get("type") == "connection_established":
                print(f"✅ Connection established with client ID: {data.get('client_id')}")
                
                # Send a ping
                await websocket.send(json.dumps({"type": "ping"}))
                
                # Wait for pong
                pong_message = await asyncio.wait_for(websocket.recv(), timeout=5.0)
                pong_data = json.loads(pong_message)
                
                if pong_data.get("type") == "pong":
                    print("✅ WebSocket ping/pong successful")
                else:
                    print(f"❌ Unexpected pong response: {pong_data}")
            else:
                print(f"❌ Unexpected connection message: {data}")
                
    except asyncio.TimeoutError:
        print("❌ WebSocket connection timeout")
    except Exception as e:
        print(f"❌ WebSocket connection failed: {e}")

async def test_calibration_endpoints():
    """Test calibration endpoints."""
    print("\n🔍 Testing calibration endpoints...")
    
    async with aiohttp.ClientSession() as session:
        # Test calibration status
        async with session.get(f"{BASE_URL}/api/v1/pose/calibration/status") as response:
            if response.status == 200:
                data = await response.json()
                print(f"✅ Calibration status: {data.get('status', 'unknown')}")
            else:
                print(f"❌ Calibration status failed: {response.status}")

async def main():
    """Run all tests."""
    print("🚀 Starting WiFi-DensePose API Tests")
    print("=" * 50)
    
    try:
        await test_health_endpoints()
        await test_api_endpoints()
        await test_pose_endpoints()
        await test_stream_endpoints()
        await test_websocket_connection()
        await test_calibration_endpoints()
        
        print("\n" + "=" * 50)
        print("✅ All tests completed!")
        
    except Exception as e:
        print(f"\n❌ Test suite failed: {e}")
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(main())
#!/usr/bin/env python3
"""
WebSocket Streaming Test Script
Tests real-time pose data streaming via WebSocket
"""

import asyncio
import json
import websockets
from datetime import datetime


async def test_pose_streaming():
    """Test pose data streaming via WebSocket."""
    uri = "ws://localhost:8000/api/v1/stream/pose?zone_ids=zone_1,zone_2&min_confidence=0.3&max_fps=10"
    
    print(f"[{datetime.now()}] Connecting to WebSocket...")
    
    try:
        async with websockets.connect(uri) as websocket:
            print(f"[{datetime.now()}] Connected successfully!")
            
            # Wait for connection confirmation
            response = await websocket.recv()
            data = json.loads(response)
            print(f"[{datetime.now()}] Connection confirmed:")
            print(json.dumps(data, indent=2))
            
            # Send a ping message
            ping_msg = {"type": "ping"}
            await websocket.send(json.dumps(ping_msg))
            print(f"[{datetime.now()}] Sent ping message")
            
            # Listen for messages for 10 seconds
            print(f"[{datetime.now()}] Listening for pose updates...")
            
            start_time = asyncio.get_event_loop().time()
            message_count = 0
            
            while asyncio.get_event_loop().time() - start_time < 10:
                try:
                    # Wait for message with timeout
                    message = await asyncio.wait_for(websocket.recv(), timeout=1.0)
                    data = json.loads(message)
                    message_count += 1
                    
                    msg_type = data.get("type", "unknown")
                    
                    if msg_type == "pose_update":
                        print(f"[{datetime.now()}] Pose update received:")
                        print(f"  - Frame ID: {data.get('frame_id')}")
                        print(f"  - Persons detected: {len(data.get('persons', []))}")
                        print(f"  - Zone summary: {data.get('zone_summary', {})}")
                    elif msg_type == "pong":
                        print(f"[{datetime.now()}] Pong received")
                    else:
                        print(f"[{datetime.now()}] Message type '{msg_type}' received")
                        
                except asyncio.TimeoutError:
                    # No message received in timeout period
                    continue
                except Exception as e:
                    print(f"[{datetime.now()}] Error receiving message: {e}")
                    
            print(f"\n[{datetime.now()}] Test completed!")
            print(f"Total messages received: {message_count}")
            
            # Send disconnect message
            disconnect_msg = {"type": "disconnect"}
            await websocket.send(json.dumps(disconnect_msg))
            
    except Exception as e:
        print(f"[{datetime.now()}] WebSocket error: {e}")


async def test_event_streaming():
    """Test event streaming via WebSocket."""
    uri = "ws://localhost:8000/api/v1/stream/events?event_types=motion,presence&zone_ids=zone_1"
    
    print(f"\n[{datetime.now()}] Testing event streaming...")
    print(f"[{datetime.now()}] Connecting to WebSocket...")
    
    try:
        async with websockets.connect(uri) as websocket:
            print(f"[{datetime.now()}] Connected successfully!")
            
            # Wait for connection confirmation
            response = await websocket.recv()
            data = json.loads(response)
            print(f"[{datetime.now()}] Connection confirmed:")
            print(json.dumps(data, indent=2))
            
            # Get status
            status_msg = {"type": "get_status"}
            await websocket.send(json.dumps(status_msg))
            print(f"[{datetime.now()}] Requested status")
            
            # Listen for a few messages
            for i in range(5):
                try:
                    message = await asyncio.wait_for(websocket.recv(), timeout=2.0)
                    data = json.loads(message)
                    print(f"[{datetime.now()}] Event received: {data.get('type')}")
                except asyncio.TimeoutError:
                    print(f"[{datetime.now()}] No event received (timeout)")
                    
    except Exception as e:
        print(f"[{datetime.now()}] WebSocket error: {e}")


async def test_websocket_errors():
    """Test WebSocket error handling."""
    print(f"\n[{datetime.now()}] Testing error handling...")
    
    # Test invalid endpoint
    try:
        uri = "ws://localhost:8000/api/v1/stream/invalid"
        async with websockets.connect(uri) as websocket:
            print("Connected to invalid endpoint (unexpected)")
    except Exception as e:
        print(f"[{datetime.now()}] Expected error for invalid endpoint: {type(e).__name__}")
    
    # Test sending invalid JSON
    try:
        uri = "ws://localhost:8000/api/v1/stream/pose"
        async with websockets.connect(uri) as websocket:
            await websocket.send("invalid json {")
            response = await websocket.recv()
            data = json.loads(response)
            if data.get("type") == "error":
                print(f"[{datetime.now()}] Received expected error for invalid JSON")
    except Exception as e:
        print(f"[{datetime.now()}] Error testing invalid JSON: {e}")


async def main():
    """Run all WebSocket tests."""
    print("=" * 60)
    print("WiFi-DensePose WebSocket Streaming Tests")
    print("=" * 60)
    
    # Test pose streaming
    await test_pose_streaming()
    
    # Test event streaming
    await test_event_streaming()
    
    # Test error handling
    await test_websocket_errors()
    
    print("\n" + "=" * 60)
    print("All tests completed!")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())
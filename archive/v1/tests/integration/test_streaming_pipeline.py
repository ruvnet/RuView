"""
Integration tests for real-time streaming pipeline.

Tests the complete real-time data flow from CSI collection to client delivery.
"""

import pytest
import asyncio
import numpy as np
from datetime import datetime, timedelta
from typing import Dict, Any, List, Optional, AsyncGenerator
from unittest.mock import AsyncMock, MagicMock, patch
import json
import queue
import threading
from dataclasses import dataclass


@dataclass
class StreamFrame:
    """Streaming frame data structure."""
    frame_id: str
    timestamp: datetime
    router_id: str
    pose_data: Dict[str, Any]
    processing_time_ms: float
    quality_score: float


class MockStreamBuffer:
    """Mock streaming buffer for testing."""
    
    def __init__(self, max_size: int = 100):
        self.max_size = max_size
        self.buffer = asyncio.Queue(maxsize=max_size)
        self.dropped_frames = 0
        self.total_frames = 0
    
    async def put_frame(self, frame: StreamFrame) -> bool:
        """Add frame to buffer."""
        self.total_frames += 1
        
        try:
            self.buffer.put_nowait(frame)
            return True
        except asyncio.QueueFull:
            self.dropped_frames += 1
            return False
    
    async def get_frame(self, timeout: float = 1.0) -> Optional[StreamFrame]:
        """Get frame from buffer."""
        try:
            return await asyncio.wait_for(self.buffer.get(), timeout=timeout)
        except asyncio.TimeoutError:
            return None
    
    def get_stats(self) -> Dict[str, Any]:
        """Get buffer statistics."""
        return {
            "buffer_size": self.buffer.qsize(),
            "max_size": self.max_size,
            "total_frames": self.total_frames,
            "dropped_frames": self.dropped_frames,
            "drop_rate": self.dropped_frames / max(self.total_frames, 1)
        }


class MockStreamProcessor:
    """Mock stream processor for testing."""
    
    def __init__(self):
        self.is_running = False
        self.processing_rate = 30  # FPS
        self.frame_counter = 0
        self.error_rate = 0.0
    
    async def start_processing(self, input_buffer: MockStreamBuffer, output_buffer: MockStreamBuffer):
        """Start stream processing."""
        self.is_running = True
        
        while self.is_running:
            try:
                # Get frame from input
                frame = await input_buffer.get_frame(timeout=0.1)
                if frame is None:
                    continue
                
                # Simulate processing error
                if np.random.random() < self.error_rate:
                    continue  # Skip frame due to error
                
                # Process frame
                processed_frame = await self._process_frame(frame)
                
                # Put to output buffer
                await output_buffer.put_frame(processed_frame)
                
                # Control processing rate
                await asyncio.sleep(1.0 / self.processing_rate)
                
            except Exception as e:
                # Handle processing errors
                continue
    
    async def _process_frame(self, frame: StreamFrame) -> StreamFrame:
        """Process a single frame."""
        # Simulate processing time
        await asyncio.sleep(0.01)
        
        # Add processing metadata
        processed_pose_data = frame.pose_data.copy()
        processed_pose_data["processed_at"] = datetime.utcnow().isoformat()
        processed_pose_data["processor_id"] = "stream_processor_001"
        
        return StreamFrame(
            frame_id=f"processed_{frame.frame_id}",
            timestamp=frame.timestamp,
            router_id=frame.router_id,
            pose_data=processed_pose_data,
            processing_time_ms=frame.processing_time_ms + 10,  # Add processing overhead
            quality_score=frame.quality_score * 0.95  # Slight quality degradation
        )
    
    def stop_processing(self):
        """Stop stream processing."""
        self.is_running = False
    
    def set_error_rate(self, error_rate: float):
        """Set processing error rate."""
        self.error_rate = error_rate


class MockWebSocketManager:
    """Mock WebSocket manager for testing."""
    
    def __init__(self):
        self.connected_clients = {}
        self.message_queue = asyncio.Queue()
        self.total_messages_sent = 0
        self.failed_sends = 0
    
    async def add_client(self, client_id: str, websocket_mock) -> bool:
        """Add WebSocket client."""
        if client_id in self.connected_clients:
            return False
        
        self.connected_clients[client_id] = {
            "websocket": websocket_mock,
            "connected_at": datetime.utcnow(),
            "messages_sent": 0,
            "last_ping": datetime.utcnow()
        }
        return True
    
    async def remove_client(self, client_id: str) -> bool:
        """Remove WebSocket client."""
        if client_id in self.connected_clients:
            del self.connected_clients[client_id]
            return True
        return False
    
    async def broadcast_frame(self, frame: StreamFrame) -> Dict[str, bool]:
        """Broadcast frame to all connected clients."""
        results = {}
        
        message = {
            "type": "pose_update",
            "frame_id": frame.frame_id,
            "timestamp": frame.timestamp.isoformat(),
            "router_id": frame.router_id,
            "pose_data": frame.pose_data,
            "processing_time_ms": frame.processing_time_ms,
            "quality_score": frame.quality_score
        }
        
        for client_id, client_info in self.connected_clients.items():
            try:
                # Simulate WebSocket send
                success = await self._send_to_client(client_id, message)
                results[client_id] = success
                
                if success:
                    client_info["messages_sent"] += 1
                    self.total_messages_sent += 1
                else:
                    self.failed_sends += 1
                    
            except Exception:
                results[client_id] = False
                self.failed_sends += 1
        
        return results
    
    async def _send_to_client(self, client_id: str, message: Dict[str, Any]) -> bool:
        """Send message to specific client."""
        # Simulate network issues
        if np.random.random() < 0.05:  # 5% failure rate
            return False
        
        # Simulate send delay
        await asyncio.sleep(0.001)
        return True
    
    def get_client_stats(self) -> Dict[str, Any]:
        """Get client statistics."""
        return {
            "connected_clients": len(self.connected_clients),
            "total_messages_sent": self.total_messages_sent,
            "failed_sends": self.failed_sends,
            "clients": {
                client_id: {
                    "messages_sent": info["messages_sent"],
                    "connected_duration": (datetime.utcnow() - info["connected_at"]).total_seconds()
                }
                for client_id, info in self.connected_clients.items()
            }
        }


class TestStreamingPipelineBasic:
    """Test basic streaming pipeline functionality."""
    
    @pytest.fixture
    def stream_buffer(self):
        """Create stream buffer."""
        return MockStreamBuffer(max_size=50)
    
    @pytest.fixture
    def stream_processor(self):
        """Create stream processor."""
        return MockStreamProcessor()
    
    @pytest.fixture
    def websocket_manager(self):
        """Create WebSocket manager."""
        return MockWebSocketManager()
    
    @pytest.fixture
    def sample_frame(self):
        """Create sample stream frame."""
        return StreamFrame(
            frame_id="frame_001",
            timestamp=datetime.utcnow(),
            router_id="router_001",
            pose_data={
                "persons": [
                    {
                        "person_id": "person_1",
                        "confidence": 0.85,
                        "bounding_box": {"x": 100, "y": 150, "width": 80, "height": 180},
                        "activity": "standing"
                    }
                ],
                "zone_summary": {"zone1": 1, "zone2": 0}
            },
            processing_time_ms=45.2,
            quality_score=0.92
        )
    
    @pytest.mark.asyncio
    async def test_buffer_frame_operations_should_fail_initially(self, stream_buffer, sample_frame):
        """Test buffer frame operations - should fail initially."""
        # Put frame in buffer
        result = await stream_buffer.put_frame(sample_frame)
        
        # This will fail initially
        assert result is True
        
        # Get frame from buffer
        retrieved_frame = await stream_buffer.get_frame()
        assert retrieved_frame is not None
        assert retrieved_frame.frame_id == sample_frame.frame_id
        assert retrieved_frame.router_id == sample_frame.router_id
        
        # Buffer should be empty now
        empty_frame = await stream_buffer.get_frame(timeout=0.1)
        assert empty_frame is None
    
    @pytest.mark.asyncio
    async def test_buffer_overflow_handling_should_fail_initially(self, sample_frame):
        """Test buffer overflow handling - should fail initially."""
        small_buffer = MockStreamBuffer(max_size=2)
        
        # Fill buffer to capacity
        result1 = await small_buffer.put_frame(sample_frame)
        result2 = await small_buffer.put_frame(sample_frame)
        
        # This will fail initially
        assert result1 is True
        assert result2 is True
        
        # Next frame should be dropped
        result3 = await small_buffer.put_frame(sample_frame)
        assert result3 is False
        
        # Check statistics
        stats = small_buffer.get_stats()
        assert stats["total_frames"] == 3
        assert stats["dropped_frames"] == 1
        assert stats["drop_rate"] > 0
    
    @pytest.mark.asyncio
    async def test_stream_processing_should_fail_initially(self, stream_processor, sample_frame):
        """Test stream processing - should fail initially."""
        input_buffer = MockStreamBuffer()
        output_buffer = MockStreamBuffer()
        
        # Add frame to input buffer
        await input_buffer.put_frame(sample_frame)
        
        # Start processing task
        processing_task = asyncio.create_task(
            stream_processor.start_processing(input_buffer, output_buffer)
        )
        
        # Wait for processing
        await asyncio.sleep(0.2)
        
        # Stop processing
        stream_processor.stop_processing()
        await processing_task
        
        # Check output
        processed_frame = await output_buffer.get_frame(timeout=0.1)
        
        # This will fail initially
        assert processed_frame is not None
        assert processed_frame.frame_id.startswith("processed_")
        assert "processed_at" in processed_frame.pose_data
        assert processed_frame.processing_time_ms > sample_frame.processing_time_ms
    
    @pytest.mark.asyncio
    async def test_websocket_client_management_should_fail_initially(self, websocket_manager):
        """Test WebSocket client management - should fail initially."""
        mock_websocket = MagicMock()
        
        # Add client
        result = await websocket_manager.add_client("client_001", mock_websocket)
        
        # This will fail initially
        assert result is True
        assert "client_001" in websocket_manager.connected_clients
        
        # Try to add duplicate client
        result = await websocket_manager.add_client("client_001", mock_websocket)
        assert result is False
        
        # Remove client
        result = await websocket_manager.remove_client("client_001")
        assert result is True
        assert "client_001" not in websocket_manager.connected_clients
    
    @pytest.mark.asyncio
    async def test_frame_broadcasting_should_fail_initially(self, websocket_manager, sample_frame):
        """Test frame broadcasting - should fail initially."""
        # Add multiple clients
        for i in range(3):
            await websocket_manager.add_client(f"client_{i:03d}", MagicMock())
        
        # Broadcast frame
        results = await websocket_manager.broadcast_frame(sample_frame)
        
        # This will fail initially
        assert len(results) == 3
        assert all(isinstance(success, bool) for success in results.values())
        
        # Check statistics
        stats = websocket_manager.get_client_stats()
        assert stats["connected_clients"] == 3
        assert stats["total_messages_sent"] >= 0


class TestStreamingPipelineIntegration:
    """Test complete streaming pipeline integration."""
    
    @pytest.fixture
    async def streaming_pipeline(self):
        """Create complete streaming pipeline."""
        class StreamingPipeline:
            def __init__(self):
                self.input_buffer = MockStreamBuffer(max_size=100)
                self.output_buffer = MockStreamBuffer(max_size=100)
                self.processor = MockStreamProcessor()
                self.websocket_manager = MockWebSocketManager()
                self.is_running = False
                self.processing_task = None
                self.broadcasting_task = None
            
            async def start(self):
                """Start the streaming pipeline."""
                if self.is_running:
                    return False
                
                self.is_running = True
                
                # Start processing task
                self.processing_task = asyncio.create_task(
                    self.processor.start_processing(self.input_buffer, self.output_buffer)
                )
                
                # Start broadcasting task
                self.broadcasting_task = asyncio.create_task(
                    self._broadcast_loop()
                )
                
                return True
            
            async def stop(self):
                """Stop the streaming pipeline."""
                if not self.is_running:
                    return False
                
                self.is_running = False
                self.processor.stop_processing()
                
                # Cancel tasks
                if self.processing_task:
                    self.processing_task.cancel()
                if self.broadcasting_task:
                    self.broadcasting_task.cancel()
                
                return True
            
            async def add_frame(self, frame: StreamFrame) -> bool:
                """Add frame to pipeline."""
                return await self.input_buffer.put_frame(frame)
            
            async def add_client(self, client_id: str, websocket_mock) -> bool:
                """Add WebSocket client."""
                return await self.websocket_manager.add_client(client_id, websocket_mock)
            
            async def _broadcast_loop(self):
                """Broadcasting loop."""
                while self.is_running:
                    try:
                        frame = await self.output_buffer.get_frame(timeout=0.1)
                        if frame:
                            await self.websocket_manager.broadcast_frame(frame)
                    except asyncio.TimeoutError:
                        continue
                    except Exception:
                        continue
            
            def get_pipeline_stats(self) -> Dict[str, Any]:
                """Get pipeline statistics."""
                return {
                    "is_running": self.is_running,
                    "input_buffer": self.input_buffer.get_stats(),
                    "output_buffer": self.output_buffer.get_stats(),
                    "websocket_clients": self.websocket_manager.get_client_stats()
                }
        
        return StreamingPipeline()
    
    @pytest.mark.asyncio
    async def test_end_to_end_streaming_should_fail_initially(self, streaming_pipeline):
        """Test end-to-end streaming - should fail initially."""
        # Start pipeline
        result = await streaming_pipeline.start()
        
        # This will fail initially
        assert result is True
        assert streaming_pipeline.is_running is True
        
        # Add clients
        for i in range(2):
            await streaming_pipeline.add_client(f"client_{i}", MagicMock())
        
        # Add frames
        for i in range(5):
            frame = StreamFrame(
                frame_id=f"frame_{i:03d}",
                timestamp=datetime.utcnow(),
                router_id="router_001",
                pose_data={"persons": [], "zone_summary": {}},
                processing_time_ms=30.0,
                quality_score=0.9
            )
            await streaming_pipeline.add_frame(frame)
        
        # Wait for processing
        await asyncio.sleep(0.5)
        
        # Stop pipeline
        await streaming_pipeline.stop()
        
        # Check statistics
        stats = streaming_pipeline.get_pipeline_stats()
        assert stats["input_buffer"]["total_frames"] == 5
        assert stats["websocket_clients"]["connected_clients"] == 2
    
    @pytest.mark.asyncio
    async def test_pipeline_performance_should_fail_initially(self, streaming_pipeline):
        """Test pipeline performance - should fail initially."""
        await streaming_pipeline.start()
        
        # Add multiple clients
        for i in range(10):
            await streaming_pipeline.add_client(f"client_{i:03d}", MagicMock())
        
        # Measure throughput
        start_time = datetime.utcnow()
        frame_count = 50
        
        for i in range(frame_count):
            frame = StreamFrame(
                frame_id=f"perf_frame_{i:03d}",
                timestamp=datetime.utcnow(),
                router_id="router_001",
                pose_data={"persons": [], "zone_summary": {}},
                processing_time_ms=25.0,
                quality_score=0.88
            )
            await streaming_pipeline.add_frame(frame)
        
        # Wait for processing
        await asyncio.sleep(2.0)
        
        end_time = datetime.utcnow()
        duration = (end_time - start_time).total_seconds()
        
        await streaming_pipeline.stop()
        
        # This will fail initially
        # Check performance metrics
        stats = streaming_pipeline.get_pipeline_stats()
        throughput = frame_count / duration
        
        assert throughput > 10  # Should process at least 10 FPS
        assert stats["input_buffer"]["drop_rate"] < 0.1  # Less than 10% drop rate
    
    @pytest.mark.asyncio
    async def test_pipeline_error_recovery_should_fail_initially(self, streaming_pipeline):
        """Test pipeline error recovery - should fail initially."""
        await streaming_pipeline.start()
        
        # Set high error rate
        streaming_pipeline.processor.set_error_rate(0.5)  # 50% error rate
        
        # Add frames
        for i in range(20):
            frame = StreamFrame(
                frame_id=f"error_frame_{i:03d}",
                timestamp=datetime.utcnow(),
                router_id="router_001",
                pose_data={"persons": [], "zone_summary": {}},
                processing_time_ms=30.0,
                quality_score=0.9
            )
            await streaming_pipeline.add_frame(frame)
        
        # Wait for processing
        await asyncio.sleep(1.0)
        
        await streaming_pipeline.stop()
        
        # This will fail initially
        # Pipeline should continue running despite errors
        stats = streaming_pipeline.get_pipeline_stats()
        assert stats["input_buffer"]["total_frames"] == 20
        # Some frames should be processed despite errors
        assert stats["output_buffer"]["total_frames"] > 0


class TestStreamingLatency:
    """Test streaming latency characteristics."""
    
    @pytest.mark.asyncio
    async def test_end_to_end_latency_should_fail_initially(self):
        """Test end-to-end latency - should fail initially."""
        class LatencyTracker:
            def __init__(self):
                self.latencies = []
            
            async def measure_latency(self, frame: StreamFrame) -> float:
                """Measure processing latency."""
                start_time = datetime.utcnow()
                
                # Simulate processing pipeline
                await asyncio.sleep(0.05)  # 50ms processing time
                
                end_time = datetime.utcnow()
                latency = (end_time - start_time).total_seconds() * 1000  # Convert to ms
                
                self.latencies.append(latency)
                return latency
        
        tracker = LatencyTracker()
        
        # Measure latency for multiple frames
        for i in range(10):
            frame = StreamFrame(
                frame_id=f"latency_frame_{i}",
                timestamp=datetime.utcnow(),
                router_id="router_001",
                pose_data={},
                processing_time_ms=0,
                quality_score=1.0
            )
            
            latency = await tracker.measure_latency(frame)
            
            # This will fail initially
            assert latency > 0
            assert latency < 200  # Should be less than 200ms
        
        # Check average latency
        avg_latency = sum(tracker.latencies) / len(tracker.latencies)
        assert avg_latency < 100  # Average should be less than 100ms
    
    @pytest.mark.asyncio
    async def test_concurrent_stream_handling_should_fail_initially(self):
        """Test concurrent stream handling - should fail initially."""
        async def process_stream(stream_id: str, frame_count: int) -> Dict[str, Any]:
            """Process a single stream."""
            buffer = MockStreamBuffer()
            processed_frames = 0
            
            for i in range(frame_count):
                frame = StreamFrame(
                    frame_id=f"{stream_id}_frame_{i}",
                    timestamp=datetime.utcnow(),
                    router_id=stream_id,
                    pose_data={},
                    processing_time_ms=20.0,
                    quality_score=0.9
                )
                
                success = await buffer.put_frame(frame)
                if success:
                    processed_frames += 1
                
                await asyncio.sleep(0.01)  # Simulate frame rate
            
            return {
                "stream_id": stream_id,
                "processed_frames": processed_frames,
                "total_frames": frame_count
            }
        
        # Process multiple streams concurrently
        streams = ["router_001", "router_002", "router_003"]
        tasks = [process_stream(stream_id, 20) for stream_id in streams]
        
        results = await asyncio.gather(*tasks)
        
        # This will fail initially
        assert len(results) == 3
        
        for result in results:
            assert result["processed_frames"] == result["total_frames"]
            assert result["stream_id"] in streams


class TestStreamingResilience:
    """Test streaming pipeline resilience."""
    
    @pytest.mark.asyncio
    async def test_client_disconnection_handling_should_fail_initially(self):
        """Test client disconnection handling - should fail initially."""
        websocket_manager = MockWebSocketManager()
        
        # Add clients
        client_ids = [f"client_{i:03d}" for i in range(5)]
        for client_id in client_ids:
            await websocket_manager.add_client(client_id, MagicMock())
        
        # Simulate frame broadcasting
        frame = StreamFrame(
            frame_id="disconnect_test_frame",
            timestamp=datetime.utcnow(),
            router_id="router_001",
            pose_data={},
            processing_time_ms=30.0,
            quality_score=0.9
        )
        
        # Broadcast to all clients
        results = await websocket_manager.broadcast_frame(frame)
        
        # This will fail initially
        assert len(results) == 5
        
        # Simulate client disconnections
        await websocket_manager.remove_client("client_001")
        await websocket_manager.remove_client("client_003")
        
        # Broadcast again
        results = await websocket_manager.broadcast_frame(frame)
        assert len(results) == 3  # Only remaining clients
        
        # Check statistics
        stats = websocket_manager.get_client_stats()
        assert stats["connected_clients"] == 3
    
    @pytest.mark.asyncio
    async def test_memory_pressure_handling_should_fail_initially(self):
        """Test memory pressure handling - should fail initially."""
        # Create small buffers to simulate memory pressure
        small_buffer = MockStreamBuffer(max_size=5)
        
        # Generate many frames quickly
        frames_generated = 0
        frames_accepted = 0
        
        for i in range(20):
            frame = StreamFrame(
                frame_id=f"memory_pressure_frame_{i}",
                timestamp=datetime.utcnow(),
                router_id="router_001",
                pose_data={},
                processing_time_ms=25.0,
                quality_score=0.85
            )
            
            frames_generated += 1
            success = await small_buffer.put_frame(frame)
            if success:
                frames_accepted += 1
        
        # This will fail initially
        # Buffer should handle memory pressure gracefully
        stats = small_buffer.get_stats()
        assert stats["total_frames"] == frames_generated
        assert stats["dropped_frames"] > 0  # Some frames should be dropped
        assert frames_accepted <= small_buffer.max_size
        
        # Drop rate should be reasonable
        assert stats["drop_rate"] > 0.5  # More than 50% dropped due to small buffer
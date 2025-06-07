"""
Real-time streaming service for WiFi-DensePose API
"""

import logging
import asyncio
import json
from typing import Dict, List, Optional, Any, Set
from datetime import datetime
from collections import deque

import numpy as np
from fastapi import WebSocket

from src.config.settings import Settings
from src.config.domains import DomainConfig

logger = logging.getLogger(__name__)


class StreamService:
    """Service for real-time data streaming."""
    
    def __init__(self, settings: Settings, domain_config: DomainConfig):
        """Initialize stream service."""
        self.settings = settings
        self.domain_config = domain_config
        self.logger = logging.getLogger(__name__)
        
        # WebSocket connections
        self.connections: Set[WebSocket] = set()
        self.connection_metadata: Dict[WebSocket, Dict[str, Any]] = {}
        
        # Stream buffers
        self.pose_buffer = deque(maxlen=self.settings.stream_buffer_size)
        self.csi_buffer = deque(maxlen=self.settings.stream_buffer_size)
        
        # Service state
        self.is_running = False
        self.last_error = None
        
        # Streaming statistics
        self.stats = {
            "active_connections": 0,
            "total_connections": 0,
            "messages_sent": 0,
            "messages_failed": 0,
            "data_points_streamed": 0,
            "average_latency_ms": 0.0
        }
        
        # Background tasks
        self.streaming_task = None
    
    async def initialize(self):
        """Initialize the stream service."""
        self.logger.info("Stream service initialized")
    
    async def start(self):
        """Start the stream service."""
        if self.is_running:
            return
        
        self.is_running = True
        self.logger.info("Stream service started")
        
        # Start background streaming task
        if self.settings.enable_real_time_processing:
            self.streaming_task = asyncio.create_task(self._streaming_loop())
    
    async def stop(self):
        """Stop the stream service."""
        self.is_running = False
        
        # Cancel background task
        if self.streaming_task:
            self.streaming_task.cancel()
            try:
                await self.streaming_task
            except asyncio.CancelledError:
                pass
        
        # Close all connections
        await self._close_all_connections()
        
        self.logger.info("Stream service stopped")
    
    async def add_connection(self, websocket: WebSocket, metadata: Dict[str, Any] = None):
        """Add a new WebSocket connection."""
        try:
            await websocket.accept()
            self.connections.add(websocket)
            self.connection_metadata[websocket] = metadata or {}
            
            self.stats["active_connections"] = len(self.connections)
            self.stats["total_connections"] += 1
            
            self.logger.info(f"New WebSocket connection added. Total: {len(self.connections)}")
            
            # Send initial data if available
            await self._send_initial_data(websocket)
            
        except Exception as e:
            self.logger.error(f"Error adding WebSocket connection: {e}")
            raise
    
    async def remove_connection(self, websocket: WebSocket):
        """Remove a WebSocket connection."""
        try:
            if websocket in self.connections:
                self.connections.remove(websocket)
                self.connection_metadata.pop(websocket, None)
                
                self.stats["active_connections"] = len(self.connections)
                
                self.logger.info(f"WebSocket connection removed. Total: {len(self.connections)}")
            
        except Exception as e:
            self.logger.error(f"Error removing WebSocket connection: {e}")
    
    async def broadcast_pose_data(self, pose_data: Dict[str, Any]):
        """Broadcast pose data to all connected clients."""
        if not self.is_running:
            return
        
        # Add to buffer
        self.pose_buffer.append({
            "type": "pose_data",
            "timestamp": datetime.now().isoformat(),
            "data": pose_data
        })
        
        # Broadcast to all connections
        await self._broadcast_message({
            "type": "pose_update",
            "timestamp": datetime.now().isoformat(),
            "data": pose_data
        })
    
    async def broadcast_csi_data(self, csi_data: np.ndarray, metadata: Dict[str, Any]):
        """Broadcast CSI data to all connected clients."""
        if not self.is_running:
            return
        
        # Convert numpy array to list for JSON serialization
        csi_list = csi_data.tolist() if isinstance(csi_data, np.ndarray) else csi_data
        
        # Add to buffer
        self.csi_buffer.append({
            "type": "csi_data",
            "timestamp": datetime.now().isoformat(),
            "data": csi_list,
            "metadata": metadata
        })
        
        # Broadcast to all connections
        await self._broadcast_message({
            "type": "csi_update",
            "timestamp": datetime.now().isoformat(),
            "data": csi_list,
            "metadata": metadata
        })
    
    async def broadcast_system_status(self, status_data: Dict[str, Any]):
        """Broadcast system status to all connected clients."""
        if not self.is_running:
            return
        
        await self._broadcast_message({
            "type": "system_status",
            "timestamp": datetime.now().isoformat(),
            "data": status_data
        })
    
    async def send_to_connection(self, websocket: WebSocket, message: Dict[str, Any]):
        """Send message to a specific connection."""
        try:
            if websocket in self.connections:
                await websocket.send_text(json.dumps(message))
                self.stats["messages_sent"] += 1
                
        except Exception as e:
            self.logger.error(f"Error sending message to connection: {e}")
            self.stats["messages_failed"] += 1
            await self.remove_connection(websocket)
    
    async def _broadcast_message(self, message: Dict[str, Any]):
        """Broadcast message to all connected clients."""
        if not self.connections:
            return
        
        disconnected = set()
        
        for websocket in self.connections.copy():
            try:
                await websocket.send_text(json.dumps(message))
                self.stats["messages_sent"] += 1
                
            except Exception as e:
                self.logger.warning(f"Failed to send message to connection: {e}")
                self.stats["messages_failed"] += 1
                disconnected.add(websocket)
        
        # Remove disconnected clients
        for websocket in disconnected:
            await self.remove_connection(websocket)
        
        if message.get("type") in ["pose_update", "csi_update"]:
            self.stats["data_points_streamed"] += 1
    
    async def _send_initial_data(self, websocket: WebSocket):
        """Send initial data to a new connection."""
        try:
            # Send recent pose data
            if self.pose_buffer:
                recent_poses = list(self.pose_buffer)[-10:]  # Last 10 poses
                await self.send_to_connection(websocket, {
                    "type": "initial_poses",
                    "timestamp": datetime.now().isoformat(),
                    "data": recent_poses
                })
            
            # Send recent CSI data
            if self.csi_buffer:
                recent_csi = list(self.csi_buffer)[-5:]  # Last 5 CSI readings
                await self.send_to_connection(websocket, {
                    "type": "initial_csi",
                    "timestamp": datetime.now().isoformat(),
                    "data": recent_csi
                })
            
            # Send service status
            status = await self.get_status()
            await self.send_to_connection(websocket, {
                "type": "service_status",
                "timestamp": datetime.now().isoformat(),
                "data": status
            })
            
        except Exception as e:
            self.logger.error(f"Error sending initial data: {e}")
    
    async def _streaming_loop(self):
        """Background streaming loop for periodic updates."""
        try:
            while self.is_running:
                # Send periodic heartbeat
                if self.connections:
                    await self._broadcast_message({
                        "type": "heartbeat",
                        "timestamp": datetime.now().isoformat(),
                        "active_connections": len(self.connections)
                    })
                
                # Wait for next iteration
                await asyncio.sleep(self.settings.websocket_ping_interval)
                
        except asyncio.CancelledError:
            self.logger.info("Streaming loop cancelled")
        except Exception as e:
            self.logger.error(f"Error in streaming loop: {e}")
            self.last_error = str(e)
    
    async def _close_all_connections(self):
        """Close all WebSocket connections."""
        disconnected = []
        
        for websocket in self.connections.copy():
            try:
                await websocket.close()
                disconnected.append(websocket)
            except Exception as e:
                self.logger.warning(f"Error closing connection: {e}")
                disconnected.append(websocket)
        
        # Clear all connections
        for websocket in disconnected:
            await self.remove_connection(websocket)
    
    async def get_status(self) -> Dict[str, Any]:
        """Get service status."""
        return {
            "status": "healthy" if self.is_running and not self.last_error else "unhealthy",
            "running": self.is_running,
            "last_error": self.last_error,
            "connections": {
                "active": len(self.connections),
                "total": self.stats["total_connections"]
            },
            "buffers": {
                "pose_buffer_size": len(self.pose_buffer),
                "csi_buffer_size": len(self.csi_buffer),
                "max_buffer_size": self.settings.stream_buffer_size
            },
            "statistics": self.stats.copy(),
            "configuration": {
                "stream_fps": self.settings.stream_fps,
                "buffer_size": self.settings.stream_buffer_size,
                "ping_interval": self.settings.websocket_ping_interval,
                "timeout": self.settings.websocket_timeout
            }
        }
    
    async def get_metrics(self) -> Dict[str, Any]:
        """Get service metrics."""
        total_messages = self.stats["messages_sent"] + self.stats["messages_failed"]
        success_rate = self.stats["messages_sent"] / max(1, total_messages)
        
        return {
            "stream_service": {
                "active_connections": self.stats["active_connections"],
                "total_connections": self.stats["total_connections"],
                "messages_sent": self.stats["messages_sent"],
                "messages_failed": self.stats["messages_failed"],
                "message_success_rate": success_rate,
                "data_points_streamed": self.stats["data_points_streamed"],
                "average_latency_ms": self.stats["average_latency_ms"]
            }
        }
    
    async def get_connection_info(self) -> List[Dict[str, Any]]:
        """Get information about active connections."""
        connections_info = []
        
        for websocket in self.connections:
            metadata = self.connection_metadata.get(websocket, {})
            
            connection_info = {
                "id": id(websocket),
                "connected_at": metadata.get("connected_at", "unknown"),
                "user_agent": metadata.get("user_agent", "unknown"),
                "ip_address": metadata.get("ip_address", "unknown"),
                "subscription_types": metadata.get("subscription_types", [])
            }
            
            connections_info.append(connection_info)
        
        return connections_info
    
    async def reset(self):
        """Reset service state."""
        # Clear buffers
        self.pose_buffer.clear()
        self.csi_buffer.clear()
        
        # Reset statistics
        self.stats = {
            "active_connections": len(self.connections),
            "total_connections": 0,
            "messages_sent": 0,
            "messages_failed": 0,
            "data_points_streamed": 0,
            "average_latency_ms": 0.0
        }
        
        self.last_error = None
        self.logger.info("Stream service reset")
    
    def get_buffer_data(self, buffer_type: str, limit: int = 100) -> List[Dict[str, Any]]:
        """Get data from buffers."""
        if buffer_type == "pose":
            return list(self.pose_buffer)[-limit:]
        elif buffer_type == "csi":
            return list(self.csi_buffer)[-limit:]
        else:
            return []
    
    @property
    def is_active(self) -> bool:
        """Check if stream service is active."""
        return self.is_running
    
    async def health_check(self) -> Dict[str, Any]:
        """Perform health check."""
        try:
            status = "healthy" if self.is_running and not self.last_error else "unhealthy"
            
            return {
                "status": status,
                "message": self.last_error if self.last_error else "Stream service is running normally",
                "active_connections": len(self.connections),
                "metrics": {
                    "messages_sent": self.stats["messages_sent"],
                    "messages_failed": self.stats["messages_failed"],
                    "data_points_streamed": self.stats["data_points_streamed"]
                }
            }
            
        except Exception as e:
            return {
                "status": "unhealthy",
                "message": f"Health check failed: {str(e)}"
            }
    
    async def is_ready(self) -> bool:
        """Check if service is ready."""
        return self.is_running
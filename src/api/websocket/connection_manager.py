"""
WebSocket connection manager for WiFi-DensePose API
"""

import asyncio
import json
import logging
import uuid
from typing import Dict, List, Optional, Any, Set
from datetime import datetime, timedelta
from collections import defaultdict

from fastapi import WebSocket, WebSocketDisconnect

logger = logging.getLogger(__name__)


class WebSocketConnection:
    """Represents a WebSocket connection with metadata."""
    
    def __init__(
        self,
        websocket: WebSocket,
        client_id: str,
        stream_type: str,
        zone_ids: Optional[List[str]] = None,
        **config
    ):
        self.websocket = websocket
        self.client_id = client_id
        self.stream_type = stream_type
        self.zone_ids = zone_ids or []
        self.config = config
        self.connected_at = datetime.utcnow()
        self.last_ping = datetime.utcnow()
        self.message_count = 0
        self.is_active = True
    
    async def send_json(self, data: Dict[str, Any]):
        """Send JSON data to client."""
        try:
            await self.websocket.send_json(data)
            self.message_count += 1
        except Exception as e:
            logger.error(f"Error sending to client {self.client_id}: {e}")
            self.is_active = False
            raise
    
    async def send_text(self, message: str):
        """Send text message to client."""
        try:
            await self.websocket.send_text(message)
            self.message_count += 1
        except Exception as e:
            logger.error(f"Error sending text to client {self.client_id}: {e}")
            self.is_active = False
            raise
    
    def update_config(self, config: Dict[str, Any]):
        """Update connection configuration."""
        self.config.update(config)
        
        # Update zone IDs if provided
        if "zone_ids" in config:
            self.zone_ids = config["zone_ids"] or []
    
    def matches_filter(
        self,
        stream_type: Optional[str] = None,
        zone_ids: Optional[List[str]] = None,
        **filters
    ) -> bool:
        """Check if connection matches given filters."""
        # Check stream type
        if stream_type and self.stream_type != stream_type:
            return False
        
        # Check zone IDs
        if zone_ids:
            if not self.zone_ids:  # Connection listens to all zones
                return True
            # Check if any requested zone is in connection's zones
            if not any(zone in self.zone_ids for zone in zone_ids):
                return False
        
        # Check additional filters
        for key, value in filters.items():
            if key in self.config and self.config[key] != value:
                return False
        
        return True
    
    def get_info(self) -> Dict[str, Any]:
        """Get connection information."""
        return {
            "client_id": self.client_id,
            "stream_type": self.stream_type,
            "zone_ids": self.zone_ids,
            "config": self.config,
            "connected_at": self.connected_at.isoformat(),
            "last_ping": self.last_ping.isoformat(),
            "message_count": self.message_count,
            "is_active": self.is_active,
            "uptime_seconds": (datetime.utcnow() - self.connected_at).total_seconds()
        }


class ConnectionManager:
    """Manages WebSocket connections for real-time streaming."""
    
    def __init__(self):
        self.connections: Dict[str, WebSocketConnection] = {}
        self.connections_by_type: Dict[str, Set[str]] = defaultdict(set)
        self.connections_by_zone: Dict[str, Set[str]] = defaultdict(set)
        self.metrics = {
            "total_connections": 0,
            "active_connections": 0,
            "messages_sent": 0,
            "errors": 0,
            "start_time": datetime.utcnow()
        }
        self._cleanup_task = None
        self._started = False
    
    async def connect(
        self,
        websocket: WebSocket,
        stream_type: str,
        zone_ids: Optional[List[str]] = None,
        **config
    ) -> str:
        """Register a new WebSocket connection."""
        client_id = str(uuid.uuid4())
        
        try:
            # Create connection object
            connection = WebSocketConnection(
                websocket=websocket,
                client_id=client_id,
                stream_type=stream_type,
                zone_ids=zone_ids,
                **config
            )
            
            # Store connection
            self.connections[client_id] = connection
            self.connections_by_type[stream_type].add(client_id)
            
            # Index by zones
            if zone_ids:
                for zone_id in zone_ids:
                    self.connections_by_zone[zone_id].add(client_id)
            
            # Update metrics
            self.metrics["total_connections"] += 1
            self.metrics["active_connections"] = len(self.connections)
            
            logger.info(f"WebSocket client {client_id} connected for {stream_type}")
            
            return client_id
            
        except Exception as e:
            logger.error(f"Error connecting WebSocket client: {e}")
            raise
    
    async def disconnect(self, client_id: str) -> bool:
        """Disconnect a WebSocket client."""
        if client_id not in self.connections:
            return False
        
        try:
            connection = self.connections[client_id]
            
            # Remove from indexes
            self.connections_by_type[connection.stream_type].discard(client_id)
            
            for zone_id in connection.zone_ids:
                self.connections_by_zone[zone_id].discard(client_id)
            
            # Close WebSocket if still active
            if connection.is_active:
                try:
                    await connection.websocket.close()
                except:
                    pass  # Connection might already be closed
            
            # Remove connection
            del self.connections[client_id]
            
            # Update metrics
            self.metrics["active_connections"] = len(self.connections)
            
            logger.info(f"WebSocket client {client_id} disconnected")
            
            return True
            
        except Exception as e:
            logger.error(f"Error disconnecting client {client_id}: {e}")
            return False
    
    async def disconnect_all(self):
        """Disconnect all WebSocket clients."""
        client_ids = list(self.connections.keys())
        
        for client_id in client_ids:
            await self.disconnect(client_id)
        
        logger.info("All WebSocket clients disconnected")
    
    async def send_to_client(self, client_id: str, data: Dict[str, Any]) -> bool:
        """Send data to a specific client."""
        if client_id not in self.connections:
            return False
        
        connection = self.connections[client_id]
        
        try:
            await connection.send_json(data)
            self.metrics["messages_sent"] += 1
            return True
            
        except Exception as e:
            logger.error(f"Error sending to client {client_id}: {e}")
            self.metrics["errors"] += 1
            
            # Mark connection as inactive and schedule for cleanup
            connection.is_active = False
            return False
    
    async def broadcast(
        self,
        data: Dict[str, Any],
        stream_type: Optional[str] = None,
        zone_ids: Optional[List[str]] = None,
        **filters
    ) -> int:
        """Broadcast data to matching clients."""
        sent_count = 0
        failed_clients = []
        
        # Get matching connections
        matching_clients = self._get_matching_clients(
            stream_type=stream_type,
            zone_ids=zone_ids,
            **filters
        )
        
        # Send to all matching clients
        for client_id in matching_clients:
            try:
                success = await self.send_to_client(client_id, data)
                if success:
                    sent_count += 1
                else:
                    failed_clients.append(client_id)
            except Exception as e:
                logger.error(f"Error broadcasting to client {client_id}: {e}")
                failed_clients.append(client_id)
        
        # Clean up failed connections
        for client_id in failed_clients:
            await self.disconnect(client_id)
        
        return sent_count
    
    async def update_client_config(self, client_id: str, config: Dict[str, Any]) -> bool:
        """Update client configuration."""
        if client_id not in self.connections:
            return False
        
        connection = self.connections[client_id]
        old_zones = set(connection.zone_ids)
        
        # Update configuration
        connection.update_config(config)
        
        # Update zone indexes if zones changed
        new_zones = set(connection.zone_ids)
        
        # Remove from old zones
        for zone_id in old_zones - new_zones:
            self.connections_by_zone[zone_id].discard(client_id)
        
        # Add to new zones
        for zone_id in new_zones - old_zones:
            self.connections_by_zone[zone_id].add(client_id)
        
        return True
    
    async def get_client_status(self, client_id: str) -> Optional[Dict[str, Any]]:
        """Get status of a specific client."""
        if client_id not in self.connections:
            return None
        
        return self.connections[client_id].get_info()
    
    async def get_connected_clients(self) -> List[Dict[str, Any]]:
        """Get list of all connected clients."""
        return [conn.get_info() for conn in self.connections.values()]
    
    async def get_connection_stats(self) -> Dict[str, Any]:
        """Get connection statistics."""
        stats = {
            "total_clients": len(self.connections),
            "clients_by_type": {
                stream_type: len(clients) 
                for stream_type, clients in self.connections_by_type.items()
            },
            "clients_by_zone": {
                zone_id: len(clients)
                for zone_id, clients in self.connections_by_zone.items()
                if clients  # Only include zones with active clients
            },
            "active_clients": sum(1 for conn in self.connections.values() if conn.is_active),
            "inactive_clients": sum(1 for conn in self.connections.values() if not conn.is_active)
        }
        
        return stats
    
    async def get_metrics(self) -> Dict[str, Any]:
        """Get detailed metrics."""
        uptime = (datetime.utcnow() - self.metrics["start_time"]).total_seconds()
        
        return {
            **self.metrics,
            "active_connections": len(self.connections),
            "uptime_seconds": uptime,
            "messages_per_second": self.metrics["messages_sent"] / max(uptime, 1),
            "error_rate": self.metrics["errors"] / max(self.metrics["messages_sent"], 1)
        }
    
    def _get_matching_clients(
        self,
        stream_type: Optional[str] = None,
        zone_ids: Optional[List[str]] = None,
        **filters
    ) -> List[str]:
        """Get client IDs that match the given filters."""
        candidates = set(self.connections.keys())
        
        # Filter by stream type
        if stream_type:
            type_clients = self.connections_by_type.get(stream_type, set())
            candidates &= type_clients
        
        # Filter by zones
        if zone_ids:
            zone_clients = set()
            for zone_id in zone_ids:
                zone_clients.update(self.connections_by_zone.get(zone_id, set()))
            
            # Also include clients listening to all zones (empty zone list)
            all_zone_clients = {
                client_id for client_id, conn in self.connections.items()
                if not conn.zone_ids
            }
            zone_clients.update(all_zone_clients)
            
            candidates &= zone_clients
        
        # Apply additional filters
        matching_clients = []
        for client_id in candidates:
            connection = self.connections[client_id]
            if connection.is_active and connection.matches_filter(**filters):
                matching_clients.append(client_id)
        
        return matching_clients
    
    async def ping_clients(self):
        """Send ping to all connected clients."""
        ping_data = {
            "type": "ping",
            "timestamp": datetime.utcnow().isoformat()
        }
        
        failed_clients = []
        
        for client_id, connection in self.connections.items():
            try:
                await connection.send_json(ping_data)
                connection.last_ping = datetime.utcnow()
            except Exception as e:
                logger.warning(f"Ping failed for client {client_id}: {e}")
                failed_clients.append(client_id)
        
        # Clean up failed connections
        for client_id in failed_clients:
            await self.disconnect(client_id)
    
    async def cleanup_inactive_connections(self):
        """Clean up inactive or stale connections."""
        now = datetime.utcnow()
        stale_threshold = timedelta(minutes=5)  # 5 minutes without ping
        
        stale_clients = []
        
        for client_id, connection in self.connections.items():
            # Check if connection is inactive
            if not connection.is_active:
                stale_clients.append(client_id)
                continue
            
            # Check if connection is stale (no ping response)
            if now - connection.last_ping > stale_threshold:
                logger.warning(f"Client {client_id} appears stale, disconnecting")
                stale_clients.append(client_id)
        
        # Clean up stale connections
        for client_id in stale_clients:
            await self.disconnect(client_id)
        
        if stale_clients:
            logger.info(f"Cleaned up {len(stale_clients)} stale connections")
    
    async def start(self):
        """Start the connection manager."""
        if not self._started:
            self._start_cleanup_task()
            self._started = True
            logger.info("Connection manager started")
    
    def _start_cleanup_task(self):
        """Start background cleanup task."""
        async def cleanup_loop():
            while True:
                try:
                    await asyncio.sleep(60)  # Run every minute
                    await self.cleanup_inactive_connections()
                    
                    # Send periodic ping every 2 minutes
                    if datetime.utcnow().minute % 2 == 0:
                        await self.ping_clients()
                        
                except Exception as e:
                    logger.error(f"Error in cleanup task: {e}")
        
        try:
            self._cleanup_task = asyncio.create_task(cleanup_loop())
        except RuntimeError:
            # No event loop running, will start later
            logger.debug("No event loop running, cleanup task will start later")
    
    async def shutdown(self):
        """Shutdown connection manager."""
        # Cancel cleanup task
        if self._cleanup_task:
            self._cleanup_task.cancel()
            try:
                await self._cleanup_task
            except asyncio.CancelledError:
                pass
        
        # Disconnect all clients
        await self.disconnect_all()
        
        logger.info("Connection manager shutdown complete")


# Global connection manager instance
connection_manager = ConnectionManager()
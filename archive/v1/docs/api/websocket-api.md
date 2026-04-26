# WebSocket API Documentation

## Overview

The WiFi-DensePose WebSocket API provides real-time streaming of pose estimation data, system events, and analytics. This enables applications to receive live updates without polling REST endpoints, making it ideal for real-time monitoring dashboards and interactive applications.

## Table of Contents

1. [Connection Setup](#connection-setup)
2. [Authentication](#authentication)
3. [Message Format](#message-format)
4. [Event Types](#event-types)
5. [Subscription Management](#subscription-management)
6. [Real-time Pose Streaming](#real-time-pose-streaming)
7. [System Events](#system-events)
8. [Analytics Streaming](#analytics-streaming)
9. [Error Handling](#error-handling)
10. [Connection Management](#connection-management)
11. [Rate Limiting](#rate-limiting)
12. [Code Examples](#code-examples)

## Connection Setup

### WebSocket Endpoint

```
Production: wss://api.wifi-densepose.com/ws/v1
Staging: wss://staging-api.wifi-densepose.com/ws/v1
Development: ws://localhost:8000/ws/v1
```

### Connection URL Parameters

```
wss://api.wifi-densepose.com/ws/v1?token=<jwt_token>&client_id=<client_id>
```

**Parameters:**
- `token` (required): JWT authentication token
- `client_id` (optional): Unique client identifier for connection tracking
- `compression` (optional): Enable compression (gzip, deflate)

### Connection Headers

```http
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Version: 13
Sec-WebSocket-Protocol: wifi-densepose-v1
Authorization: Bearer <jwt_token>
```

## Authentication

### JWT Token Authentication

Include the JWT token in the connection URL or as a header:

```javascript
// URL parameter method
const ws = new WebSocket('wss://api.wifi-densepose.com/ws/v1?token=your_jwt_token');

// Header method (if supported by client)
const ws = new WebSocket('wss://api.wifi-densepose.com/ws/v1', [], {
  headers: {
    'Authorization': 'Bearer your_jwt_token'
  }
});
```

### Token Refresh

When a token expires, the server will send a `token_expired` event. Clients should refresh their token and reconnect:

```json
{
  "type": "token_expired",
  "timestamp": "2025-01-07T10:30:00Z",
  "message": "JWT token has expired. Please refresh and reconnect."
}
```

## Message Format

### Standard Message Structure

All WebSocket messages follow this JSON structure:

```json
{
  "type": "message_type",
  "timestamp": "2025-01-07T10:30:00.123Z",
  "data": {
    // Message-specific data
  },
  "metadata": {
    "client_id": "client_123",
    "sequence": 12345,
    "compression": "gzip"
  }
}
```

### Message Types

| Type | Direction | Description |
|------|-----------|-------------|
| `subscribe` | Client → Server | Subscribe to event streams |
| `unsubscribe` | Client → Server | Unsubscribe from event streams |
| `pose_data` | Server → Client | Real-time pose estimation data |
| `system_event` | Server → Client | System status and events |
| `analytics_update` | Server → Client | Analytics and metrics updates |
| `error` | Server → Client | Error notifications |
| `heartbeat` | Bidirectional | Connection keep-alive |
| `ack` | Server → Client | Acknowledgment of client messages |

## Event Types

### Pose Data Events

#### Real-time Pose Detection

```json
{
  "type": "pose_data",
  "timestamp": "2025-01-07T10:30:00.123Z",
  "data": {
    "frame_id": 12345,
    "environment_id": "room_001",
    "processing_time_ms": 45.2,
    "persons": [
      {
        "person_id": 1,
        "track_id": 7,
        "confidence": 0.87,
        "bounding_box": {
          "x": 120,
          "y": 80,
          "width": 180,
          "height": 320
        },
        "keypoints": [
          {
            "name": "nose",
            "x": 210,
            "y": 95,
            "confidence": 0.92,
            "visible": true
          }
          // ... additional keypoints
        ],
        "activity": {
          "type": "walking",
          "confidence": 0.78,
          "velocity": {
            "x": 0.5,
            "y": 0.2
          }
        }
      }
    ],
    "metadata": {
      "model_version": "v1.2.0",
      "csi_quality": 0.85,
      "frame_rate": 29.8
    }
  }
}
```

#### Person Tracking Updates

```json
{
  "type": "tracking_update",
  "timestamp": "2025-01-07T10:30:00.123Z",
  "data": {
    "track_id": 7,
    "person_id": 1,
    "event": "track_started",
    "position": {
      "x": 210,
      "y": 240
    },
    "confidence": 0.87,
    "metadata": {
      "first_detection": "2025-01-07T10:29:45Z",
      "track_quality": 0.92
    }
  }
}
```

### System Events

#### System Status Changes

```json
{
  "type": "system_event",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "event": "system_started",
    "status": "running",
    "session_id": "session_123456",
    "configuration": {
      "domain": "healthcare",
      "environment_id": "room_001"
    },
    "components": {
      "neural_network": "healthy",
      "csi_processor": "healthy",
      "tracker": "healthy"
    }
  }
}
```

#### Hardware Events

```json
{
  "type": "hardware_event",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "event": "router_disconnected",
    "router_id": "router_001",
    "severity": "warning",
    "message": "Router connection lost. Attempting reconnection...",
    "metadata": {
      "last_seen": "2025-01-07T10:29:30Z",
      "reconnect_attempts": 1
    }
  }
}
```

### Analytics Events

#### Activity Detection

```json
{
  "type": "activity_event",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "event_type": "fall_detected",
    "severity": "high",
    "person_id": 3,
    "track_id": 15,
    "confidence": 0.92,
    "location": {
      "x": 210,
      "y": 180
    },
    "details": {
      "fall_duration": 2.3,
      "impact_severity": 0.85,
      "recovery_detected": false
    },
    "actions": [
      "alert_triggered",
      "notification_sent"
    ]
  }
}
```

#### Occupancy Updates

```json
{
  "type": "occupancy_update",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "environment_id": "room_001",
    "current_occupancy": 3,
    "previous_occupancy": 2,
    "change_type": "person_entered",
    "confidence": 0.89,
    "persons": [
      {
        "person_id": 1,
        "track_id": 7,
        "status": "active"
      },
      {
        "person_id": 2,
        "track_id": 8,
        "status": "active"
      },
      {
        "person_id": 4,
        "track_id": 12,
        "status": "new"
      }
    ]
  }
}
```

## Subscription Management

### Subscribe to Events

Send a subscription message to start receiving specific event types:

```json
{
  "type": "subscribe",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "subscriptions": [
      {
        "event_type": "pose_data",
        "filters": {
          "environment_id": "room_001",
          "min_confidence": 0.7,
          "include_keypoints": true,
          "include_dense_pose": false
        },
        "throttle": {
          "max_fps": 10,
          "buffer_size": 5
        }
      },
      {
        "event_type": "system_event",
        "filters": {
          "severity": ["warning", "error", "critical"]
        }
      },
      {
        "event_type": "activity_event",
        "filters": {
          "event_types": ["fall_detected", "alert_triggered"]
        }
      }
    ]
  }
}
```

### Subscription Acknowledgment

Server responds with subscription confirmation:

```json
{
  "type": "ack",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "message_type": "subscribe",
    "status": "success",
    "active_subscriptions": [
      {
        "subscription_id": "sub_123",
        "event_type": "pose_data",
        "status": "active"
      },
      {
        "subscription_id": "sub_124",
        "event_type": "system_event",
        "status": "active"
      }
    ]
  }
}
```

### Unsubscribe from Events

```json
{
  "type": "unsubscribe",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "subscription_ids": ["sub_123", "sub_124"]
  }
}
```

### Update Subscription Filters

```json
{
  "type": "update_subscription",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "subscription_id": "sub_123",
    "filters": {
      "min_confidence": 0.8,
      "max_fps": 15
    }
  }
}
```

## Real-time Pose Streaming

### High-Frequency Pose Data

For applications requiring high-frequency updates:

```json
{
  "type": "subscribe",
  "data": {
    "subscriptions": [
      {
        "event_type": "pose_data",
        "filters": {
          "environment_id": "room_001",
          "min_confidence": 0.5,
          "include_keypoints": true,
          "include_dense_pose": true,
          "include_velocity": true
        },
        "throttle": {
          "max_fps": 30,
          "buffer_size": 1,
          "compression": "gzip"
        },
        "quality": "high"
      }
    ]
  }
}
```

### Pose Data with Trajectory

```json
{
  "type": "pose_data_trajectory",
  "timestamp": "2025-01-07T10:30:00.123Z",
  "data": {
    "track_id": 7,
    "person_id": 1,
    "trajectory": [
      {
        "timestamp": "2025-01-07T10:29:58.123Z",
        "position": {"x": 200, "y": 230},
        "confidence": 0.89
      },
      {
        "timestamp": "2025-01-07T10:29:59.123Z",
        "position": {"x": 205, "y": 235},
        "confidence": 0.91
      },
      {
        "timestamp": "2025-01-07T10:30:00.123Z",
        "position": {"x": 210, "y": 240},
        "confidence": 0.87
      }
    ],
    "prediction": {
      "next_position": {"x": 215, "y": 245},
      "confidence": 0.73,
      "time_horizon": 1.0
    }
  }
}
```

## System Events

### Performance Monitoring

```json
{
  "type": "performance_update",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "metrics": {
      "frames_per_second": 29.8,
      "average_latency_ms": 45.2,
      "processing_queue_size": 3,
      "cpu_usage": 65.4,
      "memory_usage": 78.2,
      "gpu_usage": 82.1
    },
    "alerts": [
      {
        "type": "high_latency",
        "severity": "warning",
        "value": 67.3,
        "threshold": 50.0
      }
    ]
  }
}
```

### Configuration Changes

```json
{
  "type": "config_update",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "changed_fields": [
      "detection.confidence_threshold",
      "analytics.enable_fall_detection"
    ],
    "new_values": {
      "detection.confidence_threshold": 0.8,
      "analytics.enable_fall_detection": true
    },
    "applied_by": "admin_user",
    "requires_restart": false
  }
}
```

## Analytics Streaming

### Real-time Analytics

```json
{
  "type": "analytics_stream",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "window": "1_minute",
    "metrics": {
      "occupancy": {
        "current": 3,
        "average": 2.7,
        "peak": 5
      },
      "activity": {
        "movement_events": 15,
        "stationary_time": 45.2,
        "activity_level": 0.67
      },
      "detection": {
        "total_detections": 1800,
        "average_confidence": 0.84,
        "tracking_accuracy": 0.92
      }
    },
    "trends": {
      "occupancy_trend": "increasing",
      "activity_trend": "stable",
      "confidence_trend": "improving"
    }
  }
}
```

## Error Handling

### Connection Errors

```json
{
  "type": "error",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "error_code": "CONNECTION_ERROR",
    "message": "WebSocket connection lost",
    "details": {
      "reason": "network_timeout",
      "retry_after": 5,
      "max_retries": 3
    }
  }
}
```

### Subscription Errors

```json
{
  "type": "error",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "error_code": "SUBSCRIPTION_ERROR",
    "message": "Invalid subscription filter",
    "details": {
      "subscription_id": "sub_123",
      "field": "min_confidence",
      "reason": "Value must be between 0 and 1"
    }
  }
}
```

### Rate Limit Errors

```json
{
  "type": "error",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "error_code": "RATE_LIMIT_EXCEEDED",
    "message": "Message rate limit exceeded",
    "details": {
      "current_rate": 150,
      "limit": 100,
      "window": "1_minute",
      "retry_after": 30
    }
  }
}
```

## Connection Management

### Heartbeat

Both client and server should send periodic heartbeat messages:

```json
{
  "type": "heartbeat",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "client_id": "client_123",
    "uptime": 3600,
    "last_message": "2025-01-07T10:29:55Z"
  }
}
```

### Connection Status

```json
{
  "type": "connection_status",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "status": "connected",
    "client_id": "client_123",
    "session_id": "session_789",
    "connected_since": "2025-01-07T09:30:00Z",
    "active_subscriptions": 3,
    "message_count": 1250
  }
}
```

### Graceful Disconnect

```json
{
  "type": "disconnect",
  "timestamp": "2025-01-07T10:30:00Z",
  "data": {
    "reason": "client_requested",
    "message": "Graceful disconnect initiated by client"
  }
}
```

## Rate Limiting

### Message Rate Limits

| Message Type | Limit | Window |
|--------------|-------|--------|
| Subscribe/Unsubscribe | 10 messages | 1 minute |
| Heartbeat | 1 message | 30 seconds |
| General Commands | 60 messages | 1 minute |

### Data Rate Limits

| Subscription Type | Max Rate | Buffer Size |
|-------------------|----------|-------------|
| Pose Data (Low Quality) | 10 FPS | 5 frames |
| Pose Data (High Quality) | 30 FPS | 1 frame |
| System Events | 100 events/min | 10 events |
| Analytics | 60 updates/min | 5 updates |

## Code Examples

### JavaScript Client

```javascript
class WiFiDensePoseWebSocket {
  constructor(token, options = {}) {
    this.token = token;
    this.options = {
      url: 'wss://api.wifi-densepose.com/ws/v1',
      reconnectInterval: 5000,
      maxReconnectAttempts: 5,
      ...options
    };
    this.ws = null;
    this.reconnectAttempts = 0;
    this.subscriptions = new Map();
  }

  connect() {
    const url = `${this.options.url}?token=${this.token}`;
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      console.log('Connected to WiFi-DensePose WebSocket');
      this.reconnectAttempts = 0;
      this.startHeartbeat();
    };

    this.ws.onmessage = (event) => {
      const message = JSON.parse(event.data);
      this.handleMessage(message);
    };

    this.ws.onclose = (event) => {
      console.log('WebSocket connection closed:', event.code);
      this.stopHeartbeat();
      this.attemptReconnect();
    };

    this.ws.onerror = (error) => {
      console.error('WebSocket error:', error);
    };
  }

  subscribeToPoseData(environmentId, options = {}) {
    const subscription = {
      event_type: 'pose_data',
      filters: {
        environment_id: environmentId,
        min_confidence: options.minConfidence || 0.7,
        include_keypoints: options.includeKeypoints !== false,
        include_dense_pose: options.includeDensePose || false
      },
      throttle: {
        max_fps: options.maxFps || 10,
        buffer_size: options.bufferSize || 5
      }
    };

    this.send({
      type: 'subscribe',
      timestamp: new Date().toISOString(),
      data: {
        subscriptions: [subscription]
      }
    });
  }

  subscribeToSystemEvents() {
    this.send({
      type: 'subscribe',
      timestamp: new Date().toISOString(),
      data: {
        subscriptions: [{
          event_type: 'system_event',
          filters: {
            severity: ['warning', 'error', 'critical']
          }
        }]
      }
    });
  }

  handleMessage(message) {
    switch (message.type) {
      case 'pose_data':
        this.onPoseData(message.data);
        break;
      case 'system_event':
        this.onSystemEvent(message.data);
        break;
      case 'activity_event':
        this.onActivityEvent(message.data);
        break;
      case 'error':
        this.onError(message.data);
        break;
      case 'ack':
        this.onAcknowledgment(message.data);
        break;
    }
  }

  onPoseData(data) {
    // Handle pose data
    console.log('Received pose data:', data);
  }

  onSystemEvent(data) {
    // Handle system events
    console.log('System event:', data);
  }

  onActivityEvent(data) {
    // Handle activity events
    console.log('Activity event:', data);
  }

  onError(data) {
    console.error('WebSocket error:', data);
  }

  send(message) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    }
  }

  startHeartbeat() {
    this.heartbeatInterval = setInterval(() => {
      this.send({
        type: 'heartbeat',
        timestamp: new Date().toISOString(),
        data: {
          client_id: this.options.clientId,
          uptime: Date.now() - this.connectTime
        }
      });
    }, 30000);
  }

  stopHeartbeat() {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
    }
  }

  attemptReconnect() {
    if (this.reconnectAttempts < this.options.maxReconnectAttempts) {
      this.reconnectAttempts++;
      console.log(`Attempting to reconnect (${this.reconnectAttempts}/${this.options.maxReconnectAttempts})`);
      
      setTimeout(() => {
        this.connect();
      }, this.options.reconnectInterval);
    }
  }

  disconnect() {
    this.stopHeartbeat();
    if (this.ws) {
      this.ws.close();
    }
  }
}

// Usage example
const client = new WiFiDensePoseWebSocket('your_jwt_token', {
  clientId: 'dashboard_client_001'
});

client.onPoseData = (data) => {
  // Update UI with pose data
  updatePoseVisualization(data);
};

client.onActivityEvent = (data) => {
  if (data.event_type === 'fall_detected') {
    showFallAlert(data);
  }
};

client.connect();
client.subscribeToPoseData('room_001', {
  minConfidence: 0.8,
  maxFps: 15,
  includeKeypoints: true
});
```

### Python Client

```python
import asyncio
import websockets
import json
from datetime import datetime

class WiFiDensePoseWebSocket:
    def __init__(self, token, url='wss://api.wifi-densepose.com/ws/v1'):
        self.token = token
        self.url = f"{url}?token={token}"
        self.websocket = None
        self.subscriptions = {}
        
    async def connect(self):
        """Connect to the WebSocket server."""
        try:
            self.websocket = await websockets.connect(self.url)
            print("Connected to WiFi-DensePose WebSocket")
            
            # Start heartbeat task
            asyncio.create_task(self.heartbeat())
            
            # Listen for messages
            await self.listen()
            
        except Exception as e:
            print(f"Connection error: {e}")
            
    async def listen(self):
        """Listen for incoming messages."""
        try:
            async for message in self.websocket:
                data = json.loads(message)
                await self.handle_message(data)
        except websockets.exceptions.ConnectionClosed:
            print("WebSocket connection closed")
        except Exception as e:
            print(f"Error listening for messages: {e}")
            
    async def handle_message(self, message):
        """Handle incoming messages."""
        message_type = message.get('type')
        data = message.get('data', {})
        
        if message_type == 'pose_data':
            await self.on_pose_data(data)
        elif message_type == 'system_event':
            await self.on_system_event(data)
        elif message_type == 'activity_event':
            await self.on_activity_event(data)
        elif message_type == 'error':
            await self.on_error(data)
            
    async def subscribe_to_pose_data(self, environment_id, **options):
        """Subscribe to pose data stream."""
        subscription = {
            'event_type': 'pose_data',
            'filters': {
                'environment_id': environment_id,
                'min_confidence': options.get('min_confidence', 0.7),
                'include_keypoints': options.get('include_keypoints', True),
                'include_dense_pose': options.get('include_dense_pose', False)
            },
            'throttle': {
                'max_fps': options.get('max_fps', 10),
                'buffer_size': options.get('buffer_size', 5)
            }
        }
        
        await self.send({
            'type': 'subscribe',
            'timestamp': datetime.utcnow().isoformat() + 'Z',
            'data': {
                'subscriptions': [subscription]
            }
        })
        
    async def send(self, message):
        """Send a message to the server."""
        if self.websocket:
            await self.websocket.send(json.dumps(message))
            
    async def heartbeat(self):
        """Send periodic heartbeat messages."""
        while True:
            try:
                await self.send({
                    'type': 'heartbeat',
                    'timestamp': datetime.utcnow().isoformat() + 'Z',
                    'data': {
                        'client_id': 'python_client'
                    }
                })
                await asyncio.sleep(30)
            except Exception as e:
                print(f"Heartbeat error: {e}")
                break
                
    async def on_pose_data(self, data):
        """Handle pose data."""
        print(f"Received pose data: {len(data.get('persons', []))} persons detected")
        
    async def on_system_event(self, data):
        """Handle system events."""
        print(f"System event: {data.get('event')} - {data.get('message', '')}")
        
    async def on_activity_event(self, data):
        """Handle activity events."""
        if data.get('event_type') == 'fall_detected':
            print(f"FALL DETECTED: Person {data.get('person_id')} at {data.get('location')}")
            
    async def on_error(self, data):
        """Handle errors."""
        print(f"WebSocket error: {data.get('message')}")

# Usage example
async def main():
    client = WiFiDensePoseWebSocket('your_jwt_token')
    
    # Connect and subscribe
    await client.connect()
    await client.subscribe_to_pose_data('room_001', min_confidence=0.8)

if __name__ == "__main__":
    asyncio.run(main())
```

---

This WebSocket API documentation provides comprehensive coverage of real-time communication capabilities. For authentication details, see the [Authentication documentation](authentication.md). For REST API endpoints, see the [REST Endpoints documentation](rest-endpoints.md).
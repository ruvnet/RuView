# API Reference

## Overview

The WiFi-DensePose API provides comprehensive access to pose estimation data, system control, and configuration management through RESTful endpoints and real-time WebSocket connections.

## Table of Contents

1. [Authentication](#authentication)
2. [Base URL and Versioning](#base-url-and-versioning)
3. [Pose Data Endpoints](#pose-data-endpoints)
4. [System Control Endpoints](#system-control-endpoints)
5. [Configuration Endpoints](#configuration-endpoints)
6. [Analytics Endpoints](#analytics-endpoints)
7. [WebSocket API](#websocket-api)
8. [Error Handling](#error-handling)
9. [Rate Limiting](#rate-limiting)
10. [Code Examples](#code-examples)

## Authentication

### Bearer Token Authentication

All API endpoints require authentication using JWT Bearer tokens:

```http
Authorization: Bearer <your-jwt-token>
```

### Obtaining a Token

```bash
# Get authentication token
curl -X POST http://localhost:8000/api/v1/auth/token \
  -H "Content-Type: application/json" \
  -d '{
    "username": "your-username",
    "password": "your-password"
  }'
```

**Response:**
```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "bearer",
  "expires_in": 86400
}
```

### API Key Authentication

For service-to-service communication:

```http
X-API-Key: <your-api-key>
```

## Base URL and Versioning

- **Base URL**: `http://localhost:8000/api/v1`
- **Current Version**: v1
- **Content-Type**: `application/json`

## Pose Data Endpoints

### Get Latest Pose Data

Retrieve the most recent pose estimation results.

**Endpoint:** `GET /pose/latest`

**Headers:**
```http
Authorization: Bearer <token>
```

**Response:**
```json
{
  "timestamp": "2025-01-07T04:46:32.123Z",
  "frame_id": 12345,
  "processing_time_ms": 45,
  "persons": [
    {
      "id": 1,
      "confidence": 0.87,
      "bounding_box": {
        "x": 120,
        "y": 80,
        "width": 200,
        "height": 400
      },
      "keypoints": [
        {
          "name": "nose",
          "x": 220,
          "y": 100,
          "confidence": 0.95,
          "visible": true
        },
        {
          "name": "left_shoulder",
          "x": 200,
          "y": 150,
          "confidence": 0.89,
          "visible": true
        }
      ],
      "dense_pose": {
        "body_parts": [
          {
            "part_id": 1,
            "part_name": "torso",
            "uv_coordinates": [[0.5, 0.3], [0.6, 0.4]],
            "confidence": 0.89
          }
        ]
      },
      "tracking_info": {
        "track_id": "track_001",
        "track_age": 150,
        "velocity": {"x": 0.1, "y": 0.05}
      }
    }
  ],
  "metadata": {
    "environment_id": "room_001",
    "router_count": 3,
    "signal_quality": 0.82,
    "processing_pipeline": "standard"
  }
}
```

**Status Codes:**
- `200 OK`: Success
- `404 Not Found`: No pose data available
- `401 Unauthorized`: Authentication required
- `503 Service Unavailable`: System not initialized

### Get Historical Pose Data

Retrieve historical pose data with filtering options.

**Endpoint:** `GET /pose/history`

**Query Parameters:**
- `start_time` (optional): ISO 8601 timestamp for range start
- `end_time` (optional): ISO 8601 timestamp for range end
- `limit` (optional): Maximum number of records (default: 100, max: 1000)
- `person_id` (optional): Filter by specific person ID
- `confidence_threshold` (optional): Minimum confidence score (0.0-1.0)

**Example:**
```bash
curl "http://localhost:8000/api/v1/pose/history?start_time=2025-01-07T00:00:00Z&limit=50&confidence_threshold=0.7" \
  -H "Authorization: Bearer <token>"
```

**Response:**
```json
{
  "poses": [
    {
      "timestamp": "2025-01-07T04:46:32.123Z",
      "persons": [...],
      "metadata": {...}
    }
  ],
  "pagination": {
    "total_count": 1500,
    "returned_count": 50,
    "has_more": true,
    "next_cursor": "eyJpZCI6MTIzNDV9"
  }
}
```

### Query Pose Data

Execute complex queries on pose data with aggregation support.

**Endpoint:** `POST /pose/query`

**Request Body:**
```json
{
  "query": {
    "time_range": {
      "start": "2025-01-07T00:00:00Z",
      "end": "2025-01-07T23:59:59Z"
    },
    "filters": {
      "person_count": {"min": 1, "max": 5},
      "confidence": {"min": 0.7},
      "activity": ["walking", "standing"]
    },
    "aggregation": {
      "type": "hourly_summary",
      "metrics": ["person_count", "avg_confidence"]
    }
  }
}
```

**Response:**
```json
{
  "results": [
    {
      "timestamp": "2025-01-07T10:00:00Z",
      "person_count": 3,
      "avg_confidence": 0.85,
      "activities": {
        "walking": 0.6,
        "standing": 0.4
      }
    }
  ],
  "query_metadata": {
    "execution_time_ms": 150,
    "total_records_scanned": 10000,
    "cache_hit": false
  }
}
```

## System Control Endpoints

### Get System Status

Get comprehensive system health and status information.

**Endpoint:** `GET /system/status`

**Response:**
```json
{
  "status": "running",
  "uptime_seconds": 86400,
  "version": "1.0.0",
  "components": {
    "csi_receiver": {
      "status": "active",
      "data_rate_hz": 25.3,
      "packet_loss_rate": 0.02,
      "last_packet_time": "2025-01-07T04:46:32Z"
    },
    "neural_network": {
      "status": "active",
      "model_loaded": true,
      "inference_time_ms": 45,
      "gpu_utilization": 0.65
    },
    "tracking": {
      "status": "active",
      "active_tracks": 2,
      "track_quality": 0.89
    }
  },
  "hardware": {
    "cpu_usage": 0.45,
    "memory_usage": 0.62,
    "gpu_memory_usage": 0.78,
    "disk_usage": 0.23
  },
  "network": {
    "connected_routers": 3,
    "signal_strength": -45,
    "interference_level": 0.15
  }
}
```

### Start System

Start the pose estimation system with configuration options.

**Endpoint:** `POST /system/start`

**Request Body:**
```json
{
  "configuration": {
    "domain": "healthcare",
    "environment_id": "room_001",
    "calibration_required": true
  }
}
```

**Response:**
```json
{
  "status": "starting",
  "estimated_ready_time": "2025-01-07T04:47:00Z",
  "initialization_steps": [
    {
      "step": "hardware_initialization",
      "status": "in_progress",
      "progress": 0.3
    },
    {
      "step": "model_loading",
      "status": "pending",
      "progress": 0.0
    }
  ]
}
```

### Stop System

Gracefully stop the pose estimation system.

**Endpoint:** `POST /system/stop`

**Request Body:**
```json
{
  "force": false,
  "save_state": true
}
```

**Response:**
```json
{
  "status": "stopping",
  "estimated_stop_time": "2025-01-07T04:47:30Z",
  "shutdown_steps": [
    {
      "step": "data_pipeline_stop",
      "status": "completed",
      "progress": 1.0
    },
    {
      "step": "model_unloading",
      "status": "in_progress",
      "progress": 0.7
    }
  ]
}
```

## Configuration Endpoints

### Get Configuration

Retrieve current system configuration.

**Endpoint:** `GET /config`

**Response:**
```json
{
  "domain": "healthcare",
  "environment": {
    "id": "room_001",
    "name": "Patient Room 1",
    "calibration_timestamp": "2025-01-07T04:00:00Z"
  },
  "detection": {
    "confidence_threshold": 0.7,
    "max_persons": 5,
    "tracking_enabled": true
  },
  "alerts": {
    "fall_detection": {
      "enabled": true,
      "sensitivity": 0.8,
      "notification_delay_seconds": 5
    },
    "inactivity_detection": {
      "enabled": true,
      "threshold_minutes": 30
    }
  },
  "streaming": {
    "restream_enabled": false,
    "websocket_enabled": true,
    "mqtt_enabled": true
  }
}
```

### Update Configuration

Update system configuration with partial updates supported.

**Endpoint:** `PUT /config`

**Request Body:**
```json
{
  "detection": {
    "confidence_threshold": 0.75,
    "max_persons": 3
  },
  "alerts": {
    "fall_detection": {
      "sensitivity": 0.9
    }
  }
}
```

**Response:**
```json
{
  "status": "updated",
  "changes_applied": [
    "detection.confidence_threshold",
    "detection.max_persons",
    "alerts.fall_detection.sensitivity"
  ],
  "restart_required": false,
  "validation_warnings": []
}
```

## Analytics Endpoints

### Healthcare Analytics

Get healthcare-specific analytics and insights.

**Endpoint:** `GET /analytics/healthcare`

**Query Parameters:**
- `period`: Time period (hour, day, week, month)
- `metrics`: Comma-separated list of metrics

**Example:**
```bash
curl "http://localhost:8000/api/v1/analytics/healthcare?period=day&metrics=fall_events,activity_summary" \
  -H "Authorization: Bearer <token>"
```

**Response:**
```json
{
  "period": "day",
  "date": "2025-01-07",
  "metrics": {
    "fall_events": {
      "count": 2,
      "events": [
        {
          "timestamp": "2025-01-07T14:30:15Z",
          "person_id": 1,
          "severity": "moderate",
          "response_time_seconds": 45,
          "location": {"x": 150, "y": 200}
        }
      ]
    },
    "activity_summary": {
      "walking_minutes": 120,
      "sitting_minutes": 480,
      "lying_minutes": 360,
      "standing_minutes": 180
    },
    "mobility_score": 0.75,
    "sleep_quality": {
      "total_sleep_hours": 7.5,
      "sleep_efficiency": 0.89,
      "restlessness_events": 3
    }
  }
}
```

### Retail Analytics

Get retail-specific analytics and customer insights.

**Endpoint:** `GET /analytics/retail`

**Response:**
```json
{
  "period": "day",
  "date": "2025-01-07",
  "metrics": {
    "traffic": {
      "total_visitors": 245,
      "unique_visitors": 198,
      "peak_hour": "14:00",
      "peak_count": 15,
      "average_dwell_time_minutes": 12.5
    },
    "zones": [
      {
        "zone_id": "entrance",
        "zone_name": "Store Entrance",
        "visitor_count": 245,
        "avg_dwell_time_minutes": 2.1,
        "conversion_rate": 0.85
      },
      {
        "zone_id": "electronics",
        "zone_name": "Electronics Section",
        "visitor_count": 89,
        "avg_dwell_time_minutes": 8.7,
        "conversion_rate": 0.34
      }
    ],
    "conversion_funnel": {
      "entrance": 245,
      "product_interaction": 156,
      "checkout_area": 89,
      "purchase": 67
    },
    "heat_map": {
      "high_traffic_areas": [
        {"zone": "entrance", "intensity": 0.95},
        {"zone": "checkout", "intensity": 0.78}
      ]
    }
  }
}
```

### Security Analytics

Get security-specific analytics and threat assessments.

**Endpoint:** `GET /analytics/security`

**Response:**
```json
{
  "period": "day",
  "date": "2025-01-07",
  "metrics": {
    "intrusion_events": {
      "count": 1,
      "events": [
        {
          "timestamp": "2025-01-07T02:15:30Z",
          "zone": "restricted_area",
          "person_count": 1,
          "threat_level": "medium",
          "response_time_seconds": 120
        }
      ]
    },
    "perimeter_monitoring": {
      "total_detections": 45,
      "authorized_entries": 42,
      "unauthorized_attempts": 3,
      "false_positives": 0
    },
    "crowd_analysis": {
      "max_occupancy": 12,
      "average_occupancy": 3.2,
      "crowd_formation_events": 0
    }
  }
}
```

## WebSocket API

### Connection

Connect to the WebSocket endpoint for real-time data streaming.

**Endpoint:** `ws://localhost:8000/ws/pose`

**Authentication:** Include token as query parameter or in headers:
```javascript
const ws = new WebSocket('ws://localhost:8000/ws/pose?token=<your-jwt-token>');
```

### Connection Establishment

**Server Message:**
```json
{
  "type": "connection_established",
  "client_id": "client_12345",
  "server_time": "2025-01-07T04:46:32Z",
  "supported_protocols": ["pose_v1", "alerts_v1"]
}
```

### Subscription Management

**Subscribe to Pose Updates:**
```json
{
  "type": "subscribe",
  "channel": "pose_updates",
  "filters": {
    "min_confidence": 0.7,
    "person_ids": [1, 2, 3],
    "include_keypoints": true,
    "include_dense_pose": false
  }
}
```

**Subscription Confirmation:**
```json
{
  "type": "subscription_confirmed",
  "channel": "pose_updates",
  "subscription_id": "sub_67890",
  "filters_applied": {
    "min_confidence": 0.7,
    "person_ids": [1, 2, 3]
  }
}
```

### Real-Time Data Streaming

**Pose Update Message:**
```json
{
  "type": "pose_update",
  "subscription_id": "sub_67890",
  "timestamp": "2025-01-07T04:46:32.123Z",
  "data": {
    "frame_id": 12345,
    "persons": [...],
    "metadata": {...}
  }
}
```

**System Status Update:**
```json
{
  "type": "system_status",
  "timestamp": "2025-01-07T04:46:32Z",
  "status": {
    "processing_fps": 25.3,
    "active_persons": 2,
    "system_health": "good",
    "gpu_utilization": 0.65
  }
}
```

### Alert Streaming

**Subscribe to Alerts:**
```json
{
  "type": "subscribe",
  "channel": "alerts",
  "filters": {
    "alert_types": ["fall_detection", "intrusion"],
    "severity": ["high", "critical"]
  }
}
```

**Alert Message:**
```json
{
  "type": "alert",
  "alert_id": "alert_12345",
  "timestamp": "2025-01-07T04:46:32Z",
  "alert_type": "fall_detection",
  "severity": "high",
  "data": {
    "person_id": 1,
    "location": {"x": 220, "y": 180},
    "confidence": 0.92,
    "video_clip_url": "/clips/fall_12345.mp4"
  },
  "actions_required": ["medical_response", "notification"]
}
```

## Error Handling

### Standard Error Response Format

```json
{
  "error": {
    "code": "POSE_DATA_NOT_FOUND",
    "message": "No pose data available for the specified time range",
    "details": {
      "requested_range": {
        "start": "2025-01-07T00:00:00Z",
        "end": "2025-01-07T01:00:00Z"
      },
      "available_range": {
        "start": "2025-01-07T02:00:00Z",
        "end": "2025-01-07T04:46:32Z"
      }
    },
    "timestamp": "2025-01-07T04:46:32Z",
    "request_id": "req_12345"
  }
}
```

### HTTP Status Codes

#### Success Codes
- `200 OK`: Request successful
- `201 Created`: Resource created successfully
- `202 Accepted`: Request accepted for processing
- `204 No Content`: Request successful, no content returned

#### Client Error Codes
- `400 Bad Request`: Invalid request format or parameters
- `401 Unauthorized`: Authentication required or invalid
- `403 Forbidden`: Insufficient permissions
- `404 Not Found`: Resource not found
- `409 Conflict`: Resource conflict (e.g., system already running)
- `422 Unprocessable Entity`: Validation errors
- `429 Too Many Requests`: Rate limit exceeded

#### Server Error Codes
- `500 Internal Server Error`: Unexpected server error
- `502 Bad Gateway`: Upstream service error
- `503 Service Unavailable`: System not ready or overloaded
- `504 Gateway Timeout`: Request timeout

### Validation Error Response

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Request validation failed",
    "details": {
      "field_errors": [
        {
          "field": "confidence_threshold",
          "message": "Value must be between 0.0 and 1.0",
          "received_value": 1.5
        },
        {
          "field": "max_persons",
          "message": "Value must be a positive integer",
          "received_value": -1
        }
      ]
    },
    "timestamp": "2025-01-07T04:46:32Z",
    "request_id": "req_12346"
  }
}
```

## Rate Limiting

### Rate Limit Headers

All responses include rate limiting information:

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1704686400
X-RateLimit-Window: 3600
```

### Rate Limits by Endpoint Type

- **REST API**: 1000 requests per hour per API key
- **WebSocket**: 100 connections per IP address
- **Streaming**: 10 concurrent streams per account
- **Webhook**: 10,000 events per hour per endpoint

### Rate Limit Exceeded Response

```json
{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded. Try again later.",
    "details": {
      "limit": 1000,
      "window_seconds": 3600,
      "reset_time": "2025-01-07T05:46:32Z"
    },
    "timestamp": "2025-01-07T04:46:32Z",
    "request_id": "req_12347"
  }
}
```

## Code Examples

### Python Example

```python
import requests
import json
from datetime import datetime, timedelta

class WiFiDensePoseClient:
    def __init__(self, base_url, token):
        self.base_url = base_url
        self.headers = {
            'Authorization': f'Bearer {token}',
            'Content-Type': 'application/json'
        }
    
    def get_latest_pose(self):
        """Get the latest pose data."""
        response = requests.get(
            f'{self.base_url}/pose/latest',
            headers=self.headers
        )
        response.raise_for_status()
        return response.json()
    
    def get_historical_poses(self, start_time=None, end_time=None, limit=100):
        """Get historical pose data."""
        params = {'limit': limit}
        if start_time:
            params['start_time'] = start_time.isoformat()
        if end_time:
            params['end_time'] = end_time.isoformat()
        
        response = requests.get(
            f'{self.base_url}/pose/history',
            headers=self.headers,
            params=params
        )
        response.raise_for_status()
        return response.json()
    
    def start_system(self, domain='general', environment_id='default'):
        """Start the pose estimation system."""
        data = {
            'configuration': {
                'domain': domain,
                'environment_id': environment_id,
                'calibration_required': True
            }
        }
        response = requests.post(
            f'{self.base_url}/system/start',
            headers=self.headers,
            json=data
        )
        response.raise_for_status()
        return response.json()

# Usage example
client = WiFiDensePoseClient('http://localhost:8000/api/v1', 'your-token')

# Get latest pose data
latest = client.get_latest_pose()
print(f"Found {len(latest['persons'])} persons")

# Get historical data for the last hour
end_time = datetime.now()
start_time = end_time - timedelta(hours=1)
history = client.get_historical_poses(start_time, end_time)
print(f"Retrieved {len(history['poses'])} historical records")
```

### JavaScript Example

```javascript
class WiFiDensePoseClient {
    constructor(baseUrl, token) {
        this.baseUrl = baseUrl;
        this.headers = {
            'Authorization': `Bearer ${token}`,
            'Content-Type': 'application/json'
        };
    }

    async getLatestPose() {
        const response = await fetch(`${this.baseUrl}/pose/latest`, {
            headers: this.headers
        });
        
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        
        return await response.json();
    }

    async updateConfiguration(config) {
        const response = await fetch(`${this.baseUrl}/config`, {
            method: 'PUT',
            headers: this.headers,
            body: JSON.stringify(config)
        });
        
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        
        return await response.json();
    }

    connectWebSocket() {
        const ws = new WebSocket(`ws://localhost:8000/ws/pose?token=${this.token}`);
        
        ws.onopen = () => {
            console.log('WebSocket connected');
            // Subscribe to pose updates
            ws.send(JSON.stringify({
                type: 'subscribe',
                channel: 'pose_updates',
                filters: {
                    min_confidence: 0.7
                }
            }));
        };
        
        ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            console.log('Received:', data);
        };
        
        ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
        
        return ws;
    }
}

// Usage example
const client = new WiFiDensePoseClient('http://localhost:8000/api/v1', 'your-token');

// Get latest pose data
client.getLatestPose()
    .then(data => console.log('Latest pose:', data))
    .catch(error => console.error('Error:', error));

// Connect to WebSocket for real-time updates
const ws = client.connectWebSocket();
```

### cURL Examples

```bash
# Get authentication token
curl -X POST http://localhost:8000/api/v1/auth/token \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "password"}'

# Get latest pose data
curl http://localhost:8000/api/v1/pose/latest \
  -H "Authorization: Bearer <token>"

# Start system
curl -X POST http://localhost:8000/api/v1/system/start \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "configuration": {
      "domain": "healthcare",
      "environment_id": "room_001"
    }
  }'

# Update configuration
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "detection": {
      "confidence_threshold": 0.8
    }
  }'

# Get healthcare analytics
curl "http://localhost:8000/api/v1/analytics/healthcare?period=day" \
  -H "Authorization: Bearer <token>"
```

---

For more detailed information, see:
- [Getting Started Guide](getting-started.md)
- [Configuration Guide](configuration.md)
- [WebSocket API Documentation](../api/websocket-api.md)
- [Authentication Guide](../api/authentication.md)
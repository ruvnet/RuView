# WiFi-DensePose API Reference

## Table of Contents

1. [Overview](#overview)
2. [Authentication](#authentication)
3. [Base URL and Versioning](#base-url-and-versioning)
4. [Request/Response Format](#requestresponse-format)
5. [Error Handling](#error-handling)
6. [Rate Limiting](#rate-limiting)
7. [Pose Estimation API](#pose-estimation-api)
8. [System Management API](#system-management-api)
9. [Health Check API](#health-check-api)
10. [WebSocket API](#websocket-api)
11. [Data Models](#data-models)
12. [SDK Examples](#sdk-examples)

## Overview

The WiFi-DensePose API provides comprehensive access to WiFi-based human pose estimation capabilities. The API follows REST principles and supports both synchronous HTTP requests and real-time WebSocket connections.

### Key Features

- **RESTful Design**: Standard HTTP methods and status codes
- **Real-time Streaming**: WebSocket support for live pose data
- **Authentication**: JWT-based authentication with role-based access
- **Rate Limiting**: Configurable rate limits to prevent abuse
- **Comprehensive Documentation**: OpenAPI/Swagger documentation
- **Error Handling**: Detailed error responses with actionable messages

### API Capabilities

- Real-time pose estimation from WiFi CSI data
- Historical pose data retrieval and analysis
- System health monitoring and diagnostics
- Multi-zone occupancy tracking
- Activity recognition and analytics
- System configuration and calibration

## Authentication

### JWT Authentication

The API uses JSON Web Tokens (JWT) for authentication. Include the token in the `Authorization` header:

```http
Authorization: Bearer <your-jwt-token>
```

### Obtaining a Token

```bash
# Login to get JWT token
curl -X POST http://localhost:8000/api/v1/auth/login \
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

### Token Refresh

```bash
# Refresh expired token
curl -X POST http://localhost:8000/api/v1/auth/refresh \
  -H "Authorization: Bearer <your-refresh-token>"
```

### Public Endpoints

Some endpoints are publicly accessible without authentication:
- `GET /api/v1/health/*` - Health check endpoints
- `GET /api/v1/version` - Version information
- `GET /docs` - API documentation

## Base URL and Versioning

### Base URL
```
http://localhost:8000/api/v1
```

### API Versioning
The API uses URL path versioning. Current version is `v1`.

### Content Types
- **Request**: `application/json`
- **Response**: `application/json`
- **WebSocket**: `application/json` messages

## Request/Response Format

### Standard Response Format

```json
{
  "data": {},
  "timestamp": "2025-01-07T10:00:00Z",
  "status": "success"
}
```

### Error Response Format

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid request parameters",
    "details": {
      "field": "confidence_threshold",
      "issue": "Value must be between 0.0 and 1.0"
    }
  },
  "timestamp": "2025-01-07T10:00:00Z",
  "status": "error"
}
```

## Error Handling

### HTTP Status Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 201 | Created |
| 400 | Bad Request |
| 401 | Unauthorized |
| 403 | Forbidden |
| 404 | Not Found |
| 409 | Conflict |
| 422 | Validation Error |
| 429 | Rate Limited |
| 500 | Internal Server Error |
| 503 | Service Unavailable |

### Error Codes

| Code | Description |
|------|-------------|
| `VALIDATION_ERROR` | Request validation failed |
| `AUTHENTICATION_ERROR` | Authentication failed |
| `AUTHORIZATION_ERROR` | Insufficient permissions |
| `RESOURCE_NOT_FOUND` | Requested resource not found |
| `RATE_LIMIT_EXCEEDED` | Rate limit exceeded |
| `HARDWARE_ERROR` | Hardware communication error |
| `PROCESSING_ERROR` | Pose processing error |
| `CALIBRATION_ERROR` | System calibration error |

## Rate Limiting

### Default Limits
- **Authenticated users**: 1000 requests per hour
- **Anonymous users**: 100 requests per hour
- **WebSocket connections**: 10 concurrent per user

### Rate Limit Headers
```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1641556800
```

### Rate Limit Response
```json
{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded. Try again in 60 seconds."
  }
}
```

## Pose Estimation API

### Get Current Pose Estimation

Get real-time pose estimation from WiFi signals.

```http
GET /api/v1/pose/current
```

**Query Parameters:**
- `zone_ids` (array, optional): Specific zones to analyze
- `confidence_threshold` (float, optional): Minimum confidence (0.0-1.0)
- `max_persons` (integer, optional): Maximum persons to detect (1-50)
- `include_keypoints` (boolean, optional): Include keypoint data (default: true)
- `include_segmentation` (boolean, optional): Include segmentation masks (default: false)

**Example Request:**
```bash
curl "http://localhost:8000/api/v1/pose/current?confidence_threshold=0.7&max_persons=5" \
  -H "Authorization: Bearer <token>"
```

**Response:**
```json
{
  "timestamp": "2025-01-07T10:00:00Z",
  "frame_id": "frame_12345",
  "persons": [
    {
      "person_id": "person_001",
      "confidence": 0.85,
      "bounding_box": {
        "x": 100,
        "y": 150,
        "width": 80,
        "height": 180
      },
      "keypoints": [
        {
          "name": "nose",
          "x": 140,
          "y": 160,
          "confidence": 0.9
        }
      ],
      "zone_id": "zone_001",
      "activity": "standing",
      "timestamp": "2025-01-07T10:00:00Z"
    }
  ],
  "zone_summary": {
    "zone_001": 1,
    "zone_002": 0
  },
  "processing_time_ms": 45.2
}
```

### Analyze Pose Data

Trigger pose analysis with custom parameters.

```http
POST /api/v1/pose/analyze
```

**Request Body:**
```json
{
  "zone_ids": ["zone_001", "zone_002"],
  "confidence_threshold": 0.8,
  "max_persons": 10,
  "include_keypoints": true,
  "include_segmentation": false
}
```

**Response:** Same format as current pose estimation.

### Get Zone Occupancy

Get current occupancy for a specific zone.

```http
GET /api/v1/pose/zones/{zone_id}/occupancy
```

**Path Parameters:**
- `zone_id` (string): Zone identifier

**Example Request:**
```bash
curl "http://localhost:8000/api/v1/pose/zones/zone_001/occupancy" \
  -H "Authorization: Bearer <token>"
```

**Response:**
```json
{
  "zone_id": "zone_001",
  "current_occupancy": 3,
  "max_occupancy": 10,
  "persons": [
    {
      "person_id": "person_001",
      "confidence": 0.85,
      "activity": "standing"
    }
  ],
  "timestamp": "2025-01-07T10:00:00Z"
}
```

### Get Zones Summary

Get occupancy summary for all zones.

```http
GET /api/v1/pose/zones/summary
```

**Response:**
```json
{
  "timestamp": "2025-01-07T10:00:00Z",
  "total_persons": 5,
  "zones": {
    "zone_001": {
      "occupancy": 3,
      "max_occupancy": 10,
      "status": "normal"
    },
    "zone_002": {
      "occupancy": 2,
      "max_occupancy": 8,
      "status": "normal"
    }
  },
  "active_zones": 2
}
```

### Get Historical Data

Retrieve historical pose estimation data.

```http
POST /api/v1/pose/historical
```

**Request Body:**
```json
{
  "start_time": "2025-01-07T00:00:00Z",
  "end_time": "2025-01-07T23:59:59Z",
  "zone_ids": ["zone_001"],
  "aggregation_interval": 300,
  "include_raw_data": false
}
```

**Response:**
```json
{
  "query": {
    "start_time": "2025-01-07T00:00:00Z",
    "end_time": "2025-01-07T23:59:59Z",
    "zone_ids": ["zone_001"],
    "aggregation_interval": 300
  },
  "data": [
    {
      "timestamp": "2025-01-07T00:00:00Z",
      "average_occupancy": 2.5,
      "max_occupancy": 5,
      "total_detections": 150
    }
  ],
  "total_records": 288
}
```

### Get Detected Activities

Get recently detected activities.

```http
GET /api/v1/pose/activities
```

**Query Parameters:**
- `zone_id` (string, optional): Filter by zone
- `limit` (integer, optional): Maximum activities (1-100, default: 10)

**Response:**
```json
{
  "activities": [
    {
      "activity": "walking",
      "person_id": "person_001",
      "zone_id": "zone_001",
      "confidence": 0.9,
      "timestamp": "2025-01-07T10:00:00Z",
      "duration_seconds": 15.5
    }
  ],
  "total_count": 1,
  "zone_id": "zone_001"
}
```

### Calibrate System

Start system calibration process.

```http
POST /api/v1/pose/calibrate
```

**Response:**
```json
{
  "calibration_id": "cal_12345",
  "status": "started",
  "estimated_duration_minutes": 5,
  "message": "Calibration process started"
}
```

### Get Calibration Status

Check calibration progress.

```http
GET /api/v1/pose/calibration/status
```

**Response:**
```json
{
  "is_calibrating": true,
  "calibration_id": "cal_12345",
  "progress_percent": 60,
  "current_step": "phase_sanitization",
  "estimated_remaining_minutes": 2,
  "last_calibration": "2025-01-06T15:30:00Z"
}
```

### Get Pose Statistics

Get pose estimation statistics.

```http
GET /api/v1/pose/stats
```

**Query Parameters:**
- `hours` (integer, optional): Hours of data to analyze (1-168, default: 24)

**Response:**
```json
{
  "period": {
    "start_time": "2025-01-06T10:00:00Z",
    "end_time": "2025-01-07T10:00:00Z",
    "hours": 24
  },
  "statistics": {
    "total_detections": 1500,
    "average_confidence": 0.82,
    "unique_persons": 25,
    "average_processing_time_ms": 47.3,
    "zones": {
      "zone_001": {
        "detections": 800,
        "average_occupancy": 3.2
      }
    }
  }
}
```

## System Management API

### System Status

Get current system status.

```http
GET /api/v1/system/status
```

**Response:**
```json
{
  "status": "running",
  "uptime_seconds": 86400,
  "services": {
    "hardware": "healthy",
    "pose_estimation": "healthy",
    "streaming": "healthy"
  },
  "configuration": {
    "domain": "healthcare",
    "max_persons": 10,
    "confidence_threshold": 0.7
  },
  "timestamp": "2025-01-07T10:00:00Z"
}
```

### Start System

Start the pose estimation system.

```http
POST /api/v1/system/start
```

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

### Stop System

Stop the pose estimation system.

```http
POST /api/v1/system/stop
```

### Restart System

Restart the system with new configuration.

```http
POST /api/v1/system/restart
```

### Get Configuration

Get current system configuration.

```http
GET /api/v1/config
```

### Update Configuration

Update system configuration.

```http
PUT /api/v1/config
```

**Request Body:**
```json
{
  "detection": {
    "confidence_threshold": 0.8,
    "max_persons": 8
  },
  "analytics": {
    "enable_fall_detection": true
  }
}
```

## Health Check API

### Comprehensive Health Check

Get detailed system health information.

```http
GET /api/v1/health
```

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2025-01-07T10:00:00Z",
  "uptime_seconds": 86400,
  "components": {
    "hardware": {
      "name": "Hardware Service",
      "status": "healthy",
      "message": "All routers connected",
      "last_check": "2025-01-07T10:00:00Z",
      "uptime_seconds": 86400,
      "metrics": {
        "connected_routers": 3,
        "csi_data_rate": 30.5
      }
    },
    "pose": {
      "name": "Pose Service",
      "status": "healthy",
      "message": "Processing normally",
      "last_check": "2025-01-07T10:00:00Z",
      "metrics": {
        "processing_rate": 29.8,
        "average_latency_ms": 45.2
      }
    }
  },
  "system_metrics": {
    "cpu": {
      "percent": 65.2,
      "count": 8
    },
    "memory": {
      "total_gb": 16.0,
      "available_gb": 8.5,
      "percent": 46.9
    },
    "disk": {
      "total_gb": 500.0,
      "free_gb": 350.0,
      "percent": 30.0
    }
  }
}
```

### Readiness Check

Check if system is ready to serve requests.

```http
GET /api/v1/ready
```

**Response:**
```json
{
  "ready": true,
  "timestamp": "2025-01-07T10:00:00Z",
  "checks": {
    "hardware_ready": true,
    "pose_ready": true,
    "stream_ready": true,
    "memory_available": true,
    "disk_space_available": true
  },
  "message": "System is ready"
}
```

### Liveness Check

Simple liveness check for load balancers.

```http
GET /api/v1/live
```

**Response:**
```json
{
  "status": "alive",
  "timestamp": "2025-01-07T10:00:00Z"
}
```

### System Metrics

Get detailed system metrics.

```http
GET /api/v1/metrics
```

### Version Information

Get application version information.

```http
GET /api/v1/version
```

**Response:**
```json
{
  "name": "WiFi-DensePose API",
  "version": "1.0.0",
  "environment": "production",
  "debug": false,
  "timestamp": "2025-01-07T10:00:00Z"
}
```

## WebSocket API

### Connection

Connect to WebSocket endpoint:

```javascript
const ws = new WebSocket('ws://localhost:8000/ws/pose/stream');
```

### Authentication

Send authentication message after connection:

```javascript
ws.send(JSON.stringify({
  type: 'auth',
  token: 'your-jwt-token'
}));
```

### Subscribe to Pose Updates

```javascript
ws.send(JSON.stringify({
  type: 'subscribe',
  channel: 'pose_updates',
  filters: {
    zone_ids: ['zone_001'],
    min_confidence: 0.7
  }
}));
```

### Pose Data Message

```json
{
  "type": "pose_data",
  "channel": "pose_updates",
  "data": {
    "timestamp": "2025-01-07T10:00:00Z",
    "frame_id": "frame_12345",
    "persons": [
      {
        "person_id": "person_001",
        "confidence": 0.85,
        "bounding_box": {
          "x": 100,
          "y": 150,
          "width": 80,
          "height": 180
        },
        "zone_id": "zone_001"
      }
    ]
  }
}
```

### System Events

Subscribe to system events:

```javascript
ws.send(JSON.stringify({
  type: 'subscribe',
  channel: 'system_events'
}));
```

### Event Message

```json
{
  "type": "system_event",
  "channel": "system_events",
  "data": {
    "event_type": "fall_detected",
    "person_id": "person_001",
    "zone_id": "zone_001",
    "confidence": 0.95,
    "timestamp": "2025-01-07T10:00:00Z"
  }
}
```

## Data Models

### PersonPose

```json
{
  "person_id": "string",
  "confidence": 0.85,
  "bounding_box": {
    "x": 100,
    "y": 150,
    "width": 80,
    "height": 180
  },
  "keypoints": [
    {
      "name": "nose",
      "x": 140,
      "y": 160,
      "confidence": 0.9,
      "visible": true
    }
  ],
  "segmentation": {
    "mask": "base64-encoded-mask",
    "body_parts": ["torso", "left_arm", "right_arm"]
  },
  "zone_id": "zone_001",
  "activity": "standing",
  "timestamp": "2025-01-07T10:00:00Z"
}
```

### Keypoint Names

Standard keypoint names following COCO format:
- `nose`, `left_eye`, `right_eye`, `left_ear`, `right_ear`
- `left_shoulder`, `right_shoulder`, `left_elbow`, `right_elbow`
- `left_wrist`, `right_wrist`, `left_hip`, `right_hip`
- `left_knee`, `right_knee`, `left_ankle`, `right_ankle`

### Activity Types

Supported activity classifications:
- `standing`, `sitting`, `walking`, `running`, `lying_down`
- `falling`, `jumping`, `bending`, `reaching`, `waving`

### Zone Configuration

```json
{
  "zone_id": "zone_001",
  "name": "Living Room",
  "coordinates": {
    "x": 0,
    "y": 0,
    "width": 500,
    "height": 300
  },
  "max_occupancy": 10,
  "alerts_enabled": true,
  "privacy_level": "high"
}
```

## SDK Examples

### Python SDK

```python
from wifi_densepose import WiFiDensePoseClient

# Initialize client
client = WiFiDensePoseClient(
    base_url="http://localhost:8000",
    api_key="your-api-key"
)

# Get current poses
poses = client.get_current_poses(
    confidence_threshold=0.7,
    max_persons=5
)

# Get historical data
history = client.get_historical_data(
    start_time="2025-01-07T00:00:00Z",
    end_time="2025-01-07T23:59:59Z",
    zone_ids=["zone_001"]
)

# Subscribe to real-time updates
def pose_callback(poses):
    print(f"Received {len(poses)} poses")

client.subscribe_to_poses(callback=pose_callback)
```

### JavaScript SDK

```javascript
import { WiFiDensePoseClient } from 'wifi-densepose-js';

// Initialize client
const client = new WiFiDensePoseClient({
  baseUrl: 'http://localhost:8000',
  apiKey: 'your-api-key'
});

// Get current poses
const poses = await client.getCurrentPoses({
  confidenceThreshold: 0.7,
  maxPersons: 5
});

// Subscribe to WebSocket updates
client.subscribeToPoses({
  onData: (poses) => {
    console.log(`Received ${poses.length} poses`);
  },
  onError: (error) => {
    console.error('WebSocket error:', error);
  }
});
```

### cURL Examples

```bash
# Get current poses
curl -X GET "http://localhost:8000/api/v1/pose/current?confidence_threshold=0.7" \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json"

# Start system
curl -X POST "http://localhost:8000/api/v1/system/start" \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "configuration": {
      "domain": "healthcare",
      "environment_id": "room_001"
    }
  }'

# Get zone occupancy
curl -X GET "http://localhost:8000/api/v1/pose/zones/zone_001/occupancy" \
  -H "Authorization: Bearer <token>"
```

---

For more information, see:
- [User Guide](user_guide.md)
- [Deployment Guide](deployment.md)
- [Troubleshooting Guide](troubleshooting.md)
- [Interactive API Documentation](http://localhost:8000/docs)
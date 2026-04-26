# WiFi-DensePose API Endpoints Summary

## Overview

The WiFi-DensePose API provides RESTful endpoints and WebSocket connections for real-time human pose estimation using WiFi CSI (Channel State Information) data. The API is built with FastAPI and supports both synchronous REST operations and real-time streaming via WebSockets.

## Base URL

- **Development**: `http://localhost:8000`
- **API Prefix**: `/api/v1`
- **Documentation**: `http://localhost:8000/docs`

## Authentication

Authentication is configurable via environment variables:
- When `ENABLE_AUTHENTICATION=true`, protected endpoints require JWT tokens
- Tokens can be passed via:
  - Authorization header: `Bearer <token>`
  - Query parameter: `?token=<token>`
  - Cookie: `access_token`

## Rate Limiting

Rate limiting is configurable and when enabled (`ENABLE_RATE_LIMITING=true`):
- Anonymous: 100 requests/hour
- Authenticated: 1000 requests/hour
- Admin: 10000 requests/hour

## Endpoints

### 1. Health & Status

#### GET `/health/health`
System health check with component status and metrics.

**Response Example:**
```json
{
  "status": "healthy",
  "timestamp": "2025-06-09T16:00:00Z",
  "uptime_seconds": 3600.0,
  "components": {
    "hardware": {...},
    "pose": {...},
    "stream": {...}
  },
  "system_metrics": {
    "cpu": {"percent": 24.1, "count": 2},
    "memory": {"total_gb": 7.75, "available_gb": 3.73},
    "disk": {"total_gb": 31.33, "free_gb": 7.09}
  }
}
```

#### GET `/health/ready`
Readiness check for load balancers.

#### GET `/health/live`
Simple liveness check.

#### GET `/health/metrics` 🔒
Detailed system metrics (requires auth).

### 2. Pose Estimation

#### GET `/api/v1/pose/current`
Get current pose estimation from WiFi signals.

**Query Parameters:**
- `zone_ids`: List of zone IDs to analyze
- `confidence_threshold`: Minimum confidence (0.0-1.0)
- `max_persons`: Maximum persons to detect
- `include_keypoints`: Include keypoint data (default: true)
- `include_segmentation`: Include DensePose segmentation (default: false)

**Response Example:**
```json
{
  "timestamp": "2025-06-09T16:00:00Z",
  "frame_id": "frame_123456",
  "persons": [
    {
      "person_id": "0",
      "confidence": 0.95,
      "bounding_box": {"x": 0.1, "y": 0.2, "width": 0.3, "height": 0.6},
      "keypoints": [...],
      "zone_id": "zone_1",
      "activity": "standing"
    }
  ],
  "zone_summary": {"zone_1": 1, "zone_2": 0},
  "processing_time_ms": 45.2
}
```

#### POST `/api/v1/pose/analyze` 🔒
Analyze pose data with custom parameters (requires auth).

#### GET `/api/v1/pose/zones/{zone_id}/occupancy`
Get occupancy for a specific zone.

#### GET `/api/v1/pose/zones/summary`
Get occupancy summary for all zones.

#### GET `/api/v1/pose/activities`
Get recently detected activities.

**Query Parameters:**
- `zone_id`: Filter by zone
- `limit`: Maximum results (1-100)

#### POST `/api/v1/pose/historical` 🔒
Query historical pose data (requires auth).

**Request Body:**
```json
{
  "start_time": "2025-06-09T15:00:00Z",
  "end_time": "2025-06-09T16:00:00Z",
  "zone_ids": ["zone_1"],
  "aggregation_interval": 300,
  "include_raw_data": false
}
```

#### GET `/api/v1/pose/stats`
Get pose estimation statistics.

**Query Parameters:**
- `hours`: Hours of data to analyze (1-168)

### 3. Calibration

#### POST `/api/v1/pose/calibrate` 🔒
Start system calibration (requires auth).

#### GET `/api/v1/pose/calibration/status` 🔒
Get calibration status (requires auth).

### 4. Streaming

#### GET `/api/v1/stream/status`
Get streaming service status.

#### POST `/api/v1/stream/start` 🔒
Start streaming service (requires auth).

#### POST `/api/v1/stream/stop` 🔒
Stop streaming service (requires auth).

#### GET `/api/v1/stream/clients` 🔒
List connected WebSocket clients (requires auth).

#### DELETE `/api/v1/stream/clients/{client_id}` 🔒
Disconnect specific client (requires auth).

#### POST `/api/v1/stream/broadcast` 🔒
Broadcast message to clients (requires auth).

### 5. WebSocket Endpoints

#### WS `/api/v1/stream/pose`
Real-time pose data streaming.

**Query Parameters:**
- `zone_ids`: Comma-separated zone IDs
- `min_confidence`: Minimum confidence (0.0-1.0)
- `max_fps`: Maximum frames per second (1-60)
- `token`: Auth token (if authentication enabled)

**Message Types:**
- `connection_established`: Initial connection confirmation
- `pose_update`: Pose data updates
- `error`: Error messages
- `ping`/`pong`: Keep-alive

#### WS `/api/v1/stream/events`
Real-time event streaming.

**Query Parameters:**
- `event_types`: Comma-separated event types
- `zone_ids`: Comma-separated zone IDs
- `token`: Auth token (if authentication enabled)

### 6. API Information

#### GET `/`
Root endpoint with API information.

#### GET `/api/v1/info`
Detailed API configuration.

#### GET `/api/v1/status`
Current API and service status.

#### GET `/api/v1/metrics`
API performance metrics (if enabled).

### 7. Development Endpoints

These endpoints are only available when `ENABLE_TEST_ENDPOINTS=true`:

#### GET `/api/v1/dev/config`
Get current configuration (development only).

#### POST `/api/v1/dev/reset`
Reset services (development only).

## Error Handling

All errors follow a consistent format:

```json
{
  "error": {
    "code": 400,
    "message": "Error description",
    "type": "error_type"
  }
}
```

Error types:
- `http_error`: HTTP-related errors
- `validation_error`: Request validation errors
- `authentication_error`: Authentication failures
- `rate_limit_exceeded`: Rate limit violations
- `internal_error`: Server errors

## WebSocket Protocol

### Connection Flow

1. **Connect**: `ws://host/api/v1/stream/pose?params`
2. **Receive**: Connection confirmation message
3. **Send/Receive**: Bidirectional communication
4. **Disconnect**: Clean connection closure

### Message Format

All WebSocket messages use JSON format:

```json
{
  "type": "message_type",
  "timestamp": "ISO-8601 timestamp",
  "data": {...}
}
```

### Client Messages

- `{"type": "ping"}`: Keep-alive ping
- `{"type": "update_config", "config": {...}}`: Update stream config
- `{"type": "get_status"}`: Request status
- `{"type": "disconnect"}`: Clean disconnect

### Server Messages

- `{"type": "connection_established", ...}`: Connection confirmed
- `{"type": "pose_update", ...}`: Pose data update
- `{"type": "event", ...}`: Event notification
- `{"type": "pong"}`: Ping response
- `{"type": "error", "message": "..."}`: Error message

## CORS Configuration

CORS is enabled with configurable origins:
- Development: Allow all origins (`*`)
- Production: Restrict to specific domains

## Security Headers

The API includes security headers:
- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: DENY`
- `X-XSS-Protection: 1; mode=block`
- `Referrer-Policy: strict-origin-when-cross-origin`
- `Content-Security-Policy: ...`

## Performance Considerations

1. **Batch Requests**: Use zone summaries instead of individual zone queries
2. **WebSocket Streaming**: Adjust `max_fps` to reduce bandwidth
3. **Historical Data**: Use appropriate `aggregation_interval`
4. **Caching**: Results are cached when Redis is enabled

## Testing

Use the provided test scripts:
- `scripts/test_api_endpoints.py`: Comprehensive endpoint testing
- `scripts/test_websocket_streaming.py`: WebSocket functionality testing

## Production Deployment

For production:
1. Set `ENVIRONMENT=production`
2. Enable authentication and rate limiting
3. Configure proper database (PostgreSQL)
4. Enable Redis for caching
5. Use HTTPS with valid certificates
6. Restrict CORS origins
7. Disable debug mode and test endpoints
8. Configure monitoring and logging

## API Versioning

The API uses URL versioning:
- Current version: `v1`
- Base path: `/api/v1`

Future versions will be available at `/api/v2`, etc.
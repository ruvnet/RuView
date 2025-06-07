# REST API Endpoints

## Overview

The WiFi-DensePose REST API provides comprehensive access to pose estimation data, system configuration, and analytics. This document details all available endpoints, request/response formats, authentication requirements, and usage examples.

## Table of Contents

1. [API Overview](#api-overview)
2. [Authentication](#authentication)
3. [Common Response Formats](#common-response-formats)
4. [Error Handling](#error-handling)
5. [Pose Estimation Endpoints](#pose-estimation-endpoints)
6. [System Management Endpoints](#system-management-endpoints)
7. [Configuration Endpoints](#configuration-endpoints)
8. [Analytics Endpoints](#analytics-endpoints)
9. [Health and Status Endpoints](#health-and-status-endpoints)
10. [Rate Limiting](#rate-limiting)

## API Overview

### Base URL

```
Production: https://api.wifi-densepose.com/api/v1
Staging: https://staging-api.wifi-densepose.com/api/v1
Development: http://localhost:8000/api/v1
```

### API Versioning

The API uses URL path versioning. The current version is `v1`. Future versions will be available at `/api/v2`, etc.

### Content Types

- **Request Content-Type**: `application/json`
- **Response Content-Type**: `application/json`
- **File Upload**: `multipart/form-data`

### HTTP Methods

- **GET**: Retrieve data
- **POST**: Create new resources
- **PUT**: Update existing resources (full replacement)
- **PATCH**: Partial updates
- **DELETE**: Remove resources

## Authentication

### JWT Token Authentication

Most endpoints require JWT token authentication. Include the token in the Authorization header:

```http
Authorization: Bearer <jwt_token>
```

### API Key Authentication

For service-to-service communication, use API key authentication:

```http
X-API-Key: <api_key>
```

### Getting an Access Token

```http
POST /api/v1/auth/token
Content-Type: application/json

{
  "username": "your_username",
  "password": "your_password"
}
```

**Response:**
```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "bearer",
  "expires_in": 86400,
  "refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

## Common Response Formats

### Success Response

```json
{
  "success": true,
  "data": {
    // Response data
  },
  "timestamp": "2025-01-07T10:30:00Z",
  "request_id": "req_123456789"
}
```

### Error Response

```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid request parameters",
    "details": {
      "field": "confidence_threshold",
      "reason": "Value must be between 0 and 1"
    }
  },
  "timestamp": "2025-01-07T10:30:00Z",
  "request_id": "req_123456789"
}
```

### Pagination

```json
{
  "success": true,
  "data": [
    // Array of items
  ],
  "pagination": {
    "page": 1,
    "per_page": 50,
    "total": 1250,
    "total_pages": 25,
    "has_next": true,
    "has_prev": false
  }
}
```

## Error Handling

### HTTP Status Codes

- **200 OK**: Request successful
- **201 Created**: Resource created successfully
- **400 Bad Request**: Invalid request parameters
- **401 Unauthorized**: Authentication required
- **403 Forbidden**: Insufficient permissions
- **404 Not Found**: Resource not found
- **422 Unprocessable Entity**: Validation error
- **429 Too Many Requests**: Rate limit exceeded
- **500 Internal Server Error**: Server error

### Error Codes

| Code | Description |
|------|-------------|
| `VALIDATION_ERROR` | Request validation failed |
| `AUTHENTICATION_ERROR` | Authentication failed |
| `AUTHORIZATION_ERROR` | Insufficient permissions |
| `RESOURCE_NOT_FOUND` | Requested resource not found |
| `RATE_LIMIT_EXCEEDED` | Too many requests |
| `SYSTEM_ERROR` | Internal system error |
| `HARDWARE_ERROR` | Hardware communication error |
| `MODEL_ERROR` | Neural network model error |

## Pose Estimation Endpoints

### Get Latest Pose Data

Retrieve the most recent pose estimation results.

```http
GET /api/v1/pose/latest
Authorization: Bearer <token>
```

**Query Parameters:**
- `environment_id` (optional): Filter by environment ID
- `min_confidence` (optional): Minimum confidence threshold (0.0-1.0)
- `include_keypoints` (optional): Include detailed keypoint data (default: true)

**Response:**
```json
{
  "success": true,
  "data": {
    "timestamp": "2025-01-07T10:30:00.123Z",
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
          },
          {
            "name": "left_eye",
            "x": 205,
            "y": 90,
            "confidence": 0.89,
            "visible": true
          }
          // ... additional keypoints
        ],
        "dense_pose": {
          "iuv_image": "base64_encoded_image_data",
          "confidence_map": "base64_encoded_confidence_data"
        }
      }
    ],
    "metadata": {
      "model_version": "v1.2.0",
      "processing_mode": "real_time",
      "csi_quality": 0.85
    }
  }
}
```

### Get Historical Pose Data

Retrieve pose estimation data for a specific time range.

```http
GET /api/v1/pose/history
Authorization: Bearer <token>
```

**Query Parameters:**
- `start_time` (required): Start timestamp (ISO 8601)
- `end_time` (required): End timestamp (ISO 8601)
- `environment_id` (optional): Filter by environment ID
- `person_id` (optional): Filter by person ID
- `track_id` (optional): Filter by track ID
- `min_confidence` (optional): Minimum confidence threshold
- `page` (optional): Page number (default: 1)
- `per_page` (optional): Items per page (default: 50, max: 1000)

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "timestamp": "2025-01-07T10:30:00.123Z",
      "frame_id": 12345,
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
        // Keypoint data
      ]
    }
    // ... additional pose data
  ],
  "pagination": {
    "page": 1,
    "per_page": 50,
    "total": 1250,
    "total_pages": 25,
    "has_next": true,
    "has_prev": false
  }
}
```

### Get Person Tracking Data

Retrieve tracking information for a specific person or track.

```http
GET /api/v1/pose/tracking/{track_id}
Authorization: Bearer <token>
```

**Path Parameters:**
- `track_id` (required): Track identifier

**Query Parameters:**
- `start_time` (optional): Start timestamp
- `end_time` (optional): End timestamp
- `include_trajectory` (optional): Include movement trajectory (default: false)

**Response:**
```json
{
  "success": true,
  "data": {
    "track_id": 7,
    "person_id": 1,
    "first_seen": "2025-01-07T10:25:00Z",
    "last_seen": "2025-01-07T10:35:00Z",
    "duration_seconds": 600,
    "total_frames": 18000,
    "average_confidence": 0.84,
    "status": "active",
    "trajectory": [
      {
        "timestamp": "2025-01-07T10:25:00Z",
        "center_x": 210,
        "center_y": 240,
        "confidence": 0.87
      }
      // ... trajectory points
    ],
    "statistics": {
      "movement_distance": 15.7,
      "average_speed": 0.026,
      "time_stationary": 420,
      "time_moving": 180
    }
  }
}
```

### Submit CSI Data for Processing

Submit raw CSI data for pose estimation processing.

```http
POST /api/v1/pose/process
Authorization: Bearer <token>
Content-Type: application/json

{
  "csi_data": {
    "timestamp": "2025-01-07T10:30:00.123Z",
    "antenna_data": [
      [
        {"real": 1.23, "imag": -0.45},
        {"real": 0.87, "imag": 1.12}
        // ... subcarrier data
      ]
      // ... antenna data
    ],
    "metadata": {
      "router_id": "router_001",
      "sampling_rate": 30,
      "signal_strength": -45
    }
  },
  "processing_options": {
    "confidence_threshold": 0.5,
    "max_persons": 10,
    "enable_tracking": true,
    "return_dense_pose": false
  }
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "processing_id": "proc_123456",
    "status": "completed",
    "processing_time_ms": 67.3,
    "poses": [
      // Pose estimation results
    ]
  }
}
```

## System Management Endpoints

### Start System

Start the pose estimation system with specified configuration.

```http
POST /api/v1/system/start
Authorization: Bearer <token>
Content-Type: application/json

{
  "configuration": {
    "domain": "healthcare",
    "environment_id": "room_001",
    "detection_settings": {
      "confidence_threshold": 0.7,
      "max_persons": 5,
      "enable_tracking": true
    },
    "hardware_settings": {
      "csi_sampling_rate": 30,
      "buffer_size": 1000
    }
  }
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "status": "starting",
    "session_id": "session_123456",
    "estimated_startup_time": 15,
    "configuration_applied": {
      // Applied configuration
    }
  }
}
```

### Stop System

Stop the pose estimation system.

```http
POST /api/v1/system/stop
Authorization: Bearer <token>
```

**Response:**
```json
{
  "success": true,
  "data": {
    "status": "stopping",
    "session_id": "session_123456",
    "shutdown_initiated": "2025-01-07T10:30:00Z"
  }
}
```

### Get System Status

Get current system status and performance metrics.

```http
GET /api/v1/system/status
Authorization: Bearer <token>
```

**Response:**
```json
{
  "success": true,
  "data": {
    "status": "running",
    "session_id": "session_123456",
    "uptime_seconds": 3600,
    "started_at": "2025-01-07T09:30:00Z",
    "performance": {
      "frames_processed": 108000,
      "average_fps": 29.8,
      "average_latency_ms": 45.2,
      "cpu_usage": 65.4,
      "memory_usage": 78.2,
      "gpu_usage": 82.1
    },
    "components": {
      "csi_processor": {
        "status": "healthy",
        "last_heartbeat": "2025-01-07T10:29:55Z"
      },
      "neural_network": {
        "status": "healthy",
        "model_loaded": true,
        "inference_queue_size": 3
      },
      "tracker": {
        "status": "healthy",
        "active_tracks": 2
      },
      "database": {
        "status": "healthy",
        "connection_pool": "8/20"
      }
    }
  }
}
```

### Restart System

Restart the pose estimation system.

```http
POST /api/v1/system/restart
Authorization: Bearer <token>
```

**Response:**
```json
{
  "success": true,
  "data": {
    "status": "restarting",
    "previous_session_id": "session_123456",
    "new_session_id": "session_789012",
    "estimated_restart_time": 30
  }
}
```

## Configuration Endpoints

### Get Current Configuration

Retrieve the current system configuration.

```http
GET /api/v1/config
Authorization: Bearer <token>
```

**Response:**
```json
{
  "success": true,
  "data": {
    "domain": "healthcare",
    "environment_id": "room_001",
    "detection": {
      "confidence_threshold": 0.7,
      "max_persons": 5,
      "enable_tracking": true,
      "tracking_max_age": 30,
      "tracking_min_hits": 3
    },
    "neural_network": {
      "model_version": "v1.2.0",
      "batch_size": 32,
      "enable_gpu": true,
      "inference_timeout": 1000
    },
    "hardware": {
      "csi_sampling_rate": 30,
      "buffer_size": 1000,
      "antenna_count": 3,
      "subcarrier_count": 56
    },
    "analytics": {
      "enable_fall_detection": true,
      "enable_activity_recognition": true,
      "alert_thresholds": {
        "fall_confidence": 0.8,
        "inactivity_timeout": 300
      }
    },
    "privacy": {
      "data_retention_days": 30,
      "anonymize_data": true,
      "enable_encryption": true
    }
  }
}
```

### Update Configuration

Update system configuration (requires system restart for some changes).

```http
PUT /api/v1/config
Authorization: Bearer <token>
Content-Type: application/json

{
  "detection": {
    "confidence_threshold": 0.8,
    "max_persons": 3
  },
  "analytics": {
    "enable_fall_detection": true,
    "alert_thresholds": {
      "fall_confidence": 0.9
    }
  }
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "updated_fields": [
      "detection.confidence_threshold",
      "detection.max_persons",
      "analytics.alert_thresholds.fall_confidence"
    ],
    "requires_restart": false,
    "applied_at": "2025-01-07T10:30:00Z",
    "configuration": {
      // Updated configuration
    }
  }
}
```

### Get Configuration Schema

Get the configuration schema with validation rules and descriptions.

```http
GET /api/v1/config/schema
Authorization: Bearer <token>
```

**Response:**
```json
{
  "success": true,
  "data": {
    "schema": {
      "type": "object",
      "properties": {
        "detection": {
          "type": "object",
          "properties": {
            "confidence_threshold": {
              "type": "number",
              "minimum": 0.0,
              "maximum": 1.0,
              "description": "Minimum confidence for pose detection"
            }
          }
        }
      }
    },
    "defaults": {
      // Default configuration values
    }
  }
}
```

## Analytics Endpoints

### Get Analytics Summary

Get analytics summary for a specified time period.

```http
GET /api/v1/analytics/summary
Authorization: Bearer <token>
```

**Query Parameters:**
- `start_time` (required): Start timestamp
- `end_time` (required): End timestamp
- `environment_id` (optional): Filter by environment
- `granularity` (optional): Data granularity (hour, day, week)

**Response:**
```json
{
  "success": true,
  "data": {
    "time_period": {
      "start": "2025-01-07T00:00:00Z",
      "end": "2025-01-07T23:59:59Z",
      "duration_hours": 24
    },
    "detection_stats": {
      "total_detections": 15420,
      "unique_persons": 47,
      "average_confidence": 0.84,
      "peak_occupancy": 8,
      "peak_occupancy_time": "2025-01-07T14:30:00Z"
    },
    "activity_stats": {
      "total_movement_events": 1250,
      "fall_detections": 2,
      "alert_count": 5,
      "average_activity_level": 0.67
    },
    "system_stats": {
      "uptime_percentage": 99.8,
      "average_processing_time": 45.2,
      "frames_processed": 2592000,
      "error_count": 12
    },
    "hourly_breakdown": [
      {
        "hour": "2025-01-07T00:00:00Z",
        "detections": 420,
        "unique_persons": 2,
        "average_confidence": 0.82
      }
      // ... hourly data
    ]
  }
}
```

### Get Activity Events

Retrieve detected activity events (falls, alerts, etc.).

```http
GET /api/v1/analytics/events
Authorization: Bearer <token>
```

**Query Parameters:**
- `start_time` (optional): Start timestamp
- `end_time` (optional): End timestamp
- `event_type` (optional): Filter by event type (fall, alert, activity)
- `severity` (optional): Filter by severity (low, medium, high)
- `environment_id` (optional): Filter by environment

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "event_id": "event_123456",
      "type": "fall_detection",
      "severity": "high",
      "timestamp": "2025-01-07T14:25:30Z",
      "environment_id": "room_001",
      "person_id": 3,
      "track_id": 15,
      "confidence": 0.92,
      "location": {
        "x": 210,
        "y": 180
      },
      "metadata": {
        "fall_duration": 2.3,
        "impact_severity": 0.85,
        "recovery_detected": false
      },
      "actions_taken": [
        "alert_sent",
        "notification_dispatched"
      ]
    }
    // ... additional events
  ]
}
```

### Get Occupancy Data

Get occupancy statistics and trends.

```http
GET /api/v1/analytics/occupancy
Authorization: Bearer <token>
```

**Query Parameters:**
- `start_time` (required): Start timestamp
- `end_time` (required): End timestamp
- `environment_id` (optional): Filter by environment
- `interval` (optional): Data interval (5min, 15min, 1hour)

**Response:**
```json
{
  "success": true,
  "data": {
    "summary": {
      "average_occupancy": 3.2,
      "peak_occupancy": 8,
      "peak_time": "2025-01-07T14:30:00Z",
      "total_person_hours": 76.8
    },
    "time_series": [
      {
        "timestamp": "2025-01-07T00:00:00Z",
        "occupancy": 2,
        "confidence": 0.89
      },
      {
        "timestamp": "2025-01-07T00:15:00Z",
        "occupancy": 1,
        "confidence": 0.92
      }
      // ... time series data
    ],
    "distribution": {
      "0_persons": 15.2,
      "1_person": 42.8,
      "2_persons": 28.5,
      "3_persons": 10.1,
      "4_plus_persons": 3.4
    }
  }
}
```

## Health and Status Endpoints

### Health Check

Basic health check endpoint for load balancers and monitoring.

```http
GET /api/v1/health
```

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2025-01-07T10:30:00Z",
  "version": "1.2.0",
  "uptime": 3600
}
```

### Detailed Health Check

Comprehensive health check with component status.

```http
GET /api/v1/health/detailed
Authorization: Bearer <token>
```

**Response:**
```json
{
  "success": true,
  "data": {
    "overall_status": "healthy",
    "timestamp": "2025-01-07T10:30:00Z",
    "version": "1.2.0",
    "uptime": 3600,
    "components": {
      "api": {
        "status": "healthy",
        "response_time_ms": 12.3,
        "requests_per_second": 45.2
      },
      "database": {
        "status": "healthy",
        "connection_pool": "8/20",
        "query_time_ms": 5.7
      },
      "redis": {
        "status": "healthy",
        "memory_usage": "45%",
        "connected_clients": 12
      },
      "neural_network": {
        "status": "healthy",
        "model_loaded": true,
        "gpu_memory_usage": "78%",
        "inference_queue": 2
      },
      "csi_processor": {
        "status": "healthy",
        "data_rate": 30.1,
        "buffer_usage": "23%"
      }
    },
    "metrics": {
      "cpu_usage": 65.4,
      "memory_usage": 78.2,
      "disk_usage": 45.8,
      "network_io": {
        "bytes_in": 1024000,
        "bytes_out": 2048000
      }
    }
  }
}
```

### System Metrics

Get detailed system performance metrics.

```http
GET /api/v1/metrics
Authorization: Bearer <token>
```

**Query Parameters:**
- `start_time` (optional): Start timestamp for historical metrics
- `end_time` (optional): End timestamp for historical metrics
- `metric_type` (optional): Filter by metric type

**Response:**
```json
{
  "success": true,
  "data": {
    "current": {
      "timestamp": "2025-01-07T10:30:00Z",
      "performance": {
        "frames_per_second": 29.8,
        "average_latency_ms": 45.2,
        "processing_queue_size": 3,
        "error_rate": 0.001
      },
      "resources": {
        "cpu_usage": 65.4,
        "memory_usage": 78.2,
        "gpu_usage": 82.1,
        "disk_io": {
          "read_mb_per_sec": 12.5,
          "write_mb_per_sec": 8.3
        }
      },
      "business": {
        "active_persons": 3,
        "detections_per_minute": 89.5,
        "tracking_accuracy": 0.94
      }
    },
    "historical": [
      {
        "timestamp": "2025-01-07T10:25:00Z",
        "frames_per_second": 30.1,
        "average_latency_ms": 43.8,
        "cpu_usage": 62.1
      }
      // ... historical data points
    ]
  }
}
```

## Rate Limiting

### Rate Limit Headers

All API responses include rate limiting headers:

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1704686400
X-RateLimit-Window: 3600
```

### Rate Limits by Endpoint Category

| Category | Limit | Window |
|----------|-------|--------|
| Authentication | 10 requests | 1 minute |
| Pose Data (GET) | 1000 requests | 1 hour |
| Pose Processing (POST) | 100 requests | 1 hour |
| Configuration | 50 requests | 1 hour |
| Analytics | 500 requests | 1 hour |
| Health Checks | 10000 requests | 1 hour |

### Rate Limit Exceeded Response

```json
{
  "success": false,
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded. Try again in 45 seconds.",
    "details": {
      "limit": 1000,
      "window": 3600,
      "reset_at": "2025-01-07T11:00:00Z"
    }
  }
}
```

---

This REST API documentation provides comprehensive coverage of all available endpoints. For real-time data streaming, see the [WebSocket API documentation](websocket-api.md). For authentication details, see the [Authentication documentation](authentication.md).

For code examples in multiple languages, see the [API Examples documentation](examples.md).
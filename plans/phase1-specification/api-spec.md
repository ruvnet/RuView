# API Specification
## WiFi-DensePose System

### Document Information
- **Version**: 1.0
- **Date**: 2025-01-07
- **Project**: InvisPose - WiFi-Based Dense Human Pose Estimation
- **Status**: Draft

---

## 1. Introduction

### 1.1 Purpose
This document defines the complete API specification for the WiFi-DensePose system, including REST endpoints, WebSocket protocols, data models, authentication mechanisms, and external integration interfaces.

### 1.2 Scope
The API specification covers all programmatic interfaces for pose data access, system control, real-time streaming, external integrations, and authentication/authorization mechanisms.

### 1.3 API Overview
The system provides a comprehensive FastAPI-based REST interface with WebSocket streaming capabilities, supporting real-time pose data distribution, system management, and integration with external services including MQTT, webhooks, and Restream platforms.

---

## 2. REST API Endpoints

### 2.1 Pose Data Endpoints

#### 2.1.1 Get Latest Pose Data
**Endpoint**: `GET /pose/latest`
**Description**: Retrieve the most recent pose estimation results
**Authentication**: Bearer token required

**Response Format**:
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
          "confidence": 0.95
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
      }
    }
  ],
  "metadata": {
    "environment_id": "room_001",
    "router_count": 3,
    "signal_quality": 0.82
  }
}
```

**Error Responses**:
- `404`: No pose data available
- `503`: System not initialized
- `401`: Authentication required

// TEST: Verify latest pose endpoint returns valid pose data structure
// TEST: Confirm error handling for missing data scenarios
// TEST: Validate authentication token requirements

#### 2.1.2 Get Historical Pose Data
**Endpoint**: `GET /pose/history`
**Description**: Retrieve historical pose data with filtering options
**Authentication**: Bearer token required

**Query Parameters**:
- `start_time` (optional): ISO 8601 timestamp for range start
- `end_time` (optional): ISO 8601 timestamp for range end
- `limit` (optional): Maximum number of records (default: 100, max: 1000)
- `person_id` (optional): Filter by specific person ID
- `confidence_threshold` (optional): Minimum confidence score (0.0-1.0)

**Response Format**:
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
    "returned_count": 100,
    "has_more": true,
    "next_cursor": "eyJpZCI6MTIzNDV9"
  }
}
```

// TEST: Validate historical data retrieval with various filter combinations
// TEST: Confirm pagination functionality works correctly
// TEST: Verify time range filtering accuracy

#### 2.1.3 Query Pose Data
**Endpoint**: `POST /pose/query`
**Description**: Execute complex queries on pose data
**Authentication**: Bearer token required

**Request Body**:
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

**Response Format**:
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

// TEST: Verify complex query execution with multiple filters
// TEST: Confirm aggregation calculations are accurate
// TEST: Validate query performance within acceptable limits

### 2.2 System Control Endpoints

#### 2.2.1 System Status
**Endpoint**: `GET /system/status`
**Description**: Get comprehensive system health and status information
**Authentication**: Bearer token required

**Response Format**:
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

// TEST: Verify system status endpoint returns accurate component states
// TEST: Confirm hardware metrics are within expected ranges
// TEST: Validate network status reflects actual router connectivity

#### 2.2.2 Start System
**Endpoint**: `POST /system/start`
**Description**: Start the pose estimation system
**Authentication**: Bearer token required

**Request Body**:
```json
{
  "configuration": {
    "domain": "healthcare",
    "environment_id": "room_001",
    "calibration_required": true
  }
}
```

**Response Format**:
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

// TEST: Verify system startup sequence completes successfully
// TEST: Confirm initialization steps progress correctly
// TEST: Validate configuration parameters are applied

#### 2.2.3 Stop System
**Endpoint**: `POST /system/stop`
**Description**: Gracefully stop the pose estimation system
**Authentication**: Bearer token required

**Request Body**:
```json
{
  "force": false,
  "save_state": true
}
```

**Response Format**:
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

// TEST: Verify graceful system shutdown preserves data integrity
// TEST: Confirm force stop functionality works when needed
// TEST: Validate state saving during shutdown process

### 2.3 Configuration Management Endpoints

#### 2.3.1 Get Configuration
**Endpoint**: `GET /config`
**Description**: Retrieve current system configuration
**Authentication**: Bearer token required

**Response Format**:
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

// TEST: Verify configuration retrieval returns complete settings
// TEST: Confirm domain-specific configurations are properly loaded
// TEST: Validate configuration structure matches schema

#### 2.3.2 Update Configuration
**Endpoint**: `PUT /config`
**Description**: Update system configuration
**Authentication**: Bearer token required

**Request Body**:
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

**Response Format**:
```json
{
  "status": "updated",
  "changes_applied": [
    "detection.confidence_threshold",
    "alerts.fall_detection.sensitivity"
  ],
  "restart_required": false,
  "validation_warnings": []
}
```

// TEST: Verify configuration updates are applied correctly
// TEST: Confirm validation prevents invalid configuration values
// TEST: Validate restart requirements are accurately reported

### 2.4 Domain-Specific Endpoints

#### 2.4.1 Healthcare Analytics
**Endpoint**: `GET /analytics/healthcare`
**Description**: Retrieve healthcare-specific analytics and insights
**Authentication**: Bearer token required

**Query Parameters**:
- `period`: Time period (hour, day, week, month)
- `metrics`: Comma-separated list of metrics

**Response Format**:
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
          "response_time_seconds": 45
        }
      ]
    },
    "activity_summary": {
      "walking_minutes": 120,
      "sitting_minutes": 480,
      "lying_minutes": 360,
      "standing_minutes": 180
    },
    "mobility_score": 0.75
  }
}
```

// TEST: Verify healthcare analytics calculations are accurate
// TEST: Confirm fall detection events are properly recorded
// TEST: Validate activity classification metrics

#### 2.4.2 Retail Analytics
**Endpoint**: `GET /analytics/retail`
**Description**: Retrieve retail-specific analytics and insights
**Authentication**: Bearer token required

**Response Format**:
```json
{
  "period": "day",
  "date": "2025-01-07",
  "metrics": {
    "traffic": {
      "total_visitors": 245,
      "peak_hour": "14:00",
      "peak_count": 15,
      "average_dwell_time_minutes": 12.5
    },
    "zones": [
      {
        "zone_id": "entrance",
        "visitor_count": 245,
        "avg_dwell_time_minutes": 2.1
      },
      {
        "zone_id": "electronics",
        "visitor_count": 89,
        "avg_dwell_time_minutes": 8.7
      }
    ],
    "conversion_funnel": {
      "entrance": 245,
      "product_interaction": 156,
      "checkout": 67
    }
  }
}
```

// TEST: Verify retail traffic counting accuracy
// TEST: Confirm zone analytics provide meaningful insights
// TEST: Validate conversion funnel calculations

---

## 3. WebSocket Protocols

### 3.1 Real-Time Pose Streaming

#### 3.1.1 Connection Establishment
**Endpoint**: `ws://host:port/ws/pose`
**Authentication**: Token via query parameter or header

**Connection Message**:
```json
{
  "type": "connection_established",
  "client_id": "client_12345",
  "server_time": "2025-01-07T04:46:32Z",
  "supported_protocols": ["pose_v1", "alerts_v1"]
}
```

#### 3.1.2 Subscription Management
**Subscribe to Pose Updates**:
```json
{
  "type": "subscribe",
  "channel": "pose_updates",
  "filters": {
    "min_confidence": 0.7,
    "person_ids": [1, 2, 3]
  }
}
```

**Subscription Confirmation**:
```json
{
  "type": "subscription_confirmed",
  "channel": "pose_updates",
  "subscription_id": "sub_67890"
}
```

// TEST: Verify WebSocket connection establishment works correctly
// TEST: Confirm subscription filtering functions as expected
// TEST: Validate subscription management handles multiple channels

#### 3.1.3 Pose Data Streaming
**Pose Update Message**:
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

**System Status Update**:
```json
{
  "type": "system_status",
  "timestamp": "2025-01-07T04:46:32Z",
  "status": {
    "processing_fps": 25.3,
    "active_persons": 2,
    "system_health": "good"
  }
}
```

// TEST: Verify pose data streaming maintains real-time performance
// TEST: Confirm message ordering and delivery guarantees
// TEST: Validate system status updates are timely and accurate

### 3.2 Alert Streaming

#### 3.2.1 Alert Subscription
**Subscribe to Alerts**:
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

#### 3.2.2 Alert Messages
**Fall Detection Alert**:
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

// TEST: Verify alert streaming delivers critical notifications immediately
// TEST: Confirm alert filtering works for different severity levels
// TEST: Validate alert data contains all necessary information

---

## 4. Data Models and Schemas

### 4.1 Core Data Models

#### 4.1.1 Person Model
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "id": {
      "type": "integer",
      "description": "Unique person identifier"
    },
    "confidence": {
      "type": "number",
      "minimum": 0.0,
      "maximum": 1.0,
      "description": "Detection confidence score"
    },
    "bounding_box": {
      "$ref": "#/definitions/BoundingBox"
    },
    "keypoints": {
      "type": "array",
      "items": {"$ref": "#/definitions/Keypoint"}
    },
    "dense_pose": {
      "$ref": "#/definitions/DensePose"
    },
    "tracking_info": {
      "$ref": "#/definitions/TrackingInfo"
    }
  },
  "required": ["id", "confidence", "bounding_box", "keypoints"]
}
```

#### 4.1.2 Keypoint Model
```json
{
  "type": "object",
  "properties": {
    "name": {
      "type": "string",
      "enum": ["nose", "left_eye", "right_eye", "left_ear", "right_ear", 
               "left_shoulder", "right_shoulder", "left_elbow", "right_elbow",
               "left_wrist", "right_wrist", "left_hip", "right_hip",
               "left_knee", "right_knee", "left_ankle", "right_ankle"]
    },
    "x": {"type": "number"},
    "y": {"type": "number"},
    "confidence": {
      "type": "number",
      "minimum": 0.0,
      "maximum": 1.0
    },
    "visible": {"type": "boolean"}
  },
  "required": ["name", "x", "y", "confidence"]
}
```

#### 4.1.3 Dense Pose Model
```json
{
  "type": "object",
  "properties": {
    "body_parts": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "part_id": {"type": "integer"},
          "part_name": {"type": "string"},
          "uv_coordinates": {
            "type": "array",
            "items": {
              "type": "array",
              "items": {"type": "number"},
              "minItems": 2,
              "maxItems": 2
            }
          },
          "confidence": {
            "type": "number",
            "minimum": 0.0,
            "maximum": 1.0
          }
        },
        "required": ["part_id", "part_name", "uv_coordinates", "confidence"]
      }
    }
  }
}
```

// TEST: Verify data models validate correctly against schemas
// TEST: Confirm all required fields are present in API responses
// TEST: Validate data type constraints are enforced

### 4.2 Configuration Schemas

#### 4.2.1 System Configuration Schema
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "domain": {
      "type": "string",
      "enum": ["healthcare", "retail", "security", "general"]
    },
    "environment": {
      "type": "object",
      "properties": {
        "id": {"type": "string"},
        "name": {"type": "string"},
        "calibration_timestamp": {"type": "string", "format": "date-time"}
      },
      "required": ["id", "name"]
    },
    "detection": {
      "type": "object",
      "properties": {
        "confidence_threshold": {
          "type": "number",
          "minimum": 0.0,
          "maximum": 1.0,
          "default": 0.7
        },
        "max_persons": {
          "type": "integer",
          "minimum": 1,
          "maximum": 10,
          "default": 5
        },
        "tracking_enabled": {
          "type": "boolean",
          "default": true
        }
      }
    }
  },
  "required": ["domain", "environment", "detection"]
}
```

// TEST: Verify configuration schema validation prevents invalid settings
// TEST: Confirm default values are applied when not specified
// TEST: Validate domain-specific configuration requirements

---

## 5. Authentication and Authorization

### 5.1 Authentication Methods

#### 5.1.1 Bearer Token Authentication
**Header Format**: `Authorization: Bearer <token>`
**Token Type**: JWT (JSON Web Token)
**Expiration**: Configurable (default: 24 hours)

**Token Payload**:
```json
{
  "sub": "user_12345",
  "iat": 1704600000,
  "exp": 1704686400,
  "scope": ["pose:read", "system:control", "config:write"],
  "domain": "healthcare"
}
```

#### 5.1.2 API Key Authentication
**Header Format**: `X-API-Key: <api_key>`
**Use Case**: Service-to-service communication
**Scope**: Limited to specific endpoints

// TEST: Verify JWT token validation works correctly
// TEST: Confirm API key authentication for service accounts
// TEST: Validate token expiration handling

### 5.2 Authorization Scopes

#### 5.2.1 Permission Levels
- `pose:read` - Read pose data and analytics
- `pose:stream` - Access real-time streaming
- `system:control` - Start/stop system operations
- `system:status` - View system status and health
- `config:read` - Read configuration settings
- `config:write` - Modify configuration settings
- `alerts:manage` - Manage alert configurations
- `admin:full` - Full administrative access

#### 5.2.2 Domain-Based Access Control
- Healthcare domain: Additional HIPAA compliance requirements
- Retail domain: Customer privacy protections
- Security domain: Enhanced audit logging
- General domain: Standard access controls

// TEST: Verify permission-based access control works correctly
// TEST: Confirm domain-specific authorization rules
// TEST: Validate audit logging for sensitive operations

---

## 6. External Integration APIs

### 6.1 MQTT Integration

#### 6.1.1 Topic Structure
```
wifi-densepose/
├── pose/
│   ├── person/{person_id}     # Individual person data
│   ├── summary                # Aggregated pose data
│   └── raw                    # Raw pose frames
├── alerts/
│   ├── fall_detection         # Fall detection alerts
│   ├── intrusion             # Security alerts
│   └── system                # System alerts
├── status/
│   ├── system                # System health status
│   ├── hardware              # Hardware status
│   └── network               # Network connectivity
└── analytics/
    ├── healthcare            # Healthcare metrics
    ├── retail                # Retail analytics
    └── security              # Security metrics
```

#### 6.1.2 Message Formats
**Person Pose Message**:
```json
{
  "timestamp": "2025-01-07T04:46:32Z",
  "person_id": 1,
  "confidence": 0.87,
  "keypoints": [...],
  "activity": "walking",
  "location": {"x": 220, "y": 180}
}
```

**Alert Message**:
```json
{
  "alert_id": "alert_12345",
  "timestamp": "2025-01-07T04:46:32Z",
  "type": "fall_detection",
  "severity": "high",
  "person_id": 1,
  "location": {"x": 220, "y": 180},
  "confidence": 0.92
}
```

// TEST: Verify MQTT message publishing works reliably
// TEST: Confirm topic structure follows specification
// TEST: Validate message format consistency

### 6.2 Webhook Integration

#### 6.2.1 Webhook Configuration
**Endpoint**: `POST /webhooks`
**Description**: Configure webhook endpoints for event notifications

**Request Body**:
```json
{
  "url": "https://example.com/webhook",
  "events": ["fall_detection", "person_detected"],
  "authentication": {
    "type": "bearer",
    "token": "webhook_token_12345"
  },
  "retry_policy": {
    "max_retries": 3,
    "retry_delay_seconds": 5
  }
}
```

#### 6.2.2 Webhook Payload
**Event Notification**:
```json
{
  "webhook_id": "webhook_67890",
  "event_type": "fall_detection",
  "timestamp": "2025-01-07T04:46:32Z",
  "data": {
    "alert_id": "alert_12345",
    "person_id": 1,
    "severity": "high",
    "location": {"x": 220, "y": 180}
  },
  "metadata": {
    "environment_id": "room_001",
    "system_version": "1.0.0"
  }
}
```

// TEST: Verify webhook delivery with retry logic
// TEST: Confirm authentication methods work correctly
// TEST: Validate event filtering and payload formatting

### 6.3 Restream Integration

#### 6.3.1 Stream Configuration
**Endpoint**: `POST /streaming/restream`
**Description**: Configure Restream integration for live broadcasting

**Request Body**:
```json
{
  "restream_key": "restream_api_key",
  "platforms": ["youtube", "twitch", "facebook"],
  "video_settings": {
    "resolution": "1280x720",
    "fps": 30,
    "bitrate": 2500
  },
  "overlay_settings": {
    "show_keypoints": true,
    "show_confidence": true,
    "show_person_ids": true,
    "background_type": "transparent"
  }
}
```

#### 6.3.2 Stream Status
**Endpoint**: `GET /streaming/status`
**Response**:
```json
{
  "status": "streaming",
  "platforms": [
    {
      "name": "youtube",
      "status": "connected",
      "viewers": 45,
      "uptime_seconds": 3600
    },
    {
      "name": "twitch",
      "status": "connected",
      "viewers": 23,
      "uptime_seconds": 3600
    }
  ],
  "video_stats": {
    "fps": 29.8,
    "bitrate": 2480,
    "dropped_frames": 12
  }
}
```

// TEST: Verify Restream integration connects successfully
// TEST: Confirm multi-platform streaming works simultaneously
// TEST: Validate video quality and performance metrics

---

## 7. Error Handling and Status Codes

### 7.1 HTTP Status Codes

#### 7.1.1 Success Codes
- `200 OK` - Request successful
- `201 Created` - Resource created successfully
- `202 Accepted` - Request accepted for processing
- `204 No Content` - Request successful, no content returned

#### 7.1.2 Client Error Codes
- `400 Bad Request` - Invalid request format or parameters
- `401 Unauthorized` - Authentication required or invalid
- `403 Forbidden` - Insufficient permissions
- `404 Not Found` - Resource not found
- `409 Conflict` - Resource conflict (e.g., system already running)
- `422 Unprocessable Entity` - Validation errors
- `429 Too Many Requests` - Rate limit exceeded

#### 7.1.3 Server Error Codes
- `500 Internal Server Error` - Unexpected server error
- `502 Bad Gateway` - Upstream service error
- `503 Service Unavailable` - System not ready or overloaded
- `504 Gateway Timeout` - Request timeout

### 7.2 Error Response Format

#### 7.2.1 Standard Error Response
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

#### 7.2.2 Validation Error Response
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
        }
      ]
    },
    "timestamp": "2025-01-07T04:46:32Z",
    "request_id": "req_12346"
  }
}
```

// TEST: Verify error responses follow consistent format
// TEST: Confirm appropriate status codes are returned
// TEST: Validate error details provide actionable information

---

## 8. Rate Limiting and Performance

### 8.1 Rate Limiting

#### 8.1.1 Rate Limit Configuration
- **REST API**: 1000 requests per hour per API key
- **WebSocket**: 100 connections per IP address
- **Streaming**: 10 concurrent streams per account
- **Webhook**: 10,000 events per hour per endpoint

#### 8.1.2 Rate Limit Headers
```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1704686400
X-RateLimit-Window: 3600
```

### 8.2 Performance Requirements

#### 8.2.1 Response Time Targets
- **Pose Data Endpoints**: <100ms (95th percentile)
- **System Control**: <500ms (95th percentile)
- **Configuration Updates**: <200ms (95th percentile)
- **WebSocket Messages**: <50ms (95th percentile)

#### 8.2.2 Throughput Targets
- **REST API**: 10,000 requests per second
- **WebSocket**: 1,000 concurrent connections
- **Pose Updates**: 30 FPS per stream
- **Alert Processing**: <1 second end-to-end

// TEST: Verify rate limiting enforces configured limits
// TEST: Confirm performance targets are met under load
// TEST: Validate system scales to handle concurrent users

---

## 9. API Versioning and Compatibility

### 9.1 Versioning Strategy

#### 9.1.1 URL Versioning
- Current version: `/api/v1/`
- Future versions: `/api/v2/`, `/api/v3/`
- Version-specific endpoints maintain backward compatibility

#### 9.1.2 Header Versioning
- `Accept: application/vnd.wifi-densepose.v1+json`
- `API-Version: 1.0`

### 9.2 Deprecation Policy

#### 9.2.1 Deprecation Timeline
- **Notice Period**: 6 months advance notice
- **Support Period**: 12 months after deprecation notice
- **Migration Support**: Documentation and tools provided

#### 9.2.2 Deprecation Headers
```
Deprecation: true
Sunset: Wed, 07 Jan 2026 04:46:32 GMT
Link: </api/v2/pose/latest>; rel="successor-version"
```

// TEST: Verify API versioning works correctly
// TEST: Confirm backward compatibility is maintained
// TEST: Validate deprecation notices are properly communicated

---

## 10. Testing and Validation

### 10.1 API Testing Framework

#### 10.1.1 Test Categories
- **Unit Tests**: Individual endpoint functionality
- **Integration Tests**: End-to-end API workflows
- **Performance Tests**: Load and stress testing
- **Security Tests**: Authentication and authorization
- **Contract Tests**: API schema validation

#### 10.1.2 Test Data Management
- **Synthetic Data**: Generated test poses and scenarios
- **Recorded Data**: Real CSI data for validation
- **Mock Services**: External service simulation
- **Test Environments**: Isolated testing infrastructure

// TEST: Verify comprehensive test coverage for all endpoints
// TEST: Confirm test data accurately represents real scenarios
// TEST: Validate test automation runs reliably

### 10.2 API Documentation Testing

#### 10.2.1 Documentation Validation
- **Schema Validation**: OpenAPI specification compliance
- **Example Validation**: All examples execute successfully
- **Link Validation**: All documentation links work
- **Code Sample Testing**: All code samples are functional

// TEST: Verify API documentation matches implementation
// TEST: Confirm all examples and code samples work correctly
// TEST: Validate documentation completeness and accuracy

---

## 11. Acceptance Criteria

### 11.1 Functional Acceptance
- **Complete API Coverage**: All specified endpoints implemented and functional
- **Data Model Compliance**: All responses conform to defined schemas
- **Authentication**: Secure authentication and authorization working
- **Real-Time Streaming**: WebSocket streaming operational with <50ms latency

### 11.2 Performance Acceptance
- **Response Times**: 95th percentile response times meet targets
- **Throughput**: System handles specified concurrent load
- **Rate Limiting**: Rate limits enforced correctly
- **Scalability**: System scales to handle growth requirements

### 11.3 Integration Acceptance
- **External APIs**: MQTT, webhook, and Restream integrations functional
- **Error Handling**: Comprehensive error handling and reporting
- **Documentation**: Complete and accurate API documentation
- **Testing**: Comprehensive test coverage with automated validation

// TEST: Validate all API endpoints meet functional requirements
// TEST: Confirm performance targets are achieved under load
// TEST: Verify external integrations work reliably
// TEST: Ensure comprehensive error handling covers all scenarios
// TEST: Validate API documentation accuracy and completeness
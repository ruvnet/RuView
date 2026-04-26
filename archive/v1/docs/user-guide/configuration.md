# Configuration Guide

## Overview

This guide covers comprehensive configuration options for the WiFi-DensePose system, including domain-specific settings, hardware configuration, performance tuning, and security settings.

## Table of Contents

1. [Configuration Files](#configuration-files)
2. [Environment Variables](#environment-variables)
3. [Domain-Specific Configuration](#domain-specific-configuration)
4. [Hardware Configuration](#hardware-configuration)
5. [Performance Tuning](#performance-tuning)
6. [Security Configuration](#security-configuration)
7. [Integration Settings](#integration-settings)
8. [Monitoring and Logging](#monitoring-and-logging)
9. [Advanced Configuration](#advanced-configuration)

## Configuration Files

### Primary Configuration File

The system uses environment variables and configuration files for settings management:

```bash
# Main configuration file
.env

# Domain-specific configurations
config/domains/healthcare.yaml
config/domains/retail.yaml
config/domains/security.yaml

# Hardware configurations
config/hardware/routers.yaml
config/hardware/processing.yaml
```

### Configuration Hierarchy

Configuration is loaded in the following order (later values override earlier ones):

1. Default values in [`src/config/settings.py`](../../src/config/settings.py)
2. Environment-specific configuration files
3. `.env` file
4. Environment variables
5. Command-line arguments

## Environment Variables

### Application Settings

```bash
# Basic application settings
APP_NAME="WiFi-DensePose API"
VERSION="1.0.0"
ENVIRONMENT="development"  # development, staging, production
DEBUG=false

# Server configuration
HOST="0.0.0.0"
PORT=8000
RELOAD=false
WORKERS=1
```

### Security Settings

```bash
# JWT Configuration
SECRET_KEY="your-super-secret-key-change-in-production"
JWT_ALGORITHM="HS256"
JWT_EXPIRE_HOURS=24

# CORS and Host Settings
ALLOWED_HOSTS="localhost,127.0.0.1,your-domain.com"
CORS_ORIGINS="http://localhost:3000,https://your-frontend.com"

# Rate Limiting
RATE_LIMIT_REQUESTS=100
RATE_LIMIT_AUTHENTICATED_REQUESTS=1000
RATE_LIMIT_WINDOW=3600  # seconds
```

### Database Configuration

```bash
# Database Settings
DATABASE_URL="postgresql://user:password@localhost:5432/wifi_densepose"
DATABASE_POOL_SIZE=10
DATABASE_MAX_OVERFLOW=20

# Redis Configuration
REDIS_URL="redis://localhost:6379/0"
REDIS_PASSWORD=""
REDIS_DB=0
```

### Hardware Settings

```bash
# WiFi Interface
WIFI_INTERFACE="wlan0"
CSI_BUFFER_SIZE=1000
HARDWARE_POLLING_INTERVAL=0.1

# Development/Testing
MOCK_HARDWARE=false
MOCK_POSE_DATA=false
```

### Pose Estimation Settings

```bash
# Model Configuration
POSE_MODEL_PATH="./models/densepose_model.pth"
POSE_CONFIDENCE_THRESHOLD=0.5
POSE_PROCESSING_BATCH_SIZE=32
POSE_MAX_PERSONS=10

# Streaming Settings
STREAM_FPS=30
STREAM_BUFFER_SIZE=100
WEBSOCKET_PING_INTERVAL=60
WEBSOCKET_TIMEOUT=300
```

### Storage Settings

```bash
# Storage Paths
DATA_STORAGE_PATH="./data"
MODEL_STORAGE_PATH="./models"
TEMP_STORAGE_PATH="./temp"
MAX_STORAGE_SIZE_GB=100
```

### Feature Flags

```bash
# Feature Toggles
ENABLE_AUTHENTICATION=true
ENABLE_RATE_LIMITING=true
ENABLE_WEBSOCKETS=true
ENABLE_HISTORICAL_DATA=true
ENABLE_REAL_TIME_PROCESSING=true
ENABLE_TEST_ENDPOINTS=false
```

## Domain-Specific Configuration

### Healthcare Domain

Healthcare deployments require enhanced privacy and accuracy settings:

```yaml
# config/domains/healthcare.yaml
domain: healthcare
description: "Healthcare monitoring and patient safety"

detection:
  confidence_threshold: 0.8
  max_persons: 3
  tracking_enabled: true
  privacy_mode: true

alerts:
  fall_detection:
    enabled: true
    sensitivity: 0.9
    notification_delay_seconds: 5
    emergency_contacts:
      - "nurse-station@hospital.com"
      - "+1-555-0123"
  
  inactivity_detection:
    enabled: true
    threshold_minutes: 30
    alert_levels: ["warning", "critical"]
  
  vital_signs_monitoring:
    enabled: true
    heart_rate_estimation: true
    breathing_pattern_analysis: true

privacy:
  data_retention_days: 30
  anonymization_enabled: true
  audit_logging: true
  hipaa_compliance: true

notifications:
  webhook_urls:
    - "https://hospital-system.com/api/alerts"
  mqtt_topics:
    - "hospital/room/{room_id}/alerts"
  email_alerts: true
```

### Retail Domain

Retail deployments focus on customer analytics and traffic patterns:

```yaml
# config/domains/retail.yaml
domain: retail
description: "Retail analytics and customer insights"

detection:
  confidence_threshold: 0.7
  max_persons: 15
  tracking_enabled: true
  zone_detection: true

analytics:
  traffic_counting:
    enabled: true
    entrance_zones: ["entrance", "exit"]
    dwell_time_tracking: true
  
  heat_mapping:
    enabled: true
    zone_definitions:
      - name: "entrance"
        coordinates: [[0, 0], [100, 50]]
      - name: "electronics"
        coordinates: [[100, 0], [200, 100]]
      - name: "checkout"
        coordinates: [[200, 0], [300, 50]]
  
  conversion_tracking:
    enabled: true
    interaction_threshold_seconds: 10
    purchase_correlation: true

privacy:
  data_retention_days: 90
  anonymization_enabled: true
  gdpr_compliance: true

reporting:
  daily_reports: true
  weekly_summaries: true
  real_time_dashboard: true
```

### Security Domain

Security deployments prioritize intrusion detection and perimeter monitoring:

```yaml
# config/domains/security.yaml
domain: security
description: "Security monitoring and intrusion detection"

detection:
  confidence_threshold: 0.9
  max_persons: 10
  tracking_enabled: true
  motion_sensitivity: 0.95

security:
  intrusion_detection:
    enabled: true
    restricted_zones:
      - name: "secure_area"
        coordinates: [[50, 50], [150, 150]]
        alert_immediately: true
      - name: "perimeter"
        coordinates: [[0, 0], [300, 300]]
        alert_delay_seconds: 10
  
  unauthorized_access:
    enabled: true
    authorized_persons: []  # Empty for general detection
    time_restrictions:
      - days: ["monday", "tuesday", "wednesday", "thursday", "friday"]
        hours: ["09:00", "17:00"]
  
  threat_assessment:
    enabled: true
    aggressive_behavior_detection: true
    crowd_formation_detection: true

alerts:
  immediate_notification: true
  escalation_levels:
    - level: 1
      delay_seconds: 0
      contacts: ["security@company.com"]
    - level: 2
      delay_seconds: 30
      contacts: ["security@company.com", "manager@company.com"]
    - level: 3
      delay_seconds: 60
      contacts: ["security@company.com", "manager@company.com", "emergency@company.com"]

integration:
  security_system_api: "https://security-system.com/api"
  camera_system_integration: true
  access_control_integration: true
```

## Hardware Configuration

### Router Configuration

```yaml
# config/hardware/routers.yaml
routers:
  - id: "router_001"
    type: "atheros"
    model: "TP-Link Archer C7"
    ip_address: "192.168.1.1"
    mac_address: "aa:bb:cc:dd:ee:01"
    location:
      room: "living_room"
      coordinates: [0, 0, 2.5]  # x, y, z in meters
    csi_config:
      sampling_rate: 30  # Hz
      antenna_count: 3
      subcarrier_count: 56
      data_port: 5500
    
  - id: "router_002"
    type: "atheros"
    model: "Netgear Nighthawk"
    ip_address: "192.168.1.2"
    mac_address: "aa:bb:cc:dd:ee:02"
    location:
      room: "living_room"
      coordinates: [5, 0, 2.5]
    csi_config:
      sampling_rate: 30
      antenna_count: 3
      subcarrier_count: 56
      data_port: 5501

network:
  csi_data_interface: "eth0"
  buffer_size: 1000
  timeout_seconds: 5
  retry_attempts: 3
```

### Processing Hardware Configuration

```yaml
# config/hardware/processing.yaml
processing:
  cpu:
    cores: 8
    threads_per_core: 2
    optimization: "performance"  # performance, balanced, power_save
  
  memory:
    total_gb: 16
    allocation:
      csi_processing: 4
      neural_network: 8
      api_services: 2
      system_overhead: 2
  
  gpu:
    enabled: true
    device_id: 0
    memory_gb: 8
    cuda_version: "11.8"
    optimization:
      batch_size: 32
      mixed_precision: true
      tensor_cores: true

storage:
  data_drive:
    path: "/data"
    type: "ssd"
    size_gb: 500
  
  model_drive:
    path: "/models"
    type: "ssd"
    size_gb: 100
  
  temp_drive:
    path: "/tmp"
    type: "ram"
    size_gb: 8
```

## Performance Tuning

### Processing Pipeline Optimization

```bash
# Neural Network Settings
POSE_PROCESSING_BATCH_SIZE=32  # Adjust based on GPU memory
POSE_CONFIDENCE_THRESHOLD=0.7  # Higher = fewer false positives
POSE_MAX_PERSONS=5             # Limit for performance

# Streaming Optimization
STREAM_FPS=30                  # Reduce for lower bandwidth
STREAM_BUFFER_SIZE=100         # Increase for smoother streaming
WEBSOCKET_PING_INTERVAL=60     # Connection keep-alive

# Database Optimization
DATABASE_POOL_SIZE=20          # Increase for high concurrency
DATABASE_MAX_OVERFLOW=40       # Additional connections when needed

# Caching Settings
REDIS_URL="redis://localhost:6379/0"
CACHE_TTL_SECONDS=300          # Cache expiration time
```

### Resource Allocation

```yaml
# docker-compose.override.yml
version: '3.8'
services:
  wifi-densepose-api:
    deploy:
      resources:
        limits:
          cpus: '4.0'
          memory: 8G
        reservations:
          cpus: '2.0'
          memory: 4G
    environment:
      - WORKERS=4
      - POSE_PROCESSING_BATCH_SIZE=64
  
  neural-network:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 6G
        reservations:
          cpus: '1.0'
          memory: 4G
    runtime: nvidia
    environment:
      - CUDA_VISIBLE_DEVICES=0
```

### Performance Monitoring

```bash
# Enable performance monitoring
PERFORMANCE_MONITORING=true
METRICS_ENABLED=true
HEALTH_CHECK_INTERVAL=30

# Logging for performance analysis
LOG_LEVEL="INFO"
LOG_PERFORMANCE_METRICS=true
LOG_SLOW_QUERIES=true
SLOW_QUERY_THRESHOLD_MS=1000
```

## Security Configuration

### Authentication and Authorization

```bash
# JWT Configuration
SECRET_KEY="$(openssl rand -base64 32)"  # Generate secure key
JWT_ALGORITHM="HS256"
JWT_EXPIRE_HOURS=8  # Shorter expiration for production

# API Key Configuration
API_KEY_LENGTH=32
API_KEY_EXPIRY_DAYS=90
API_KEY_ROTATION_ENABLED=true
```

### Network Security

```bash
# HTTPS Configuration
ENABLE_HTTPS=true
SSL_CERT_PATH="/etc/ssl/certs/wifi-densepose.crt"
SSL_KEY_PATH="/etc/ssl/private/wifi-densepose.key"

# Firewall Settings
ALLOWED_IPS="192.168.1.0/24,10.0.0.0/8"
BLOCKED_IPS=""
RATE_LIMIT_ENABLED=true
```

### Data Protection

```bash
# Encryption Settings
DATABASE_ENCRYPTION=true
DATA_AT_REST_ENCRYPTION=true
BACKUP_ENCRYPTION=true

# Privacy Settings
ANONYMIZATION_ENABLED=true
DATA_RETENTION_DAYS=30
AUDIT_LOGGING=true
GDPR_COMPLIANCE=true
```

## Integration Settings

### MQTT Configuration

```bash
# MQTT Broker Settings
MQTT_BROKER_HOST="localhost"
MQTT_BROKER_PORT=1883
MQTT_USERNAME="wifi_densepose"
MQTT_PASSWORD="secure_password"
MQTT_TLS_ENABLED=true

# Topic Configuration
MQTT_TOPIC_PREFIX="wifi-densepose"
MQTT_QOS_LEVEL=1
MQTT_RETAIN_MESSAGES=false
```

### Webhook Configuration

```bash
# Webhook Settings
WEBHOOK_TIMEOUT_SECONDS=30
WEBHOOK_RETRY_ATTEMPTS=3
WEBHOOK_RETRY_DELAY_SECONDS=5

# Security
WEBHOOK_SIGNATURE_ENABLED=true
WEBHOOK_SECRET_KEY="webhook_secret_key"
```

### External API Integration

```bash
# Restream Integration
RESTREAM_API_KEY="your_restream_api_key"
RESTREAM_ENABLED=false
RESTREAM_PLATFORMS="youtube,twitch"

# Third-party APIs
EXTERNAL_API_TIMEOUT=30
EXTERNAL_API_RETRY_ATTEMPTS=3
```

## Monitoring and Logging

### Logging Configuration

```bash
# Log Levels
LOG_LEVEL="INFO"  # DEBUG, INFO, WARNING, ERROR, CRITICAL
LOG_FORMAT="%(asctime)s - %(name)s - %(levelname)s - %(message)s"

# Log Files
LOG_FILE="/var/log/wifi-densepose/app.log"
LOG_MAX_SIZE=10485760  # 10MB
LOG_BACKUP_COUNT=5

# Structured Logging
LOG_JSON_FORMAT=true
LOG_CORRELATION_ID=true
```

### Metrics and Monitoring

```bash
# Prometheus Metrics
METRICS_ENABLED=true
METRICS_PORT=9090
METRICS_PATH="/metrics"

# Health Checks
HEALTH_CHECK_INTERVAL=30
HEALTH_CHECK_TIMEOUT=10
DEEP_HEALTH_CHECKS=true

# Performance Monitoring
PERFORMANCE_MONITORING=true
SLOW_QUERY_LOGGING=true
RESOURCE_MONITORING=true
```

## Advanced Configuration

### Custom Model Configuration

```yaml
# config/models/custom_model.yaml
model:
  name: "custom_densepose_v2"
  path: "./models/custom_densepose_v2.pth"
  type: "pytorch"
  
  preprocessing:
    input_size: [256, 256]
    normalization:
      mean: [0.485, 0.456, 0.406]
      std: [0.229, 0.224, 0.225]
  
  inference:
    batch_size: 32
    device: "cuda:0"
    precision: "fp16"  # fp32, fp16, int8
  
  postprocessing:
    confidence_threshold: 0.7
    nms_threshold: 0.5
    max_detections: 10
```

### Environment-Specific Overrides

```bash
# config/environments/production.env
ENVIRONMENT=production
DEBUG=false
LOG_LEVEL=WARNING
WORKERS=8
POSE_PROCESSING_BATCH_SIZE=64
ENABLE_TEST_ENDPOINTS=false
MOCK_HARDWARE=false
```

```bash
# config/environments/development.env
ENVIRONMENT=development
DEBUG=true
LOG_LEVEL=DEBUG
WORKERS=1
RELOAD=true
MOCK_HARDWARE=true
ENABLE_TEST_ENDPOINTS=true
```

### Configuration Validation

The system automatically validates configuration on startup:

```bash
# Run configuration validation
python -m src.config.validate

# Check specific configuration
python -c "
from src.config.settings import get_settings, validate_settings
settings = get_settings()
issues = validate_settings(settings)
if issues:
    print('Configuration issues:')
    for issue in issues:
        print(f'  - {issue}')
else:
    print('Configuration is valid')
"
```

### Dynamic Configuration Updates

Some settings can be updated without restarting the system:

```bash
# Update detection settings
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Content-Type: application/json" \
  -d '{
    "detection": {
      "confidence_threshold": 0.8,
      "max_persons": 3
    }
  }'

# Update alert settings
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Content-Type: application/json" \
  -d '{
    "alerts": {
      "fall_detection": {
        "sensitivity": 0.9
      }
    }
  }'
```

## Configuration Best Practices

### Security Best Practices

1. **Use Strong Secret Keys**: Generate cryptographically secure keys
2. **Restrict CORS Origins**: Don't use wildcards in production
3. **Enable Rate Limiting**: Protect against abuse
4. **Use HTTPS**: Encrypt all communications
5. **Regular Key Rotation**: Rotate API keys and JWT secrets

### Performance Best Practices

1. **Right-size Resources**: Allocate appropriate CPU/memory
2. **Use GPU Acceleration**: Enable CUDA for neural network processing
3. **Optimize Batch Sizes**: Balance throughput and latency
4. **Configure Caching**: Use Redis for frequently accessed data
5. **Monitor Resource Usage**: Set up alerts for resource exhaustion

### Operational Best Practices

1. **Environment Separation**: Use different configs for dev/staging/prod
2. **Configuration Validation**: Validate settings before deployment
3. **Backup Configurations**: Version control all configuration files
4. **Document Changes**: Maintain change logs for configuration updates
5. **Test Configuration**: Validate configuration in staging environment

---

For more specific configuration examples, see:
- [Hardware Setup Guide](../hardware/router-setup.md)
- [API Reference](api-reference.md)
- [Deployment Guide](../developer/deployment-guide.md)
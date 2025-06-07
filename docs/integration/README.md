# WiFi-DensePose System Integration Guide

This document provides a comprehensive guide to the WiFi-DensePose system integration, covering all components and their interactions.

## Overview

The WiFi-DensePose system is a fully integrated solution for WiFi-based human pose estimation using CSI data and DensePose neural networks. The system consists of multiple interconnected components that work together to provide real-time pose detection capabilities.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    WiFi-DensePose System                        │
├─────────────────────────────────────────────────────────────────┤
│  CLI Interface (src/cli.py)                                    │
│  ├── Commands: start, stop, status, config                     │
│  └── Entry Point: wifi-densepose                               │
├─────────────────────────────────────────────────────────────────┤
│  FastAPI Application (src/app.py)                              │
│  ├── REST API Endpoints                                        │
│  ├── WebSocket Connections                                     │
│  ├── Middleware Stack                                          │
│  └── Error Handling                                            │
├─────────────────────────────────────────────────────────────────┤
│  Core Processing Components                                     │
│  ├── CSI Processor (src/core/csi_processor.py)                │
│  ├── Phase Sanitizer (src/core/phase_sanitizer.py)            │
│  ├── Pose Estimator (src/core/pose_estimator.py)              │
│  └── Router Interface (src/core/router_interface.py)          │
├─────────────────────────────────────────────────────────────────┤
│  Service Layer                                                  │
│  ├── Service Orchestrator (src/services/orchestrator.py)      │
│  ├── Health Check Service (src/services/health_check.py)      │
│  └── Metrics Service (src/services/metrics.py)                │
├─────────────────────────────────────────────────────────────────┤
│  Middleware Layer                                               │
│  ├── Authentication (src/middleware/auth.py)                   │
│  ├── CORS (src/middleware/cors.py)                            │
│  ├── Rate Limiting (src/middleware/rate_limit.py)             │
│  └── Error Handler (src/middleware/error_handler.py)          │
├─────────────────────────────────────────────────────────────────┤
│  Database Layer                                                 │
│  ├── Connection Manager (src/database/connection.py)           │
│  ├── Models (src/database/models.py)                          │
│  └── Migrations (src/database/migrations/)                    │
├─────────────────────────────────────────────────────────────────┤
│  Background Tasks                                               │
│  ├── Cleanup Tasks (src/tasks/cleanup.py)                     │
│  ├── Monitoring Tasks (src/tasks/monitoring.py)               │
│  └── Backup Tasks (src/tasks/backup.py)                       │
└─────────────────────────────────────────────────────────────────┘
```

## Component Integration

### 1. Application Entry Points

#### Main Application (`src/main.py`)
- Primary entry point for the application
- Handles application lifecycle management
- Integrates with all system components

#### FastAPI Application (`src/app.py`)
- Web application setup and configuration
- API endpoint registration
- Middleware integration
- Error handling setup

#### CLI Interface (`src/cli.py`)
- Command-line interface for system management
- Integration with all system services
- Configuration management commands

### 2. Configuration Management

#### Centralized Settings (`src/config.py`)
- Environment-based configuration
- Database connection settings
- Service configuration parameters
- Security settings

#### Logger Configuration (`src/logger.py`)
- Structured logging setup
- Log level management
- Integration with monitoring systems

### 3. Core Processing Pipeline

The core processing components work together in a pipeline:

```
Router Interface → CSI Processor → Phase Sanitizer → Pose Estimator
```

#### Router Interface
- Connects to WiFi routers
- Collects CSI data
- Manages device connections

#### CSI Processor
- Processes raw CSI data
- Applies signal processing algorithms
- Prepares data for pose estimation

#### Phase Sanitizer
- Removes phase noise and artifacts
- Improves signal quality
- Enhances pose detection accuracy

#### Pose Estimator
- Applies DensePose neural networks
- Generates pose predictions
- Provides confidence scores

### 4. Service Integration

#### Service Orchestrator
- Coordinates all system services
- Manages service lifecycle
- Handles inter-service communication

#### Health Check Service
- Monitors system health
- Provides health status endpoints
- Integrates with monitoring systems

#### Metrics Service
- Collects system metrics
- Provides Prometheus-compatible metrics
- Monitors performance indicators

### 5. Database Integration

#### Connection Management
- Async database connections
- Connection pooling
- Transaction management

#### Data Models
- SQLAlchemy ORM models
- Database schema definitions
- Relationship management

#### Migrations
- Database schema versioning
- Automated migration system
- Data integrity maintenance

### 6. Background Task Integration

#### Cleanup Tasks
- Periodic data cleanup
- Resource management
- System maintenance

#### Monitoring Tasks
- System monitoring
- Performance tracking
- Alert generation

#### Backup Tasks
- Data backup operations
- System state preservation
- Disaster recovery

## Integration Patterns

### 1. Dependency Injection

The system uses dependency injection for component integration:

```python
# Example: Service integration
from src.services.orchestrator import get_service_orchestrator
from src.database.connection import get_database_manager

async def initialize_system():
    settings = get_settings()
    db_manager = get_database_manager(settings)
    orchestrator = get_service_orchestrator(settings)
    
    await db_manager.initialize()
    await orchestrator.initialize()
```

### 2. Event-Driven Architecture

Components communicate through events:

```python
# Example: Event handling
from src.core.events import EventBus

event_bus = EventBus()

# Publisher
await event_bus.publish("csi_data_received", data)

# Subscriber
@event_bus.subscribe("csi_data_received")
async def process_csi_data(data):
    # Process the data
    pass
```

### 3. Middleware Pipeline

Request processing through middleware:

```python
# Middleware stack
app.add_middleware(ErrorHandlerMiddleware)
app.add_middleware(AuthenticationMiddleware)
app.add_middleware(RateLimitMiddleware)
app.add_middleware(CORSMiddleware)
```

### 4. Resource Management

Proper resource lifecycle management:

```python
# Context managers for resources
async with db_manager.get_async_session() as session:
    # Database operations
    pass

async with router_interface.get_connection() as connection:
    # Router operations
    pass
```

## Configuration Integration

### Environment Variables

```bash
# Core settings
WIFI_DENSEPOSE_ENVIRONMENT=production
WIFI_DENSEPOSE_DEBUG=false
WIFI_DENSEPOSE_LOG_LEVEL=INFO

# Database settings
WIFI_DENSEPOSE_DATABASE_URL=postgresql+asyncpg://user:pass@localhost/db
WIFI_DENSEPOSE_DATABASE_POOL_SIZE=20

# Redis settings
WIFI_DENSEPOSE_REDIS_URL=redis://localhost:6379/0
WIFI_DENSEPOSE_REDIS_ENABLED=true

# Security settings
WIFI_DENSEPOSE_SECRET_KEY=your-secret-key
WIFI_DENSEPOSE_JWT_ALGORITHM=HS256
```

### Configuration Files

```yaml
# config/production.yaml
database:
  pool_size: 20
  max_overflow: 30
  pool_timeout: 30

services:
  health_check:
    interval: 30
    timeout: 10
  
  metrics:
    enabled: true
    port: 9090

processing:
  csi:
    sampling_rate: 1000
    buffer_size: 1024
  
  pose:
    model_path: "models/densepose.pth"
    confidence_threshold: 0.7
```

## API Integration

### REST Endpoints

```python
# Device management
GET    /api/v1/devices
POST   /api/v1/devices
GET    /api/v1/devices/{device_id}
PUT    /api/v1/devices/{device_id}
DELETE /api/v1/devices/{device_id}

# Session management
GET    /api/v1/sessions
POST   /api/v1/sessions
GET    /api/v1/sessions/{session_id}
PATCH  /api/v1/sessions/{session_id}
DELETE /api/v1/sessions/{session_id}

# Data endpoints
POST   /api/v1/csi-data
GET    /api/v1/sessions/{session_id}/pose-detections
GET    /api/v1/sessions/{session_id}/csi-data
```

### WebSocket Integration

```python
# Real-time data streaming
WS /ws/csi-data/{session_id}
WS /ws/pose-detections/{session_id}
WS /ws/system-status
```

## Monitoring Integration

### Health Checks

```python
# Health check endpoints
GET /health              # Basic health check
GET /health?detailed=true # Detailed health information
GET /metrics             # Prometheus metrics
```

### Metrics Collection

```python
# System metrics
- http_requests_total
- http_request_duration_seconds
- database_connections_active
- csi_data_processed_total
- pose_detections_total
- system_memory_usage
- system_cpu_usage
```

## Testing Integration

### Unit Tests

```bash
# Run unit tests
pytest tests/unit/ -v

# Run with coverage
pytest tests/unit/ --cov=src --cov-report=html
```

### Integration Tests

```bash
# Run integration tests
pytest tests/integration/ -v

# Run specific integration test
pytest tests/integration/test_full_system_integration.py -v
```

### End-to-End Tests

```bash
# Run E2E tests
pytest tests/e2e/ -v

# Run with real hardware
pytest tests/e2e/ --hardware=true -v
```

## Deployment Integration

### Docker Integration

```dockerfile
# Multi-stage build
FROM python:3.11-slim as builder
# Build stage

FROM python:3.11-slim as runtime
# Runtime stage
```

### Kubernetes Integration

```yaml
# Deployment configuration
apiVersion: apps/v1
kind: Deployment
metadata:
  name: wifi-densepose
spec:
  replicas: 3
  selector:
    matchLabels:
      app: wifi-densepose
  template:
    metadata:
      labels:
        app: wifi-densepose
    spec:
      containers:
      - name: wifi-densepose
        image: wifi-densepose:latest
        ports:
        - containerPort: 8000
```

## Security Integration

### Authentication

```python
# JWT-based authentication
from src.middleware.auth import AuthenticationMiddleware

app.add_middleware(AuthenticationMiddleware)
```

### Authorization

```python
# Role-based access control
from src.middleware.auth import require_role

@require_role("admin")
async def admin_endpoint():
    pass
```

### Rate Limiting

```python
# Rate limiting middleware
from src.middleware.rate_limit import RateLimitMiddleware

app.add_middleware(RateLimitMiddleware, 
                  requests_per_minute=100)
```

## Performance Integration

### Caching

```python
# Redis caching
from src.cache import get_cache_manager

cache = get_cache_manager()
await cache.set("key", value, ttl=300)
value = await cache.get("key")
```

### Connection Pooling

```python
# Database connection pooling
from src.database.connection import get_database_manager

db_manager = get_database_manager(settings)
# Automatic connection pooling
```

### Async Processing

```python
# Async task processing
from src.tasks import get_task_manager

task_manager = get_task_manager()
await task_manager.submit_task("process_csi_data", data)
```

## Troubleshooting Integration

### Common Issues

1. **Database Connection Issues**
   ```bash
   # Check database connectivity
   wifi-densepose config validate
   ```

2. **Service Startup Issues**
   ```bash
   # Check service status
   wifi-densepose status
   
   # View logs
   wifi-densepose logs --tail=100
   ```

3. **Performance Issues**
   ```bash
   # Check system metrics
   curl http://localhost:8000/metrics
   
   # Check health status
   curl http://localhost:8000/health?detailed=true
   ```

### Debug Mode

```bash
# Enable debug mode
export WIFI_DENSEPOSE_DEBUG=true
export WIFI_DENSEPOSE_LOG_LEVEL=DEBUG

# Start with debug logging
wifi-densepose start --debug
```

## Integration Validation

### Automated Validation

```bash
# Run integration validation
./scripts/validate-integration.sh

# Run specific validation
./scripts/validate-integration.sh --component=database
```

### Manual Validation

```bash
# Check package installation
pip install -e .

# Verify imports
python -c "import src; print(src.__version__)"

# Test CLI
wifi-densepose --help

# Test API
curl http://localhost:8000/health
```

## Best Practices

### 1. Error Handling
- Use structured error responses
- Implement proper exception handling
- Log errors with context

### 2. Resource Management
- Use context managers for resources
- Implement proper cleanup procedures
- Monitor resource usage

### 3. Configuration Management
- Use environment-specific configurations
- Validate configuration on startup
- Provide sensible defaults

### 4. Testing
- Write comprehensive integration tests
- Use mocking for external dependencies
- Test error conditions

### 5. Monitoring
- Implement health checks
- Collect relevant metrics
- Set up alerting

### 6. Security
- Validate all inputs
- Use secure authentication
- Implement rate limiting

### 7. Performance
- Use async/await patterns
- Implement caching where appropriate
- Monitor performance metrics

## Next Steps

1. **Run Integration Validation**
   ```bash
   ./scripts/validate-integration.sh
   ```

2. **Start the System**
   ```bash
   wifi-densepose start
   ```

3. **Monitor System Health**
   ```bash
   wifi-densepose status
   curl http://localhost:8000/health
   ```

4. **Run Tests**
   ```bash
   pytest tests/ -v
   ```

5. **Deploy to Production**
   ```bash
   docker build -t wifi-densepose .
   docker run -p 8000:8000 wifi-densepose
   ```

For more detailed information, refer to the specific component documentation in the `docs/` directory.
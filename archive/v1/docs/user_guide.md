# WiFi-DensePose User Guide

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Configuration](#configuration)
5. [Basic Usage](#basic-usage)
6. [Advanced Features](#advanced-features)
7. [Examples](#examples)
8. [Best Practices](#best-practices)

## Overview

WiFi-DensePose is a revolutionary privacy-preserving human pose estimation system that leverages Channel State Information (CSI) data from standard WiFi infrastructure. Unlike traditional camera-based systems, WiFi-DensePose provides real-time pose detection while maintaining complete privacy.

### Key Features

- **Privacy-First Design**: No cameras or visual data required
- **Real-Time Processing**: Sub-50ms latency with 30 FPS pose estimation
- **Multi-Person Tracking**: Simultaneous tracking of up to 10 individuals
- **Domain-Specific Optimization**: Tailored for healthcare, fitness, retail, and security
- **Enterprise-Ready**: Production-grade API with authentication and monitoring
- **Hardware Agnostic**: Works with standard WiFi routers and access points

### System Architecture

```
WiFi Routers → CSI Data → Signal Processing → Neural Network → Pose Estimation
     ↓              ↓            ↓              ↓              ↓
   Hardware    Data Collection  Phase Cleaning  DensePose    Person Tracking
  Interface    & Buffering      & Filtering     Model        & Analytics
```

## Installation

### Prerequisites

- **Python**: 3.9 or higher
- **Operating System**: Linux (Ubuntu 18.04+), macOS (10.15+), Windows 10+
- **Memory**: Minimum 4GB RAM, Recommended 8GB+
- **Storage**: 2GB free space for models and data
- **Network**: WiFi interface with CSI capability

### Method 1: Install from PyPI (Recommended)

```bash
# Install the latest stable version
pip install wifi-densepose

# Install with optional dependencies
pip install wifi-densepose[gpu,monitoring,deployment]

# Verify installation
wifi-densepose --version
```

### Method 2: Install from Source

```bash
# Clone the repository
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose

# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt

# Install in development mode
pip install -e .
```

### Method 3: Docker Installation

```bash
# Pull the latest image
docker pull ruvnet/wifi-densepose:latest

# Run with default configuration
docker run -p 8000:8000 ruvnet/wifi-densepose:latest

# Run with custom configuration
docker run -p 8000:8000 -v $(pwd)/config:/app/config ruvnet/wifi-densepose:latest
```

### Verify Installation

```bash
# Check system information
python -c "import wifi_densepose; wifi_densepose.print_system_info()"

# Test API server
wifi-densepose start --test-mode

# Check health endpoint
curl http://localhost:8000/api/v1/health
```

## Quick Start

### 1. Basic Setup

```bash
# Create configuration file
wifi-densepose init

# Edit configuration (optional)
nano .env

# Start the system
wifi-densepose start
```

### 2. Python API Usage

```python
from wifi_densepose import WiFiDensePose

# Initialize with default configuration
system = WiFiDensePose()

# Start pose estimation
system.start()

# Get latest pose data
poses = system.get_latest_poses()
print(f"Detected {len(poses)} persons")

# Stop the system
system.stop()
```

### 3. REST API Usage

```bash
# Start the API server
wifi-densepose start --api

# Get latest poses
curl http://localhost:8000/api/v1/pose/latest

# Get system status
curl http://localhost:8000/api/v1/system/status
```

### 4. WebSocket Streaming

```python
import asyncio
import websockets
import json

async def stream_poses():
    uri = "ws://localhost:8000/ws/pose/stream"
    async with websockets.connect(uri) as websocket:
        while True:
            data = await websocket.recv()
            poses = json.loads(data)
            print(f"Received: {len(poses['persons'])} persons")

asyncio.run(stream_poses())
```

## Configuration

### Environment Variables

Create a `.env` file in your project directory:

```bash
# Application Settings
APP_NAME=WiFi-DensePose API
VERSION=1.0.0
ENVIRONMENT=production
DEBUG=false

# Server Settings
HOST=0.0.0.0
PORT=8000
WORKERS=4

# Security Settings
SECRET_KEY=your-secure-secret-key-here
JWT_ALGORITHM=HS256
JWT_EXPIRE_HOURS=24

# Hardware Settings
WIFI_INTERFACE=wlan0
CSI_BUFFER_SIZE=1000
HARDWARE_POLLING_INTERVAL=0.1

# Pose Estimation Settings
POSE_CONFIDENCE_THRESHOLD=0.7
POSE_PROCESSING_BATCH_SIZE=32
POSE_MAX_PERSONS=10

# Feature Flags
ENABLE_AUTHENTICATION=true
ENABLE_RATE_LIMITING=true
ENABLE_WEBSOCKETS=true
ENABLE_REAL_TIME_PROCESSING=true
```

### Domain-Specific Configuration

#### Healthcare Configuration

```python
from wifi_densepose.config import Settings

config = Settings(
    domain="healthcare",
    detection={
        "confidence_threshold": 0.8,
        "max_persons": 5,
        "enable_tracking": True
    },
    analytics={
        "enable_fall_detection": True,
        "enable_activity_recognition": True,
        "alert_thresholds": {
            "fall_confidence": 0.9,
            "inactivity_timeout": 300
        }
    },
    privacy={
        "data_retention_days": 30,
        "anonymize_data": True,
        "enable_encryption": True
    }
)
```

#### Fitness Configuration

```python
config = Settings(
    domain="fitness",
    detection={
        "confidence_threshold": 0.6,
        "max_persons": 20,
        "enable_tracking": True
    },
    analytics={
        "enable_activity_recognition": True,
        "enable_form_analysis": True,
        "metrics": ["rep_count", "form_score", "intensity"]
    }
)
```

#### Retail Configuration

```python
config = Settings(
    domain="retail",
    detection={
        "confidence_threshold": 0.7,
        "max_persons": 50,
        "enable_tracking": True
    },
    analytics={
        "enable_traffic_analytics": True,
        "enable_zone_tracking": True,
        "heatmap_generation": True
    }
)
```

## Basic Usage

### Starting the System

#### Command Line Interface

```bash
# Start with default configuration
wifi-densepose start

# Start with custom configuration
wifi-densepose start --config /path/to/config.yaml

# Start in development mode
wifi-densepose start --dev --reload

# Start with specific domain
wifi-densepose start --domain healthcare

# Start API server only
wifi-densepose start --api-only
```

#### Python API

```python
from wifi_densepose import WiFiDensePose
from wifi_densepose.config import Settings

# Initialize with custom settings
settings = Settings(
    pose_confidence_threshold=0.8,
    max_persons=5,
    enable_gpu=True
)

system = WiFiDensePose(settings=settings)

# Start the system
system.start()

# Check if system is running
if system.is_running():
    print("System is active")

# Get system status
status = system.get_status()
print(f"Status: {status}")
```

### Getting Pose Data

#### Latest Poses

```python
# Get the most recent pose data
poses = system.get_latest_poses()

for person in poses:
    print(f"Person {person.id}:")
    print(f"  Confidence: {person.confidence}")
    print(f"  Keypoints: {len(person.keypoints)}")
    print(f"  Bounding box: {person.bbox}")
```

#### Historical Data

```python
from datetime import datetime, timedelta

# Get poses from the last hour
end_time = datetime.now()
start_time = end_time - timedelta(hours=1)

history = system.get_pose_history(
    start_time=start_time,
    end_time=end_time,
    min_confidence=0.7
)

print(f"Found {len(history)} pose records")
```

#### Real-Time Streaming

```python
def pose_callback(poses):
    """Callback function for real-time pose updates"""
    print(f"Received {len(poses)} poses at {datetime.now()}")
    
    for person in poses:
        if person.confidence > 0.8:
            print(f"High-confidence detection: Person {person.id}")

# Subscribe to real-time updates
system.subscribe_to_poses(callback=pose_callback)

# Unsubscribe when done
system.unsubscribe_from_poses()
```

### System Control

#### Starting and Stopping

```python
# Start the pose estimation system
system.start()

# Pause processing (keeps connections alive)
system.pause()

# Resume processing
system.resume()

# Stop the system
system.stop()

# Restart with new configuration
system.restart(new_settings)
```

#### Configuration Updates

```python
# Update configuration at runtime
new_config = {
    "detection": {
        "confidence_threshold": 0.8,
        "max_persons": 8
    }
}

system.update_config(new_config)

# Get current configuration
current_config = system.get_config()
print(current_config)
```

## Advanced Features

### Multi-Environment Support

```python
# Configure multiple environments
environments = {
    "room_001": {
        "calibration_file": "/path/to/room_001_cal.json",
        "router_ips": ["192.168.1.1", "192.168.1.2"]
    },
    "room_002": {
        "calibration_file": "/path/to/room_002_cal.json",
        "router_ips": ["192.168.2.1", "192.168.2.2"]
    }
}

# Switch between environments
system.set_environment("room_001")
poses_room1 = system.get_latest_poses()

system.set_environment("room_002")
poses_room2 = system.get_latest_poses()
```

### Custom Analytics

```python
from wifi_densepose.analytics import AnalyticsEngine

# Initialize analytics engine
analytics = AnalyticsEngine(system)

# Enable fall detection
analytics.enable_fall_detection(
    sensitivity=0.9,
    callback=lambda event: print(f"Fall detected: {event}")
)

# Enable activity recognition
analytics.enable_activity_recognition(
    activities=["sitting", "standing", "walking", "running"],
    callback=lambda activity: print(f"Activity: {activity}")
)

# Custom analytics function
def custom_analytics(poses):
    """Custom analytics function"""
    person_count = len(poses)
    avg_confidence = sum(p.confidence for p in poses) / person_count if person_count > 0 else 0
    
    return {
        "person_count": person_count,
        "average_confidence": avg_confidence,
        "timestamp": datetime.now().isoformat()
    }

analytics.add_custom_function(custom_analytics)
```

### Hardware Integration

```python
from wifi_densepose.hardware import RouterManager

# Configure router connections
router_manager = RouterManager()

# Add routers
router_manager.add_router(
    ip="192.168.1.1",
    username="admin",
    password="password",
    router_type="asus_ac68u"
)

# Check router status
status = router_manager.get_router_status("192.168.1.1")
print(f"Router status: {status}")

# Configure CSI extraction
router_manager.configure_csi_extraction(
    router_ip="192.168.1.1",
    extraction_rate=30,
    target_ip="192.168.1.100",
    target_port=5500
)
```

## Examples

### Example 1: Healthcare Monitoring

```python
from wifi_densepose import WiFiDensePose
from wifi_densepose.analytics import FallDetector
import logging

# Configure for healthcare
system = WiFiDensePose(domain="healthcare")

# Set up fall detection
fall_detector = FallDetector(
    sensitivity=0.95,
    alert_callback=lambda event: send_alert(event)
)

def send_alert(fall_event):
    """Send alert to healthcare staff"""
    logging.critical(f"FALL DETECTED: {fall_event}")
    # Send notification to staff
    # notify_healthcare_staff(fall_event)

# Start monitoring
system.start()
system.add_analytics_module(fall_detector)

print("Healthcare monitoring active...")
```

### Example 2: Fitness Tracking

```python
from wifi_densepose import WiFiDensePose
from wifi_densepose.analytics import ActivityTracker

# Configure for fitness
system = WiFiDensePose(domain="fitness")

# Set up activity tracking
activity_tracker = ActivityTracker(
    activities=["squats", "pushups", "jumping_jacks"],
    rep_counting=True
)

def workout_callback(activity_data):
    """Handle workout data"""
    print(f"Exercise: {activity_data['exercise']}")
    print(f"Reps: {activity_data['rep_count']}")
    print(f"Form score: {activity_data['form_score']}")

activity_tracker.set_callback(workout_callback)

# Start fitness tracking
system.start()
system.add_analytics_module(activity_tracker)

print("Fitness tracking active...")
```

### Example 3: Retail Analytics

```python
from wifi_densepose import WiFiDensePose
from wifi_densepose.analytics import TrafficAnalyzer

# Configure for retail
system = WiFiDensePose(domain="retail")

# Set up traffic analysis
traffic_analyzer = TrafficAnalyzer(
    zones={
        "entrance": {"x": 0, "y": 0, "width": 100, "height": 50},
        "checkout": {"x": 200, "y": 150, "width": 100, "height": 50},
        "electronics": {"x": 50, "y": 100, "width": 150, "height": 100}
    }
)

def traffic_callback(traffic_data):
    """Handle traffic analytics"""
    print(f"Zone occupancy: {traffic_data['zone_occupancy']}")
    print(f"Traffic flow: {traffic_data['flow_patterns']}")
    print(f"Dwell times: {traffic_data['dwell_times']}")

traffic_analyzer.set_callback(traffic_callback)

# Start retail analytics
system.start()
system.add_analytics_module(traffic_analyzer)

print("Retail analytics active...")
```

### Example 4: Security Monitoring

```python
from wifi_densepose import WiFiDensePose
from wifi_densepose.analytics import IntrusionDetector

# Configure for security
system = WiFiDensePose(domain="security")

# Set up intrusion detection
intrusion_detector = IntrusionDetector(
    restricted_zones=[
        {"x": 100, "y": 100, "width": 50, "height": 50, "name": "server_room"},
        {"x": 200, "y": 50, "width": 75, "height": 75, "name": "executive_office"}
    ],
    alert_threshold=0.9
)

def security_alert(intrusion_event):
    """Handle security alerts"""
    logging.warning(f"INTRUSION DETECTED: {intrusion_event}")
    # Trigger security response
    # activate_security_protocol(intrusion_event)

intrusion_detector.set_alert_callback(security_alert)

# Start security monitoring
system.start()
system.add_analytics_module(intrusion_detector)

print("Security monitoring active...")
```

## Best Practices

### Performance Optimization

1. **Hardware Configuration**
   ```python
   # Enable GPU acceleration when available
   settings = Settings(
       enable_gpu=True,
       batch_size=64,
       mixed_precision=True
   )
   ```

2. **Memory Management**
   ```python
   # Configure appropriate buffer sizes
   settings = Settings(
       csi_buffer_size=1000,
       pose_history_limit=10000,
       cleanup_interval=3600  # 1 hour
   )
   ```

3. **Network Optimization**
   ```python
   # Optimize network settings
   settings = Settings(
       hardware_polling_interval=0.05,  # 50ms
       network_timeout=5.0,
       max_concurrent_connections=100
   )
   ```

### Security Best Practices

1. **Authentication**
   ```python
   # Enable authentication in production
   settings = Settings(
       enable_authentication=True,
       jwt_secret_key="your-secure-secret-key",
       jwt_expire_hours=24
   )
   ```

2. **Rate Limiting**
   ```python
   # Configure rate limiting
   settings = Settings(
       enable_rate_limiting=True,
       rate_limit_requests=100,
       rate_limit_window=60  # per minute
   )
   ```

3. **Data Privacy**
   ```python
   # Enable privacy features
   settings = Settings(
       anonymize_data=True,
       data_retention_days=30,
       enable_encryption=True
   )
   ```

### Monitoring and Logging

1. **Structured Logging**
   ```python
   import logging
   from wifi_densepose.logger import setup_logging
   
   # Configure structured logging
   setup_logging(
       level=logging.INFO,
       format="json",
       output_file="/var/log/wifi-densepose.log"
   )
   ```

2. **Metrics Collection**
   ```python
   from wifi_densepose.monitoring import MetricsCollector
   
   # Enable metrics collection
   metrics = MetricsCollector()
   metrics.enable_prometheus_export(port=9090)
   ```

3. **Health Monitoring**
   ```python
   # Set up health checks
   system.enable_health_monitoring(
       check_interval=30,  # seconds
       alert_on_failure=True
   )
   ```

### Error Handling

1. **Graceful Degradation**
   ```python
   try:
       system.start()
   except HardwareNotAvailableError:
       # Fall back to mock mode
       system.start(mock_mode=True)
       logging.warning("Running in mock mode - no hardware detected")
   ```

2. **Retry Logic**
   ```python
   from wifi_densepose.utils import retry_on_failure
   
   @retry_on_failure(max_attempts=3, delay=5.0)
   def connect_to_router():
       return router_manager.connect("192.168.1.1")
   ```

3. **Circuit Breaker Pattern**
   ```python
   from wifi_densepose.resilience import CircuitBreaker
   
   # Protect against failing services
   circuit_breaker = CircuitBreaker(
       failure_threshold=5,
       recovery_timeout=60
   )
   
   @circuit_breaker
   def process_csi_data(data):
       return csi_processor.process(data)
   ```

---

For more detailed information, see:
- [API Reference Guide](api_reference.md)
- [Deployment Guide](deployment.md)
- [Troubleshooting Guide](troubleshooting.md)
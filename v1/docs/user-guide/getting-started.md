# Getting Started with WiFi-DensePose

## Overview

WiFi-DensePose is a revolutionary privacy-preserving human pose estimation system that transforms commodity WiFi infrastructure into a powerful human sensing platform. This guide will help you install, configure, and start using the system.

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Basic Configuration](#basic-configuration)
5. [First Pose Detection](#first-pose-detection)
6. [Troubleshooting](#troubleshooting)
7. [Next Steps](#next-steps)

## System Requirements

### Hardware Requirements

#### WiFi Router Requirements
- **Compatible Hardware**: Atheros-based routers (TP-Link Archer series, Netgear Nighthawk), Intel 5300 NIC-based systems, or ASUS RT-AC68U series
- **Antenna Configuration**: Minimum 3×3 MIMO antenna configuration
- **Frequency Bands**: 2.4GHz and 5GHz support
- **Firmware**: OpenWRT firmware compatibility with CSI extraction patches

#### Processing Hardware
- **CPU**: Multi-core processor (4+ cores recommended)
- **RAM**: 8GB minimum, 16GB recommended
- **Storage**: 50GB available space
- **Network**: Gigabit Ethernet for CSI data streams
- **GPU** (Optional): NVIDIA GPU with CUDA capability and 4GB+ memory for real-time processing

### Software Requirements

#### Operating System
- **Primary**: Linux (Ubuntu 20.04+, CentOS 8+)
- **Secondary**: Windows 10/11 with WSL2
- **Container**: Docker support for deployment

#### Runtime Dependencies
- Python 3.8+
- PyTorch (GPU-accelerated recommended)
- OpenCV
- FFmpeg
- FastAPI

## Installation

### Method 1: Docker Installation (Recommended)

#### Prerequisites
```bash
# Install Docker and Docker Compose
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER

# Install Docker Compose
sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose
```

#### Download and Setup
```bash
# Clone the repository
git clone https://github.com/your-org/wifi-densepose.git
cd wifi-densepose

# Copy environment configuration
cp .env.example .env

# Edit configuration (see Configuration section)
nano .env

# Start the system
docker-compose up -d
```

### Method 2: Native Installation

#### Install System Dependencies
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y python3.9 python3.9-pip python3.9-venv
sudo apt install -y build-essential cmake
sudo apt install -y libopencv-dev ffmpeg

# CentOS/RHEL
sudo yum update
sudo yum install -y python39 python39-pip
sudo yum groupinstall -y "Development Tools"
sudo yum install -y opencv-devel ffmpeg
```

#### Install Python Dependencies
```bash
# Create virtual environment
python3.9 -m venv venv
source venv/bin/activate

# Install requirements
pip install -r requirements.txt

# Install PyTorch with CUDA support (if GPU available)
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu118
```

#### Install WiFi-DensePose
```bash
# Install in development mode
pip install -e .

# Or install from PyPI (when available)
pip install wifi-densepose
```

## Quick Start

### 1. Environment Configuration

Create and configure your environment file:

```bash
# Copy the example configuration
cp .env.example .env
```

Edit the `.env` file with your settings:

```bash
# Application settings
APP_NAME="WiFi-DensePose API"
VERSION="1.0.0"
ENVIRONMENT="development"
DEBUG=true

# Server settings
HOST="0.0.0.0"
PORT=8000

# Security settings (CHANGE IN PRODUCTION!)
SECRET_KEY="your-secret-key-here"
JWT_EXPIRE_HOURS=24

# Hardware settings
WIFI_INTERFACE="wlan0"
CSI_BUFFER_SIZE=1000
MOCK_HARDWARE=true  # Set to false when using real hardware

# Pose estimation settings
POSE_CONFIDENCE_THRESHOLD=0.5
POSE_MAX_PERSONS=5

# Storage settings
DATA_STORAGE_PATH="./data"
MODEL_STORAGE_PATH="./models"
```

### 2. Start the System

#### Using Docker
```bash
# Start all services
docker-compose up -d

# Check service status
docker-compose ps

# View logs
docker-compose logs -f
```

#### Using Native Installation
```bash
# Activate virtual environment
source venv/bin/activate

# Start the API server
python -m src.api.main

# Or use uvicorn directly
uvicorn src.api.main:app --host 0.0.0.0 --port 8000 --reload
```

### 3. Verify Installation

Check that the system is running:

```bash
# Check API health
curl http://localhost:8000/health

# Expected response:
# {"status": "healthy", "timestamp": "2025-01-07T10:00:00Z"}
```

Access the web interface:
- **API Documentation**: http://localhost:8000/docs
- **Alternative Docs**: http://localhost:8000/redoc
- **Health Check**: http://localhost:8000/health

## Basic Configuration

### Domain Configuration

WiFi-DensePose supports different domain-specific configurations:

#### Healthcare Domain
```bash
# Set healthcare-specific settings
export DOMAIN="healthcare"
export POSE_CONFIDENCE_THRESHOLD=0.8
export ENABLE_FALL_DETECTION=true
export ALERT_SENSITIVITY=0.9
```

#### Retail Domain
```bash
# Set retail-specific settings
export DOMAIN="retail"
export POSE_CONFIDENCE_THRESHOLD=0.7
export ENABLE_TRAFFIC_ANALYTICS=true
export ZONE_TRACKING=true
```

#### Security Domain
```bash
# Set security-specific settings
export DOMAIN="security"
export POSE_CONFIDENCE_THRESHOLD=0.9
export ENABLE_INTRUSION_DETECTION=true
export ALERT_IMMEDIATE=true
```

### Router Configuration

#### Configure WiFi Routers for CSI Extraction

1. **Flash OpenWRT Firmware**:
   ```bash
   # Download OpenWRT firmware for your router model
   wget https://downloads.openwrt.org/releases/22.03.0/targets/...
   
   # Flash firmware (router-specific process)
   # Follow your router's flashing instructions
   ```

2. **Install CSI Extraction Patches**:
   ```bash
   # SSH into router
   ssh root@192.168.1.1
   
   # Install CSI tools
   opkg update
   opkg install csi-tools
   
   # Configure CSI extraction
   echo "csi_enable=1" >> /etc/config/wireless
   echo "csi_rate=30" >> /etc/config/wireless
   ```

3. **Configure Network Settings**:
   ```bash
   # Set router to monitor mode
   iwconfig wlan0 mode monitor
   
   # Start CSI data streaming
   csi_tool -i wlan0 -d 192.168.1.100 -p 5500
   ```

### Database Configuration

#### SQLite (Development)
```bash
# Default SQLite database (no additional configuration needed)
DATABASE_URL="sqlite:///./data/wifi_densepose.db"
```

#### PostgreSQL (Production)
```bash
# Install PostgreSQL with TimescaleDB extension
sudo apt install postgresql-14 postgresql-14-timescaledb

# Configure database
DATABASE_URL="postgresql://user:password@localhost:5432/wifi_densepose"
DATABASE_POOL_SIZE=10
DATABASE_MAX_OVERFLOW=20
```

#### Redis (Caching)
```bash
# Install Redis
sudo apt install redis-server

# Configure Redis
REDIS_URL="redis://localhost:6379/0"
REDIS_PASSWORD=""  # Set password for production
```

## First Pose Detection

### 1. Start the System

```bash
# Using Docker
docker-compose up -d

# Using native installation
python -m src.api.main
```

### 2. Initialize Hardware

```bash
# Check system status
curl http://localhost:8000/api/v1/system/status

# Start pose estimation system
curl -X POST http://localhost:8000/api/v1/system/start \
  -H "Content-Type: application/json" \
  -d '{
    "configuration": {
      "domain": "general",
      "environment_id": "room_001",
      "calibration_required": true
    }
  }'
```

### 3. Get Pose Data

#### REST API
```bash
# Get latest pose data
curl http://localhost:8000/api/v1/pose/latest

# Get historical data
curl "http://localhost:8000/api/v1/pose/history?limit=10"
```

#### WebSocket Streaming
```javascript
// Connect to WebSocket
const ws = new WebSocket('ws://localhost:8000/ws/pose');

// Subscribe to pose updates
ws.onopen = function() {
  ws.send(JSON.stringify({
    type: 'subscribe',
    channel: 'pose_updates',
    filters: {
      min_confidence: 0.7
    }
  }));
};

// Handle pose data
ws.onmessage = function(event) {
  const data = JSON.parse(event.data);
  console.log('Pose data:', data);
};
```

### 4. View Results

Access the web dashboard:
- **Main Dashboard**: http://localhost:8000/dashboard
- **Real-time View**: http://localhost:8000/dashboard/live
- **Analytics**: http://localhost:8000/dashboard/analytics

## Troubleshooting

### Common Issues

#### 1. System Won't Start
```bash
# Check logs
docker-compose logs

# Common solutions:
# - Verify port 8000 is available
# - Check environment variables
# - Ensure sufficient disk space
```

#### 2. No Pose Data
```bash
# Check hardware status
curl http://localhost:8000/api/v1/system/status

# Verify router connectivity
ping 192.168.1.1

# Check CSI data reception
netstat -an | grep 5500
```

#### 3. Poor Detection Accuracy
```bash
# Adjust confidence threshold
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Content-Type: application/json" \
  -d '{"detection": {"confidence_threshold": 0.6}}'

# Recalibrate environment
curl -X POST http://localhost:8000/api/v1/system/calibrate
```

#### 4. High CPU/Memory Usage
```bash
# Check resource usage
docker stats

# Optimize settings
export POSE_PROCESSING_BATCH_SIZE=16
export STREAM_FPS=15
```

### Getting Help

#### Log Analysis
```bash
# View application logs
docker-compose logs wifi-densepose-api

# View system logs
journalctl -u wifi-densepose

# Enable debug logging
export LOG_LEVEL="DEBUG"
```

#### Health Checks
```bash
# Comprehensive system check
curl http://localhost:8000/api/v1/system/status

# Component-specific checks
curl http://localhost:8000/api/v1/hardware/status
curl http://localhost:8000/api/v1/processing/status
```

#### Support Resources
- **Documentation**: [docs/](../README.md)
- **API Reference**: [api-reference.md](api-reference.md)
- **Troubleshooting Guide**: [troubleshooting.md](troubleshooting.md)
- **GitHub Issues**: https://github.com/your-org/wifi-densepose/issues

## Next Steps

### 1. Configure for Your Domain
- Review [configuration.md](configuration.md) for domain-specific settings
- Set up alerts and notifications
- Configure external integrations

### 2. Integrate with Your Applications
- Review [API Reference](api-reference.md)
- Set up webhooks for events
- Configure MQTT for IoT integration

### 3. Deploy to Production
- Review [deployment guide](../developer/deployment-guide.md)
- Set up monitoring and alerting
- Configure backup and recovery

### 4. Optimize Performance
- Tune processing parameters
- Set up GPU acceleration
- Configure load balancing

## Security Considerations

### Development Environment
- Use strong secret keys
- Enable authentication
- Restrict network access

### Production Environment
- Use HTTPS/TLS encryption
- Configure firewall rules
- Set up audit logging
- Regular security updates

## Performance Tips

### Hardware Optimization
- Use SSD storage for better I/O performance
- Ensure adequate cooling for continuous operation
- Use dedicated network interface for CSI data

### Software Optimization
- Enable GPU acceleration when available
- Tune batch sizes for your hardware
- Configure appropriate worker processes
- Use Redis for caching frequently accessed data

---

**Congratulations!** You now have WiFi-DensePose up and running. Continue with the [Configuration Guide](configuration.md) to customize the system for your specific needs.
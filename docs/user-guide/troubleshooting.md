# Troubleshooting Guide

## Overview

This guide provides solutions to common issues encountered when using the WiFi-DensePose system, including installation problems, hardware connectivity issues, performance optimization, and error resolution.

## Table of Contents

1. [Quick Diagnostics](#quick-diagnostics)
2. [Installation Issues](#installation-issues)
3. [Hardware Problems](#hardware-problems)
4. [Performance Issues](#performance-issues)
5. [API and Connectivity Issues](#api-and-connectivity-issues)
6. [Data Quality Issues](#data-quality-issues)
7. [System Errors](#system-errors)
8. [Domain-Specific Issues](#domain-specific-issues)
9. [Advanced Troubleshooting](#advanced-troubleshooting)
10. [Getting Support](#getting-support)

## Quick Diagnostics

### System Health Check

Run a comprehensive system health check to identify issues:

```bash
# Check system status
curl http://localhost:8000/api/v1/system/status

# Run built-in diagnostics
curl http://localhost:8000/api/v1/system/diagnostics

# Check component health
curl http://localhost:8000/api/v1/health
```

### Log Analysis

Check system logs for error patterns:

```bash
# View recent logs
docker-compose logs --tail=100 wifi-densepose-api

# Search for errors
docker-compose logs | grep -i error

# Check specific component logs
docker-compose logs neural-network
docker-compose logs csi-processor
```

### Resource Monitoring

Monitor system resources:

```bash
# Check Docker container resources
docker stats

# Check system resources
htop
nvidia-smi  # For GPU monitoring

# Check disk space
df -h
```

## Installation Issues

### Docker Installation Problems

#### Issue: Docker Compose Fails to Start

**Symptoms:**
- Services fail to start
- Port conflicts
- Permission errors

**Solutions:**

1. **Check Port Availability:**
```bash
# Check if port 8000 is in use
netstat -tulpn | grep :8000
lsof -i :8000

# Kill process using the port
sudo kill -9 <PID>
```

2. **Fix Permission Issues:**
```bash
# Add user to docker group
sudo usermod -aG docker $USER
newgrp docker

# Fix file permissions
sudo chown -R $USER:$USER .
```

3. **Update Docker Compose:**
```bash
# Update Docker Compose
sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose
```

#### Issue: Out of Disk Space

**Symptoms:**
- Build failures
- Container crashes
- Database errors

**Solutions:**

1. **Clean Docker Resources:**
```bash
# Remove unused containers, networks, images
docker system prune -a

# Remove unused volumes
docker volume prune

# Check disk usage
docker system df
```

2. **Configure Storage Location:**
```bash
# Edit docker-compose.yml to use external storage
volumes:
  - /external/storage/data:/app/data
  - /external/storage/models:/app/models
```

### Native Installation Problems

#### Issue: Python Dependencies Fail to Install

**Symptoms:**
- pip install errors
- Compilation failures
- Missing system libraries

**Solutions:**

1. **Install System Dependencies:**
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y build-essential cmake python3-dev
sudo apt install -y libopencv-dev libffi-dev libssl-dev

# CentOS/RHEL
sudo yum groupinstall -y "Development Tools"
sudo yum install -y python3-devel opencv-devel
```

2. **Use Virtual Environment:**
```bash
# Create clean virtual environment
python3 -m venv venv_clean
source venv_clean/bin/activate
pip install --upgrade pip setuptools wheel
pip install -r requirements.txt
```

3. **Install PyTorch Separately:**
```bash
# Install PyTorch with specific CUDA version
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu118

# Or CPU-only version
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cpu
```

#### Issue: CUDA/GPU Setup Problems

**Symptoms:**
- GPU not detected
- CUDA version mismatch
- Out of GPU memory

**Solutions:**

1. **Verify CUDA Installation:**
```bash
# Check CUDA version
nvcc --version
nvidia-smi

# Check PyTorch CUDA support
python -c "import torch; print(torch.cuda.is_available())"
```

2. **Install Correct CUDA Version:**
```bash
# Install CUDA 11.8 (example)
wget https://developer.download.nvidia.com/compute/cuda/11.8.0/local_installers/cuda_11.8.0_520.61.05_linux.run
sudo sh cuda_11.8.0_520.61.05_linux.run
```

3. **Configure GPU Memory:**
```bash
# Set GPU memory limit
export CUDA_VISIBLE_DEVICES=0
export PYTORCH_CUDA_ALLOC_CONF=max_split_size_mb:512
```

## Hardware Problems

### Router Connectivity Issues

#### Issue: Cannot Connect to Router

**Symptoms:**
- No CSI data received
- Connection timeouts
- Authentication failures

**Solutions:**

1. **Verify Network Connectivity:**
```bash
# Ping router
ping 192.168.1.1

# Check SSH access
ssh root@192.168.1.1

# Test CSI port
telnet 192.168.1.1 5500
```

2. **Check Router Configuration:**
```bash
# SSH into router and check CSI tools
ssh root@192.168.1.1
csi_tool --status

# Restart CSI service
/etc/init.d/csi restart
```

3. **Verify Firewall Settings:**
```bash
# Check iptables rules
iptables -L

# Allow CSI port
iptables -A INPUT -p tcp --dport 5500 -j ACCEPT
```

#### Issue: Poor CSI Data Quality

**Symptoms:**
- High packet loss
- Inconsistent data rates
- Signal interference

**Solutions:**

1. **Optimize Router Placement:**
```bash
# Check signal strength
iwconfig wlan0

# Analyze interference
iwlist wlan0 scan | grep -E "(ESSID|Frequency|Quality)"
```

2. **Adjust CSI Parameters:**
```bash
# Reduce sampling rate
echo "csi_rate=20" >> /etc/config/wireless

# Change channel
echo "channel=6" >> /etc/config/wireless
uci commit wireless
wifi reload
```

3. **Monitor Data Quality:**
```bash
# Check CSI data statistics
curl http://localhost:8000/api/v1/hardware/csi/stats

# View real-time quality metrics
curl http://localhost:8000/api/v1/hardware/status
```

### Hardware Resource Issues

#### Issue: High CPU Usage

**Symptoms:**
- System slowdown
- Processing delays
- High temperature

**Solutions:**

1. **Optimize Processing Settings:**
```bash
# Reduce batch size
export POSE_PROCESSING_BATCH_SIZE=16

# Lower frame rate
export STREAM_FPS=15

# Disable unnecessary features
export ENABLE_HISTORICAL_DATA=false
```

2. **Scale Resources:**
```bash
# Increase worker processes
export WORKERS=4

# Use process affinity
taskset -c 0-3 python -m src.api.main
```

#### Issue: GPU Memory Errors

**Symptoms:**
- CUDA out of memory errors
- Model loading failures
- Inference crashes

**Solutions:**

1. **Optimize GPU Usage:**
```bash
# Reduce batch size
export POSE_PROCESSING_BATCH_SIZE=8

# Enable mixed precision
export ENABLE_MIXED_PRECISION=true

# Clear GPU cache
python -c "import torch; torch.cuda.empty_cache()"
```

2. **Monitor GPU Memory:**
```bash
# Watch GPU memory usage
watch -n 1 nvidia-smi

# Check memory allocation
python -c "
import torch
print(f'Allocated: {torch.cuda.memory_allocated()/1024**3:.2f} GB')
print(f'Cached: {torch.cuda.memory_reserved()/1024**3:.2f} GB')
"
```

## Performance Issues

### Slow Pose Detection

#### Issue: Low Processing Frame Rate

**Symptoms:**
- FPS below expected rate
- High latency
- Delayed responses

**Solutions:**

1. **Optimize Neural Network:**
```bash
# Use TensorRT optimization
export ENABLE_TENSORRT=true

# Enable model quantization
export MODEL_QUANTIZATION=int8

# Use smaller model variant
export POSE_MODEL_PATH="./models/densepose_mobile.pth"
```

2. **Tune Processing Pipeline:**
```bash
# Increase batch size (if GPU memory allows)
export POSE_PROCESSING_BATCH_SIZE=64

# Reduce input resolution
export INPUT_RESOLUTION=256

# Skip frames for real-time processing
export FRAME_SKIP_RATIO=2
```

3. **Parallel Processing:**
```bash
# Enable multi-threading
export OMP_NUM_THREADS=4
export MKL_NUM_THREADS=4

# Use multiple GPU devices
export CUDA_VISIBLE_DEVICES=0,1
```

### Memory Issues

#### Issue: High Memory Usage

**Symptoms:**
- System running out of RAM
- Swap usage increasing
- OOM killer activated

**Solutions:**

1. **Optimize Memory Usage:**
```bash
# Reduce buffer sizes
export CSI_BUFFER_SIZE=500
export STREAM_BUFFER_SIZE=50

# Limit historical data retention
export DATA_RETENTION_HOURS=24

# Enable memory mapping for large files
export USE_MEMORY_MAPPING=true
```

2. **Configure Swap:**
```bash
# Add swap space
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

## API and Connectivity Issues

### Authentication Problems

#### Issue: JWT Token Errors

**Symptoms:**
- 401 Unauthorized responses
- Token expired errors
- Invalid signature errors

**Solutions:**

1. **Verify Token Configuration:**
```bash
# Check secret key
echo $SECRET_KEY

# Verify token expiration
curl -X POST http://localhost:8000/api/v1/auth/verify \
  -H "Authorization: Bearer <token>"
```

2. **Regenerate Tokens:**
```bash
# Get new token
curl -X POST http://localhost:8000/api/v1/auth/token \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "password"}'
```

3. **Check System Time:**
```bash
# Ensure system time is correct
timedatectl status
sudo ntpdate -s time.nist.gov
```

### WebSocket Connection Issues

#### Issue: WebSocket Disconnections

**Symptoms:**
- Frequent disconnections
- Connection timeouts
- No real-time data

**Solutions:**

1. **Adjust WebSocket Settings:**
```bash
# Increase timeout values
export WEBSOCKET_TIMEOUT=600
export WEBSOCKET_PING_INTERVAL=30

# Enable keep-alive
export WEBSOCKET_KEEPALIVE=true
```

2. **Check Network Configuration:**
```bash
# Test WebSocket connection
wscat -c ws://localhost:8000/ws/pose

# Check proxy settings
curl -I http://localhost:8000/ws/pose
```

### Rate Limiting Issues

#### Issue: Rate Limit Exceeded

**Symptoms:**
- 429 Too Many Requests errors
- API calls being rejected
- Slow response times

**Solutions:**

1. **Adjust Rate Limits:**
```bash
# Increase rate limits
export RATE_LIMIT_REQUESTS=1000
export RATE_LIMIT_WINDOW=3600

# Disable rate limiting for development
export ENABLE_RATE_LIMITING=false
```

2. **Implement Request Batching:**
```python
# Batch multiple requests
def batch_requests(requests, batch_size=10):
    for i in range(0, len(requests), batch_size):
        batch = requests[i:i+batch_size]
        # Process batch
        time.sleep(1)  # Rate limiting delay
```

## Data Quality Issues

### Poor Detection Accuracy

#### Issue: Low Confidence Scores

**Symptoms:**
- Many false positives
- Missing detections
- Inconsistent tracking

**Solutions:**

1. **Adjust Detection Thresholds:**
```bash
# Increase confidence threshold
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Content-Type: application/json" \
  -d '{"detection": {"confidence_threshold": 0.8}}'
```

2. **Improve Environment Setup:**
```bash
# Recalibrate system
curl -X POST http://localhost:8000/api/v1/system/calibrate

# Check for interference
curl http://localhost:8000/api/v1/hardware/interference
```

3. **Optimize Model Parameters:**
```bash
# Use domain-specific model
export POSE_MODEL_PATH="./models/healthcare_optimized.pth"

# Enable post-processing filters
export ENABLE_TEMPORAL_SMOOTHING=true
export ENABLE_OUTLIER_FILTERING=true
```

### Tracking Issues

#### Issue: Person ID Switching

**Symptoms:**
- IDs change frequently
- Lost tracks
- Duplicate persons

**Solutions:**

1. **Tune Tracking Parameters:**
```bash
# Adjust tracking thresholds
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Content-Type: application/json" \
  -d '{
    "tracking": {
      "max_age": 30,
      "min_hits": 3,
      "iou_threshold": 0.3
    }
  }'
```

2. **Improve Detection Consistency:**
```bash
# Enable temporal smoothing
export ENABLE_TEMPORAL_SMOOTHING=true

# Use appearance features
export USE_APPEARANCE_FEATURES=true
```

## System Errors

### Database Issues

#### Issue: Database Connection Errors

**Symptoms:**
- Connection refused errors
- Timeout errors
- Data not persisting

**Solutions:**

1. **Check Database Status:**
```bash
# PostgreSQL
sudo systemctl status postgresql
sudo -u postgres psql -c "SELECT version();"

# SQLite
ls -la ./data/wifi_densepose.db
sqlite3 ./data/wifi_densepose.db ".tables"
```

2. **Fix Connection Issues:**
```bash
# Reset database connection
export DATABASE_URL="postgresql://user:password@localhost:5432/wifi_densepose"

# Restart database service
sudo systemctl restart postgresql
```

3. **Database Migration:**
```bash
# Run database migrations
python -m src.database.migrate

# Reset database (WARNING: Data loss)
python -m src.database.reset --confirm
```

### Service Crashes

#### Issue: API Service Crashes

**Symptoms:**
- Service stops unexpectedly
- No response from API
- Error 502/503 responses

**Solutions:**

1. **Check Service Logs:**
```bash
# View crash logs
journalctl -u wifi-densepose -f

# Check for segmentation faults
dmesg | grep -i "segfault"
```

2. **Restart Services:**
```bash
# Restart with Docker
docker-compose restart wifi-densepose-api

# Restart native service
sudo systemctl restart wifi-densepose
```

3. **Debug Memory Issues:**
```bash
# Run with memory debugging
valgrind --tool=memcheck python -m src.api.main

# Check for memory leaks
python -m tracemalloc
```

## Domain-Specific Issues

### Healthcare Domain Issues

#### Issue: Fall Detection False Alarms

**Symptoms:**
- Too many fall alerts
- Normal activities triggering alerts
- Delayed detection

**Solutions:**

1. **Adjust Sensitivity:**
```bash
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Content-Type: application/json" \
  -d '{
    "alerts": {
      "fall_detection": {
        "sensitivity": 0.7,
        "notification_delay_seconds": 10
      }
    }
  }'
```

2. **Improve Training Data:**
```bash
# Collect domain-specific training data
python -m src.training.collect_healthcare_data

# Retrain model with healthcare data
python -m src.training.train_healthcare_model
```

### Retail Domain Issues

#### Issue: Inaccurate Traffic Counting

**Symptoms:**
- Wrong visitor counts
- Missing entries/exits
- Double counting

**Solutions:**

1. **Calibrate Zone Detection:**
```bash
# Define entrance/exit zones
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Content-Type: application/json" \
  -d '{
    "zones": {
      "entrance": {
        "coordinates": [[0, 0], [100, 50]],
        "type": "entrance"
      }
    }
  }'
```

2. **Optimize Tracking:**
```bash
# Enable zone-based tracking
export ENABLE_ZONE_TRACKING=true

# Adjust dwell time thresholds
export MIN_DWELL_TIME_SECONDS=5
```

## Advanced Troubleshooting

### Performance Profiling

#### CPU Profiling

```bash
# Profile Python code
python -m cProfile -o profile.stats -m src.api.main

# Analyze profile
python -c "
import pstats
p = pstats.Stats('profile.stats')
p.sort_stats('cumulative').print_stats(20)
"
```

#### GPU Profiling

```bash
# Profile CUDA kernels
nvprof python -m src.neural_network.inference

# Use PyTorch profiler
python -c "
import torch
with torch.profiler.profile() as prof:
    # Your code here
    pass
print(prof.key_averages().table())
"
```

### Network Debugging

#### Packet Capture

```bash
# Capture CSI packets
sudo tcpdump -i eth0 port 5500 -w csi_capture.pcap

# Analyze with Wireshark
wireshark csi_capture.pcap
```

#### Network Latency Testing

```bash
# Test network latency
ping -c 100 192.168.1.1 | tail -1

# Test bandwidth
iperf3 -c 192.168.1.1 -t 60
```

### System Monitoring

#### Real-time Monitoring

```bash
# Monitor system resources
htop
iotop
nethogs

# Monitor GPU
nvidia-smi -l 1

# Monitor Docker containers
docker stats --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}"
```

#### Log Aggregation

```bash
# Centralized logging with ELK stack
docker run -d --name elasticsearch elasticsearch:7.17.0
docker run -d --name kibana kibana:7.17.0

# Configure log shipping
echo 'LOGGING_DRIVER=syslog' >> .env
echo 'SYSLOG_ADDRESS=tcp://localhost:514' >> .env
```

## Getting Support

### Collecting Diagnostic Information

Before contacting support, collect the following information:

```bash
# System information
uname -a
cat /etc/os-release
docker --version
python --version

# Application logs
docker-compose logs --tail=1000 > logs.txt

# Configuration
cat .env > config.txt
curl http://localhost:8000/api/v1/system/status > status.json

# Hardware information
lscpu
free -h
nvidia-smi > gpu_info.txt
```

### Support Channels

1. **Documentation**: Check the comprehensive documentation first
2. **GitHub Issues**: Report bugs and feature requests
3. **Community Forum**: Ask questions and share solutions
4. **Enterprise Support**: For commercial deployments

### Creating Effective Bug Reports

Include the following information:

1. **Environment Details**:
   - Operating system and version
   - Hardware specifications
   - Docker/Python versions

2. **Steps to Reproduce**:
   - Exact commands or API calls
   - Configuration settings
   - Input data characteristics

3. **Expected vs Actual Behavior**:
   - What you expected to happen
   - What actually happened
   - Error messages and logs

4. **Additional Context**:
   - Screenshots or videos
   - Configuration files
   - System logs

### Emergency Procedures

For critical production issues:

1. **Immediate Actions**:
   ```bash
   # Stop the system safely
   curl -X POST http://localhost:8000/api/v1/system/stop
   
   # Backup current data
   cp -r ./data ./data_backup_$(date +%Y%m%d_%H%M%S)
   
   # Restart with minimal configuration
   export MOCK_HARDWARE=true
   docker-compose up -d
   ```

2. **Rollback Procedures**:
   ```bash
   # Rollback to previous version
   git checkout <previous-tag>
   docker-compose down
   docker-compose up -d
   
   # Restore data backup
   rm -rf ./data
   cp -r ./data_backup_<timestamp> ./data
   ```

3. **Contact Information**:
   - Emergency support: support@wifi-densepose.com
   - Phone: +1-555-SUPPORT
   - Slack: #wifi-densepose-emergency

---

**Remember**: Most issues can be resolved by checking logs, verifying configuration, and ensuring proper hardware setup. When in doubt, start with the basic diagnostics and work your way through the troubleshooting steps systematically.

For additional help, see:
- [Configuration Guide](configuration.md)
- [API Reference](api-reference.md)
- [Hardware Setup Guide](../hardware/router-setup.md)
- [Deployment Guide](../developer/deployment-guide.md)
# WiFi-DensePose Troubleshooting Guide

## Table of Contents

1. [Overview](#overview)
2. [Quick Diagnostics](#quick-diagnostics)
3. [Installation Issues](#installation-issues)
4. [Hardware and Network Issues](#hardware-and-network-issues)
5. [Pose Detection Issues](#pose-detection-issues)
6. [Performance Issues](#performance-issues)
7. [API and WebSocket Issues](#api-and-websocket-issues)
8. [Database and Storage Issues](#database-and-storage-issues)
9. [Authentication and Security Issues](#authentication-and-security-issues)
10. [Deployment Issues](#deployment-issues)
11. [Monitoring and Logging](#monitoring-and-logging)
12. [Common Error Messages](#common-error-messages)
13. [Support and Resources](#support-and-resources)

## Overview

This guide helps diagnose and resolve common issues with WiFi-DensePose. Issues are organized by category with step-by-step troubleshooting procedures.

### Before You Start

1. **Check System Status**: Always start with a health check
2. **Review Logs**: Check application and system logs for errors
3. **Verify Configuration**: Ensure environment variables are correct
4. **Test Connectivity**: Verify network and hardware connections

### Diagnostic Tools

```bash
# System health check
curl http://localhost:8000/api/v1/health

# Check system information
python -c "import wifi_densepose; wifi_densepose.print_system_info()"

# View logs
docker-compose logs -f wifi-densepose
kubectl logs -f deployment/wifi-densepose -n wifi-densepose
```

## Quick Diagnostics

### System Health Check

```bash
#!/bin/bash
# quick-health-check.sh

echo "=== WiFi-DensePose Health Check ==="

# Check if service is running
if curl -s http://localhost:8000/api/v1/health > /dev/null; then
    echo "✅ API service is responding"
else
    echo "❌ API service is not responding"
fi

# Check database connection
if curl -s http://localhost:8000/api/v1/health | grep -q "postgres.*healthy"; then
    echo "✅ Database connection is healthy"
else
    echo "❌ Database connection issues detected"
fi

# Check hardware status
if curl -s http://localhost:8000/api/v1/health | grep -q "hardware.*healthy"; then
    echo "✅ Hardware service is healthy"
else
    echo "❌ Hardware service issues detected"
fi

# Check pose detection
if curl -s http://localhost:8000/api/v1/pose/current > /dev/null; then
    echo "✅ Pose detection is working"
else
    echo "❌ Pose detection issues detected"
fi

echo "=== End Health Check ==="
```

### Log Analysis

```bash
# Check for common error patterns
grep -i "error\|exception\|failed" /var/log/wifi-densepose.log | tail -20

# Check hardware warnings
grep -i "hardware\|router\|csi" /var/log/wifi-densepose.log | tail -10

# Check pose processing issues
grep -i "pose\|detection\|confidence" /var/log/wifi-densepose.log | tail -10
```

## Installation Issues

### Package Installation Problems

#### Issue: `pip install wifi-densepose` fails

**Symptoms:**
- Package not found on PyPI
- Dependency conflicts
- Build errors

**Solutions:**

1. **Update pip and setuptools:**
```bash
pip install --upgrade pip setuptools wheel
```

2. **Install with specific Python version:**
```bash
python3.9 -m pip install wifi-densepose
```

3. **Install from source:**
```bash
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose
pip install -e .
```

4. **Resolve dependency conflicts:**
```bash
pip install --no-deps wifi-densepose
pip install -r requirements.txt
```

#### Issue: Missing system dependencies

**Symptoms:**
- OpenCV import errors
- PyTorch installation failures
- Build tool errors

**Solutions:**

1. **Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install -y build-essential cmake
sudo apt install -y libopencv-dev python3-opencv
sudo apt install -y python3.9-dev python3.9-venv
```

2. **CentOS/RHEL:**
```bash
sudo yum groupinstall -y "Development Tools"
sudo yum install -y opencv-devel python39-devel
```

3. **macOS:**
```bash
brew install cmake opencv python@3.9
```

### Docker Installation Issues

#### Issue: Docker build fails

**Symptoms:**
- Build context too large
- Network timeouts
- Permission errors

**Solutions:**

1. **Optimize build context:**
```bash
# Add to .dockerignore
echo "data/" >> .dockerignore
echo "logs/" >> .dockerignore
echo "*.pyc" >> .dockerignore
echo "__pycache__/" >> .dockerignore
```

2. **Build with specific target:**
```bash
docker build --target production -t wifi-densepose:latest .
```

3. **Fix permission issues:**
```bash
sudo usermod -aG docker $USER
newgrp docker
```

## Hardware and Network Issues

### Router Connection Problems

#### Issue: Router not responding

**Symptoms:**
- "Router main_router is unhealthy" warnings
- No CSI data received
- Connection timeouts

**Diagnostic Steps:**

1. **Check network connectivity:**
```bash
ping 192.168.1.1  # Replace with your router IP
telnet 192.168.1.1 22  # Check SSH access
```

2. **Verify router configuration:**
```bash
ssh admin@192.168.1.1
# Check if CSI extraction is enabled
cat /etc/config/wireless | grep csi
```

3. **Test CSI data stream:**
```bash
# Listen for CSI data
nc -l 5500  # Default CSI port
```

**Solutions:**

1. **Restart router service:**
```bash
ssh admin@192.168.1.1
/etc/init.d/csi-tools restart
```

2. **Reconfigure CSI extraction:**
```bash
# On router
echo "csi_enable=1" >> /etc/config/wireless
echo "csi_rate=30" >> /etc/config/wireless
wifi reload
```

3. **Update router firmware:**
```bash
# Flash OpenWRT with CSI patches
sysupgrade -v openwrt-csi-enabled.bin
```

#### Issue: CSI data quality problems

**Symptoms:**
- Low signal strength
- High noise levels
- Inconsistent data rates

**Solutions:**

1. **Optimize antenna placement:**
   - Ensure 3×3 MIMO configuration
   - Position antennas for optimal coverage
   - Avoid interference sources

2. **Adjust CSI parameters:**
```bash
# Increase sampling rate
echo "csi_rate=50" >> /etc/config/wireless

# Filter noise
echo "csi_filter=1" >> /etc/config/wireless
```

3. **Calibrate environment:**
```bash
curl -X POST http://localhost:8000/api/v1/pose/calibrate
```

### Network Configuration Issues

#### Issue: Firewall blocking connections

**Symptoms:**
- Connection refused errors
- Timeouts on specific ports
- Intermittent connectivity

**Solutions:**

1. **Configure firewall rules:**
```bash
# Ubuntu/Debian
sudo ufw allow 8000/tcp  # API port
sudo ufw allow 5500/tcp  # CSI data port
sudo ufw allow 8080/tcp  # Metrics port

# CentOS/RHEL
sudo firewall-cmd --permanent --add-port=8000/tcp
sudo firewall-cmd --permanent --add-port=5500/tcp
sudo firewall-cmd --reload
```

2. **Check iptables rules:**
```bash
sudo iptables -L -n | grep -E "8000|5500"
```

3. **Disable firewall temporarily for testing:**
```bash
sudo ufw disable  # Ubuntu
sudo systemctl stop firewalld  # CentOS
```

## Pose Detection Issues

### No Pose Detections

#### Issue: System running but no poses detected

**Symptoms:**
- API returns empty pose arrays
- Zero detection count in metrics
- No activity in pose logs

**Diagnostic Steps:**

1. **Check CSI data reception:**
```bash
curl http://localhost:8000/api/v1/system/status | jq '.hardware'
```

2. **Verify confidence threshold:**
```bash
curl http://localhost:8000/api/v1/config | jq '.detection.confidence_threshold'
```

3. **Test with lower threshold:**
```bash
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Content-Type: application/json" \
  -d '{"detection": {"confidence_threshold": 0.3}}'
```

**Solutions:**

1. **Recalibrate system:**
```bash
curl -X POST http://localhost:8000/api/v1/pose/calibrate
```

2. **Check environment setup:**
   - Ensure people are in detection area
   - Verify router placement and coverage
   - Check for interference sources

3. **Adjust detection parameters:**
```bash
curl -X PUT http://localhost:8000/api/v1/config \
  -H "Content-Type: application/json" \
  -d '{
    "detection": {
      "confidence_threshold": 0.5,
      "max_persons": 10,
      "enable_tracking": true
    }
  }'
```

### Poor Detection Accuracy

#### Issue: Low confidence scores or false positives

**Symptoms:**
- Confidence scores below 0.7
- Ghost detections
- Missed detections

**Solutions:**

1. **Improve environment conditions:**
   - Remove metallic objects that cause reflections
   - Ensure stable WiFi signal strength
   - Minimize movement of non-human objects

2. **Retrain or update models:**
```bash
# Download latest models
curl -O https://models.wifi-densepose.com/latest/densepose_model.pth
mv densepose_model.pth /app/models/
```

3. **Adjust processing parameters:**
```python
# In configuration
{
    "pose_processing": {
        "batch_size": 32,
        "nms_threshold": 0.5,
        "keypoint_threshold": 0.3
    }
}
```

### Zone Detection Issues

#### Issue: Incorrect zone assignments

**Symptoms:**
- People detected in wrong zones
- Zone boundaries not respected
- Inconsistent zone occupancy

**Solutions:**

1. **Verify zone configuration:**
```bash
curl http://localhost:8000/api/v1/zones | jq '.'
```

2. **Recalibrate zone boundaries:**
```bash
curl -X PUT http://localhost:8000/api/v1/zones/zone_001 \
  -H "Content-Type: application/json" \
  -d '{
    "coordinates": {
      "x": 0, "y": 0,
      "width": 500, "height": 300
    }
  }'
```

3. **Test zone detection:**
```bash
curl "http://localhost:8000/api/v1/pose/zones/zone_001/occupancy"
```

## Performance Issues

### High CPU Usage

#### Issue: CPU usage consistently above 80%

**Symptoms:**
- Slow response times
- High system load
- Processing delays

**Diagnostic Steps:**

1. **Check CPU usage by component:**
```bash
top -p $(pgrep -f wifi-densepose)
htop -p $(pgrep -f python)
```

2. **Monitor processing metrics:**
```bash
curl http://localhost:8080/metrics | grep cpu
```

**Solutions:**

1. **Optimize processing parameters:**
```bash
# Reduce batch size
export POSE_PROCESSING_BATCH_SIZE=16

# Lower frame rate
export STREAM_FPS=15

# Reduce worker count
export WORKERS=2
```

2. **Enable GPU acceleration:**
```bash
export ENABLE_GPU=true
export CUDA_VISIBLE_DEVICES=0
```

3. **Scale horizontally:**
```bash
# Docker Compose
docker-compose up -d --scale wifi-densepose=3

# Kubernetes
kubectl scale deployment wifi-densepose --replicas=5
```

### High Memory Usage

#### Issue: Memory usage growing over time

**Symptoms:**
- Out of memory errors
- Gradual memory increase
- System swapping

**Solutions:**

1. **Configure memory limits:**
```bash
# Docker
docker run --memory=4g wifi-densepose

# Kubernetes
resources:
  limits:
    memory: 4Gi
```

2. **Optimize buffer sizes:**
```bash
export CSI_BUFFER_SIZE=500
export POSE_HISTORY_LIMIT=1000
```

3. **Enable garbage collection:**
```python
import gc
gc.set_threshold(700, 10, 10)
```

### Slow Response Times

#### Issue: API responses taking >1 second

**Symptoms:**
- High latency in API calls
- Timeout errors
- Poor user experience

**Solutions:**

1. **Enable caching:**
```bash
export REDIS_URL=redis://localhost:6379/0
export ENABLE_CACHING=true
```

2. **Optimize database queries:**
```sql
-- Add indexes
CREATE INDEX idx_pose_detections_timestamp ON pose_detections (timestamp);
CREATE INDEX idx_csi_data_timestamp ON csi_data (timestamp);
```

3. **Use connection pooling:**
```bash
export DATABASE_POOL_SIZE=20
export DATABASE_MAX_OVERFLOW=30
```

## API and WebSocket Issues

### API Not Responding

#### Issue: HTTP 500 errors or connection refused

**Symptoms:**
- Cannot connect to API
- Internal server errors
- Service unavailable

**Diagnostic Steps:**

1. **Check service status:**
```bash
curl -I http://localhost:8000/api/v1/health
systemctl status wifi-densepose
```

2. **Check port availability:**
```bash
netstat -tlnp | grep 8000
lsof -i :8000
```

**Solutions:**

1. **Restart service:**
```bash
# Docker
docker-compose restart wifi-densepose

# Systemd
sudo systemctl restart wifi-densepose

# Kubernetes
kubectl rollout restart deployment/wifi-densepose
```

2. **Check configuration:**
```bash
# Verify environment variables
env | grep -E "HOST|PORT|DATABASE_URL"
```

3. **Review logs for errors:**
```bash
tail -f /var/log/wifi-densepose.log
```

### WebSocket Connection Issues

#### Issue: WebSocket connections failing or dropping

**Symptoms:**
- Connection refused on WebSocket endpoint
- Frequent disconnections
- No real-time updates

**Solutions:**

1. **Test WebSocket connectivity:**
```javascript
const ws = new WebSocket('ws://localhost:8000/ws/pose/stream');
ws.onopen = () => console.log('Connected');
ws.onerror = (error) => console.error('Error:', error);
```

2. **Check proxy configuration:**
```nginx
# Nginx WebSocket support
location /ws/ {
    proxy_pass http://backend;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
}
```

3. **Increase connection limits:**
```bash
export WEBSOCKET_MAX_CONNECTIONS=100
export WEBSOCKET_TIMEOUT=300
```

### Authentication Issues

#### Issue: JWT token errors

**Symptoms:**
- 401 Unauthorized errors
- Token expired messages
- Authentication failures

**Solutions:**

1. **Verify token validity:**
```bash
# Decode JWT token
echo "eyJ..." | base64 -d
```

2. **Check token expiration:**
```bash
curl -H "Authorization: Bearer <token>" \
  http://localhost:8000/api/v1/auth/verify
```

3. **Refresh token:**
```bash
curl -X POST http://localhost:8000/api/v1/auth/refresh \
  -H "Authorization: Bearer <refresh-token>"
```

## Database and Storage Issues

### Database Connection Errors

#### Issue: Cannot connect to PostgreSQL

**Symptoms:**
- "Connection refused" errors
- Database timeout errors
- Service startup failures

**Diagnostic Steps:**

1. **Check database status:**
```bash
# Docker
docker-compose logs postgres

# Direct connection test
psql -h localhost -U postgres -d wifi_densepose
```

2. **Verify connection string:**
```bash
echo $DATABASE_URL
```

**Solutions:**

1. **Restart database:**
```bash
docker-compose restart postgres
sudo systemctl restart postgresql
```

2. **Check database configuration:**
```sql
-- Check connections
SELECT * FROM pg_stat_activity;

-- Check database size
SELECT pg_size_pretty(pg_database_size('wifi_densepose'));
```

3. **Fix connection limits:**
```sql
-- Increase max connections
ALTER SYSTEM SET max_connections = 200;
SELECT pg_reload_conf();
```

### Storage Space Issues

#### Issue: Disk space running low

**Symptoms:**
- "No space left on device" errors
- Database write failures
- Log rotation issues

**Solutions:**

1. **Check disk usage:**
```bash
df -h
du -sh /app/data /app/logs /app/models
```

2. **Clean old data:**
```bash
# Remove old logs
find /app/logs -name "*.log" -mtime +7 -delete

# Clean old pose data
psql -c "DELETE FROM pose_detections WHERE timestamp < NOW() - INTERVAL '30 days';"
```

3. **Configure log rotation:**
```bash
# /etc/logrotate.d/wifi-densepose
/app/logs/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
}
```

## Authentication and Security Issues

### SSL/TLS Certificate Issues

#### Issue: HTTPS certificate errors

**Symptoms:**
- Certificate validation failures
- Browser security warnings
- SSL handshake errors

**Solutions:**

1. **Check certificate validity:**
```bash
openssl x509 -in /etc/ssl/certs/wifi-densepose.crt -text -noout
```

2. **Renew Let's Encrypt certificate:**
```bash
certbot renew --nginx
```

3. **Update certificate in Kubernetes:**
```bash
kubectl create secret tls tls-secret \
  --cert=path/to/tls.crt \
  --key=path/to/tls.key
```

### Rate Limiting Issues

#### Issue: Requests being rate limited

**Symptoms:**
- HTTP 429 errors
- "Rate limit exceeded" messages
- Blocked API access

**Solutions:**

1. **Check rate limit status:**
```bash
curl -I http://localhost:8000/api/v1/pose/current
# Look for X-RateLimit-* headers
```

2. **Adjust rate limits:**
```bash
export RATE_LIMIT_REQUESTS=1000
export RATE_LIMIT_WINDOW=3600
```

3. **Implement authentication for higher limits:**
```bash
curl -H "Authorization: Bearer <token>" \
  http://localhost:8000/api/v1/pose/current
```

## Deployment Issues

### Docker Compose Issues

#### Issue: Services not starting properly

**Symptoms:**
- Container exit codes
- Dependency failures
- Network connectivity issues

**Solutions:**

1. **Check service dependencies:**
```bash
docker-compose ps
docker-compose logs
```

2. **Rebuild containers:**
```bash
docker-compose down
docker-compose build --no-cache
docker-compose up -d
```

3. **Fix network issues:**
```bash
docker network ls
docker network inspect wifi-densepose_default
```

### Kubernetes Deployment Issues

#### Issue: Pods not starting

**Symptoms:**
- Pods in Pending/CrashLoopBackOff state
- Image pull errors
- Resource constraints

**Solutions:**

1. **Check pod status:**
```bash
kubectl get pods -n wifi-densepose
kubectl describe pod <pod-name> -n wifi-densepose
```

2. **Check resource availability:**
```bash
kubectl top nodes
kubectl describe node <node-name>
```

3. **Fix image issues:**
```bash
# Check image availability
docker pull wifi-densepose:latest

# Update deployment
kubectl set image deployment/wifi-densepose \
  wifi-densepose=wifi-densepose:latest
```

## Monitoring and Logging

### Log Analysis

#### Common log patterns to monitor:

1. **Error patterns:**
```bash
grep -E "ERROR|CRITICAL|Exception" /var/log/wifi-densepose.log
```

2. **Performance patterns:**
```bash
grep -E "slow|timeout|latency" /var/log/wifi-densepose.log
```

3. **Hardware patterns:**
```bash
grep -E "router|hardware|csi" /var/log/wifi-densepose.log
```

### Metrics Collection

#### Key metrics to monitor:

1. **System metrics:**
   - CPU usage
   - Memory usage
   - Disk I/O
   - Network traffic

2. **Application metrics:**
   - Request rate
   - Response time
   - Error rate
   - Pose detection rate

3. **Hardware metrics:**
   - CSI data rate
   - Signal strength
   - Router connectivity

## Common Error Messages

### Error: "Router main_router is unhealthy"

**Cause:** Router connectivity or CSI extraction issues

**Solution:**
1. Check router network connectivity
2. Verify CSI extraction configuration
3. Restart router CSI service
4. Check firewall rules

### Error: "Database connection failed"

**Cause:** PostgreSQL connectivity issues

**Solution:**
1. Check database service status
2. Verify connection string
3. Check network connectivity
4. Review database logs

### Error: "CUDA out of memory"

**Cause:** GPU memory exhaustion

**Solution:**
1. Reduce batch size
2. Enable mixed precision
3. Clear GPU cache
4. Use CPU processing

### Error: "Rate limit exceeded"

**Cause:** Too many API requests

**Solution:**
1. Implement request throttling
2. Use authentication for higher limits
3. Cache responses
4. Optimize request patterns

### Error: "Pose detection timeout"

**Cause:** Processing taking too long

**Solution:**
1. Optimize processing parameters
2. Scale processing resources
3. Check hardware performance
4. Review model complexity

## Support and Resources

### Getting Help

1. **Documentation:**
   - [User Guide](user_guide.md)
   - [API Reference](api_reference.md)
   - [Deployment Guide](deployment.md)

2. **Community Support:**
   - GitHub Issues: https://github.com/ruvnet/wifi-densepose/issues
   - Discord Server: https://discord.gg/wifi-densepose
   - Stack Overflow: Tag `wifi-densepose`

3. **Professional Support:**
   - Enterprise support available
   - Custom deployment assistance
   - Performance optimization consulting

### Diagnostic Information to Collect

When reporting issues, include:

1. **System Information:**
```bash
# System details
uname -a
python --version
docker --version

# WiFi-DensePose version
python -c "import wifi_densepose; print(wifi_densepose.__version__)"
```

2. **Configuration:**
```bash
# Environment variables (sanitized)
env | grep -E "WIFI|POSE|DATABASE" | sed 's/=.*/=***/'
```

3. **Logs:**
```bash
# Recent logs
tail -100 /var/log/wifi-densepose.log

# Error logs
grep -E "ERROR|CRITICAL" /var/log/wifi-densepose.log | tail -20
```

4. **Health Status:**
```bash
curl http://localhost:8000/api/v1/health | jq '.'
```

### Emergency Procedures

#### System Recovery

1. **Stop all services:**
```bash
docker-compose down
kubectl delete deployment wifi-densepose
```

2. **Backup critical data:**
```bash
pg_dump wifi_densepose > backup.sql
cp -r /app/data /backup/
```

3. **Restore from backup:**
```bash
psql wifi_densepose < backup.sql
cp -r /backup/data /app/
```

4. **Restart with minimal configuration:**
```bash
# Use safe defaults
export DEBUG=true
export MOCK_HARDWARE=true
docker-compose up -d
```

---

For additional support, contact the WiFi-DensePose team or consult the community resources listed above.
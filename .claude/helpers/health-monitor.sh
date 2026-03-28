#!/bin/bash
# Claude Flow V3 - Health Monitor Worker
# Checks disk space, memory pressure, process health

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
METRICS_DIR="$PROJECT_ROOT/.claude-flow/metrics"
HEALTH_FILE="$METRICS_DIR/health.json"
LAST_RUN_FILE="$METRICS_DIR/.health-last-run"

mkdir -p "$METRICS_DIR"

should_run() {
  if [ ! -f "$LAST_RUN_FILE" ]; then return 0; fi
  local last_run=$(cat "$LAST_RUN_FILE" 2>/dev/null || echo "0")
  local now=$(date +%s)
  [ $((now - last_run)) -ge 300 ]  # 5 minutes
}

check_health() {
  echo "[$(date +%H:%M:%S)] Running health check..."

  # Disk usage
  local disk_usage=$(df -h "$PROJECT_ROOT" 2>/dev/null | awk 'NR==2 {print $5}' | tr -d '%')
  local disk_free=$(df -h "$PROJECT_ROOT" 2>/dev/null | awk 'NR==2 {print $4}')

  # Memory usage
  local mem_total=$(free -m 2>/dev/null | awk '/Mem:/ {print $2}' || echo "0")
  local mem_used=$(free -m 2>/dev/null | awk '/Mem:/ {print $3}' || echo "0")
  local mem_pct=$((mem_used * 100 / (mem_total + 1)))

  # Process counts
  local node_procs=$(pgrep -c node 2>/dev/null || echo "0")
  local agentic_procs=$(ps aux 2>/dev/null | grep -c "agentic-flow" | grep -v grep || echo "0")

  # CPU load
  local load_avg=$(cat /proc/loadavg 2>/dev/null | awk '{print $1}' || echo "0")

  # File descriptor usage
  local fd_used=$(ls /proc/$$/fd 2>/dev/null | wc -l || echo "0")

  # Determine health status
  local status="healthy"
  local warnings=""

  if [ "$disk_usage" -gt 90 ]; then
    status="critical"
    warnings="$warnings disk_full"
  elif [ "$disk_usage" -gt 80 ]; then
    status="warning"
    warnings="$warnings disk_high"
  fi

  if [ "$mem_pct" -gt 90 ]; then
    status="critical"
    warnings="$warnings memory_full"
  elif [ "$mem_pct" -gt 80 ]; then
    [ "$status" != "critical" ] && status="warning"
    warnings="$warnings memory_high"
  fi

  # Write health metrics
  cat > "$HEALTH_FILE" << EOF
{
  "status": "$status",
  "timestamp": "$(date -Iseconds)",
  "disk": {
    "usage_pct": $disk_usage,
    "free": "$disk_free"
  },
  "memory": {
    "total_mb": $mem_total,
    "used_mb": $mem_used,
    "usage_pct": $mem_pct
  },
  "processes": {
    "node": $node_procs,
    "agentic_flow": $agentic_procs
  },
  "load_avg": $load_avg,
  "fd_used": $fd_used,
  "warnings": "$(echo $warnings | xargs)"
}
EOF

  echo "[$(date +%H:%M:%S)] ✓ Health: $status | Disk: ${disk_usage}% | Memory: ${mem_pct}% | Load: $load_avg"

  date +%s > "$LAST_RUN_FILE"

  # Return non-zero if unhealthy
  [ "$status" = "healthy" ] && return 0 || return 1
}

case "${1:-check}" in
  "run") check_health ;;
  "check") should_run && check_health || echo "[$(date +%H:%M:%S)] Skipping (throttled)" ;;
  "force") rm -f "$LAST_RUN_FILE"; check_health ;;
  "status")
    if [ -f "$HEALTH_FILE" ]; then
      jq -r '"Status: \(.status) | Disk: \(.disk.usage_pct)% | Memory: \(.memory.usage_pct)% | Load: \(.load_avg)"' "$HEALTH_FILE"
    else
      echo "No health data available"
    fi
    ;;
  *) echo "Usage: $0 [run|check|force|status]" ;;
esac

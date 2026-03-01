#!/bin/bash
# Claude Flow V3 - Unified Worker Manager
# Orchestrates all background workers with proper scheduling

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
METRICS_DIR="$PROJECT_ROOT/.claude-flow/metrics"
PID_FILE="$METRICS_DIR/worker-manager.pid"
LOG_FILE="$METRICS_DIR/worker-manager.log"

mkdir -p "$METRICS_DIR"

# Worker definitions: name:script:interval_seconds
WORKERS=(
  "perf:perf-worker.sh:300"           # 5 min
  "health:health-monitor.sh:300"       # 5 min
  "patterns:pattern-consolidator.sh:900"  # 15 min
  "ddd:ddd-tracker.sh:600"             # 10 min
  "adr:adr-compliance.sh:900"          # 15 min
  "security:security-scanner.sh:1800"  # 30 min
  "learning:learning-optimizer.sh:1800" # 30 min
)

log() {
  echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG_FILE"
}

run_worker() {
  local name="$1"
  local script="$2"
  local script_path="$SCRIPT_DIR/$script"

  if [ -x "$script_path" ]; then
    "$script_path" check 2>/dev/null &
  fi
}

run_all_workers() {
  log "Running all workers (non-blocking)..."

  for worker_def in "${WORKERS[@]}"; do
    IFS=':' read -r name script interval <<< "$worker_def"
    run_worker "$name" "$script"
  done

  # Don't wait - truly non-blocking
  log "All workers spawned"
}

run_daemon() {
  local interval="${1:-60}"

  log "Starting worker manager daemon (interval: ${interval}s)"
  echo $$ > "$PID_FILE"

  trap 'log "Shutting down..."; rm -f "$PID_FILE"; exit 0' SIGTERM SIGINT

  while true; do
    run_all_workers
    sleep "$interval"
  done
}

status_all() {
  echo "╔══════════════════════════════════════════════════════════════╗"
  echo "║           Claude Flow V3 - Worker Status                      ║"
  echo "╠══════════════════════════════════════════════════════════════╣"

  for worker_def in "${WORKERS[@]}"; do
    IFS=':' read -r name script interval <<< "$worker_def"
    local script_path="$SCRIPT_DIR/$script"

    if [ -x "$script_path" ]; then
      local status=$("$script_path" status 2>/dev/null || echo "No data")
      printf "║ %-10s │ %-48s ║\n" "$name" "$status"
    fi
  done

  echo "╠══════════════════════════════════════════════════════════════╣"

  # Check if daemon is running
  if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
    echo "║ Daemon: RUNNING (PID: $(cat "$PID_FILE"))                           ║"
  else
    echo "║ Daemon: NOT RUNNING                                          ║"
  fi

  echo "╚══════════════════════════════════════════════════════════════╝"
}

force_all() {
  log "Force running all workers..."

  for worker_def in "${WORKERS[@]}"; do
    IFS=':' read -r name script interval <<< "$worker_def"
    local script_path="$SCRIPT_DIR/$script"

    if [ -x "$script_path" ]; then
      log "Running $name..."
      "$script_path" force 2>&1 | while read -r line; do
        log "  [$name] $line"
      done
    fi
  done

  log "All workers completed"
}

case "${1:-help}" in
  "start"|"daemon")
    if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
      echo "Worker manager already running (PID: $(cat "$PID_FILE"))"
      exit 1
    fi
    run_daemon "${2:-60}" &
    echo "Worker manager started (PID: $!)"
    ;;
  "stop")
    if [ -f "$PID_FILE" ]; then
      kill "$(cat "$PID_FILE")" 2>/dev/null || true
      rm -f "$PID_FILE"
      echo "Worker manager stopped"
    else
      echo "Worker manager not running"
    fi
    ;;
  "run"|"once")
    run_all_workers
    ;;
  "force")
    force_all
    ;;
  "status")
    status_all
    ;;
  "logs")
    tail -50 "$LOG_FILE" 2>/dev/null || echo "No logs available"
    ;;
  "help"|*)
    cat << EOF
Claude Flow V3 - Worker Manager

Usage: $0 <command> [options]

Commands:
  start [interval]  Start daemon (default: 60s cycle)
  stop              Stop daemon
  run               Run all workers once
  force             Force run all workers (ignore throttle)
  status            Show all worker status
  logs              Show recent logs

Workers:
  perf              Performance benchmarks (5 min)
  health            System health monitoring (5 min)
  patterns          Pattern consolidation (15 min)
  ddd               DDD progress tracking (10 min)
  adr               ADR compliance checking (15 min)
  security          Security scanning (30 min)
  learning          Learning optimization (30 min)

Examples:
  $0 start 120      # Start with 2-minute cycle
  $0 force          # Run all now
  $0 status         # Check all status
EOF
    ;;
esac

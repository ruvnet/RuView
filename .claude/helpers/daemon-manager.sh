#!/bin/bash
# Claude Flow V3 - Daemon Manager
# Manages background services for real-time statusline updates

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PID_DIR="$PROJECT_ROOT/.claude-flow/pids"
LOG_DIR="$PROJECT_ROOT/.claude-flow/logs"
METRICS_DIR="$PROJECT_ROOT/.claude-flow/metrics"

# Ensure directories exist
mkdir -p "$PID_DIR" "$LOG_DIR" "$METRICS_DIR"

# PID files
SWARM_MONITOR_PID="$PID_DIR/swarm-monitor.pid"
METRICS_DAEMON_PID="$PID_DIR/metrics-daemon.pid"

# Log files
DAEMON_LOG="$LOG_DIR/daemon.log"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
RESET='\033[0m'

log() {
    local msg="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo -e "${CYAN}$msg${RESET}"
    echo "$msg" >> "$DAEMON_LOG"
}

success() {
    local msg="[$(date '+%Y-%m-%d %H:%M:%S')] SUCCESS: $1"
    echo -e "${GREEN}$msg${RESET}"
    echo "$msg" >> "$DAEMON_LOG"
}

error() {
    local msg="[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $1"
    echo -e "${RED}$msg${RESET}"
    echo "$msg" >> "$DAEMON_LOG"
}

# Check if a process is running
is_running() {
    local pid_file="$1"
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p "$pid" > /dev/null 2>&1; then
            return 0
        fi
    fi
    return 1
}

# Start the swarm monitor daemon
start_swarm_monitor() {
    local interval="${1:-3}"

    if is_running "$SWARM_MONITOR_PID"; then
        log "Swarm monitor already running (PID: $(cat "$SWARM_MONITOR_PID"))"
        return 0
    fi

    log "Starting swarm monitor daemon (interval: ${interval}s)..."

    # Run the monitor in background
    nohup "$SCRIPT_DIR/swarm-monitor.sh" monitor "$interval" >> "$LOG_DIR/swarm-monitor.log" 2>&1 &
    local pid=$!

    echo "$pid" > "$SWARM_MONITOR_PID"
    success "Swarm monitor started (PID: $pid)"

    return 0
}

# Start the metrics update daemon
start_metrics_daemon() {
    local interval="${1:-30}"  # Default 30 seconds for V3 sync

    if is_running "$METRICS_DAEMON_PID"; then
        log "Metrics daemon already running (PID: $(cat "$METRICS_DAEMON_PID"))"
        return 0
    fi

    log "Starting metrics daemon (interval: ${interval}s, using SQLite)..."

    # Use SQLite-based metrics (10.5x faster than bash/JSON)
    # Run as Node.js daemon process
    nohup node "$SCRIPT_DIR/metrics-db.mjs" daemon "$interval" >> "$LOG_DIR/metrics-daemon.log" 2>&1 &
    local pid=$!

    echo "$pid" > "$METRICS_DAEMON_PID"
    success "Metrics daemon started (PID: $pid) - SQLite backend"

    return 0
}

# Stop a daemon by PID file
stop_daemon() {
    local pid_file="$1"
    local name="$2"

    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p "$pid" > /dev/null 2>&1; then
            log "Stopping $name (PID: $pid)..."
            kill "$pid" 2>/dev/null
            sleep 1

            # Force kill if still running
            if ps -p "$pid" > /dev/null 2>&1; then
                kill -9 "$pid" 2>/dev/null
            fi

            success "$name stopped"
        fi
        rm -f "$pid_file"
    else
        log "$name not running"
    fi
}

# Start all daemons
start_all() {
    log "Starting all Claude Flow daemons..."
    start_swarm_monitor "${1:-3}"
    start_metrics_daemon "${2:-5}"

    # Initial metrics update
    "$SCRIPT_DIR/swarm-monitor.sh" check > /dev/null 2>&1

    success "All daemons started"
    show_status
}

# Stop all daemons
stop_all() {
    log "Stopping all Claude Flow daemons..."
    stop_daemon "$SWARM_MONITOR_PID" "Swarm monitor"
    stop_daemon "$METRICS_DAEMON_PID" "Metrics daemon"
    success "All daemons stopped"
}

# Restart all daemons
restart_all() {
    stop_all
    sleep 1
    start_all "$@"
}

# Show daemon status
show_status() {
    echo ""
    echo -e "${CYAN}═══════════════════════════════════════════════════${RESET}"
    echo -e "${CYAN}       Claude Flow V3 Daemon Status${RESET}"
    echo -e "${CYAN}═══════════════════════════════════════════════════${RESET}"
    echo ""

    # Swarm Monitor
    if is_running "$SWARM_MONITOR_PID"; then
        echo -e "  ${GREEN}●${RESET} Swarm Monitor    ${GREEN}RUNNING${RESET} (PID: $(cat "$SWARM_MONITOR_PID"))"
    else
        echo -e "  ${RED}○${RESET} Swarm Monitor    ${RED}STOPPED${RESET}"
    fi

    # Metrics Daemon
    if is_running "$METRICS_DAEMON_PID"; then
        echo -e "  ${GREEN}●${RESET} Metrics Daemon   ${GREEN}RUNNING${RESET} (PID: $(cat "$METRICS_DAEMON_PID"))"
    else
        echo -e "  ${RED}○${RESET} Metrics Daemon   ${RED}STOPPED${RESET}"
    fi

    # MCP Server
    local mcp_count=$(ps aux 2>/dev/null | grep -E "mcp.*start" | grep -v grep | wc -l)
    if [ "$mcp_count" -gt 0 ]; then
        echo -e "  ${GREEN}●${RESET} MCP Server       ${GREEN}RUNNING${RESET}"
    else
        echo -e "  ${YELLOW}○${RESET} MCP Server       ${YELLOW}NOT DETECTED${RESET}"
    fi

    # Agentic Flow
    local af_count=$(ps aux 2>/dev/null | grep -E "agentic-flow" | grep -v grep | grep -v "daemon-manager" | wc -l)
    if [ "$af_count" -gt 0 ]; then
        echo -e "  ${GREEN}●${RESET} Agentic Flow     ${GREEN}ACTIVE${RESET} ($af_count processes)"
    else
        echo -e "  ${YELLOW}○${RESET} Agentic Flow     ${YELLOW}IDLE${RESET}"
    fi

    echo ""
    echo -e "${CYAN}───────────────────────────────────────────────────${RESET}"

    # Show latest metrics
    if [ -f "$METRICS_DIR/swarm-activity.json" ]; then
        local last_update=$(jq -r '.timestamp // "unknown"' "$METRICS_DIR/swarm-activity.json" 2>/dev/null)
        local agent_count=$(jq -r '.swarm.agent_count // 0' "$METRICS_DIR/swarm-activity.json" 2>/dev/null)
        echo -e "  Last Update: ${last_update}"
        echo -e "  Active Agents: ${agent_count}"
    fi

    echo -e "${CYAN}═══════════════════════════════════════════════════${RESET}"
    echo ""
}

# Main command handling
case "${1:-status}" in
    "start")
        start_all "${2:-3}" "${3:-5}"
        ;;
    "stop")
        stop_all
        ;;
    "restart")
        restart_all "${2:-3}" "${3:-5}"
        ;;
    "status")
        show_status
        ;;
    "start-swarm")
        start_swarm_monitor "${2:-3}"
        ;;
    "start-metrics")
        start_metrics_daemon "${2:-5}"
        ;;
    "help"|"-h"|"--help")
        echo "Claude Flow V3 Daemon Manager"
        echo ""
        echo "Usage: $0 [command] [options]"
        echo ""
        echo "Commands:"
        echo "  start [swarm_interval] [metrics_interval]  Start all daemons"
        echo "  stop                                       Stop all daemons"
        echo "  restart [swarm_interval] [metrics_interval] Restart all daemons"
        echo "  status                                     Show daemon status"
        echo "  start-swarm [interval]                     Start swarm monitor only"
        echo "  start-metrics [interval]                   Start metrics daemon only"
        echo "  help                                       Show this help"
        echo ""
        echo "Examples:"
        echo "  $0 start           # Start with defaults (3s swarm, 5s metrics)"
        echo "  $0 start 2 3       # Start with 2s swarm, 3s metrics intervals"
        echo "  $0 status          # Show current status"
        echo "  $0 stop            # Stop all daemons"
        ;;
    *)
        error "Unknown command: $1"
        echo "Use '$0 help' for usage information"
        exit 1
        ;;
esac

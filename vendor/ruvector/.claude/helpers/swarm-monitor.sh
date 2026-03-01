#!/bin/bash
# Claude Flow V3 - Real-time Swarm Activity Monitor
# Continuously monitors and updates metrics based on running processes

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
METRICS_DIR="$PROJECT_ROOT/.claude-flow/metrics"
UPDATE_SCRIPT="$SCRIPT_DIR/update-v3-progress.sh"

# Ensure metrics directory exists
mkdir -p "$METRICS_DIR"

# Colors for logging
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
RESET='\033[0m'

log() {
    echo -e "${CYAN}[$(date '+%H:%M:%S')] ${1}${RESET}"
}

warn() {
    echo -e "${YELLOW}[$(date '+%H:%M:%S')] WARNING: ${1}${RESET}"
}

error() {
    echo -e "${RED}[$(date '+%H:%M:%S')] ERROR: ${1}${RESET}"
}

success() {
    echo -e "${GREEN}[$(date '+%H:%M:%S')] ${1}${RESET}"
}

# Function to count active processes
count_active_processes() {
    local agentic_flow_count=0
    local mcp_count=0
    local agent_count=0

    # Count agentic-flow processes
    agentic_flow_count=$(ps aux 2>/dev/null | grep -E "agentic-flow" | grep -v grep | grep -v "swarm-monitor" | wc -l)

    # Count MCP server processes
    mcp_count=$(ps aux 2>/dev/null | grep -E "mcp.*start" | grep -v grep | wc -l)

    # Count specific agent processes
    agent_count=$(ps aux 2>/dev/null | grep -E "(agent|swarm|coordinator)" | grep -v grep | grep -v "swarm-monitor" | wc -l)

    # Calculate total active "agents" using heuristic
    local total_agents=0
    if [ "$agentic_flow_count" -gt 0 ]; then
        # Use agent count if available, otherwise estimate from processes
        if [ "$agent_count" -gt 0 ]; then
            total_agents="$agent_count"
        else
            # Heuristic: some processes are management, some are agents
            total_agents=$((agentic_flow_count / 2))
            if [ "$total_agents" -eq 0 ] && [ "$agentic_flow_count" -gt 0 ]; then
                total_agents=1
            fi
        fi
    fi

    echo "agentic:$agentic_flow_count mcp:$mcp_count agents:$total_agents"
}

# Function to update metrics based on detected activity
update_activity_metrics() {
    local process_info="$1"
    local agentic_count=$(echo "$process_info" | cut -d' ' -f1 | cut -d':' -f2)
    local mcp_count=$(echo "$process_info" | cut -d' ' -f2 | cut -d':' -f2)
    local agent_count=$(echo "$process_info" | cut -d' ' -f3 | cut -d':' -f2)

    # Update active agents in metrics
    if [ -f "$UPDATE_SCRIPT" ]; then
        "$UPDATE_SCRIPT" agent "$agent_count" >/dev/null 2>&1
    fi

    # Update integration status based on activity
    local integration_status="false"
    if [ "$agentic_count" -gt 0 ] || [ "$mcp_count" -gt 0 ]; then
        integration_status="true"
    fi

    # Create/update activity metrics file
    local activity_file="$METRICS_DIR/swarm-activity.json"
    cat > "$activity_file" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "processes": {
    "agentic_flow": $agentic_count,
    "mcp_server": $mcp_count,
    "estimated_agents": $agent_count
  },
  "swarm": {
    "active": $([ "$agent_count" -gt 0 ] && echo "true" || echo "false"),
    "agent_count": $agent_count,
    "coordination_active": $([ "$agentic_count" -gt 0 ] && echo "true" || echo "false")
  },
  "integration": {
    "agentic_flow_active": $integration_status,
    "mcp_active": $([ "$mcp_count" -gt 0 ] && echo "true" || echo "false")
  }
}
EOF

    return 0
}

# Function to monitor continuously
monitor_continuous() {
    local monitor_interval="${1:-5}"  # Default 5 seconds
    local last_state=""
    local current_state=""

    log "Starting continuous swarm monitoring (interval: ${monitor_interval}s)"
    log "Press Ctrl+C to stop monitoring"

    while true; do
        current_state=$(count_active_processes)

        # Only update if state changed
        if [ "$current_state" != "$last_state" ]; then
            update_activity_metrics "$current_state"

            local agent_count=$(echo "$current_state" | cut -d' ' -f3 | cut -d':' -f2)
            local agentic_count=$(echo "$current_state" | cut -d' ' -f1 | cut -d':' -f2)

            if [ "$agent_count" -gt 0 ] || [ "$agentic_count" -gt 0 ]; then
                success "Swarm activity detected: $current_state"
            else
                warn "No swarm activity detected"
            fi

            last_state="$current_state"
        fi

        sleep "$monitor_interval"
    done
}

# Function to run a single check
check_once() {
    log "Running single swarm activity check..."

    local process_info=$(count_active_processes)
    update_activity_metrics "$process_info"

    local agent_count=$(echo "$process_info" | cut -d' ' -f3 | cut -d':' -f2)
    local agentic_count=$(echo "$process_info" | cut -d' ' -f1 | cut -d':' -f2)
    local mcp_count=$(echo "$process_info" | cut -d' ' -f2 | cut -d':' -f2)

    log "Process Detection Results:"
    log "  Agentic Flow processes: $agentic_count"
    log "  MCP Server processes: $mcp_count"
    log "  Estimated agents: $agent_count"

    if [ "$agent_count" -gt 0 ] || [ "$agentic_count" -gt 0 ]; then
        success "✓ Swarm activity detected and metrics updated"
    else
        warn "⚠ No swarm activity detected"
    fi

    # Run performance benchmarks (throttled to every 5 min)
    if [ -x "$SCRIPT_DIR/perf-worker.sh" ]; then
        "$SCRIPT_DIR/perf-worker.sh" check 2>/dev/null &
    fi

    return 0
}

# Main command handling
case "${1:-check}" in
    "monitor"|"continuous")
        monitor_continuous "${2:-5}"
        ;;
    "check"|"once")
        check_once
        ;;
    "status")
        if [ -f "$METRICS_DIR/swarm-activity.json" ]; then
            log "Current swarm activity status:"
            cat "$METRICS_DIR/swarm-activity.json" | jq . 2>/dev/null || cat "$METRICS_DIR/swarm-activity.json"
        else
            warn "No activity data available. Run 'check' first."
        fi
        ;;
    "help"|"-h"|"--help")
        echo "Claude Flow V3 Swarm Monitor"
        echo ""
        echo "Usage: $0 [command] [options]"
        echo ""
        echo "Commands:"
        echo "  check, once     Run a single activity check and update metrics"
        echo "  monitor [N]     Monitor continuously every N seconds (default: 5)"
        echo "  status          Show current activity status"
        echo "  help            Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0 check                    # Single check"
        echo "  $0 monitor 3                # Monitor every 3 seconds"
        echo "  $0 status                   # Show current status"
        ;;
    *)
        error "Unknown command: $1"
        echo "Use '$0 help' for usage information"
        exit 1
        ;;
esac

#!/bin/bash

# Read JSON input from stdin
INPUT=$(cat)
MODEL=$(echo "$INPUT" | jq -r '.model.display_name // "Claude"')
CWD=$(echo "$INPUT" | jq -r '.workspace.current_dir // .cwd')
DIR=$(basename "$CWD")

# Replace claude-code-flow with branded name
if [ "$DIR" = "claude-code-flow" ]; then
  DIR="🌊 Claude Flow"
fi

# Get git branch
BRANCH=$(cd "$CWD" 2>/dev/null && git branch --show-current 2>/dev/null)

# Start building statusline
printf "\033[1m$MODEL\033[0m in \033[36m$DIR\033[0m"
[ -n "$BRANCH" ] && printf " on \033[33m⎇ $BRANCH\033[0m"

# Claude-Flow integration
FLOW_DIR="$CWD/.claude-flow"

if [ -d "$FLOW_DIR" ]; then
  printf " │"

  # 1. Swarm Configuration & Topology
  if [ -f "$FLOW_DIR/swarm-config.json" ]; then
    STRATEGY=$(jq -r '.defaultStrategy // empty' "$FLOW_DIR/swarm-config.json" 2>/dev/null)
    if [ -n "$STRATEGY" ]; then
      # Map strategy to topology icon
      case "$STRATEGY" in
        "balanced") TOPO_ICON="⚡mesh" ;;
        "conservative") TOPO_ICON="⚡hier" ;;
        "aggressive") TOPO_ICON="⚡ring" ;;
        *) TOPO_ICON="⚡$STRATEGY" ;;
      esac
      printf " \033[35m$TOPO_ICON\033[0m"

      # Count agent profiles as "configured agents"
      AGENT_COUNT=$(jq -r '.agentProfiles | length' "$FLOW_DIR/swarm-config.json" 2>/dev/null)
      if [ -n "$AGENT_COUNT" ] && [ "$AGENT_COUNT" != "null" ] && [ "$AGENT_COUNT" -gt 0 ]; then
        printf "  \033[35m🤖 $AGENT_COUNT\033[0m"
      fi
    fi
  fi

  # 2. Real-time System Metrics
  if [ -f "$FLOW_DIR/metrics/system-metrics.json" ]; then
    # Get latest metrics (last entry in array)
    LATEST=$(jq -r '.[-1]' "$FLOW_DIR/metrics/system-metrics.json" 2>/dev/null)

    if [ -n "$LATEST" ] && [ "$LATEST" != "null" ]; then
      # Memory usage
      MEM_PERCENT=$(echo "$LATEST" | jq -r '.memoryUsagePercent // 0' | awk '{printf "%.0f", $1}')
      if [ -n "$MEM_PERCENT" ] && [ "$MEM_PERCENT" != "null" ]; then
        # Color-coded memory (green <60%, yellow 60-80%, red >80%)
        if [ "$MEM_PERCENT" -lt 60 ]; then
          MEM_COLOR="\033[32m"  # Green
        elif [ "$MEM_PERCENT" -lt 80 ]; then
          MEM_COLOR="\033[33m"  # Yellow
        else
          MEM_COLOR="\033[31m"  # Red
        fi
        printf "  ${MEM_COLOR}💾 ${MEM_PERCENT}%\033[0m"
      fi

      # CPU load
      CPU_LOAD=$(echo "$LATEST" | jq -r '.cpuLoad // 0' | awk '{printf "%.0f", $1 * 100}')
      if [ -n "$CPU_LOAD" ] && [ "$CPU_LOAD" != "null" ]; then
        # Color-coded CPU (green <50%, yellow 50-75%, red >75%)
        if [ "$CPU_LOAD" -lt 50 ]; then
          CPU_COLOR="\033[32m"  # Green
        elif [ "$CPU_LOAD" -lt 75 ]; then
          CPU_COLOR="\033[33m"  # Yellow
        else
          CPU_COLOR="\033[31m"  # Red
        fi
        printf "  ${CPU_COLOR}⚙ ${CPU_LOAD}%\033[0m"
      fi
    fi
  fi

  # 3. Session State
  if [ -f "$FLOW_DIR/session-state.json" ]; then
    SESSION_ID=$(jq -r '.sessionId // empty' "$FLOW_DIR/session-state.json" 2>/dev/null)
    ACTIVE=$(jq -r '.active // false' "$FLOW_DIR/session-state.json" 2>/dev/null)

    if [ "$ACTIVE" = "true" ] && [ -n "$SESSION_ID" ]; then
      # Show abbreviated session ID
      SHORT_ID=$(echo "$SESSION_ID" | cut -d'-' -f1)
      printf "  \033[34m🔄 $SHORT_ID\033[0m"
    fi
  fi

  # 4. Performance Metrics from task-metrics.json
  if [ -f "$FLOW_DIR/metrics/task-metrics.json" ]; then
    # Parse task metrics for success rate, avg time, and streak
    METRICS=$(jq -r '
      # Calculate metrics
      (map(select(.success == true)) | length) as $successful |
      (length) as $total |
      (if $total > 0 then ($successful / $total * 100) else 0 end) as $success_rate |
      (map(.duration // 0) | add / length) as $avg_duration |
      # Calculate streak (consecutive successes from end)
      (reverse |
        reduce .[] as $task (0;
          if $task.success == true then . + 1 else 0 end
        )
      ) as $streak |
      {
        success_rate: $success_rate,
        avg_duration: $avg_duration,
        streak: $streak,
        total: $total
      } | @json
    ' "$FLOW_DIR/metrics/task-metrics.json" 2>/dev/null)

    if [ -n "$METRICS" ] && [ "$METRICS" != "null" ]; then
      # Success Rate
      SUCCESS_RATE=$(echo "$METRICS" | jq -r '.success_rate // 0' | awk '{printf "%.0f", $1}')
      TOTAL_TASKS=$(echo "$METRICS" | jq -r '.total // 0')

      if [ -n "$SUCCESS_RATE" ] && [ "$TOTAL_TASKS" -gt 0 ]; then
        # Color-code: Green (>80%), Yellow (60-80%), Red (<60%)
        if [ "$SUCCESS_RATE" -gt 80 ]; then
          SUCCESS_COLOR="\033[32m"  # Green
        elif [ "$SUCCESS_RATE" -ge 60 ]; then
          SUCCESS_COLOR="\033[33m"  # Yellow
        else
          SUCCESS_COLOR="\033[31m"  # Red
        fi
        printf "  ${SUCCESS_COLOR}🎯 ${SUCCESS_RATE}%\033[0m"
      fi

      # Average Time
      AVG_TIME=$(echo "$METRICS" | jq -r '.avg_duration // 0')
      if [ -n "$AVG_TIME" ] && [ "$TOTAL_TASKS" -gt 0 ]; then
        # Format smartly: seconds, minutes, or hours
        if [ $(echo "$AVG_TIME < 60" | bc -l 2>/dev/null || echo 0) -eq 1 ]; then
          TIME_STR=$(echo "$AVG_TIME" | awk '{printf "%.1fs", $1}')
        elif [ $(echo "$AVG_TIME < 3600" | bc -l 2>/dev/null || echo 0) -eq 1 ]; then
          TIME_STR=$(echo "$AVG_TIME" | awk '{printf "%.1fm", $1/60}')
        else
          TIME_STR=$(echo "$AVG_TIME" | awk '{printf "%.1fh", $1/3600}')
        fi
        printf "  \033[36m⏱️  $TIME_STR\033[0m"
      fi

      # Streak (only show if > 0)
      STREAK=$(echo "$METRICS" | jq -r '.streak // 0')
      if [ -n "$STREAK" ] && [ "$STREAK" -gt 0 ]; then
        printf "  \033[91m🔥 $STREAK\033[0m"
      fi
    fi
  fi

  # 5. Active Tasks (check for task files)
  if [ -d "$FLOW_DIR/tasks" ]; then
    TASK_COUNT=$(find "$FLOW_DIR/tasks" -name "*.json" -type f 2>/dev/null | wc -l)
    if [ "$TASK_COUNT" -gt 0 ]; then
      printf "  \033[36m📋 $TASK_COUNT\033[0m"
    fi
  fi

  # 6. Check for hooks activity
  if [ -f "$FLOW_DIR/hooks-state.json" ]; then
    HOOKS_ACTIVE=$(jq -r '.enabled // false' "$FLOW_DIR/hooks-state.json" 2>/dev/null)
    if [ "$HOOKS_ACTIVE" = "true" ]; then
      printf " \033[35m🔗\033[0m"
    fi
  fi
fi

echo

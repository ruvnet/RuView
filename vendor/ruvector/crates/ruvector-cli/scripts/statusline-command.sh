#!/bin/bash

# RuVector Intelligence Statusline
# Multi-line display showcasing self-learning capabilities

INPUT=$(cat)
MODEL=$(echo "$INPUT" | jq -r '.model.display_name // "Claude"')
CWD=$(echo "$INPUT" | jq -r '.workspace.current_dir // .cwd')
DIR=$(basename "$CWD")

# Get git branch
BRANCH=$(cd "$CWD" 2>/dev/null && git branch --show-current 2>/dev/null)

# Colors
RESET="\033[0m"
BOLD="\033[1m"
CYAN="\033[36m"
YELLOW="\033[33m"
GREEN="\033[32m"
MAGENTA="\033[35m"
BLUE="\033[34m"
RED="\033[31m"
DIM="\033[2m"

# ═══════════════════════════════════════════════════════════════════════════════
# LINE 1: Model, Directory, Git
# ═══════════════════════════════════════════════════════════════════════════════
printf "${BOLD}${MODEL}${RESET} in ${CYAN}${DIR}${RESET}"
[ -n "$BRANCH" ] && printf " on ${YELLOW}⎇ ${BRANCH}${RESET}"
echo

# ═══════════════════════════════════════════════════════════════════════════════
# LINE 2: RuVector Intelligence Stats
# ═══════════════════════════════════════════════════════════════════════════════
# Check multiple locations for intelligence file
INTEL_FILE=""
for INTEL_PATH in "$CWD/.ruvector/intelligence.json" \
                  "$CWD/npm/packages/ruvector/.ruvector/intelligence.json" \
                  "$HOME/.ruvector/intelligence.json"; do
  if [ -f "$INTEL_PATH" ]; then
    INTEL_FILE="$INTEL_PATH"
    break
  fi
done

if [ -n "$INTEL_FILE" ]; then
  # Extract learning metrics
  INTEL=$(cat "$INTEL_FILE" 2>/dev/null)

  # Detect schema version (v2 has .learning.qTables, v1 has .patterns)
  HAS_LEARNING=$(echo "$INTEL" | jq -r 'has("learning")' 2>/dev/null)

  if [ "$HAS_LEARNING" = "true" ]; then
    # v2 Schema: Multi-algorithm learning engine
    PATTERN_COUNT=$(echo "$INTEL" | jq -r '[.learning.qTables // {} | to_entries[].value | to_entries | length] | add // 0' 2>/dev/null)
    ACTIVE_ALGOS=$(echo "$INTEL" | jq -r '[.learning.stats // {} | to_entries[] | select(.value.updates > 0)] | length' 2>/dev/null)
    TOTAL_ALGOS=$(echo "$INTEL" | jq -r '[.learning.stats // {} | keys] | length' 2>/dev/null)
    BEST_ALGO=$(echo "$INTEL" | jq -r '
      .learning.stats // {} | to_entries
      | map(select(.value.updates > 0))
      | sort_by(-.value.convergenceScore)
      | .[0].key // "none"
    ' 2>/dev/null)
    BEST_SCORE=$(echo "$INTEL" | jq -r ".learning.stats.\"$BEST_ALGO\".convergenceScore // 0" 2>/dev/null | awk '{printf "%.0f", $1 * 100}')
    TOTAL_UPDATES=$(echo "$INTEL" | jq -r '[.learning.stats // {} | to_entries[].value.updates] | add // 0' 2>/dev/null)
    MEMORY_COUNT=$(echo "$INTEL" | jq -r '.memory.entries | length // 0' 2>/dev/null)
    TRAJ_COUNT=$(echo "$INTEL" | jq -r '.learning.trajectories | length // 0' 2>/dev/null)
    ROUTING_ALGO=$(echo "$INTEL" | jq -r '.learning.configs."agent-routing".algorithm // "double-q"' 2>/dev/null)
    LEARNING_RATE=$(echo "$INTEL" | jq -r '.learning.configs."agent-routing".learningRate // 0.1' 2>/dev/null)
    EPSILON=$(echo "$INTEL" | jq -r '.learning.configs."agent-routing".epsilon // 0.1' 2>/dev/null)
    TOP_AGENTS=$(echo "$INTEL" | jq -r '
      .learning.qTables // {} | to_entries |
      map(.value | to_entries | sort_by(-.value) | .[0] | select(.value > 0)) |
      map(.key) | unique | .[0:3] | join(", ")
    ' 2>/dev/null)
    SCHEMA="v2"
  else
    # v1 Schema: Simple patterns/memories
    PATTERN_COUNT=$(echo "$INTEL" | jq -r '.patterns | length // 0' 2>/dev/null)
    MEMORY_COUNT=$(echo "$INTEL" | jq -r '.memories | length // 0' 2>/dev/null)
    TRAJ_COUNT=$(echo "$INTEL" | jq -r '.trajectories | length // 0' 2>/dev/null)
    ACTIVE_ALGOS=0
    TOTAL_ALGOS=0
    BEST_ALGO="none"
    BEST_SCORE=0
    TOTAL_UPDATES=0
    ROUTING_ALGO="q-learning"
    LEARNING_RATE="0.1"
    EPSILON="0.1"
    TOP_AGENTS=""
    SCHEMA="v1"
  fi

  # Common fields (both schemas)
  ERROR_COUNT=$(echo "$INTEL" | jq -r '.errors | length // 0' 2>/dev/null)
  SESSION_COUNT=$(echo "$INTEL" | jq -r '.stats.session_count // 0' 2>/dev/null)
  FILE_SEQ_COUNT=$(echo "$INTEL" | jq -r '.file_sequences | length // 0' 2>/dev/null)
  AGENT_COUNT=$(echo "$INTEL" | jq -r '.agents | keys | length // 0' 2>/dev/null)

  # Build Line 2
  printf "${MAGENTA}🧠 RuVector${RESET}"

  # Patterns learned
  if [ "$PATTERN_COUNT" != "null" ] && [ "$PATTERN_COUNT" -gt 0 ]; then
    printf " ${GREEN}◆${RESET} ${PATTERN_COUNT} patterns"
  else
    printf " ${DIM}◇ learning${RESET}"
  fi

  # Active algorithms
  if [ "$ACTIVE_ALGOS" != "null" ] && [ "$ACTIVE_ALGOS" -gt 0 ]; then
    printf " ${CYAN}⚙${RESET} ${ACTIVE_ALGOS}/${TOTAL_ALGOS} algos"
  fi

  # Best algorithm with convergence
  if [ "$BEST_ALGO" != "none" ] && [ "$BEST_ALGO" != "null" ]; then
    # Shorten algorithm name
    case "$BEST_ALGO" in
      "double-q") SHORT_ALGO="DQ" ;;
      "q-learning") SHORT_ALGO="QL" ;;
      "actor-critic") SHORT_ALGO="AC" ;;
      "decision-transformer") SHORT_ALGO="DT" ;;
      "monte-carlo") SHORT_ALGO="MC" ;;
      "td-lambda") SHORT_ALGO="TD" ;;
      *) SHORT_ALGO="${BEST_ALGO:0:3}" ;;
    esac

    # Color based on convergence
    if [ "$BEST_SCORE" -ge 80 ]; then
      SCORE_COLOR="$GREEN"
    elif [ "$BEST_SCORE" -ge 50 ]; then
      SCORE_COLOR="$YELLOW"
    else
      SCORE_COLOR="$RED"
    fi
    printf " ${SCORE_COLOR}★${SHORT_ALGO}:${BEST_SCORE}%%${RESET}"
  fi

  # Memory entries
  if [ "$MEMORY_COUNT" != "null" ] && [ "$MEMORY_COUNT" -gt 0 ]; then
    printf " ${BLUE}⬡${RESET} ${MEMORY_COUNT} mem"
  fi

  # Trajectories
  if [ "$TRAJ_COUNT" != "null" ] && [ "$TRAJ_COUNT" -gt 0 ]; then
    printf " ${YELLOW}↝${RESET}${TRAJ_COUNT}"
  fi

  # Error fixes available
  if [ "$ERROR_COUNT" != "null" ] && [ "$ERROR_COUNT" -gt 0 ]; then
    printf " ${RED}🔧${RESET}${ERROR_COUNT}"
  fi

  # Sessions
  if [ "$SESSION_COUNT" != "null" ] && [ "$SESSION_COUNT" -gt 0 ]; then
    printf " ${DIM}#${SESSION_COUNT}${RESET}"
  fi

  echo

  # ═══════════════════════════════════════════════════════════════════════════════
  # LINE 3: Agent Routing & Session Performance
  # ═══════════════════════════════════════════════════════════════════════════════

  # Compression stats (v2 only)
  COMPRESSION=$(echo "$INTEL" | jq -r '.tensorCompress.compressionRatio // 0' 2>/dev/null | awk '{printf "%.0f", $1 * 100}')

  printf "${BLUE}🎯 Routing${RESET}"

  # Show routing algorithm
  case "$ROUTING_ALGO" in
    "double-q") ALGO_ICON="⚡DQ" ;;
    "sarsa") ALGO_ICON="🔄SA" ;;
    "actor-critic") ALGO_ICON="🎭AC" ;;
    *) ALGO_ICON="$ROUTING_ALGO" ;;
  esac
  printf " ${CYAN}${ALGO_ICON}${RESET}"

  # Learning rate
  LR_PCT=$(echo "$LEARNING_RATE" | awk '{printf "%.0f", $1 * 100}')
  printf " lr:${LR_PCT}%%"

  # Exploration rate
  EPS_PCT=$(echo "$EPSILON" | awk '{printf "%.0f", $1 * 100}')
  printf " ε:${EPS_PCT}%%"

  # Top learned agents
  if [ -n "$TOP_AGENTS" ] && [ "$TOP_AGENTS" != "null" ] && [ "$TOP_AGENTS" != "" ]; then
    printf " ${GREEN}→${RESET} ${TOP_AGENTS}"
  fi

  # Session info
  if [ "$TOTAL_UPDATES" != "null" ] && [ "$TOTAL_UPDATES" -gt 0 ]; then
    printf " ${DIM}│${RESET} ${YELLOW}↻${RESET}${TOTAL_UPDATES}"
  fi

  # Compression ratio
  if [ "$COMPRESSION" != "null" ] && [ "$COMPRESSION" -gt 0 ]; then
    printf " ${MAGENTA}◊${RESET}${COMPRESSION}%%"
  fi

  # File sequences learned
  if [ "$FILE_SEQ_COUNT" != "null" ] && [ "$FILE_SEQ_COUNT" -gt 0 ]; then
    printf " ${CYAN}📂${RESET}${FILE_SEQ_COUNT}"
  fi

  # Agents learned
  if [ "$AGENT_COUNT" != "null" ] && [ "$AGENT_COUNT" -gt 0 ]; then
    printf " ${GREEN}🤖${RESET}${AGENT_COUNT}"
  fi

  echo

else
  # No intelligence file - show initialization hint
  printf "${DIM}🧠 RuVector: run 'npx ruvector hooks session-start' to initialize${RESET}\n"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# LINE 4: Claude Flow Integration (only if meaningful data exists)
# ═══════════════════════════════════════════════════════════════════════════════
FLOW_DIR="$CWD/.claude-flow"
FLOW_OUTPUT=""

if [ -d "$FLOW_DIR" ]; then
  # Swarm config
  if [ -f "$FLOW_DIR/swarm-config.json" ]; then
    STRATEGY=$(jq -r '.defaultStrategy // empty' "$FLOW_DIR/swarm-config.json" 2>/dev/null)
    AGENT_COUNT=$(jq -r '.agentProfiles | length' "$FLOW_DIR/swarm-config.json" 2>/dev/null)

    if [ -n "$STRATEGY" ]; then
      case "$STRATEGY" in
        "balanced") TOPO="mesh" ;;
        "conservative") TOPO="hier" ;;
        "aggressive") TOPO="ring" ;;
        *) TOPO="$STRATEGY" ;;
      esac
      FLOW_OUTPUT="${FLOW_OUTPUT} ${MAGENTA}${TOPO}${RESET}"
    fi

    if [ -n "$AGENT_COUNT" ] && [ "$AGENT_COUNT" != "null" ] && [ "$AGENT_COUNT" -gt 0 ]; then
      FLOW_OUTPUT="${FLOW_OUTPUT} ${CYAN}🤖${AGENT_COUNT}${RESET}"
    fi
  fi

  # Active tasks
  if [ -d "$FLOW_DIR/tasks" ]; then
    TASK_COUNT=$(find "$FLOW_DIR/tasks" -name "*.json" -type f 2>/dev/null | wc -l)
    if [ "$TASK_COUNT" -gt 0 ]; then
      FLOW_OUTPUT="${FLOW_OUTPUT} ${YELLOW}📋${TASK_COUNT}${RESET}"
    fi
  fi

  # Session state
  if [ -f "$FLOW_DIR/session-state.json" ]; then
    ACTIVE=$(jq -r '.active // false' "$FLOW_DIR/session-state.json" 2>/dev/null)
    if [ "$ACTIVE" = "true" ]; then
      FLOW_OUTPUT="${FLOW_OUTPUT} ${GREEN}●${RESET}"
    fi
  fi

  # Only print if we have content
  if [ -n "$FLOW_OUTPUT" ]; then
    printf "${DIM}⚡ Flow:${RESET}${FLOW_OUTPUT}\n"
  fi
fi

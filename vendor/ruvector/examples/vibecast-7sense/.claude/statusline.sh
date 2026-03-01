#!/bin/bash
# 7sense Bioacoustics Platform - Development Status Line
# Shows DDD bounded contexts, ADR compliance, and pipeline status

# Read Claude Code JSON input from stdin (if available)
CLAUDE_INPUT=$(cat 2>/dev/null || echo "{}")

# Get project directory from Claude Code input or use current directory
PROJECT_DIR=$(echo "$CLAUDE_INPUT" | jq -r '.workspace.project_dir // ""' 2>/dev/null)
if [ -z "$PROJECT_DIR" ] || [ "$PROJECT_DIR" = "null" ]; then
  PROJECT_DIR=$(pwd)
fi

# File paths relative to project directory
DDD_METRICS="${PROJECT_DIR}/.claude-flow/metrics/ddd-progress.json"
ADR_METRICS="${PROJECT_DIR}/.claude-flow/metrics/adr-compliance.json"
SECURITY_AUDIT="${PROJECT_DIR}/.claude-flow/security/audit-status.json"
PERFORMANCE_METRICS="${PROJECT_DIR}/.claude-flow/metrics/performance.json"

# ANSI Color Codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[0;37m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

# Bright colors
BRIGHT_RED='\033[1;31m'
BRIGHT_GREEN='\033[1;32m'
BRIGHT_YELLOW='\033[1;33m'
BRIGHT_BLUE='\033[1;34m'
BRIGHT_PURPLE='\033[1;35m'
BRIGHT_CYAN='\033[1;36m'

# 7sense Architecture Targets
CONTEXTS_TOTAL=6      # Audio, Embedding, Vector, Learning, Analysis, Interpretation
ADRS_TOTAL=9          # ADR-001 through ADR-009
AGENTS_TARGET=12
PERF_TARGET="150x"    # HNSW search improvement target

# Default values
CONTEXTS_COMPLETED=0
ADR_COMPLIANCE=0
ADRS_COMPLIANT=0
AGENTS_ACTIVE=0
DDD_PROGRESS=0
HNSW_SPEEDUP="--"
SECURITY_STATUS="PENDING"

# Get current git branch
GIT_BRANCH=""
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  GIT_BRANCH=$(git branch --show-current 2>/dev/null || echo "")
fi

# Get GitHub username
GH_USER=""
if command -v gh >/dev/null 2>&1; then
  GH_USER=$(gh api user --jq '.login' 2>/dev/null || echo "")
fi
if [ -z "$GH_USER" ]; then
  GH_USER=$(git config user.name 2>/dev/null || echo "dev")
fi

# Check DDD bounded context progress
if [ -f "$DDD_METRICS" ]; then
  CONTEXTS_COMPLETED=$(jq -r '.completed // 0' "$DDD_METRICS" 2>/dev/null || echo "0")
  DDD_PROGRESS=$(jq -r '.progress // 0' "$DDD_METRICS" 2>/dev/null || echo "0")
else
  # Check for actual domain directories (crates or src/domains)
  CONTEXTS_COMPLETED=0
  for ctx in audio embedding vector learning analysis interpretation; do
    [ -d "$PROJECT_DIR/crates/sevensense-$ctx" ] && ((CONTEXTS_COMPLETED++)) && continue
    [ -d "$PROJECT_DIR/src/domains/$ctx" ] && ((CONTEXTS_COMPLETED++))
  done
fi

# Check ADR compliance
if [ -f "$ADR_METRICS" ]; then
  ADR_COMPLIANCE=$(jq -r '.overallCompliance // 0' "$ADR_METRICS" 2>/dev/null || echo "0")
  ADRS_COMPLIANT=$(jq -r '.compliantCount // 0' "$ADR_METRICS" 2>/dev/null || echo "0")
fi

# Check security status
if [ -f "$SECURITY_AUDIT" ]; then
  SECURITY_STATUS=$(jq -r '.status // "PENDING"' "$SECURITY_AUDIT" 2>/dev/null || echo "PENDING")
fi

# Check performance metrics (HNSW speedup)
if [ -f "$PERFORMANCE_METRICS" ]; then
  HNSW_SPEEDUP=$(jq -r '.hnsw.speedup // "--"' "$PERFORMANCE_METRICS" 2>/dev/null || echo "--")
fi

# Real-time swarm detection
ACTIVE_PROCESSES=$(ps aux 2>/dev/null | grep -E "(agentic-flow|claude-flow)" | grep -v grep | wc -l)
SWARM_ACTIVITY="${PROJECT_DIR}/.claude-flow/metrics/swarm-activity.json"
if [ -f "$SWARM_ACTIVITY" ]; then
  DYNAMIC_AGENTS=$(jq -r '.swarm.agent_count // 0' "$SWARM_ACTIVITY" 2>/dev/null || echo "0")
  [ "$DYNAMIC_AGENTS" -gt 0 ] && AGENTS_ACTIVE="$DYNAMIC_AGENTS"
elif [ "$ACTIVE_PROCESSES" -gt 0 ]; then
  AGENTS_ACTIVE=$((ACTIVE_PROCESSES / 2))
  [ "$AGENTS_ACTIVE" -eq 0 ] && AGENTS_ACTIVE=1
fi

# Context window usage
CONTEXT_PCT=0
CONTEXT_COLOR="${DIM}"
if [ "$CLAUDE_INPUT" != "{}" ]; then
  CONTEXT_REMAINING=$(echo "$CLAUDE_INPUT" | jq '.context_window.remaining_percentage // null' 2>/dev/null)
  if [ "$CONTEXT_REMAINING" != "null" ] && [ -n "$CONTEXT_REMAINING" ]; then
    CONTEXT_PCT=$((100 - CONTEXT_REMAINING))
  fi
  if [ "$CONTEXT_PCT" -lt 50 ]; then
    CONTEXT_COLOR="${BRIGHT_GREEN}"
  elif [ "$CONTEXT_PCT" -lt 75 ]; then
    CONTEXT_COLOR="${BRIGHT_YELLOW}"
  else
    CONTEXT_COLOR="${BRIGHT_RED}"
  fi
fi

# Intelligence score from learning patterns
INTEL_SCORE=0
INTEL_COLOR="${DIM}"
PATTERNS_DB="${PROJECT_DIR}/.claude-flow/learning/patterns.db"
if [ -f "$PATTERNS_DB" ] && command -v sqlite3 &>/dev/null; then
  PATTERN_COUNT=$(sqlite3 "$PATTERNS_DB" "SELECT COUNT(*) FROM short_term_patterns" 2>/dev/null || echo "0")
  INTEL_SCORE=$((PATTERN_COUNT * 10))
  [ "$INTEL_SCORE" -gt 100 ] && INTEL_SCORE=100
fi
if [ "$INTEL_SCORE" -lt 25 ]; then
  INTEL_COLOR="${DIM}"
elif [ "$INTEL_SCORE" -lt 50 ]; then
  INTEL_COLOR="${YELLOW}"
elif [ "$INTEL_SCORE" -lt 75 ]; then
  INTEL_COLOR="${BRIGHT_CYAN}"
else
  INTEL_COLOR="${BRIGHT_GREEN}"
fi

# Domain status indicators (6 bounded contexts)
COMPLETED_CTX="${BRIGHT_GREEN}●${RESET}"
PENDING_CTX="${DIM}○${RESET}"
CTX_STATUS="${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}"

case $CONTEXTS_COMPLETED in
  1) CTX_STATUS="${COMPLETED_CTX}${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}" ;;
  2) CTX_STATUS="${COMPLETED_CTX}${COMPLETED_CTX}${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}" ;;
  3) CTX_STATUS="${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}${PENDING_CTX}${PENDING_CTX}${PENDING_CTX}" ;;
  4) CTX_STATUS="${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}${PENDING_CTX}${PENDING_CTX}" ;;
  5) CTX_STATUS="${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}${PENDING_CTX}" ;;
  6) CTX_STATUS="${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}${COMPLETED_CTX}" ;;
esac

# Security status color
SECURITY_ICON="🔴"
SECURITY_COLOR="${BRIGHT_RED}"
if [ "$SECURITY_STATUS" = "CLEAN" ]; then
  SECURITY_ICON="🟢"
  SECURITY_COLOR="${BRIGHT_GREEN}"
elif [ "$SECURITY_STATUS" = "AUDIT" ]; then
  SECURITY_ICON="🟡"
  SECURITY_COLOR="${BRIGHT_YELLOW}"
fi

# ADR compliance color
ADR_COLOR="${BRIGHT_GREEN}"
if [ "$ADR_COMPLIANCE" -lt 50 ]; then
  ADR_COLOR="${RED}"
elif [ "$ADR_COMPLIANCE" -lt 75 ]; then
  ADR_COLOR="${YELLOW}"
fi

# Swarm status color
AGENTS_COLOR="${BRIGHT_GREEN}"
if [ "$AGENTS_ACTIVE" -lt 4 ]; then
  AGENTS_COLOR="${YELLOW}"
fi
if [ "$AGENTS_ACTIVE" -eq 0 ]; then
  AGENTS_COLOR="${DIM}"
fi

# Activity indicator
ACTIVITY_INDICATOR="${DIM}○${RESET}"
if [ "$ACTIVE_PROCESSES" -gt 0 ]; then
  ACTIVITY_INDICATOR="${BRIGHT_GREEN}◉${RESET}"
fi

# Model name from Claude Code input
MODEL_NAME=""
if [ "$CLAUDE_INPUT" != "{}" ]; then
  MODEL_NAME=$(echo "$CLAUDE_INPUT" | jq -r '.model.display_name // ""' 2>/dev/null)
fi

# Memory display
MEMORY_DISPLAY="--"
NODE_MEM=$(ps aux 2>/dev/null | grep -E "(node|claude)" | grep -v grep | awk '{sum += $6} END {print int(sum/1024)}')
if [ -n "$NODE_MEM" ] && [ "$NODE_MEM" -gt 0 ]; then
  MEMORY_DISPLAY="${NODE_MEM}MB"
fi

# Format values with padding
CONTEXT_DISPLAY=$(printf "%3d" "$CONTEXT_PCT")
INTEL_DISPLAY=$(printf "%3d" "$INTEL_SCORE")
AGENT_DISPLAY=$(printf "%2d" "$AGENTS_ACTIVE")
ADR_DISPLAY=$(printf "%3d" "$ADR_COMPLIANCE")
DDD_DISPLAY=$(printf "%3d" "$DDD_PROGRESS")

# Build output
OUTPUT=""

# Header: 7sense + Branch + User
OUTPUT="${BOLD}${BRIGHT_CYAN}▊ 7sense${RESET} ${DIM}bioacoustics${RESET}"
OUTPUT="${OUTPUT}  ${BRIGHT_PURPLE}${GH_USER}${RESET}"
if [ -n "$GIT_BRANCH" ]; then
  OUTPUT="${OUTPUT}  ${DIM}│${RESET}  ${BRIGHT_BLUE}⎇ ${GIT_BRANCH}${RESET}"
fi
if [ -n "$MODEL_NAME" ]; then
  OUTPUT="${OUTPUT}  ${DIM}│${RESET}  ${PURPLE}${MODEL_NAME}${RESET}"
fi

# Separator
OUTPUT="${OUTPUT}\n${DIM}──────────────────────────────────────────────────────────${RESET}"

# Line 1: DDD Bounded Contexts (6 total)
DDD_COLOR="${BRIGHT_GREEN}"
[ "$DDD_PROGRESS" -lt 50 ] && DDD_COLOR="${YELLOW}"
[ "$DDD_PROGRESS" -eq 0 ] && DDD_COLOR="${RED}"

OUTPUT="${OUTPUT}\n${BRIGHT_CYAN}🎵 DDD Contexts${RESET}   [${CTX_STATUS}]  ${DDD_COLOR}${CONTEXTS_COMPLETED}${RESET}/${BRIGHT_WHITE}${CONTEXTS_TOTAL}${RESET}"
OUTPUT="${OUTPUT}    ${CYAN}ADR${RESET} ${ADR_COLOR}${ADR_DISPLAY}%${RESET} (${ADRS_COMPLIANT}/${ADRS_TOTAL})"

# Line 2: Swarm + Performance + Security
OUTPUT="${OUTPUT}\n${BRIGHT_YELLOW}🐝 Swarm${RESET}  ${ACTIVITY_INDICATOR}[${AGENTS_COLOR}${AGENT_DISPLAY}${RESET}/${BRIGHT_WHITE}${AGENTS_TARGET}${RESET}]"
OUTPUT="${OUTPUT}    ${CYAN}HNSW${RESET} ${BRIGHT_GREEN}${HNSW_SPEEDUP}${RESET}→${BRIGHT_YELLOW}${PERF_TARGET}${RESET}"
OUTPUT="${OUTPUT}    ${SECURITY_ICON} ${SECURITY_COLOR}${SECURITY_STATUS}${RESET}"
OUTPUT="${OUTPUT}    ${CONTEXT_COLOR}📂 ${CONTEXT_DISPLAY}%${RESET}"
OUTPUT="${OUTPUT}    ${INTEL_COLOR}🧠 ${INTEL_DISPLAY}%${RESET}"

# Line 3: Architecture Components
OUTPUT="${OUTPUT}\n${BRIGHT_PURPLE}🔧 Pipeline${RESET}  ${DIM}Audio→Mel→Perch→HNSW→GNN→RAB${RESET}"
OUTPUT="${OUTPUT}    ${CYAN}Mem${RESET} ${BRIGHT_CYAN}${MEMORY_DISPLAY}${RESET}"

# Footer separator
OUTPUT="${OUTPUT}\n${DIM}──────────────────────────────────────────────────────────${RESET}"

printf "%b\n" "$OUTPUT"

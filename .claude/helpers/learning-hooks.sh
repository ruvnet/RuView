#!/bin/bash
# Claude Flow V3 - Learning Hooks
# Integrates learning-service.mjs with session lifecycle

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LEARNING_SERVICE="$SCRIPT_DIR/learning-service.mjs"
LEARNING_DIR="$PROJECT_ROOT/.claude-flow/learning"
METRICS_DIR="$PROJECT_ROOT/.claude-flow/metrics"

# Ensure directories exist
mkdir -p "$LEARNING_DIR" "$METRICS_DIR"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
DIM='\033[2m'
RESET='\033[0m'

log() { echo -e "${CYAN}[Learning] $1${RESET}"; }
success() { echo -e "${GREEN}[Learning] ✓ $1${RESET}"; }
warn() { echo -e "${YELLOW}[Learning] ⚠ $1${RESET}"; }
error() { echo -e "${RED}[Learning] ✗ $1${RESET}"; }

# Generate session ID
generate_session_id() {
  echo "session_$(date +%Y%m%d_%H%M%S)_$$"
}

# =============================================================================
# Session Start Hook
# =============================================================================
session_start() {
  local session_id="${1:-$(generate_session_id)}"

  log "Initializing learning service for session: $session_id"

  # Check if better-sqlite3 is available
  if ! npm list better-sqlite3 --prefix "$PROJECT_ROOT" >/dev/null 2>&1; then
    log "Installing better-sqlite3..."
    npm install --prefix "$PROJECT_ROOT" better-sqlite3 --save-dev --silent 2>/dev/null || true
  fi

  # Initialize learning service
  local init_result
  init_result=$(node "$LEARNING_SERVICE" init "$session_id" 2>&1)

  if [ $? -eq 0 ]; then
    # Parse and display stats
    local short_term=$(echo "$init_result" | grep -o '"shortTermPatterns":[0-9]*' | cut -d: -f2)
    local long_term=$(echo "$init_result" | grep -o '"longTermPatterns":[0-9]*' | cut -d: -f2)

    success "Learning service initialized"
    echo -e "  ${DIM}├─ Short-term patterns: ${short_term:-0}${RESET}"
    echo -e "  ${DIM}├─ Long-term patterns: ${long_term:-0}${RESET}"
    echo -e "  ${DIM}└─ Session ID: $session_id${RESET}"

    # Store session ID for later hooks
    echo "$session_id" > "$LEARNING_DIR/current-session-id"

    # Update metrics
    cat > "$METRICS_DIR/learning-status.json" << EOF
{
  "sessionId": "$session_id",
  "initialized": true,
  "shortTermPatterns": ${short_term:-0},
  "longTermPatterns": ${long_term:-0},
  "hnswEnabled": true,
  "timestamp": "$(date -Iseconds)"
}
EOF

    return 0
  else
    warn "Learning service initialization failed (non-critical)"
    echo "$init_result" | head -5
    return 1
  fi
}

# =============================================================================
# Session End Hook
# =============================================================================
session_end() {
  log "Consolidating learning data..."

  # Get session ID
  local session_id=""
  if [ -f "$LEARNING_DIR/current-session-id" ]; then
    session_id=$(cat "$LEARNING_DIR/current-session-id")
  fi

  # Export session data
  local export_result
  export_result=$(node "$LEARNING_SERVICE" export 2>&1)

  if [ $? -eq 0 ]; then
    # Save export
    echo "$export_result" > "$LEARNING_DIR/session-export-$(date +%Y%m%d_%H%M%S).json"

    local patterns=$(echo "$export_result" | grep -o '"patterns":[0-9]*' | cut -d: -f2)
    log "Session exported: $patterns patterns"
  fi

  # Run consolidation
  local consolidate_result
  consolidate_result=$(node "$LEARNING_SERVICE" consolidate 2>&1)

  if [ $? -eq 0 ]; then
    local removed=$(echo "$consolidate_result" | grep -o '"duplicatesRemoved":[0-9]*' | cut -d: -f2)
    local pruned=$(echo "$consolidate_result" | grep -o '"patternsProned":[0-9]*' | cut -d: -f2)
    local duration=$(echo "$consolidate_result" | grep -o '"durationMs":[0-9]*' | cut -d: -f2)

    success "Consolidation complete"
    echo -e "  ${DIM}├─ Duplicates removed: ${removed:-0}${RESET}"
    echo -e "  ${DIM}├─ Patterns pruned: ${pruned:-0}${RESET}"
    echo -e "  ${DIM}└─ Duration: ${duration:-0}ms${RESET}"
  else
    warn "Consolidation failed (non-critical)"
  fi

  # Get final stats
  local stats_result
  stats_result=$(node "$LEARNING_SERVICE" stats 2>&1)

  if [ $? -eq 0 ]; then
    echo "$stats_result" > "$METRICS_DIR/learning-final-stats.json"

    local total_short=$(echo "$stats_result" | grep -o '"shortTermPatterns":[0-9]*' | cut -d: -f2)
    local total_long=$(echo "$stats_result" | grep -o '"longTermPatterns":[0-9]*' | cut -d: -f2)
    local avg_search=$(echo "$stats_result" | grep -o '"avgSearchTimeMs":[0-9.]*' | cut -d: -f2)

    log "Final stats:"
    echo -e "  ${DIM}├─ Short-term: ${total_short:-0}${RESET}"
    echo -e "  ${DIM}├─ Long-term: ${total_long:-0}${RESET}"
    echo -e "  ${DIM}└─ Avg search: ${avg_search:-0}ms${RESET}"
  fi

  # Clean up session file
  rm -f "$LEARNING_DIR/current-session-id"

  return 0
}

# =============================================================================
# Store Pattern (called by post-edit hooks)
# =============================================================================
store_pattern() {
  local strategy="$1"
  local domain="${2:-general}"
  local quality="${3:-0.7}"

  if [ -z "$strategy" ]; then
    error "No strategy provided"
    return 1
  fi

  # Escape quotes in strategy
  local escaped_strategy="${strategy//\"/\\\"}"

  local result
  result=$(node "$LEARNING_SERVICE" store "$escaped_strategy" "$domain" 2>&1)

  if [ $? -eq 0 ]; then
    local action=$(echo "$result" | grep -o '"action":"[^"]*"' | cut -d'"' -f4)
    local id=$(echo "$result" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

    if [ "$action" = "created" ]; then
      success "Pattern stored: $id"
    else
      log "Pattern updated: $id"
    fi
    return 0
  else
    warn "Pattern storage failed"
    return 1
  fi
}

# =============================================================================
# Search Patterns (called by pre-edit hooks)
# =============================================================================
search_patterns() {
  local query="$1"
  local k="${2:-3}"

  if [ -z "$query" ]; then
    error "No query provided"
    return 1
  fi

  # Escape quotes
  local escaped_query="${query//\"/\\\"}"

  local result
  result=$(node "$LEARNING_SERVICE" search "$escaped_query" "$k" 2>&1)

  if [ $? -eq 0 ]; then
    local patterns=$(echo "$result" | grep -o '"patterns":\[' | wc -l)
    local search_time=$(echo "$result" | grep -o '"searchTimeMs":[0-9.]*' | cut -d: -f2)

    echo "$result"

    if [ -n "$search_time" ]; then
      log "Search completed in ${search_time}ms"
    fi
    return 0
  else
    warn "Pattern search failed"
    return 1
  fi
}

# =============================================================================
# Record Pattern Usage (for promotion tracking)
# =============================================================================
record_usage() {
  local pattern_id="$1"
  local success="${2:-true}"

  if [ -z "$pattern_id" ]; then
    return 1
  fi

  # This would call into the learning service to record usage
  # For now, log it
  log "Recording usage: $pattern_id (success=$success)"
}

# =============================================================================
# Run Benchmark
# =============================================================================
run_benchmark() {
  log "Running HNSW benchmark..."

  local result
  result=$(node "$LEARNING_SERVICE" benchmark 2>&1)

  if [ $? -eq 0 ]; then
    local avg_search=$(echo "$result" | grep -o '"avgSearchMs":"[^"]*"' | cut -d'"' -f4)
    local p95_search=$(echo "$result" | grep -o '"p95SearchMs":"[^"]*"' | cut -d'"' -f4)
    local improvement=$(echo "$result" | grep -o '"searchImprovementEstimate":"[^"]*"' | cut -d'"' -f4)

    success "HNSW Benchmark Complete"
    echo -e "  ${DIM}├─ Avg search: ${avg_search}ms${RESET}"
    echo -e "  ${DIM}├─ P95 search: ${p95_search}ms${RESET}"
    echo -e "  ${DIM}└─ Estimated improvement: ${improvement}${RESET}"

    echo "$result"
    return 0
  else
    error "Benchmark failed"
    echo "$result"
    return 1
  fi
}

# =============================================================================
# Get Stats
# =============================================================================
get_stats() {
  local result
  result=$(node "$LEARNING_SERVICE" stats 2>&1)

  if [ $? -eq 0 ]; then
    echo "$result"
    return 0
  else
    error "Failed to get stats"
    return 1
  fi
}

# =============================================================================
# Main
# =============================================================================
case "${1:-help}" in
  "session-start"|"start")
    session_start "$2"
    ;;
  "session-end"|"end")
    session_end
    ;;
  "store")
    store_pattern "$2" "$3" "$4"
    ;;
  "search")
    search_patterns "$2" "$3"
    ;;
  "record-usage"|"usage")
    record_usage "$2" "$3"
    ;;
  "benchmark")
    run_benchmark
    ;;
  "stats")
    get_stats
    ;;
  "help"|"-h"|"--help")
    cat << 'EOF'
Claude Flow V3 Learning Hooks

Usage: learning-hooks.sh <command> [args]

Commands:
  session-start [id]    Initialize learning for new session
  session-end           Consolidate and export session data
  store <strategy>      Store a new pattern
  search <query> [k]    Search for similar patterns
  record-usage <id>     Record pattern usage
  benchmark             Run HNSW performance benchmark
  stats                 Get learning statistics
  help                  Show this help

Examples:
  ./learning-hooks.sh session-start
  ./learning-hooks.sh store "Fix authentication bug" code
  ./learning-hooks.sh search "authentication error" 5
  ./learning-hooks.sh session-end
EOF
    ;;
  *)
    error "Unknown command: $1"
    echo "Use 'learning-hooks.sh help' for usage"
    exit 1
    ;;
esac

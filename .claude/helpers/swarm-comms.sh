#!/bin/bash
# Claude Flow V3 - Optimized Swarm Communications
# Non-blocking, batched, priority-based inter-agent messaging

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SWARM_DIR="$PROJECT_ROOT/.claude-flow/swarm"
QUEUE_DIR="$SWARM_DIR/queue"
BATCH_DIR="$SWARM_DIR/batch"
POOL_FILE="$SWARM_DIR/connection-pool.json"

mkdir -p "$QUEUE_DIR" "$BATCH_DIR"

# Priority levels
PRIORITY_CRITICAL=0
PRIORITY_HIGH=1
PRIORITY_NORMAL=2
PRIORITY_LOW=3

# Batch settings
BATCH_SIZE=10
BATCH_TIMEOUT_MS=100

# =============================================================================
# NON-BLOCKING MESSAGE QUEUE
# =============================================================================

# Enqueue message (instant return, async processing)
enqueue() {
  local to="${1:-*}"
  local content="${2:-}"
  local priority="${3:-$PRIORITY_NORMAL}"
  local msg_type="${4:-context}"

  local msg_id="msg_$(date +%s%N)"
  local timestamp=$(date +%s)

  # Write to priority queue (non-blocking)
  cat > "$QUEUE_DIR/${priority}_${msg_id}.json" << EOF
{"id":"$msg_id","to":"$to","content":"$content","type":"$msg_type","priority":$priority,"timestamp":$timestamp}
EOF

  echo "$msg_id"
}

# Process queue in background
process_queue() {
  local processed=0

  # Process by priority (0=critical first)
  for priority in 0 1 2 3; do
    shopt -s nullglob
    for msg_file in "$QUEUE_DIR"/${priority}_*.json; do
      [ -f "$msg_file" ] || continue

      # Process message
      local msg=$(cat "$msg_file")
      local to=$(echo "$msg" | jq -r '.to' 2>/dev/null)

      # Route to agent mailbox
      if [ "$to" != "*" ]; then
        mkdir -p "$SWARM_DIR/mailbox/$to"
        mv "$msg_file" "$SWARM_DIR/mailbox/$to/"
      else
        # Broadcast - copy to all agent mailboxes
        for agent_dir in "$SWARM_DIR/mailbox"/*; do
          [ -d "$agent_dir" ] && cp "$msg_file" "$agent_dir/"
        done
        rm "$msg_file"
      fi

      processed=$((processed + 1))
    done
  done

  echo "$processed"
}

# =============================================================================
# MESSAGE BATCHING
# =============================================================================

# Add to batch (collects messages, flushes when full or timeout)
batch_add() {
  local agent_id="${1:-}"
  local content="${2:-}"
  local batch_file="$BATCH_DIR/${agent_id}.batch"

  # Append to batch
  echo "$content" >> "$batch_file"

  # Check batch size
  local count=$(wc -l < "$batch_file" 2>/dev/null || echo "0")

  if [ "$count" -ge "$BATCH_SIZE" ]; then
    batch_flush "$agent_id"
  fi
}

# Flush batch (send all at once)
batch_flush() {
  local agent_id="${1:-}"
  local batch_file="$BATCH_DIR/${agent_id}.batch"

  if [ -f "$batch_file" ]; then
    local content=$(cat "$batch_file")
    rm "$batch_file"

    # Send as single batched message
    enqueue "$agent_id" "$content" "$PRIORITY_NORMAL" "batch"
  fi
}

# Flush all pending batches
batch_flush_all() {
  shopt -s nullglob
  for batch_file in "$BATCH_DIR"/*.batch; do
    [ -f "$batch_file" ] || continue
    local agent_id=$(basename "$batch_file" .batch)
    batch_flush "$agent_id"
  done
}

# =============================================================================
# CONNECTION POOLING
# =============================================================================

# Initialize connection pool
pool_init() {
  cat > "$POOL_FILE" << EOF
{
  "maxConnections": 10,
  "activeConnections": 0,
  "available": [],
  "inUse": [],
  "lastUpdated": "$(date -Iseconds)"
}
EOF
}

# Get connection from pool (or create new)
pool_acquire() {
  local agent_id="${1:-}"

  if [ ! -f "$POOL_FILE" ]; then
    pool_init
  fi

  # Check for available connection
  local available=$(jq -r '.available[0] // ""' "$POOL_FILE" 2>/dev/null)

  if [ -n "$available" ]; then
    # Reuse existing connection
    jq ".available = .available[1:] | .inUse += [\"$available\"]" "$POOL_FILE" > "$POOL_FILE.tmp" && mv "$POOL_FILE.tmp" "$POOL_FILE"
    echo "$available"
  else
    # Create new connection ID
    local conn_id="conn_$(date +%s%N | tail -c 8)"
    jq ".inUse += [\"$conn_id\"] | .activeConnections += 1" "$POOL_FILE" > "$POOL_FILE.tmp" && mv "$POOL_FILE.tmp" "$POOL_FILE"
    echo "$conn_id"
  fi
}

# Release connection back to pool
pool_release() {
  local conn_id="${1:-}"

  if [ -f "$POOL_FILE" ]; then
    jq ".inUse = (.inUse | map(select(. != \"$conn_id\"))) | .available += [\"$conn_id\"]" "$POOL_FILE" > "$POOL_FILE.tmp" && mv "$POOL_FILE.tmp" "$POOL_FILE"
  fi
}

# =============================================================================
# ASYNC PATTERN BROADCAST
# =============================================================================

# Broadcast pattern to swarm (non-blocking)
broadcast_pattern_async() {
  local strategy="${1:-}"
  local domain="${2:-general}"
  local quality="${3:-0.7}"

  # Fire and forget
  (
    local broadcast_id="pattern_$(date +%s%N)"

    # Write pattern broadcast
    mkdir -p "$SWARM_DIR/patterns"
    cat > "$SWARM_DIR/patterns/$broadcast_id.json" << EOF
{"id":"$broadcast_id","strategy":"$strategy","domain":"$domain","quality":$quality,"timestamp":$(date +%s),"status":"pending"}
EOF

    # Notify all agents via queue
    enqueue "*" "{\"type\":\"pattern_broadcast\",\"id\":\"$broadcast_id\"}" "$PRIORITY_HIGH" "event"

  ) &

  echo "pattern_broadcast_queued"
}

# =============================================================================
# OPTIMIZED CONSENSUS
# =============================================================================

# Start consensus (non-blocking)
start_consensus_async() {
  local question="${1:-}"
  local options="${2:-}"
  local timeout="${3:-30}"

  (
    local consensus_id="consensus_$(date +%s%N)"
    mkdir -p "$SWARM_DIR/consensus"

    cat > "$SWARM_DIR/consensus/$consensus_id.json" << EOF
{"id":"$consensus_id","question":"$question","options":"$options","votes":{},"timeout":$timeout,"created":$(date +%s),"status":"open"}
EOF

    # Notify agents
    enqueue "*" "{\"type\":\"consensus_request\",\"id\":\"$consensus_id\"}" "$PRIORITY_HIGH" "event"

    # Auto-resolve after timeout (background)
    (
      sleep "$timeout"
      if [ -f "$SWARM_DIR/consensus/$consensus_id.json" ]; then
        jq '.status = "resolved"' "$SWARM_DIR/consensus/$consensus_id.json" > "$SWARM_DIR/consensus/$consensus_id.json.tmp" && mv "$SWARM_DIR/consensus/$consensus_id.json.tmp" "$SWARM_DIR/consensus/$consensus_id.json"
      fi
    ) &

    echo "$consensus_id"
  ) &
}

# Vote on consensus (non-blocking)
vote_async() {
  local consensus_id="${1:-}"
  local vote="${2:-}"
  local agent_id="${AGENTIC_FLOW_AGENT_ID:-anonymous}"

  (
    local file="$SWARM_DIR/consensus/$consensus_id.json"
    if [ -f "$file" ]; then
      jq ".votes[\"$agent_id\"] = \"$vote\"" "$file" > "$file.tmp" && mv "$file.tmp" "$file"
    fi
  ) &
}

# =============================================================================
# PERFORMANCE METRICS
# =============================================================================

get_comms_stats() {
  local queued=$(ls "$QUEUE_DIR"/*.json 2>/dev/null | wc -l | tr -d '[:space:]')
  queued=${queued:-0}
  local batched=$(ls "$BATCH_DIR"/*.batch 2>/dev/null | wc -l | tr -d '[:space:]')
  batched=${batched:-0}
  local patterns=$(ls "$SWARM_DIR/patterns"/*.json 2>/dev/null | wc -l | tr -d '[:space:]')
  patterns=${patterns:-0}
  local consensus=$(ls "$SWARM_DIR/consensus"/*.json 2>/dev/null | wc -l | tr -d '[:space:]')
  consensus=${consensus:-0}

  local pool_active=0
  if [ -f "$POOL_FILE" ]; then
    pool_active=$(jq '.activeConnections // 0' "$POOL_FILE" 2>/dev/null | tr -d '[:space:]')
    pool_active=${pool_active:-0}
  fi

  echo "{\"queue\":$queued,\"batch\":$batched,\"patterns\":$patterns,\"consensus\":$consensus,\"pool\":$pool_active}"
}

# =============================================================================
# MAIN DISPATCHER
# =============================================================================

case "${1:-help}" in
  # Queue operations
  "enqueue"|"send")
    enqueue "${2:-*}" "${3:-}" "${4:-2}" "${5:-context}"
    ;;
  "process")
    process_queue
    ;;

  # Batch operations
  "batch")
    batch_add "${2:-}" "${3:-}"
    ;;
  "flush")
    batch_flush_all
    ;;

  # Pool operations
  "acquire")
    pool_acquire "${2:-}"
    ;;
  "release")
    pool_release "${2:-}"
    ;;

  # Async operations
  "broadcast-pattern")
    broadcast_pattern_async "${2:-}" "${3:-general}" "${4:-0.7}"
    ;;
  "consensus")
    start_consensus_async "${2:-}" "${3:-}" "${4:-30}"
    ;;
  "vote")
    vote_async "${2:-}" "${3:-}"
    ;;

  # Stats
  "stats")
    get_comms_stats
    ;;

  "help"|*)
    cat << 'EOF'
Claude Flow V3 - Optimized Swarm Communications

Non-blocking, batched, priority-based inter-agent messaging.

Usage: swarm-comms.sh <command> [args]

Queue (Non-blocking):
  enqueue <to> <content> [priority] [type]   Add to queue (instant return)
  process                                     Process pending queue

Batching:
  batch <agent> <content>                     Add to batch
  flush                                       Flush all batches

Connection Pool:
  acquire [agent]                             Get connection from pool
  release <conn_id>                           Return connection to pool

Async Operations:
  broadcast-pattern <strategy> [domain] [quality]   Async pattern broadcast
  consensus <question> <options> [timeout]          Start async consensus
  vote <consensus_id> <vote>                        Vote (non-blocking)

Stats:
  stats                                       Get communication stats

Priority Levels:
  0 = Critical (processed first)
  1 = High
  2 = Normal (default)
  3 = Low
EOF
    ;;
esac

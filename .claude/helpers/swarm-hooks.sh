#!/bin/bash
# Claude Flow V3 - Swarm Communication Hooks
# Enables agent-to-agent messaging, pattern sharing, consensus, and task handoffs
#
# Integration with:
# - @claude-flow/hooks SwarmCommunication module
# - agentic-flow@alpha swarm coordination
# - Local hooks system for real-time agent coordination
#
# Key mechanisms:
# - Exit 0 + stdout = Context added to Claude's view
# - Exit 2 + stderr = Block with explanation
# - JSON additionalContext = Swarm coordination messages

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SWARM_DIR="$PROJECT_ROOT/.claude-flow/swarm"
MESSAGES_DIR="$SWARM_DIR/messages"
PATTERNS_DIR="$SWARM_DIR/patterns"
CONSENSUS_DIR="$SWARM_DIR/consensus"
HANDOFFS_DIR="$SWARM_DIR/handoffs"
AGENTS_FILE="$SWARM_DIR/agents.json"
STATS_FILE="$SWARM_DIR/stats.json"

# Agent identity
AGENT_ID="${AGENTIC_FLOW_AGENT_ID:-agent_$(date +%s)_$(head -c 4 /dev/urandom | xxd -p)}"
AGENT_NAME="${AGENTIC_FLOW_AGENT_NAME:-claude-code}"

# Initialize directories
mkdir -p "$MESSAGES_DIR" "$PATTERNS_DIR" "$CONSENSUS_DIR" "$HANDOFFS_DIR"

# =============================================================================
# UTILITY FUNCTIONS
# =============================================================================

init_stats() {
  if [ ! -f "$STATS_FILE" ]; then
    cat > "$STATS_FILE" << EOF
{
  "messagesSent": 0,
  "messagesReceived": 0,
  "patternsBroadcast": 0,
  "consensusInitiated": 0,
  "consensusResolved": 0,
  "handoffsInitiated": 0,
  "handoffsCompleted": 0,
  "lastUpdated": "$(date -Iseconds)"
}
EOF
  fi
}

update_stat() {
  local key="$1"
  local increment="${2:-1}"
  init_stats

  if command -v jq &>/dev/null; then
    local current=$(jq -r ".$key // 0" "$STATS_FILE")
    local new=$((current + increment))
    jq ".$key = $new | .lastUpdated = \"$(date -Iseconds)\"" "$STATS_FILE" > "$STATS_FILE.tmp" && mv "$STATS_FILE.tmp" "$STATS_FILE"
  fi
}

register_agent() {
  init_stats
  local timestamp=$(date +%s)

  if [ ! -f "$AGENTS_FILE" ]; then
    echo '{"agents":[]}' > "$AGENTS_FILE"
  fi

  if command -v jq &>/dev/null; then
    # Check if agent already exists
    local exists=$(jq -r ".agents[] | select(.id == \"$AGENT_ID\") | .id" "$AGENTS_FILE" 2>/dev/null || echo "")

    if [ -z "$exists" ]; then
      jq ".agents += [{\"id\":\"$AGENT_ID\",\"name\":\"$AGENT_NAME\",\"status\":\"active\",\"lastSeen\":$timestamp}]" "$AGENTS_FILE" > "$AGENTS_FILE.tmp" && mv "$AGENTS_FILE.tmp" "$AGENTS_FILE"
    else
      # Update lastSeen
      jq "(.agents[] | select(.id == \"$AGENT_ID\")).lastSeen = $timestamp" "$AGENTS_FILE" > "$AGENTS_FILE.tmp" && mv "$AGENTS_FILE.tmp" "$AGENTS_FILE"
    fi
  fi
}

# =============================================================================
# AGENT-TO-AGENT MESSAGING
# =============================================================================

send_message() {
  local to="${1:-*}"
  local content="${2:-}"
  local msg_type="${3:-context}"
  local priority="${4:-normal}"

  local msg_id="msg_$(date +%s)_$(head -c 4 /dev/urandom | xxd -p)"
  local timestamp=$(date +%s)

  local msg_file="$MESSAGES_DIR/$msg_id.json"
  cat > "$msg_file" << EOF
{
  "id": "$msg_id",
  "from": "$AGENT_ID",
  "fromName": "$AGENT_NAME",
  "to": "$to",
  "type": "$msg_type",
  "content": $(echo "$content" | jq -Rs .),
  "priority": "$priority",
  "timestamp": $timestamp,
  "read": false
}
EOF

  update_stat "messagesSent"

  echo "$msg_id"
  exit 0
}

get_messages() {
  local limit="${1:-10}"
  local msg_type="${2:-}"

  register_agent

  local messages="[]"
  local count=0

  for msg_file in $(ls -t "$MESSAGES_DIR"/*.json 2>/dev/null | head -n "$limit"); do
    if [ -f "$msg_file" ]; then
      local to=$(jq -r '.to' "$msg_file" 2>/dev/null)

      # Check if message is for us or broadcast
      if [ "$to" = "$AGENT_ID" ] || [ "$to" = "*" ] || [ "$to" = "$AGENT_NAME" ]; then
        # Filter by type if specified
        if [ -n "$msg_type" ]; then
          local mtype=$(jq -r '.type' "$msg_file" 2>/dev/null)
          if [ "$mtype" != "$msg_type" ]; then
            continue
          fi
        fi

        if command -v jq &>/dev/null; then
          messages=$(echo "$messages" | jq ". += [$(cat "$msg_file")]")
          count=$((count + 1))

          # Mark as read
          jq '.read = true' "$msg_file" > "$msg_file.tmp" && mv "$msg_file.tmp" "$msg_file"
        fi
      fi
    fi
  done

  update_stat "messagesReceived" "$count"

  if command -v jq &>/dev/null; then
    echo "$messages" | jq -c "{count: $count, messages: .}"
  else
    echo "{\"count\": $count, \"messages\": []}"
  fi

  exit 0
}

broadcast_context() {
  local content="${1:-}"
  send_message "*" "$content" "context" "normal"
}

# =============================================================================
# PATTERN BROADCASTING
# =============================================================================

broadcast_pattern() {
  local strategy="${1:-}"
  local domain="${2:-general}"
  local quality="${3:-0.7}"

  local bc_id="bc_$(date +%s)_$(head -c 4 /dev/urandom | xxd -p)"
  local timestamp=$(date +%s)

  local bc_file="$PATTERNS_DIR/$bc_id.json"
  cat > "$bc_file" << EOF
{
  "id": "$bc_id",
  "sourceAgent": "$AGENT_ID",
  "sourceAgentName": "$AGENT_NAME",
  "pattern": {
    "strategy": $(echo "$strategy" | jq -Rs .),
    "domain": "$domain",
    "quality": $quality
  },
  "broadcastTime": $timestamp,
  "acknowledgments": []
}
EOF

  update_stat "patternsBroadcast"

  # Also store in learning hooks if available
  if [ -f "$SCRIPT_DIR/learning-hooks.sh" ]; then
    "$SCRIPT_DIR/learning-hooks.sh" store "$strategy" "$domain" "$quality" 2>/dev/null || true
  fi

  cat << EOF
{"broadcastId":"$bc_id","strategy":$(echo "$strategy" | jq -Rs .),"domain":"$domain","quality":$quality}
EOF

  exit 0
}

get_pattern_broadcasts() {
  local domain="${1:-}"
  local min_quality="${2:-0}"
  local limit="${3:-10}"

  local broadcasts="[]"
  local count=0

  for bc_file in $(ls -t "$PATTERNS_DIR"/*.json 2>/dev/null | head -n "$limit"); do
    if [ -f "$bc_file" ] && command -v jq &>/dev/null; then
      local bc_domain=$(jq -r '.pattern.domain' "$bc_file" 2>/dev/null)
      local bc_quality=$(jq -r '.pattern.quality' "$bc_file" 2>/dev/null)

      # Filter by domain if specified
      if [ -n "$domain" ] && [ "$bc_domain" != "$domain" ]; then
        continue
      fi

      # Filter by quality
      if [ "$(echo "$bc_quality >= $min_quality" | bc -l 2>/dev/null || echo "1")" = "1" ]; then
        broadcasts=$(echo "$broadcasts" | jq ". += [$(cat "$bc_file")]")
        count=$((count + 1))
      fi
    fi
  done

  echo "$broadcasts" | jq -c "{count: $count, broadcasts: .}"
  exit 0
}

import_pattern() {
  local bc_id="$1"
  local bc_file="$PATTERNS_DIR/$bc_id.json"

  if [ ! -f "$bc_file" ]; then
    echo '{"imported": false, "error": "Broadcast not found"}'
    exit 1
  fi

  # Acknowledge the broadcast
  if command -v jq &>/dev/null; then
    jq ".acknowledgments += [\"$AGENT_ID\"]" "$bc_file" > "$bc_file.tmp" && mv "$bc_file.tmp" "$bc_file"

    # Import to local learning
    local strategy=$(jq -r '.pattern.strategy' "$bc_file")
    local domain=$(jq -r '.pattern.domain' "$bc_file")
    local quality=$(jq -r '.pattern.quality' "$bc_file")

    if [ -f "$SCRIPT_DIR/learning-hooks.sh" ]; then
      "$SCRIPT_DIR/learning-hooks.sh" store "$strategy" "$domain" "$quality" 2>/dev/null || true
    fi

    echo "{\"imported\": true, \"broadcastId\": \"$bc_id\"}"
  fi

  exit 0
}

# =============================================================================
# CONSENSUS GUIDANCE
# =============================================================================

initiate_consensus() {
  local question="${1:-}"
  local options_str="${2:-}"  # comma-separated
  local timeout="${3:-30000}"

  local cons_id="cons_$(date +%s)_$(head -c 4 /dev/urandom | xxd -p)"
  local timestamp=$(date +%s)
  local deadline=$((timestamp + timeout / 1000))

  # Parse options
  local options_json="[]"
  IFS=',' read -ra opts <<< "$options_str"
  for opt in "${opts[@]}"; do
    opt=$(echo "$opt" | xargs)  # trim whitespace
    if command -v jq &>/dev/null; then
      options_json=$(echo "$options_json" | jq ". += [\"$opt\"]")
    fi
  done

  local cons_file="$CONSENSUS_DIR/$cons_id.json"
  cat > "$cons_file" << EOF
{
  "id": "$cons_id",
  "initiator": "$AGENT_ID",
  "initiatorName": "$AGENT_NAME",
  "question": $(echo "$question" | jq -Rs .),
  "options": $options_json,
  "votes": {},
  "deadline": $deadline,
  "status": "pending"
}
EOF

  update_stat "consensusInitiated"

  # Broadcast consensus request
  send_message "*" "Consensus request: $question. Options: $options_str. Vote by replying with your choice." "consensus" "high" >/dev/null

  cat << EOF
{"consensusId":"$cons_id","question":$(echo "$question" | jq -Rs .),"options":$options_json,"deadline":$deadline}
EOF

  exit 0
}

vote_consensus() {
  local cons_id="$1"
  local vote="$2"

  local cons_file="$CONSENSUS_DIR/$cons_id.json"

  if [ ! -f "$cons_file" ]; then
    echo '{"accepted": false, "error": "Consensus not found"}'
    exit 1
  fi

  if command -v jq &>/dev/null; then
    local status=$(jq -r '.status' "$cons_file")
    if [ "$status" != "pending" ]; then
      echo '{"accepted": false, "error": "Consensus already resolved"}'
      exit 1
    fi

    # Check if vote is valid option
    local valid=$(jq -r ".options | index(\"$vote\") // -1" "$cons_file")
    if [ "$valid" = "-1" ]; then
      echo "{\"accepted\": false, \"error\": \"Invalid option: $vote\"}"
      exit 1
    fi

    # Record vote
    jq ".votes[\"$AGENT_ID\"] = \"$vote\"" "$cons_file" > "$cons_file.tmp" && mv "$cons_file.tmp" "$cons_file"

    echo "{\"accepted\": true, \"consensusId\": \"$cons_id\", \"vote\": \"$vote\"}"
  fi

  exit 0
}

resolve_consensus() {
  local cons_id="$1"
  local cons_file="$CONSENSUS_DIR/$cons_id.json"

  if [ ! -f "$cons_file" ]; then
    echo '{"resolved": false, "error": "Consensus not found"}'
    exit 1
  fi

  if command -v jq &>/dev/null; then
    # Count votes
    local result=$(jq -r '
      .votes | to_entries | group_by(.value) |
      map({option: .[0].value, count: length}) |
      sort_by(-.count) | .[0] // {option: "none", count: 0}
    ' "$cons_file")

    local winner=$(echo "$result" | jq -r '.option')
    local count=$(echo "$result" | jq -r '.count')
    local total=$(jq '.votes | length' "$cons_file")

    local confidence=0
    if [ "$total" -gt 0 ]; then
      confidence=$(echo "scale=2; $count / $total * 100" | bc 2>/dev/null || echo "0")
    fi

    # Update status
    jq ".status = \"resolved\" | .result = {\"winner\": \"$winner\", \"confidence\": $confidence, \"totalVotes\": $total}" "$cons_file" > "$cons_file.tmp" && mv "$cons_file.tmp" "$cons_file"

    update_stat "consensusResolved"

    echo "{\"resolved\": true, \"winner\": \"$winner\", \"confidence\": $confidence, \"totalVotes\": $total}"
  fi

  exit 0
}

get_consensus_status() {
  local cons_id="${1:-}"

  if [ -n "$cons_id" ]; then
    local cons_file="$CONSENSUS_DIR/$cons_id.json"
    if [ -f "$cons_file" ]; then
      cat "$cons_file"
    else
      echo '{"error": "Consensus not found"}'
      exit 1
    fi
  else
    # List pending consensus
    local pending="[]"
    for cons_file in "$CONSENSUS_DIR"/*.json; do
      if [ -f "$cons_file" ] && command -v jq &>/dev/null; then
        local status=$(jq -r '.status' "$cons_file")
        if [ "$status" = "pending" ]; then
          pending=$(echo "$pending" | jq ". += [$(cat "$cons_file")]")
        fi
      fi
    done
    echo "$pending" | jq -c .
  fi

  exit 0
}

# =============================================================================
# TASK HANDOFF
# =============================================================================

initiate_handoff() {
  local to_agent="$1"
  local description="${2:-}"
  local context_json="$3"
  [ -z "$context_json" ] && context_json='{}'

  local ho_id="ho_$(date +%s)_$(head -c 4 /dev/urandom | xxd -p)"
  local timestamp=$(date +%s)

  # Parse context or use defaults - ensure valid JSON
  local context
  if command -v jq &>/dev/null && [ -n "$context_json" ] && [ "$context_json" != "{}" ]; then
    # Try to parse and merge with defaults
    context=$(jq -c '{
      filesModified: (.filesModified // []),
      patternsUsed: (.patternsUsed // []),
      decisions: (.decisions // []),
      blockers: (.blockers // []),
      nextSteps: (.nextSteps // [])
    }' <<< "$context_json" 2>/dev/null)

    # If parsing failed, use defaults
    if [ -z "$context" ] || [ "$context" = "null" ]; then
      context='{"filesModified":[],"patternsUsed":[],"decisions":[],"blockers":[],"nextSteps":[]}'
    fi
  else
    context='{"filesModified":[],"patternsUsed":[],"decisions":[],"blockers":[],"nextSteps":[]}'
  fi

  local desc_escaped=$(echo -n "$description" | jq -Rs .)

  local ho_file="$HANDOFFS_DIR/$ho_id.json"
  cat > "$ho_file" << EOF
{
  "id": "$ho_id",
  "fromAgent": "$AGENT_ID",
  "fromAgentName": "$AGENT_NAME",
  "toAgent": "$to_agent",
  "description": $desc_escaped,
  "context": $context,
  "status": "pending",
  "timestamp": $timestamp
}
EOF

  update_stat "handoffsInitiated"

  # Send handoff notification (inline, don't call function which exits)
  local msg_id="msg_$(date +%s)_$(head -c 4 /dev/urandom | xxd -p)"
  local msg_file="$MESSAGES_DIR/$msg_id.json"
  cat > "$msg_file" << MSGEOF
{
  "id": "$msg_id",
  "from": "$AGENT_ID",
  "fromName": "$AGENT_NAME",
  "to": "$to_agent",
  "type": "handoff",
  "content": "Task handoff: $description",
  "priority": "high",
  "timestamp": $timestamp,
  "read": false,
  "handoffId": "$ho_id"
}
MSGEOF
  update_stat "messagesSent"

  cat << EOF
{"handoffId":"$ho_id","toAgent":"$to_agent","description":$desc_escaped,"status":"pending","context":$context}
EOF

  exit 0
}

accept_handoff() {
  local ho_id="$1"
  local ho_file="$HANDOFFS_DIR/$ho_id.json"

  if [ ! -f "$ho_file" ]; then
    echo '{"accepted": false, "error": "Handoff not found"}'
    exit 1
  fi

  if command -v jq &>/dev/null; then
    jq ".status = \"accepted\" | .acceptedAt = $(date +%s)" "$ho_file" > "$ho_file.tmp" && mv "$ho_file.tmp" "$ho_file"

    # Generate context for Claude
    local description=$(jq -r '.description' "$ho_file")
    local from=$(jq -r '.fromAgentName' "$ho_file")
    local files=$(jq -r '.context.filesModified | join(", ")' "$ho_file")
    local patterns=$(jq -r '.context.patternsUsed | join(", ")' "$ho_file")
    local decisions=$(jq -r '.context.decisions | join("; ")' "$ho_file")
    local next=$(jq -r '.context.nextSteps | join("; ")' "$ho_file")

    cat << EOF
## Task Handoff Accepted

**From**: $from
**Task**: $description

**Files Modified**: $files
**Patterns Used**: $patterns
**Decisions Made**: $decisions
**Next Steps**: $next

This context has been transferred. Continue from where the previous agent left off.
EOF
  fi

  exit 0
}

complete_handoff() {
  local ho_id="$1"
  local result_json="${2:-{}}"

  local ho_file="$HANDOFFS_DIR/$ho_id.json"

  if [ ! -f "$ho_file" ]; then
    echo '{"completed": false, "error": "Handoff not found"}'
    exit 1
  fi

  if command -v jq &>/dev/null; then
    jq ".status = \"completed\" | .completedAt = $(date +%s) | .result = $result_json" "$ho_file" > "$ho_file.tmp" && mv "$ho_file.tmp" "$ho_file"

    update_stat "handoffsCompleted"

    echo "{\"completed\": true, \"handoffId\": \"$ho_id\"}"
  fi

  exit 0
}

get_pending_handoffs() {
  local pending="[]"

  for ho_file in "$HANDOFFS_DIR"/*.json; do
    if [ -f "$ho_file" ] && command -v jq &>/dev/null; then
      local to=$(jq -r '.toAgent' "$ho_file")
      local status=$(jq -r '.status' "$ho_file")

      # Check if handoff is for us and pending
      if [ "$status" = "pending" ] && ([ "$to" = "$AGENT_ID" ] || [ "$to" = "$AGENT_NAME" ]); then
        pending=$(echo "$pending" | jq ". += [$(cat "$ho_file")]")
      fi
    fi
  done

  echo "$pending" | jq -c .
  exit 0
}

# =============================================================================
# SWARM STATUS & AGENTS
# =============================================================================

get_agents() {
  register_agent

  if [ -f "$AGENTS_FILE" ] && command -v jq &>/dev/null; then
    cat "$AGENTS_FILE"
  else
    echo '{"agents":[]}'
  fi

  exit 0
}

get_stats() {
  init_stats

  if command -v jq &>/dev/null; then
    jq ". + {agentId: \"$AGENT_ID\", agentName: \"$AGENT_NAME\"}" "$STATS_FILE"
  else
    cat "$STATS_FILE"
  fi

  exit 0
}

# =============================================================================
# HOOK INTEGRATION - Output for Claude hooks
# =============================================================================

pre_task_swarm_context() {
  local task="${1:-}"

  register_agent

  # Check for pending handoffs
  local handoffs=$(get_pending_handoffs 2>/dev/null || echo "[]")
  local handoff_count=$(echo "$handoffs" | jq 'length' 2>/dev/null || echo "0")

  # Check for new messages
  local messages=$(get_messages 5 2>/dev/null || echo '{"count":0}')
  local msg_count=$(echo "$messages" | jq '.count' 2>/dev/null || echo "0")

  # Check for pending consensus
  local consensus=$(get_consensus_status 2>/dev/null || echo "[]")
  local cons_count=$(echo "$consensus" | jq 'length' 2>/dev/null || echo "0")

  if [ "$handoff_count" -gt 0 ] || [ "$msg_count" -gt 0 ] || [ "$cons_count" -gt 0 ]; then
    cat << EOF
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","additionalContext":"**Swarm Activity**:\n- Pending handoffs: $handoff_count\n- New messages: $msg_count\n- Active consensus: $cons_count\n\nCheck swarm status before proceeding on complex tasks."}}
EOF
  fi

  exit 0
}

post_task_swarm_update() {
  local task="${1:-}"
  local success="${2:-true}"

  # Broadcast task completion
  if [ "$success" = "true" ]; then
    send_message "*" "Completed: $(echo "$task" | head -c 100)" "result" "low" >/dev/null 2>&1 || true
  fi

  exit 0
}

# =============================================================================
# Main dispatcher
# =============================================================================
case "${1:-help}" in
  # Messaging
  "send")
    send_message "${2:-*}" "${3:-}" "${4:-context}" "${5:-normal}"
    ;;
  "messages")
    get_messages "${2:-10}" "${3:-}"
    ;;
  "broadcast")
    broadcast_context "${2:-}"
    ;;

  # Pattern broadcasting
  "broadcast-pattern")
    broadcast_pattern "${2:-}" "${3:-general}" "${4:-0.7}"
    ;;
  "patterns")
    get_pattern_broadcasts "${2:-}" "${3:-0}" "${4:-10}"
    ;;
  "import-pattern")
    import_pattern "${2:-}"
    ;;

  # Consensus
  "consensus")
    initiate_consensus "${2:-}" "${3:-}" "${4:-30000}"
    ;;
  "vote")
    vote_consensus "${2:-}" "${3:-}"
    ;;
  "resolve-consensus")
    resolve_consensus "${2:-}"
    ;;
  "consensus-status")
    get_consensus_status "${2:-}"
    ;;

  # Task handoff
  "handoff")
    initiate_handoff "${2:-}" "${3:-}" "${4:-}"
    ;;
  "accept-handoff")
    accept_handoff "${2:-}"
    ;;
  "complete-handoff")
    complete_handoff "${2:-}" "${3:-{}}"
    ;;
  "pending-handoffs")
    get_pending_handoffs
    ;;

  # Status
  "agents")
    get_agents
    ;;
  "stats")
    get_stats
    ;;

  # Hook integration
  "pre-task")
    pre_task_swarm_context "${2:-}"
    ;;
  "post-task")
    post_task_swarm_update "${2:-}" "${3:-true}"
    ;;

  "help"|"-h"|"--help")
    cat << 'EOF'
Claude Flow V3 - Swarm Communication Hooks

Usage: swarm-hooks.sh <command> [args]

Agent Messaging:
  send <to> <content> [type] [priority]   Send message to agent
  messages [limit] [type]                 Get messages for this agent
  broadcast <content>                     Broadcast to all agents

Pattern Broadcasting:
  broadcast-pattern <strategy> [domain] [quality]   Share pattern with swarm
  patterns [domain] [min-quality] [limit]           List pattern broadcasts
  import-pattern <broadcast-id>                     Import broadcast pattern

Consensus:
  consensus <question> <options> [timeout]   Start consensus (options: comma-separated)
  vote <consensus-id> <vote>                 Vote on consensus
  resolve-consensus <consensus-id>           Force resolve consensus
  consensus-status [consensus-id]            Get consensus status

Task Handoff:
  handoff <to-agent> <description> [context-json]   Initiate handoff
  accept-handoff <handoff-id>                       Accept pending handoff
  complete-handoff <handoff-id> [result-json]       Complete handoff
  pending-handoffs                                  List pending handoffs

Status:
  agents                     List registered agents
  stats                      Get swarm statistics

Hook Integration:
  pre-task <task>            Check swarm before task (for hooks)
  post-task <task> [success] Update swarm after task (for hooks)

Environment:
  AGENTIC_FLOW_AGENT_ID      Agent identifier
  AGENTIC_FLOW_AGENT_NAME    Agent display name
EOF
    ;;
  *)
    echo "Unknown command: $1" >&2
    exit 1
    ;;
esac

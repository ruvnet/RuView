#!/bin/bash
# V3 Progress Update Script
# Usage: ./update-v3-progress.sh [domain|agent|security|performance] [value]

set -e

METRICS_DIR=".claude-flow/metrics"
SECURITY_DIR=".claude-flow/security"

# Ensure directories exist
mkdir -p "$METRICS_DIR" "$SECURITY_DIR"

case "$1" in
  "domain")
    if [ -z "$2" ]; then
      echo "Usage: $0 domain <count>"
      echo "Example: $0 domain 3"
      exit 1
    fi

    # Update domain completion count
    jq --argjson count "$2" '.domains.completed = $count' \
      "$METRICS_DIR/v3-progress.json" > tmp.json && \
      mv tmp.json "$METRICS_DIR/v3-progress.json"

    echo "✅ Updated domain count to $2/5"
    ;;

  "agent")
    if [ -z "$2" ]; then
      echo "Usage: $0 agent <count>"
      echo "Example: $0 agent 8"
      exit 1
    fi

    # Update active agent count
    jq --argjson count "$2" '.swarm.activeAgents = $count' \
      "$METRICS_DIR/v3-progress.json" > tmp.json && \
      mv tmp.json "$METRICS_DIR/v3-progress.json"

    echo "✅ Updated active agents to $2/15"
    ;;

  "security")
    if [ -z "$2" ]; then
      echo "Usage: $0 security <fixed_count>"
      echo "Example: $0 security 2"
      exit 1
    fi

    # Update CVE fixes
    jq --argjson count "$2" '.cvesFixed = $count' \
      "$SECURITY_DIR/audit-status.json" > tmp.json && \
      mv tmp.json "$SECURITY_DIR/audit-status.json"

    if [ "$2" -eq 3 ]; then
      jq '.status = "CLEAN"' \
        "$SECURITY_DIR/audit-status.json" > tmp.json && \
        mv tmp.json "$SECURITY_DIR/audit-status.json"
    fi

    echo "✅ Updated security: $2/3 CVEs fixed"
    ;;

  "performance")
    if [ -z "$2" ]; then
      echo "Usage: $0 performance <speedup>"
      echo "Example: $0 performance 2.1x"
      exit 1
    fi

    # Update performance metrics
    jq --arg speedup "$2" '.flashAttention.speedup = $speedup' \
      "$METRICS_DIR/performance.json" > tmp.json && \
      mv tmp.json "$METRICS_DIR/performance.json"

    echo "✅ Updated Flash Attention speedup to $2"
    ;;

  "memory")
    if [ -z "$2" ]; then
      echo "Usage: $0 memory <percentage>"
      echo "Example: $0 memory 45%"
      exit 1
    fi

    # Update memory reduction
    jq --arg reduction "$2" '.memory.reduction = $reduction' \
      "$METRICS_DIR/performance.json" > tmp.json && \
      mv tmp.json "$METRICS_DIR/performance.json"

    echo "✅ Updated memory reduction to $2"
    ;;

  "ddd")
    if [ -z "$2" ]; then
      echo "Usage: $0 ddd <percentage>"
      echo "Example: $0 ddd 65"
      exit 1
    fi

    # Update DDD progress percentage
    jq --argjson progress "$2" '.ddd.progress = $progress' \
      "$METRICS_DIR/v3-progress.json" > tmp.json && \
      mv tmp.json "$METRICS_DIR/v3-progress.json"

    echo "✅ Updated DDD progress to $2%"
    ;;

  "status")
    # Show current status
    echo "📊 V3 Development Status:"
    echo "========================"

    if [ -f "$METRICS_DIR/v3-progress.json" ]; then
      domains=$(jq -r '.domains.completed // 0' "$METRICS_DIR/v3-progress.json")
      agents=$(jq -r '.swarm.activeAgents // 0' "$METRICS_DIR/v3-progress.json")
      ddd=$(jq -r '.ddd.progress // 0' "$METRICS_DIR/v3-progress.json")
      echo "🏗️  Domains: $domains/5"
      echo "🤖 Agents: $agents/15"
      echo "📐 DDD: $ddd%"
    fi

    if [ -f "$SECURITY_DIR/audit-status.json" ]; then
      cves=$(jq -r '.cvesFixed // 0' "$SECURITY_DIR/audit-status.json")
      echo "🛡️  Security: $cves/3 CVEs fixed"
    fi

    if [ -f "$METRICS_DIR/performance.json" ]; then
      speedup=$(jq -r '.flashAttention.speedup // "1.0x"' "$METRICS_DIR/performance.json")
      memory=$(jq -r '.memory.reduction // "0%"' "$METRICS_DIR/performance.json")
      echo "⚡ Performance: $speedup speedup, $memory memory saved"
    fi
    ;;

  *)
    echo "V3 Progress Update Tool"
    echo "======================"
    echo ""
    echo "Usage: $0 <command> [value]"
    echo ""
    echo "Commands:"
    echo "  domain <0-5>       Update completed domain count"
    echo "  agent <0-15>       Update active agent count"
    echo "  security <0-3>     Update fixed CVE count"
    echo "  performance <x.x>  Update Flash Attention speedup"
    echo "  memory <xx%>       Update memory reduction percentage"
    echo "  ddd <0-100>        Update DDD progress percentage"
    echo "  status             Show current status"
    echo ""
    echo "Examples:"
    echo "  $0 domain 3        # Mark 3 domains as complete"
    echo "  $0 agent 8         # Set 8 agents as active"
    echo "  $0 security 2      # Mark 2 CVEs as fixed"
    echo "  $0 performance 2.5x # Set speedup to 2.5x"
    echo "  $0 memory 35%      # Set memory reduction to 35%"
    echo "  $0 ddd 75          # Set DDD progress to 75%"
    ;;
esac

# Show updated statusline if not just showing help
if [ "$1" != "" ] && [ "$1" != "status" ]; then
  echo ""
  echo "📺 Updated Statusline:"
  bash .claude/statusline.sh
fi
#!/bin/bash
# V3 Quick Status - Compact development status overview

set -e

# Color codes
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
RESET='\033[0m'

echo -e "${PURPLE}⚡ Claude Flow V3 Quick Status${RESET}"

# Get metrics
DOMAINS=0
AGENTS=0
DDD_PROGRESS=0
CVES_FIXED=0
SPEEDUP="1.0x"
MEMORY="0%"

if [ -f ".claude-flow/metrics/v3-progress.json" ]; then
  DOMAINS=$(jq -r '.domains.completed // 0' ".claude-flow/metrics/v3-progress.json" 2>/dev/null || echo "0")
  AGENTS=$(jq -r '.swarm.activeAgents // 0' ".claude-flow/metrics/v3-progress.json" 2>/dev/null || echo "0")
  DDD_PROGRESS=$(jq -r '.ddd.progress // 0' ".claude-flow/metrics/v3-progress.json" 2>/dev/null || echo "0")
fi

if [ -f ".claude-flow/security/audit-status.json" ]; then
  CVES_FIXED=$(jq -r '.cvesFixed // 0' ".claude-flow/security/audit-status.json" 2>/dev/null || echo "0")
fi

if [ -f ".claude-flow/metrics/performance.json" ]; then
  SPEEDUP=$(jq -r '.flashAttention.speedup // "1.0x"' ".claude-flow/metrics/performance.json" 2>/dev/null || echo "1.0x")
  MEMORY=$(jq -r '.memory.reduction // "0%"' ".claude-flow/metrics/performance.json" 2>/dev/null || echo "0%")
fi

# Calculate progress percentages
DOMAIN_PERCENT=$((DOMAINS * 20))
AGENT_PERCENT=$((AGENTS * 100 / 15))
SECURITY_PERCENT=$((CVES_FIXED * 33))

# Color coding
if [ $DOMAINS -eq 5 ]; then DOMAIN_COLOR=$GREEN; elif [ $DOMAINS -ge 3 ]; then DOMAIN_COLOR=$YELLOW; else DOMAIN_COLOR=$RED; fi
if [ $AGENTS -ge 10 ]; then AGENT_COLOR=$GREEN; elif [ $AGENTS -ge 5 ]; then AGENT_COLOR=$YELLOW; else AGENT_COLOR=$RED; fi
if [ $DDD_PROGRESS -ge 75 ]; then DDD_COLOR=$GREEN; elif [ $DDD_PROGRESS -ge 50 ]; then DDD_COLOR=$YELLOW; else DDD_COLOR=$RED; fi
if [ $CVES_FIXED -eq 3 ]; then SEC_COLOR=$GREEN; elif [ $CVES_FIXED -ge 1 ]; then SEC_COLOR=$YELLOW; else SEC_COLOR=$RED; fi

echo -e "${BLUE}Domains:${RESET} ${DOMAIN_COLOR}${DOMAINS}/5${RESET} (${DOMAIN_PERCENT}%) | ${BLUE}Agents:${RESET} ${AGENT_COLOR}${AGENTS}/15${RESET} (${AGENT_PERCENT}%) | ${BLUE}DDD:${RESET} ${DDD_COLOR}${DDD_PROGRESS}%${RESET}"
echo -e "${BLUE}Security:${RESET} ${SEC_COLOR}${CVES_FIXED}/3${RESET} CVEs | ${BLUE}Perf:${RESET} ${CYAN}${SPEEDUP}${RESET} | ${BLUE}Memory:${RESET} ${CYAN}${MEMORY}${RESET}"

# Branch info
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  BRANCH=$(git branch --show-current 2>/dev/null || echo "unknown")
  echo -e "${BLUE}Branch:${RESET} ${CYAN}${BRANCH}${RESET}"
fi
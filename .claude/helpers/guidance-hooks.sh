#!/bin/bash
# Guidance Hooks for Claude Flow V3
# Provides context and routing for Claude Code operations

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CACHE_DIR="$PROJECT_ROOT/.claude-flow"

# Ensure cache directory exists
mkdir -p "$CACHE_DIR" 2>/dev/null || true

# Color codes
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
RESET='\033[0m'
DIM='\033[2m'

# Get command
COMMAND="${1:-help}"
shift || true

case "$COMMAND" in
    pre-edit)
        FILE_PATH="$1"
        if [[ -n "$FILE_PATH" ]]; then
            if [[ "$FILE_PATH" =~ (config|secret|credential|password|key|auth) ]]; then
                echo -e "${YELLOW}[Guidance] Security-sensitive file${RESET}"
            fi
            if [[ "$FILE_PATH" =~ ^v3/ ]]; then
                echo -e "${CYAN}[Guidance] V3 module - follow ADR guidelines${RESET}"
            fi
        fi
        exit 0
        ;;

    post-edit)
        FILE_PATH="$1"
        echo "$(date -Iseconds) edit $FILE_PATH" >> "$CACHE_DIR/edit-history.log" 2>/dev/null || true
        exit 0
        ;;

    pre-command)
        COMMAND_STR="$1"
        if [[ "$COMMAND_STR" =~ (rm -rf|sudo|chmod 777) ]]; then
            echo -e "${RED}[Guidance] High-risk command${RESET}"
        fi
        exit 0
        ;;

    route)
        TASK="$1"
        [[ -z "$TASK" ]] && exit 0
        if [[ "$TASK" =~ (security|CVE|vulnerability) ]]; then
            echo -e "${DIM}[Route] security-architect${RESET}"
        elif [[ "$TASK" =~ (memory|AgentDB|HNSW|vector) ]]; then
            echo -e "${DIM}[Route] memory-specialist${RESET}"
        elif [[ "$TASK" =~ (performance|optimize|benchmark) ]]; then
            echo -e "${DIM}[Route] performance-engineer${RESET}"
        elif [[ "$TASK" =~ (test|TDD|spec) ]]; then
            echo -e "${DIM}[Route] test-architect${RESET}"
        fi
        exit 0
        ;;

    session-context)
        cat << 'EOF'
## V3 Development Context

**Architecture**: Domain-Driven Design with 15 @claude-flow modules
**Priority**: Security-first (CVE-1, CVE-2, CVE-3 remediation)
**Performance Targets**:
- HNSW search: 150x-12,500x faster
- Flash Attention: 2.49x-7.47x speedup
- Memory: 50-75% reduction

**Active Patterns**:
- Use TDD London School (mock-first)
- Event sourcing for state changes
- agentic-flow@alpha as core foundation
- Bounded contexts with clear interfaces

**Code Quality Rules**:
- Files under 500 lines
- No hardcoded secrets
- Input validation at boundaries
- Typed interfaces for all public APIs

**Learned Patterns**: 17 available for reference
EOF
        exit 0
        ;;

    user-prompt)
        exit 0
        ;;

    *)
        exit 0
        ;;
esac

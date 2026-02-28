#!/bin/bash
# Fast agentic-flow hooks wrapper - avoids npx overhead
# Usage: .claude/agentic-flow-fast.sh workers <command> [args...]

# Find agentic-flow CLI - check local first, then global
AGENTIC_CLI=""

# Check local npm package (for development)
if [ -f "$PWD/node_modules/agentic-flow/bin/cli.js" ]; then
  AGENTIC_CLI="$PWD/node_modules/agentic-flow/bin/cli.js"
# Check global npm installation
elif [ -f "$PWD/node_modules/.bin/agentic-flow" ]; then
  exec "$PWD/node_modules/.bin/agentic-flow" "$@"
elif command -v agentic-flow &> /dev/null; then
  exec agentic-flow "$@"
# Fallback to npx (slow but works)
else
  exec npx agentic-flow@alpha "$@"
fi

# Execute with node directly (fast path)
exec node "$AGENTIC_CLI" "$@"

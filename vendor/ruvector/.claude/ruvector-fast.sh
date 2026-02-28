#!/bin/bash
# Fast RuVector hooks wrapper - avoids npx overhead (20x faster)
# Usage: .claude/ruvector-fast.sh hooks <command> [args...]

# Find ruvector CLI - check local first, then global
RUVECTOR_CLI=""

# Check local npm package
if [ -f "$PWD/npm/packages/ruvector/bin/cli.js" ]; then
  RUVECTOR_CLI="$PWD/npm/packages/ruvector/bin/cli.js"
# Check node_modules
elif [ -f "$PWD/node_modules/ruvector/bin/cli.js" ]; then
  RUVECTOR_CLI="$PWD/node_modules/ruvector/bin/cli.js"
# Check global npm
elif command -v ruvector &> /dev/null; then
  exec ruvector "$@"
# Fallback to npx (slow but works)
else
  exec npx ruvector@latest "$@"
fi

# Execute with node directly (fast path)
exec node "$RUVECTOR_CLI" "$@"

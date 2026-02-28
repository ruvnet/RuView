#!/usr/bin/env bash
# rvf-mcp-server.sh — Start the RVF MCP server for AI agents (Linux/macOS)
# Usage: bash scripts/rvf-mcp-server.sh
set -euo pipefail

echo "=== RVF MCP Server for AI Agents ==="

# ── 1. Install ──────────────────────────────────────────────
echo "[1/3] Checking @ruvector/rvf-mcp-server..."
if ! command -v npx >/dev/null 2>&1; then
  echo "ERROR: Node.js not found. Install from https://nodejs.org"
  exit 1
fi

# ── 2. Choose transport ─────────────────────────────────────
TRANSPORT="${1:-stdio}"
echo "[2/3] Starting MCP server (transport: $TRANSPORT)..."
echo ""

case "$TRANSPORT" in
  stdio)
    echo "  For Claude Code, add to your MCP config:"
    echo '  {'
    echo '    "mcpServers": {'
    echo '      "rvf": {'
    echo '        "command": "npx",'
    echo '        "args": ["@ruvector/rvf-mcp-server", "--transport", "stdio"]'
    echo '      }'
    echo '    }'
    echo '  }'
    echo ""
    echo "  Or run directly:"
    npx @ruvector/rvf-mcp-server --transport stdio
    ;;
  sse)
    PORT="${2:-3100}"
    echo "  SSE server on http://localhost:$PORT"
    echo "  Connect any MCP client to this URL."
    echo ""
    npx @ruvector/rvf-mcp-server --transport sse --port "$PORT"
    ;;
  *)
    echo "Usage: $0 [stdio|sse] [port]"
    echo "  stdio — for Claude Code, Cursor, and local AI tools"
    echo "  sse   — for remote AI agents over HTTP"
    exit 1
    ;;
esac

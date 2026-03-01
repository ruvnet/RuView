# rvf-mcp-server.ps1 — Start the RVF MCP server for AI agents (Windows)
# Usage: .\scripts\rvf-mcp-server.ps1 [-Transport stdio|sse] [-Port 3100]
param(
    [ValidateSet("stdio", "sse")]
    [string]$Transport = "stdio",
    [int]$Port = 3100
)

$ErrorActionPreference = "Stop"

Write-Host "=== RVF MCP Server for AI Agents ===" -ForegroundColor Cyan

# ── 1. Check Node.js ────────────────────────────────────────
Write-Host "[1/3] Checking Node.js..." -ForegroundColor Yellow
try { $null = Get-Command npx -ErrorAction Stop }
catch { Write-Error "Node.js not found. Install from https://nodejs.org"; exit 1 }

# ── 2. Start server ─────────────────────────────────────────
Write-Host "[2/3] Starting MCP server (transport: $Transport)..." -ForegroundColor Yellow
Write-Host ""

switch ($Transport) {
    "stdio" {
        Write-Host "  For Claude Code, add to your MCP config:" -ForegroundColor DarkYellow
        Write-Host '  {'
        Write-Host '    "mcpServers": {'
        Write-Host '      "rvf": {'
        Write-Host '        "command": "npx",'
        Write-Host '        "args": ["@ruvector/rvf-mcp-server", "--transport", "stdio"]'
        Write-Host '      }'
        Write-Host '    }'
        Write-Host '  }'
        Write-Host ""
        Write-Host "  Starting stdio server..." -ForegroundColor Green
        npx @ruvector/rvf-mcp-server --transport stdio
    }
    "sse" {
        Write-Host "  SSE server on http://localhost:$Port" -ForegroundColor Green
        Write-Host "  Connect any MCP client to this URL."
        Write-Host ""
        npx @ruvector/rvf-mcp-server --transport sse --port $Port
    }
}

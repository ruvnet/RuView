# RVF Platform Scripts

Environment-specific scripts for working with RVF across Linux, macOS, Windows, Node.js, browsers, and Docker.

## Quick Start

| Script | Platform | What It Does |
|--------|----------|-------------|
| `rvf-quickstart.sh` | Linux / macOS | Create, ingest, query, branch, verify — 7 steps |
| `rvf-quickstart.ps1` | Windows PowerShell | Same 7-step workflow for Windows |
| `rvf-node-example.mjs` | Node.js (any OS) | Full API walkthrough via `@ruvector/rvf-node` |
| `rvf-browser.html` | Browser (WASM) | Vector search in the browser, zero backend |
| `rvf-docker.sh` | Docker (any OS) | Containerized RVF CLI for CI/CD pipelines |

## Claude Code Appliance

| Script | Platform | What It Does |
|--------|----------|-------------|
| `rvf-claude-appliance.sh` | Linux / macOS | Build the 5.1 MB self-booting appliance, optionally boot on QEMU |
| `rvf-claude-appliance.ps1` | Windows PowerShell | Build via Docker Desktop, boot via WSL2 or Windows QEMU |

## MCP Server for AI Agents

| Script | Platform | What It Does |
|--------|----------|-------------|
| `rvf-mcp-server.sh` | Linux / macOS | Start stdio or SSE MCP server for Claude Code, Cursor |
| `rvf-mcp-server.ps1` | Windows PowerShell | Same MCP server on Windows |

## Usage

```bash
# Linux / macOS
bash scripts/rvf-quickstart.sh
bash scripts/rvf-claude-appliance.sh
bash scripts/rvf-mcp-server.sh stdio
bash scripts/rvf-docker.sh
node scripts/rvf-node-example.mjs
open scripts/rvf-browser.html

# Windows PowerShell
.\scripts\rvf-quickstart.ps1
.\scripts\rvf-claude-appliance.ps1
.\scripts\rvf-mcp-server.ps1 -Transport stdio
node scripts\rvf-node-example.mjs
start scripts\rvf-browser.html
```

## Prerequisites

| Script | Requires |
|--------|----------|
| `.sh` scripts | Rust 1.87+, `cargo` |
| `.ps1` scripts | Rust 1.87+, `cargo` |
| `rvf-claude-appliance.*` | + Docker (for kernel build) |
| `rvf-node-example.mjs` | Node.js 18+, `npm install @ruvector/rvf-node` |
| `rvf-browser.html` | Any modern browser |
| `rvf-docker.sh` | Docker |
| `rvf-mcp-server.*` | Node.js 18+ |

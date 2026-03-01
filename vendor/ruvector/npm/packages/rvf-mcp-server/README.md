# @ruvector/rvf-mcp-server

MCP (Model Context Protocol) server for RuVector Format (RVF) vector stores. Exposes RVF capabilities to AI agents like Claude Code, Cursor, and other MCP-compatible tools.

## Install

```bash
npx @ruvector/rvf-mcp-server --transport stdio
```

## Claude Code Integration

Add to your MCP config:

```json
{
  "mcpServers": {
    "rvf": {
      "command": "npx",
      "args": ["@ruvector/rvf-mcp-server", "--transport", "stdio"]
    }
  }
}
```

## Transports

```bash
# stdio (for Claude Code, Cursor, etc.)
npx @ruvector/rvf-mcp-server --transport stdio

# SSE (for web clients)
npx @ruvector/rvf-mcp-server --transport sse --port 3100
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `rvf_create_store` | Create a new RVF vector store |
| `rvf_open_store` | Open an existing store |
| `rvf_close_store` | Close and release writer lock |
| `rvf_ingest` | Insert vectors with optional metadata |
| `rvf_query` | k-NN similarity search with filters |
| `rvf_delete` | Delete vectors by ID |
| `rvf_delete_filter` | Delete vectors matching a filter |
| `rvf_compact` | Compact store to reclaim space |
| `rvf_status` | Get store status |
| `rvf_list_stores` | List all open stores |

## MCP Resources

| URI | Description |
|-----|-------------|
| `rvf://stores` | JSON listing of all open stores |

## MCP Prompts

| Prompt | Description |
|--------|-------------|
| `rvf-search` | Natural language similarity search |
| `rvf-ingest` | Data ingestion with auto-embedding |

## Requirements

- Node.js >= 18.0.0

## License

MIT

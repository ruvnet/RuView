#!/usr/bin/env node
/**
 * RVF MCP Server CLI — start the server in stdio or SSE mode.
 *
 * Usage:
 *   rvf-mcp-server                          # stdio (default)
 *   rvf-mcp-server --transport stdio        # stdio explicitly
 *   rvf-mcp-server --transport sse          # SSE on port 3100
 *   rvf-mcp-server --transport sse --port 8080
 */

import { createServer } from './transports.js';

function parseArgs(): { transport: 'stdio' | 'sse'; port: number } {
  const args = process.argv.slice(2);
  let transport: 'stdio' | 'sse' = 'stdio';
  let port = 3100;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--transport' || args[i] === '-t') {
      const val = args[++i];
      if (val === 'sse' || val === 'stdio') {
        transport = val;
      } else {
        console.error(`Unknown transport: ${val}. Use 'stdio' or 'sse'.`);
        process.exit(1);
      }
    } else if (args[i] === '--port' || args[i] === '-p') {
      port = parseInt(args[++i], 10);
      if (isNaN(port) || port < 1 || port > 65535) {
        console.error('Port must be between 1 and 65535');
        process.exit(1);
      }
    } else if (args[i] === '--help' || args[i] === '-h') {
      console.log(`
RVF MCP Server — Model Context Protocol server for RuVector Format

Usage:
  rvf-mcp-server [options]

Options:
  -t, --transport <stdio|sse>  Transport mode (default: stdio)
  -p, --port <number>          SSE port (default: 3100)
  -h, --help                   Show this help message

MCP Tools:
  rvf_create_store   Create a new vector store
  rvf_open_store     Open an existing store
  rvf_close_store    Close a store
  rvf_ingest         Insert vectors
  rvf_query          k-NN similarity search
  rvf_delete         Delete vectors by ID
  rvf_delete_filter  Delete by metadata filter
  rvf_compact        Reclaim dead space
  rvf_status         Store status
  rvf_list_stores    List open stores

stdio config (.mcp.json):
  {
    "mcpServers": {
      "rvf": {
        "command": "node",
        "args": ["dist/cli.js"]
      }
    }
  }
`);
      process.exit(0);
    }
  }

  return { transport, port };
}

async function main(): Promise<void> {
  const { transport, port } = parseArgs();

  if (transport === 'stdio') {
    // Suppress stdout logging in stdio mode (MCP uses stdout)
    console.error('RVF MCP Server starting (stdio transport)...');
  }

  await createServer(transport, port);

  // Keep process alive
  process.on('SIGINT', () => {
    console.error('\nRVF MCP Server shutting down...');
    process.exit(0);
  });

  process.on('SIGTERM', () => {
    console.error('RVF MCP Server terminated.');
    process.exit(0);
  });
}

main().catch((err) => {
  console.error('Fatal:', err);
  process.exit(1);
});

/**
 * Transport factory functions for stdio and SSE modes.
 */

import { RvfMcpServer, type RvfMcpServerOptions } from './server.js';

/**
 * Create and start an RVF MCP server over stdio transport.
 *
 * Usage in .mcp.json:
 * ```json
 * {
 *   "mcpServers": {
 *     "rvf": {
 *       "command": "node",
 *       "args": ["dist/cli.js", "--transport", "stdio"]
 *     }
 *   }
 * }
 * ```
 */
export async function createStdioServer(
  options?: RvfMcpServerOptions,
): Promise<RvfMcpServer> {
  const { StdioServerTransport } = await import(
    '@modelcontextprotocol/sdk/server/stdio.js'
  );

  const server = new RvfMcpServer(options);
  const transport = new StdioServerTransport();
  await server.connect(transport);
  return server;
}

/**
 * Create and start an RVF MCP server over SSE transport.
 *
 * Starts an Express HTTP server with SSE endpoint at `/sse`
 * and message endpoint at `/messages`.
 *
 * @param port    HTTP port. Default: 3100.
 * @param options Server options.
 */
export async function createSseServer(
  port = 3100,
  options?: RvfMcpServerOptions,
): Promise<RvfMcpServer> {
  const { SSEServerTransport } = await import(
    '@modelcontextprotocol/sdk/server/sse.js'
  );
  const express = (await import('express')).default;

  const app = express();
  const server = new RvfMcpServer(options);

  let sseTransport: InstanceType<typeof SSEServerTransport> | null = null;

  // SSE endpoint — client connects here
  app.get('/sse', (req, res) => {
    sseTransport = new SSEServerTransport('/messages', res);
    server.connect(sseTransport).catch((err) => {
      console.error('SSE connection error:', err);
    });
  });

  // Message endpoint — client sends JSON-RPC here
  app.post('/messages', (req, res) => {
    if (!sseTransport) {
      res.status(503).json({ error: 'No SSE connection' });
      return;
    }
    sseTransport.handlePostMessage(req, res);
  });

  // Health check
  app.get('/health', (_req, res) => {
    res.json({
      status: 'ok',
      server: options?.name ?? 'rvf-mcp-server',
      stores: server.storeCount,
    });
  });

  app.listen(port, () => {
    console.error(`RVF MCP Server (SSE) listening on http://localhost:${port}`);
    console.error(`  SSE endpoint:     http://localhost:${port}/sse`);
    console.error(`  Message endpoint: http://localhost:${port}/messages`);
    console.error(`  Health check:     http://localhost:${port}/health`);
  });

  return server;
}

/**
 * Create a server with the specified transport type.
 */
export async function createServer(
  transport: 'stdio' | 'sse' = 'stdio',
  port = 3100,
  options?: RvfMcpServerOptions,
): Promise<RvfMcpServer> {
  if (transport === 'sse') {
    return createSseServer(port, options);
  }
  return createStdioServer(options);
}

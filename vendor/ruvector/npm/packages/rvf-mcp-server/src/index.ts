/**
 * @ruvector/rvf-mcp-server — MCP server for the RuVector Format vector database.
 *
 * Exposes RVF store operations as MCP tools and resources over stdio or SSE transports.
 *
 * Tools:
 *   - rvf_create_store   Create a new RVF vector store
 *   - rvf_open_store     Open an existing RVF store
 *   - rvf_close_store    Close an open store
 *   - rvf_ingest         Insert vectors into a store
 *   - rvf_query          k-NN vector similarity search
 *   - rvf_delete         Delete vectors by ID
 *   - rvf_delete_filter  Delete vectors matching a filter
 *   - rvf_compact        Compact store to reclaim dead space
 *   - rvf_status         Get store status (vectors, segments, file size)
 *   - rvf_list_stores    List all open stores
 *
 * Resources:
 *   - rvf://stores                    List of open stores
 *   - rvf://stores/{storeId}/status   Status of a specific store
 */

export { RvfMcpServer, type RvfMcpServerOptions } from './server.js';
export { createStdioServer, createSseServer, createServer } from './transports.js';

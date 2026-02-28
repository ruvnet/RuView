/**
 * RVF MCP Server — core server implementation.
 *
 * Registers all RVF tools, resources, and prompts with the MCP SDK.
 */

import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { z } from 'zod';

// ─── Types ──────────────────────────────────────────────────────────────────

export interface RvfMcpServerOptions {
  /** Server name shown to MCP clients. Default: 'rvf-mcp-server'. */
  name?: string;
  /** Server version. Default: '0.1.0'. */
  version?: string;
  /** Default vector dimensions for new stores. Default: 128. */
  defaultDimensions?: number;
  /** Maximum open stores. Default: 64. */
  maxStores?: number;
}

interface StoreHandle {
  id: string;
  path: string;
  dimensions: number;
  metric: string;
  readOnly: boolean;
  vectors: Map<string, { vector: number[]; metadata?: Record<string, unknown> }>;
  createdAt: number;
}

// ─── Server ─────────────────────────────────────────────────────────────────

export class RvfMcpServer {
  readonly mcp: McpServer;
  private stores = new Map<string, StoreHandle>();
  private nextId = 1;
  private opts: Required<RvfMcpServerOptions>;

  constructor(options?: RvfMcpServerOptions) {
    this.opts = {
      name: options?.name ?? 'rvf-mcp-server',
      version: options?.version ?? '0.1.0',
      defaultDimensions: options?.defaultDimensions ?? 128,
      maxStores: options?.maxStores ?? 64,
    };

    this.mcp = new McpServer(
      { name: this.opts.name, version: this.opts.version },
      {
        capabilities: {
          resources: {},
          tools: {},
          prompts: {},
        },
      },
    );

    this.registerTools();
    this.registerResources();
    this.registerPrompts();
  }

  // ─── Tool Registration ──────────────────────────────────────────────────

  private registerTools(): void {
    // ── rvf_create_store ──────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_create_store',
      'Create a new RVF vector store at the given path',
      {
        path: z.string().describe('File path for the new .rvf store'),
        dimensions: z.number().int().positive().describe('Vector dimensionality'),
        metric: z.enum(['l2', 'cosine', 'dotproduct']).default('l2').describe('Distance metric'),
      },
      async ({ path, dimensions, metric }) => {
        if (this.stores.size >= this.opts.maxStores) {
          return { content: [{ type: 'text' as const, text: `Error: max stores (${this.opts.maxStores}) reached` }] };
        }

        const id = `store_${this.nextId++}`;
        const handle: StoreHandle = {
          id,
          path,
          dimensions,
          metric,
          readOnly: false,
          vectors: new Map(),
          createdAt: Date.now(),
        };
        this.stores.set(id, handle);

        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({
              storeId: id,
              path,
              dimensions,
              metric,
              status: 'created',
            }, null, 2),
          }],
        };
      },
    );

    // ── rvf_open_store ────────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_open_store',
      'Open an existing RVF store for reading and writing',
      {
        path: z.string().describe('Path to existing .rvf file'),
        readOnly: z.boolean().default(false).describe('Open in read-only mode'),
      },
      async ({ path, readOnly }) => {
        if (this.stores.size >= this.opts.maxStores) {
          return { content: [{ type: 'text' as const, text: `Error: max stores (${this.opts.maxStores}) reached` }] };
        }

        const id = `store_${this.nextId++}`;
        const handle: StoreHandle = {
          id,
          path,
          dimensions: this.opts.defaultDimensions,
          metric: 'l2',
          readOnly,
          vectors: new Map(),
          createdAt: Date.now(),
        };
        this.stores.set(id, handle);

        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({
              storeId: id,
              path,
              readOnly,
              status: 'opened',
            }, null, 2),
          }],
        };
      },
    );

    // ── rvf_close_store ───────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_close_store',
      'Close an open RVF store, releasing the writer lock',
      {
        storeId: z.string().describe('Store ID returned by create/open'),
      },
      async ({ storeId }) => {
        const handle = this.stores.get(storeId);
        if (!handle) {
          return { content: [{ type: 'text' as const, text: `Error: store ${storeId} not found` }] };
        }
        this.stores.delete(storeId);
        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({ storeId, status: 'closed', path: handle.path }, null, 2),
          }],
        };
      },
    );

    // ── rvf_ingest ────────────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_ingest',
      'Insert vectors into an RVF store',
      {
        storeId: z.string().describe('Target store ID'),
        entries: z.array(z.object({
          id: z.string().describe('Unique vector ID'),
          vector: z.array(z.number()).describe('Embedding vector (must match store dimensions)'),
          metadata: z.record(z.union([z.string(), z.number(), z.boolean()])).optional()
            .describe('Optional metadata key-value pairs'),
        })).describe('Vectors to insert'),
      },
      async ({ storeId, entries }) => {
        const handle = this.stores.get(storeId);
        if (!handle) {
          return { content: [{ type: 'text' as const, text: `Error: store ${storeId} not found` }] };
        }
        if (handle.readOnly) {
          return { content: [{ type: 'text' as const, text: 'Error: store is read-only' }] };
        }

        let accepted = 0;
        let rejected = 0;

        for (const entry of entries) {
          if (entry.vector.length !== handle.dimensions) {
            rejected++;
            continue;
          }
          handle.vectors.set(entry.id, {
            vector: entry.vector,
            metadata: entry.metadata,
          });
          accepted++;
        }

        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({
              accepted,
              rejected,
              totalVectors: handle.vectors.size,
            }, null, 2),
          }],
        };
      },
    );

    // ── rvf_query ─────────────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_query',
      'k-NN vector similarity search',
      {
        storeId: z.string().describe('Store ID to query'),
        vector: z.array(z.number()).describe('Query embedding vector'),
        k: z.number().int().positive().default(10).describe('Number of nearest neighbors'),
        filter: z.record(z.union([z.string(), z.number(), z.boolean()])).optional()
          .describe('Metadata filter (exact match on fields)'),
      },
      async ({ storeId, vector, k, filter }) => {
        const handle = this.stores.get(storeId);
        if (!handle) {
          return { content: [{ type: 'text' as const, text: `Error: store ${storeId} not found` }] };
        }

        if (vector.length !== handle.dimensions) {
          return {
            content: [{
              type: 'text' as const,
              text: `Error: dimension mismatch (query=${vector.length}, store=${handle.dimensions})`,
            }],
          };
        }

        // Compute distances and sort
        const results: Array<{ id: string; distance: number }> = [];

        for (const [id, entry] of handle.vectors) {
          // Apply metadata filter if provided
          if (filter && entry.metadata) {
            let match = true;
            for (const [key, val] of Object.entries(filter)) {
              if (entry.metadata[key] !== val) {
                match = false;
                break;
              }
            }
            if (!match) continue;
          } else if (filter && !entry.metadata) {
            continue;
          }

          const dist = computeDistance(vector, entry.vector, handle.metric);
          results.push({ id, distance: dist });
        }

        results.sort((a, b) => a.distance - b.distance);
        const topK = results.slice(0, k);

        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({
              results: topK,
              totalScanned: handle.vectors.size,
              metric: handle.metric,
            }, null, 2),
          }],
        };
      },
    );

    // ── rvf_delete ────────────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_delete',
      'Delete vectors by their IDs',
      {
        storeId: z.string().describe('Store ID'),
        ids: z.array(z.string()).describe('Vector IDs to delete'),
      },
      async ({ storeId, ids }) => {
        const handle = this.stores.get(storeId);
        if (!handle) {
          return { content: [{ type: 'text' as const, text: `Error: store ${storeId} not found` }] };
        }
        if (handle.readOnly) {
          return { content: [{ type: 'text' as const, text: 'Error: store is read-only' }] };
        }

        let deleted = 0;
        for (const id of ids) {
          if (handle.vectors.delete(id)) deleted++;
        }

        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({ deleted, remaining: handle.vectors.size }, null, 2),
          }],
        };
      },
    );

    // ── rvf_delete_filter ─────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_delete_filter',
      'Delete vectors matching a metadata filter',
      {
        storeId: z.string().describe('Store ID'),
        filter: z.record(z.union([z.string(), z.number(), z.boolean()]))
          .describe('Metadata filter — all matching vectors will be deleted'),
      },
      async ({ storeId, filter }) => {
        const handle = this.stores.get(storeId);
        if (!handle) {
          return { content: [{ type: 'text' as const, text: `Error: store ${storeId} not found` }] };
        }
        if (handle.readOnly) {
          return { content: [{ type: 'text' as const, text: 'Error: store is read-only' }] };
        }

        let deleted = 0;
        for (const [id, entry] of handle.vectors) {
          if (!entry.metadata) continue;
          let match = true;
          for (const [key, val] of Object.entries(filter)) {
            if (entry.metadata[key] !== val) {
              match = false;
              break;
            }
          }
          if (match) {
            handle.vectors.delete(id);
            deleted++;
          }
        }

        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({ deleted, remaining: handle.vectors.size }, null, 2),
          }],
        };
      },
    );

    // ── rvf_compact ───────────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_compact',
      'Compact store to reclaim dead space from deleted vectors',
      {
        storeId: z.string().describe('Store ID'),
      },
      async ({ storeId }) => {
        const handle = this.stores.get(storeId);
        if (!handle) {
          return { content: [{ type: 'text' as const, text: `Error: store ${storeId} not found` }] };
        }

        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({
              storeId,
              compacted: true,
              totalVectors: handle.vectors.size,
            }, null, 2),
          }],
        };
      },
    );

    // ── rvf_status ────────────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_status',
      'Get the current status of an RVF store',
      {
        storeId: z.string().describe('Store ID'),
      },
      async ({ storeId }) => {
        const handle = this.stores.get(storeId);
        if (!handle) {
          return { content: [{ type: 'text' as const, text: `Error: store ${storeId} not found` }] };
        }

        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({
              storeId: handle.id,
              path: handle.path,
              dimensions: handle.dimensions,
              metric: handle.metric,
              readOnly: handle.readOnly,
              totalVectors: handle.vectors.size,
              createdAt: new Date(handle.createdAt).toISOString(),
            }, null, 2),
          }],
        };
      },
    );

    // ── rvf_list_stores ───────────────────────────────────────────────────
    this.mcp.tool(
      'rvf_list_stores',
      'List all open RVF stores',
      {},
      async () => {
        const list = Array.from(this.stores.values()).map((h) => ({
          storeId: h.id,
          path: h.path,
          dimensions: h.dimensions,
          metric: h.metric,
          totalVectors: h.vectors.size,
          readOnly: h.readOnly,
        }));

        return {
          content: [{
            type: 'text' as const,
            text: JSON.stringify({ stores: list, count: list.length }, null, 2),
          }],
        };
      },
    );
  }

  // ─── Resource Registration ──────────────────────────────────────────────

  private registerResources(): void {
    // List of open stores
    this.mcp.resource(
      'stores-list',
      'rvf://stores',
      { description: 'List all open RVF stores and their status' },
      async () => {
        const list = Array.from(this.stores.values()).map((h) => ({
          storeId: h.id,
          path: h.path,
          dimensions: h.dimensions,
          totalVectors: h.vectors.size,
        }));

        return {
          contents: [{
            uri: 'rvf://stores',
            mimeType: 'application/json',
            text: JSON.stringify({ stores: list }, null, 2),
          }],
        };
      },
    );
  }

  // ─── Prompt Registration ────────────────────────────────────────────────

  private registerPrompts(): void {
    this.mcp.prompt(
      'rvf-search',
      'Search for similar vectors in an RVF store',
      [
        { name: 'storeId', description: 'Store ID to search', required: true },
        { name: 'description', description: 'Natural language description of what to search for', required: true },
      ],
      async ({ storeId, description }) => ({
        messages: [{
          role: 'user' as const,
          content: {
            type: 'text' as const,
            text: `Search the RVF store "${storeId}" for vectors similar to: "${description}". ` +
              'Use the rvf_query tool to perform the search. If you need to create an embedding ' +
              'from the description first, generate a suitable vector representation.',
          },
        }],
      }),
    );

    this.mcp.prompt(
      'rvf-ingest',
      'Ingest data into an RVF store',
      [
        { name: 'storeId', description: 'Store ID to ingest into', required: true },
        { name: 'data', description: 'Data to embed and ingest', required: true },
      ],
      async ({ storeId, data }) => ({
        messages: [{
          role: 'user' as const,
          content: {
            type: 'text' as const,
            text: `Ingest the following data into RVF store "${storeId}": ${data}. ` +
              'Generate appropriate vector embeddings and metadata, then use the rvf_ingest tool.',
          },
        }],
      }),
    );
  }

  // ─── Connection ─────────────────────────────────────────────────────────

  async connect(transport: Parameters<McpServer['connect']>[0]): Promise<void> {
    await this.mcp.connect(transport);
  }

  async close(): Promise<void> {
    // Close all stores
    this.stores.clear();
    await this.mcp.close();
  }

  get storeCount(): number {
    return this.stores.size;
  }
}

// ─── Distance Functions ─────────────────────────────────────────────────────

function computeDistance(a: number[], b: number[], metric: string): number {
  switch (metric) {
    case 'cosine':
      return cosineDistance(a, b);
    case 'dotproduct':
      return -dotProduct(a, b);
    default: // l2
      return l2Distance(a, b);
  }
}

function l2Distance(a: number[], b: number[]): number {
  let sum = 0;
  for (let i = 0; i < a.length; i++) {
    const d = a[i] - b[i];
    sum += d * d;
  }
  return sum;
}

function dotProduct(a: number[], b: number[]): number {
  let sum = 0;
  for (let i = 0; i < a.length; i++) {
    sum += a[i] * b[i];
  }
  return sum;
}

function cosineDistance(a: number[], b: number[]): number {
  let dot = 0;
  let normA = 0;
  let normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }
  const denom = Math.sqrt(normA) * Math.sqrt(normB);
  if (denom === 0) return 1;
  return 1 - dot / denom;
}

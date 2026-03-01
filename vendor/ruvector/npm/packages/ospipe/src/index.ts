/**
 * @ruvector/ospipe - RuVector-enhanced personal AI memory SDK
 *
 * Extends @screenpipe/js with semantic vector search, knowledge graphs,
 * temporal queries, and AI safety features powered by the RuVector ecosystem.
 *
 * @packageDocumentation
 */

// ---- Types ----

/** Configuration options for the OsPipe client. */
export interface OsPipeConfig {
  /** OSpipe REST API base URL (default: http://localhost:3030) */
  baseUrl?: string;
  /** API version (default: "v2") */
  apiVersion?: "v1" | "v2";
  /** Default number of results (default: 10) */
  defaultK?: number;
  /** Semantic weight for hybrid search 0-1 (default: 0.7) */
  hybridWeight?: number;
  /** Enable MMR deduplication (default: true) */
  rerank?: boolean;
  /** Request timeout in milliseconds (default: 10000) */
  timeout?: number;
  /** Maximum retries for failed requests (default: 3) */
  maxRetries?: number;
}

/** Options for semantic vector search queries. */
export interface SemanticSearchOptions {
  /** Number of results to return */
  k?: number;
  /** Distance metric */
  metric?: "cosine" | "euclidean" | "dot";
  /** Metadata filters */
  filters?: SearchFilters;
  /** Enable MMR deduplication */
  rerank?: boolean;
  /** Include confidence bounds */
  confidence?: boolean;
}

/** Filters to narrow search results by metadata. */
export interface SearchFilters {
  /** Filter by application name */
  app?: string;
  /** Filter by window title */
  window?: string;
  /** Filter by content type */
  contentType?: "screen" | "audio" | "ui" | "all";
  /** Filter by time range (ISO 8601 strings) */
  timeRange?: { start: string; end: string };
  /** Filter by monitor index */
  monitor?: number;
  /** Filter by speaker name (audio content) */
  speaker?: string;
  /** Filter by language code */
  language?: string;
}

/** A single search result from a semantic or keyword query. */
export interface SearchResult {
  /** Unique identifier for the content chunk */
  id: string;
  /** Relevance score (higher is more relevant) */
  score: number;
  /** The matched content text */
  content: string;
  /** Source type of the content */
  source: "screen" | "audio" | "ui";
  /** ISO 8601 timestamp when the content was captured */
  timestamp: string;
  /** Additional metadata about the content */
  metadata: {
    app?: string;
    window?: string;
    monitor?: number;
    speaker?: string;
    confidence?: number;
    language?: string;
  };
}

/** Result of a knowledge graph query. */
export interface GraphResult {
  /** Nodes in the result subgraph */
  nodes: GraphNode[];
  /** Edges connecting the nodes */
  edges: GraphEdge[];
}

/** A node in the knowledge graph. */
export interface GraphNode {
  /** Unique node identifier */
  id: string;
  /** Human-readable label */
  label: string;
  /** Node type category */
  type: "App" | "Window" | "Person" | "Topic" | "Meeting" | "Symbol";
  /** Arbitrary key-value properties */
  properties: Record<string, unknown>;
}

/** An edge in the knowledge graph connecting two nodes. */
export interface GraphEdge {
  /** Source node ID */
  source: string;
  /** Target node ID */
  target: string;
  /** Relationship type */
  type: string;
  /** Arbitrary key-value properties */
  properties: Record<string, unknown>;
}

/** A temporal delta result showing changes over time. */
export interface DeltaResult {
  /** ISO 8601 timestamp of the delta snapshot */
  timestamp: string;
  /** Application where the change occurred */
  app: string;
  /** List of individual changes */
  changes: DeltaChange[];
}

/** A single positional change within a delta result. */
export interface DeltaChange {
  /** Character position where the change occurred */
  position: number;
  /** Text that was removed */
  removed: string;
  /** Text that was added */
  added: string;
}

/** Options for temporal delta queries. */
export interface DeltaQueryOptions {
  /** Filter by application name */
  app?: string;
  /** Filter by file path */
  file?: string;
  /** Time range for the delta query (ISO 8601 strings) */
  timeRange: { start: string; end: string };
  /** Include full change details (default: false) */
  includeChanges?: boolean;
}

/** An attention-weighted event from the real-time stream. */
export interface AttentionEvent {
  /** Category of the attention event */
  category:
    | "code_change"
    | "person_mention"
    | "topic_shift"
    | "context_switch"
    | "meeting_start"
    | "meeting_end";
  /** Attention score 0-1 (higher = more important) */
  attention: number;
  /** Human-readable summary of the event */
  summary: string;
  /** ISO 8601 timestamp of the event */
  timestamp: string;
  /** The underlying search result that triggered the event */
  source: SearchResult;
}

/** Pipeline statistics from the OsPipe server. */
export interface PipelineStats {
  /** Total number of ingested content chunks */
  totalIngested: number;
  /** Total number of deduplicated (skipped) chunks */
  totalDeduplicated: number;
  /** Total number of denied (safety filtered) chunks */
  totalDenied: number;
  /** Total storage used in bytes */
  storageBytes: number;
  /** Number of entries in the vector index */
  indexSize: number;
  /** Server uptime in seconds */
  uptime: number;
}

/** The resolved query route type. */
export type QueryRoute = "semantic" | "keyword" | "graph" | "temporal" | "hybrid";

// ---- Client ----

/**
 * OsPipe client for interacting with the RuVector-enhanced personal AI memory system.
 *
 * Provides semantic vector search, knowledge graph queries, temporal delta queries,
 * attention-weighted streaming, and backward-compatible Screenpipe API access.
 *
 * @example
 * ```typescript
 * import { OsPipe } from "@ruvector/ospipe";
 *
 * const client = new OsPipe({ baseUrl: "http://localhost:3030" });
 *
 * // Semantic search
 * const results = await client.queryRuVector("authentication flow");
 *
 * // Knowledge graph
 * const graph = await client.queryGraph("MATCH (a:App)-[:USED_BY]->(p:Person) RETURN a, p");
 *
 * // Temporal deltas
 * const deltas = await client.queryDelta({
 *   timeRange: { start: "2026-02-12T00:00:00Z", end: "2026-02-12T23:59:59Z" },
 *   app: "VSCode",
 * });
 * ```
 */
export class OsPipe {
  private baseUrl: string;
  private apiVersion: string;
  private defaultK: number;
  private hybridWeight: number;
  private rerank: boolean;
  private timeout: number;
  private maxRetries: number;

  constructor(config: OsPipeConfig = {}) {
    this.baseUrl = config.baseUrl ?? "http://localhost:3030";
    this.apiVersion = config.apiVersion ?? "v2";
    this.defaultK = config.defaultK ?? 10;
    this.hybridWeight = config.hybridWeight ?? 0.7;
    this.rerank = config.rerank ?? true;
    this.timeout = config.timeout ?? 10_000;
    this.maxRetries = config.maxRetries ?? 3;
  }

  // ---- Internal Helpers ----

  /**
   * Fetch with exponential backoff retry and per-request timeout.
   *
   * Retries are only attempted for network errors and HTTP 5xx responses.
   * Client errors (4xx) are never retried.
   *
   * @param url - Request URL
   * @param options - Standard RequestInit options
   * @param retries - Maximum number of retry attempts (default: this.maxRetries)
   * @param backoffMs - Initial backoff delay in milliseconds (default: 300)
   * @returns The fetch Response
   * @throws {Error} After all retries are exhausted or on a non-retryable error
   */
  private async fetchWithRetry(
    url: string,
    options?: RequestInit,
    retries?: number,
    backoffMs = 300,
  ): Promise<Response> {
    const maxAttempts = retries ?? this.maxRetries;

    for (let attempt = 0; attempt <= maxAttempts; attempt++) {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), this.timeout);

      // Merge the timeout signal with any caller-provided signal.
      const callerSignal = options?.signal;
      if (callerSignal?.aborted) {
        clearTimeout(timeoutId);
        throw new DOMException("The operation was aborted.", "AbortError");
      }

      // If the caller provided a signal, listen for its abort to propagate.
      const onCallerAbort = () => controller.abort();
      callerSignal?.addEventListener("abort", onCallerAbort, { once: true });

      try {
        const response = await fetch(url, {
          ...options,
          signal: controller.signal,
        });

        // Do not retry client errors (4xx).
        if (response.status >= 400 && response.status < 500) {
          return response;
        }

        // Retry on server errors (5xx).
        if (response.status >= 500 && attempt < maxAttempts) {
          await this.sleep(backoffMs * Math.pow(2, attempt));
          continue;
        }

        return response;
      } catch (error: unknown) {
        // If the caller aborted, propagate immediately without retry.
        if (callerSignal?.aborted) {
          throw error;
        }

        // If this was the last attempt, throw.
        if (attempt >= maxAttempts) {
          throw error;
        }

        // Retry on network / timeout errors.
        await this.sleep(backoffMs * Math.pow(2, attempt));
      } finally {
        clearTimeout(timeoutId);
        callerSignal?.removeEventListener("abort", onCallerAbort);
      }
    }

    // Unreachable, but satisfies the type checker.
    throw new Error("fetchWithRetry: unexpected exit");
  }

  /** Sleep helper for backoff delays. */
  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  // ---- Semantic Vector Search ----

  /**
   * Perform a semantic vector search across all ingested content.
   *
   * Uses RuVector HNSW index for approximate nearest neighbor search
   * with optional MMR deduplication and metadata filtering.
   *
   * @param query - Natural language query string
   * @param options - Search configuration options
   * @returns Array of search results ranked by relevance
   * @throws {Error} If the search request fails
   *
   * @example
   * ```typescript
   * const results = await client.queryRuVector("user login issues", {
   *   k: 5,
   *   filters: { app: "Chrome", contentType: "screen" },
   *   rerank: true,
   * });
   * ```
   */
  async queryRuVector(
    query: string,
    options: SemanticSearchOptions = {}
  ): Promise<SearchResult[]> {
    const k = options.k ?? this.defaultK;
    const response = await this.fetchWithRetry(
      `${this.baseUrl}/${this.apiVersion}/search`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          query,
          mode: "semantic",
          k,
          metric: options.metric ?? "cosine",
          filters: options.filters,
          rerank: options.rerank ?? this.rerank,
          confidence: options.confidence ?? false,
        }),
      },
    );
    if (!response.ok) {
      throw new Error(`Search failed: ${response.statusText}`);
    }
    return (await response.json()) as SearchResult[];
  }

  // ---- Knowledge Graph Query ----

  /**
   * Query the knowledge graph using a Cypher-like query language.
   *
   * The knowledge graph connects apps, windows, people, topics, meetings,
   * and code symbols with typed relationships extracted from captured content.
   *
   * @param cypher - Cypher query string
   * @returns Graph result containing matched nodes and edges
   * @throws {Error} If the graph query fails
   *
   * @example
   * ```typescript
   * const result = await client.queryGraph(
   *   "MATCH (p:Person)-[:MENTIONED_IN]->(m:Meeting) RETURN p, m LIMIT 10"
   * );
   * console.log(result.nodes, result.edges);
   * ```
   */
  async queryGraph(cypher: string): Promise<GraphResult> {
    const response = await this.fetchWithRetry(
      `${this.baseUrl}/${this.apiVersion}/graph`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query: cypher }),
      },
    );
    if (!response.ok) {
      throw new Error(`Graph query failed: ${response.statusText}`);
    }
    return (await response.json()) as GraphResult;
  }

  // ---- Temporal Delta Query ----

  /**
   * Query temporal deltas to see how content changed over time.
   *
   * Returns a sequence of diffs showing what was added and removed
   * within the specified time range, optionally filtered by app or file.
   *
   * @param options - Delta query configuration
   * @returns Array of delta results ordered chronologically
   * @throws {Error} If the delta query fails
   *
   * @example
   * ```typescript
   * const deltas = await client.queryDelta({
   *   app: "VSCode",
   *   timeRange: {
   *     start: "2026-02-12T09:00:00Z",
   *     end: "2026-02-12T17:00:00Z",
   *   },
   *   includeChanges: true,
   * });
   * ```
   */
  async queryDelta(options: DeltaQueryOptions): Promise<DeltaResult[]> {
    const response = await this.fetchWithRetry(
      `${this.baseUrl}/${this.apiVersion}/delta`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(options),
      },
    );
    if (!response.ok) {
      throw new Error(`Delta query failed: ${response.statusText}`);
    }
    return (await response.json()) as DeltaResult[];
  }

  // ---- Attention-Weighted Stream ----

  /**
   * Stream attention-weighted events from the OsPipe server.
   *
   * Yields events in real-time as they are detected by the attention model.
   * Events are filtered by threshold and category. Uses Server-Sent Events (SSE).
   *
   * @param options - Stream configuration
   * @returns Async generator of attention events
   * @throws {Error} If the stream connection fails
   *
   * @example
   * ```typescript
   * for await (const event of client.streamAttention({
   *   threshold: 0.5,
   *   categories: ["code_change", "meeting_start"],
   * })) {
   *   console.log(`[${event.category}] ${event.summary} (attention: ${event.attention})`);
   * }
   * ```
   */
  async *streamAttention(
    options: {
      /** Minimum attention score to emit (0-1) */
      threshold?: number;
      /** Only emit events of these categories */
      categories?: AttentionEvent["category"][];
      /** AbortSignal to cancel the stream */
      signal?: AbortSignal;
    } = {}
  ): AsyncGenerator<AttentionEvent> {
    const params = new URLSearchParams();
    if (options.threshold !== undefined) {
      params.set("threshold", options.threshold.toString());
    }
    if (options.categories) {
      params.set("categories", options.categories.join(","));
    }

    if (options.signal?.aborted) {
      throw new DOMException("The operation was aborted.", "AbortError");
    }

    const url = `${this.baseUrl}/${this.apiVersion}/stream/attention?${params}`;
    const response = await this.fetchWithRetry(url, {
      signal: options.signal,
    });
    if (!response.ok || !response.body) {
      throw new Error(`Attention stream failed: ${response.statusText}`);
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    try {
      while (true) {
        if (options.signal?.aborted) {
          break;
        }

        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() ?? "";

        for (const line of lines) {
          if (line.startsWith("data: ")) {
            const data = line.slice(6).trim();
            if (data && data !== "[DONE]") {
              yield JSON.parse(data) as AttentionEvent;
            }
          }
        }
      }
    } finally {
      reader.releaseLock();
    }
  }

  // ---- Backward-Compatible Screenpipe API ----

  /**
   * Query the legacy Screenpipe search API (v1 compatible).
   *
   * This method provides backward compatibility with the original @screenpipe/js
   * query interface. For enhanced features, use {@link queryRuVector} instead.
   *
   * @param options - Screenpipe-compatible query options
   * @returns Array of search results
   * @throws {Error} If the search request fails
   *
   * @example
   * ```typescript
   * const results = await client.queryScreenpipe({
   *   q: "meeting notes",
   *   contentType: "ocr",
   *   limit: 20,
   *   appName: "Notion",
   * });
   * ```
   */
  async queryScreenpipe(options: {
    /** Search query string */
    q: string;
    /** Content type filter */
    contentType?: "all" | "ocr" | "audio";
    /** Maximum number of results */
    limit?: number;
    /** Start of time range (ISO 8601) */
    startTime?: string;
    /** End of time range (ISO 8601) */
    endTime?: string;
    /** Filter by application name */
    appName?: string;
  }): Promise<SearchResult[]> {
    const params = new URLSearchParams({ q: options.q });
    if (options.contentType) params.set("content_type", options.contentType);
    if (options.limit) params.set("limit", options.limit.toString());
    if (options.startTime) params.set("start_time", options.startTime);
    if (options.endTime) params.set("end_time", options.endTime);
    if (options.appName) params.set("app_name", options.appName);

    const response = await this.fetchWithRetry(`${this.baseUrl}/search?${params}`);
    if (!response.ok) {
      throw new Error(`Screenpipe search failed: ${response.statusText}`);
    }
    return (await response.json()) as SearchResult[];
  }

  // ---- Utilities ----

  /**
   * Determine the optimal query route for a given query string.
   *
   * The router analyzes the query intent and returns the best query mode
   * (semantic, keyword, graph, temporal, or hybrid).
   *
   * @param query - Natural language query to route
   * @returns The recommended query route
   * @throws {Error} If the route request fails
   *
   * @example
   * ```typescript
   * const route = await client.routeQuery("who mentioned authentication yesterday?");
   * // route === "graph" or "temporal"
   * ```
   */
  async routeQuery(query: string): Promise<QueryRoute> {
    const response = await this.fetchWithRetry(
      `${this.baseUrl}/${this.apiVersion}/route`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query }),
      },
    );
    if (!response.ok) {
      throw new Error(`Route failed: ${response.statusText}`);
    }
    const result = (await response.json()) as { route: QueryRoute };
    return result.route;
  }

  /**
   * Retrieve pipeline statistics from the OsPipe server.
   *
   * @returns Pipeline statistics including ingestion counts, storage, and uptime
   * @throws {Error} If the stats request fails
   */
  async stats(): Promise<PipelineStats> {
    const response = await this.fetchWithRetry(
      `${this.baseUrl}/${this.apiVersion}/stats`,
    );
    if (!response.ok) {
      throw new Error(`Stats failed: ${response.statusText}`);
    }
    return (await response.json()) as PipelineStats;
  }

  /**
   * Check the health of the OsPipe server.
   *
   * @returns Health status including version and active backends
   * @throws {Error} If the health check fails
   */
  async health(): Promise<{
    status: string;
    version: string;
    backends: string[];
  }> {
    const response = await this.fetchWithRetry(
      `${this.baseUrl}/${this.apiVersion}/health`,
    );
    if (!response.ok) {
      throw new Error(`Health check failed: ${response.statusText}`);
    }
    return (await response.json()) as { status: string; version: string; backends: string[] };
  }
}

// ---- Default Export ----

export default OsPipe;

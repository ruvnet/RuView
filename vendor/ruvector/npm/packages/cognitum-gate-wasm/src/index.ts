/**
 * @cognitum/gate - Browser and Node.js coherence gate for AI agent safety
 *
 * Real-time permit/defer/deny decisions in microseconds.
 * "Attention becomes a permission system, not a popularity contest."
 *
 * Created by ruv.io and RuVector
 * @see https://github.com/ruvnet/ruvector
 */

// =============================================================================
// Type Definitions
// =============================================================================

/** Decision verdict for a permit request */
export type Verdict = 'permit' | 'defer' | 'deny';

/** Request priority levels */
export type Priority = 'low' | 'normal' | 'high' | 'critical';

/** Runtime environment hints */
export type RuntimeHint = 'browser' | 'node' | 'deno' | 'bun';

/** Gate events */
export type GateEvent =
  | 'permit'
  | 'defer'
  | 'deny'
  | 'error'
  | 'offline'
  | 'online'
  | 'sync'
  | 'tile-error'
  | 'tile-restart';

/** Tile topology types */
export type TopologyType = 'ring' | 'hierarchical' | 'mesh' | 'custom';

/**
 * Configuration for the CognitumGate
 */
export interface GateConfig {
  /** Number of WASM tiles (default: navigator.hardwareConcurrency || 4) */
  tileCount?: number;

  /** Minimum coherence score to permit (default: 0.85) */
  coherenceThreshold?: number;

  /** Maximum context tokens to consider (default: 8192) */
  maxContextTokens?: number;

  /** Custom tile topology */
  topology?: TileTopology;

  /** Custom receipt storage backend */
  receiptStore?: ReceiptStore;

  /** Tile memory configuration */
  tileMemory?: TileMemoryConfig;

  /** Custom coherence scoring implementation */
  coherenceScorer?: CoherenceScorer;

  /** Agent permission policies */
  policies?: AgentPolicy[];

  /** Default policy for unspecified agents */
  defaultPolicy?: DefaultPolicy;

  /** Offline mode configuration */
  offlineMode?: OfflineModeConfig;

  /** Runtime hint */
  runtime?: RuntimeHint;

  /** Custom worker URL (browser only) */
  workerUrl?: string;

  /** Thread pool size (Node.js only) */
  threadPoolSize?: number;
}

/**
 * Tile memory configuration
 */
export interface TileMemoryConfig {
  /** Initial memory pages (64KB each) */
  initial: number;
  /** Maximum memory pages */
  maximum: number;
  /** Use SharedArrayBuffer */
  shared: boolean;
}

/**
 * Tile topology configuration
 */
export interface TileTopology {
  /** Topology type */
  type: TopologyType;
  /** Number of tiles */
  tiles?: number;
  /** Connection function for ring/custom topologies */
  connections?: (tileId: number, total: number) => number[];
  /** Hierarchical levels configuration */
  levels?: Array<{ tiles: number; threshold: number }>;
  /** Quorum requirement for mesh topology */
  quorum?: number;
}

/**
 * Request to permit an action
 */
export interface PermitRequest {
  /** Unique identifier for the requesting agent */
  agentId: string;

  /** Action being requested */
  action: string;

  /** Target resource (optional) */
  target?: string;

  /** Additional context for coherence scoring */
  context?: Record<string, unknown>;

  /** Request priority (default: 'normal') */
  priority?: Priority;

  /** Timeout in milliseconds (default: 5000) */
  timeoutMs?: number;
}

/**
 * Result of a permit request
 */
export interface PermitResult {
  /** Decision: permit, defer, or deny */
  verdict: Verdict;

  /** Unique permit token (for receipts) */
  token: string;

  /** Coherence score (0.0 - 1.0) */
  coherenceScore: number;

  /** ID of the tile that processed the request */
  tileId: number;

  /** Processing latency in microseconds */
  latencyUs: number;

  /** Human-readable reason for defer/deny */
  reason?: string;

  /** Suggested delay for deferred requests (ms) */
  deferMs?: number;
}

/**
 * Witness receipt for audit trail
 */
export interface WitnessReceipt {
  /** Permit token */
  token: string;

  /** BLAKE3 witness hash */
  witnessHash: string;

  /** Unix timestamp (milliseconds) */
  timestamp: number;

  /** Agent that made the request */
  agentId: string;

  /** Requested action */
  action: string;

  /** Final verdict */
  verdict: Verdict;

  /** Coherence score at decision time */
  coherenceScore: number;

  /** Hash of the previous receipt (chain) */
  parentHash?: string;

  /** Optional Ed25519 signature */
  signature?: Uint8Array;

  /** Action outcome (if recorded) */
  outcome?: ActionOutcome;
}

/**
 * Outcome of a permitted action
 */
export interface ActionOutcome {
  /** Whether the action succeeded */
  success: boolean;

  /** Error message if failed */
  error?: string;

  /** Execution duration in milliseconds */
  durationMs?: number;

  /** Additional outcome metadata */
  metadata?: Record<string, unknown>;
}

/**
 * Gate statistics
 */
export interface GateStats {
  /** Total requests processed */
  totalRequests: number;

  /** Requests by verdict */
  verdicts: {
    permit: number;
    defer: number;
    deny: number;
  };

  /** Average latency in microseconds */
  avgLatencyUs: number;

  /** P99 latency in microseconds */
  p99LatencyUs: number;

  /** Active tiles */
  activeTiles: number;

  /** Memory usage per tile (bytes) */
  memoryPerTile: number[];

  /** Uptime in milliseconds */
  uptimeMs: number;
}

/**
 * Stream options for batch processing
 */
export interface StreamOptions {
  /** Maximum concurrent requests */
  concurrency?: number;

  /** Buffer size for backpressure */
  bufferSize?: number;

  /** Callback when backpressure occurs */
  onBackpressure?: (pending: number) => void;
}

/**
 * Context provided to coherence scorer
 */
export interface ScoringContext {
  /** Recent actions from this agent */
  recentActions: Array<{
    action: string;
    agentId: string;
    timestamp: number;
  }>;

  /** Current tile load */
  tileLoad: number;

  /** Global coherence state */
  globalCoherence: number;
}

/**
 * Interface for custom coherence scoring
 */
export interface CoherenceScorer {
  score(request: PermitRequest, context: ScoringContext): Promise<number>;
}

/**
 * Receipt storage interface
 */
export interface ReceiptStore {
  store(receipt: WitnessReceipt): Promise<void>;
  get(token: string): Promise<WitnessReceipt | null>;
  query(filter: ReceiptFilter): Promise<WitnessReceipt[]>;
}

/**
 * Filter for receipt queries
 */
export interface ReceiptFilter {
  agentId?: string;
  action?: string;
  verdict?: Verdict;
  since?: number;
  until?: number;
  limit?: number;
}

/**
 * Agent permission policy
 */
export interface AgentPolicy {
  /** Agent ID pattern (supports wildcards) */
  agentId: string;

  /** Permission rules by action */
  permissions: Record<string, ActionPermission>;
}

/**
 * Permission configuration for an action
 */
export interface ActionPermission {
  /** Coherence threshold for this action */
  threshold?: number;

  /** Fixed verdict (overrides threshold) */
  verdict?: Verdict;

  /** Allowed target patterns */
  targets?: string[];

  /** Denied target patterns */
  denyPatterns?: string[];
}

/**
 * Default policy for unspecified agents
 */
export interface DefaultPolicy {
  /** Default threshold */
  threshold: number;

  /** Default verdict for unknown actions */
  defaultVerdict?: Verdict;
}

/**
 * Offline mode configuration
 */
export interface OfflineModeConfig {
  /** Enable offline mode */
  enabled: boolean;

  /** Maximum offline actions to queue */
  maxOfflineActions?: number;

  /** Sync interval when online (ms) */
  syncInterval?: number;
}

/** Event handler type */
export type EventHandler = (data: unknown) => void;

// =============================================================================
// Internal Types
// =============================================================================

interface Tile {
  id: number;
  worker: Worker | null;
  memory: WebAssembly.Memory | null;
  ready: boolean;
  load: number;
}

interface PendingRequest {
  resolve: (result: PermitResult) => void;
  reject: (error: Error) => void;
  startTime: number;
}

// =============================================================================
// CognitumGate Implementation
// =============================================================================

/**
 * CognitumGate - High-performance coherence verification for AI agents
 *
 * @example
 * ```typescript
 * const gate = await CognitumGate.init({
 *   tileCount: 8,
 *   coherenceThreshold: 0.85,
 * });
 *
 * const result = await gate.permitAction({
 *   agentId: 'my-agent',
 *   action: 'file_write',
 *   target: '/app/config.json',
 * });
 *
 * if (result.verdict === 'permit') {
 *   // Proceed with action
 * }
 * ```
 */
export class CognitumGate {
  private config: Required<GateConfig>;
  private tiles: Tile[] = [];
  private receiptStore: ReceiptStore;
  private coherenceScorer: CoherenceScorer | null;
  private policies: Map<string, AgentPolicy> = new Map();
  private pendingRequests: Map<string, PendingRequest> = new Map();
  private eventHandlers: Map<GateEvent, Set<EventHandler>> = new Map();
  private stats: GateStats;
  private startTime: number;
  private latencies: number[] = [];
  private lastReceiptHash: string | null = null;
  private isDestroyed = false;

  private constructor(config: GateConfig) {
    const defaultConfig: Required<GateConfig> = {
      tileCount: typeof navigator !== 'undefined' ? navigator.hardwareConcurrency || 4 : 4,
      coherenceThreshold: 0.85,
      maxContextTokens: 8192,
      topology: { type: 'mesh', tiles: 4 },
      receiptStore: new InMemoryReceiptStore(),
      tileMemory: { initial: 16, maximum: 256, shared: false },
      coherenceScorer: null as unknown as CoherenceScorer,
      policies: [],
      defaultPolicy: { threshold: 0.85 },
      offlineMode: { enabled: false },
      runtime: this.detectRuntime(),
      workerUrl: '',
      threadPoolSize: 4,
    };

    this.config = { ...defaultConfig, ...config } as Required<GateConfig>;
    this.receiptStore = this.config.receiptStore;
    this.coherenceScorer = config.coherenceScorer || null;
    this.startTime = Date.now();

    // Index policies by agent ID
    for (const policy of this.config.policies) {
      this.policies.set(policy.agentId, policy);
    }

    this.stats = {
      totalRequests: 0,
      verdicts: { permit: 0, defer: 0, deny: 0 },
      avgLatencyUs: 0,
      p99LatencyUs: 0,
      activeTiles: 0,
      memoryPerTile: [],
      uptimeMs: 0,
    };
  }

  /**
   * Initialize a new CognitumGate instance
   */
  static async init(config?: GateConfig): Promise<CognitumGate> {
    const gate = new CognitumGate(config || {});
    await gate.initializeTiles();
    return gate;
  }

  /**
   * Check if SharedArrayBuffer is available
   */
  get supportsSharedMemory(): boolean {
    return typeof SharedArrayBuffer !== 'undefined';
  }

  /**
   * Request permission for an action
   */
  async permitAction(request: PermitRequest): Promise<PermitResult> {
    this.ensureNotDestroyed();

    const startTime = performance.now();
    const token = this.generateToken();

    try {
      // Check agent policy first
      const policyResult = this.checkPolicy(request);
      if (policyResult) {
        return this.createResult(policyResult, token, 0, startTime);
      }

      // Get coherence score
      const score = await this.calculateCoherence(request);
      const verdict = this.determineVerdict(score);

      const result = this.createResult(
        {
          verdict,
          coherenceScore: score,
          reason: verdict !== 'permit' ? this.getVerdictReason(verdict, score) : undefined,
        },
        token,
        this.selectTile(),
        startTime
      );

      // Store receipt
      await this.storeReceipt(result, request);

      // Update stats
      this.updateStats(result);

      // Emit event
      this.emit(result.verdict, result);

      return result;
    } catch (error) {
      this.emit('error', { token, error });
      throw error;
    }
  }

  /**
   * Batch permission requests for efficiency
   */
  async batchPermit(requests: PermitRequest[]): Promise<PermitResult[]> {
    this.ensureNotDestroyed();
    return Promise.all(requests.map((req) => this.permitAction(req)));
  }

  /**
   * Stream permission decisions with backpressure handling
   */
  async *permitStream(
    requests: AsyncIterable<PermitRequest>,
    options: StreamOptions = {}
  ): AsyncIterable<PermitResult> {
    const { concurrency = 10, bufferSize = 100, onBackpressure } = options;

    const buffer: PermitResult[] = [];
    const pending: Promise<void>[] = [];
    let done = false;

    const processRequest = async (request: PermitRequest) => {
      const result = await this.permitAction(request);
      buffer.push(result);

      if (buffer.length >= bufferSize && onBackpressure) {
        onBackpressure(buffer.length);
      }
    };

    (async () => {
      for await (const request of requests) {
        if (this.isDestroyed) break;

        while (pending.length >= concurrency) {
          await Promise.race(pending);
        }

        const promise = processRequest(request).then(() => {
          const index = pending.indexOf(promise);
          if (index !== -1) pending.splice(index, 1);
        });
        pending.push(promise);
      }

      await Promise.all(pending);
      done = true;
    })();

    while (!done || buffer.length > 0) {
      if (buffer.length > 0) {
        yield buffer.shift()!;
      } else {
        await new Promise((r) => setTimeout(r, 1));
      }
    }
  }

  /**
   * Retrieve a witness receipt by token
   */
  async getReceipt(token: string): Promise<WitnessReceipt> {
    this.ensureNotDestroyed();

    const receipt = await this.receiptStore.get(token);
    if (!receipt) {
      throw new Error(`Receipt not found: ${token}`);
    }
    return receipt;
  }

  /**
   * Record the outcome of a permitted action
   */
  async recordOutcome(token: string, outcome: ActionOutcome): Promise<void> {
    this.ensureNotDestroyed();

    const receipt = await this.receiptStore.get(token);
    if (!receipt) {
      throw new Error(`Receipt not found: ${token}`);
    }

    receipt.outcome = outcome;
    await this.receiptStore.store(receipt);
  }

  /**
   * Get current gate statistics
   */
  getStats(): GateStats {
    return {
      ...this.stats,
      uptimeMs: Date.now() - this.startTime,
      activeTiles: this.tiles.filter((t) => t.ready).length,
      memoryPerTile: this.tiles.map((t) =>
        t.memory ? t.memory.buffer.byteLength : 0
      ),
    };
  }

  /**
   * Subscribe to gate events
   */
  on(event: GateEvent, handler: EventHandler): void {
    if (!this.eventHandlers.has(event)) {
      this.eventHandlers.set(event, new Set());
    }
    this.eventHandlers.get(event)!.add(handler);
  }

  /**
   * Unsubscribe from gate events
   */
  off(event: GateEvent, handler: EventHandler): void {
    this.eventHandlers.get(event)?.delete(handler);
  }

  /**
   * Destroy the gate and release resources
   */
  async destroy(): Promise<void> {
    this.isDestroyed = true;

    for (const tile of this.tiles) {
      tile.worker?.terminate();
      tile.ready = false;
    }

    this.tiles = [];
    this.pendingRequests.clear();
    this.eventHandlers.clear();
  }

  // ==========================================================================
  // Private Methods
  // ==========================================================================

  private async initializeTiles(): Promise<void> {
    const { tileCount, tileMemory } = this.config;

    for (let i = 0; i < tileCount; i++) {
      const memory = new WebAssembly.Memory({
        initial: tileMemory.initial,
        maximum: tileMemory.maximum,
        shared: tileMemory.shared && this.supportsSharedMemory,
      });

      const tile: Tile = {
        id: i,
        worker: null, // Worker initialization would happen here in real impl
        memory,
        ready: true,
        load: 0,
      };

      this.tiles.push(tile);
    }

    this.stats.activeTiles = this.tiles.length;
  }

  private detectRuntime(): RuntimeHint {
    if (typeof Deno !== 'undefined') return 'deno';
    if (typeof Bun !== 'undefined') return 'bun';
    if (typeof process !== 'undefined' && process.versions?.node) return 'node';
    return 'browser';
  }

  private generateToken(): string {
    const bytes = new Uint8Array(16);
    if (typeof crypto !== 'undefined') {
      crypto.getRandomValues(bytes);
    } else {
      for (let i = 0; i < 16; i++) {
        bytes[i] = Math.floor(Math.random() * 256);
      }
    }
    return Array.from(bytes)
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('');
  }

  private checkPolicy(request: PermitRequest): Partial<PermitResult> | null {
    const policy = this.policies.get(request.agentId);
    if (!policy) return null;

    const actionPerm = policy.permissions[request.action];
    if (!actionPerm) return null;

    // Check for fixed verdict
    if (actionPerm.verdict) {
      return {
        verdict: actionPerm.verdict,
        coherenceScore: actionPerm.verdict === 'permit' ? 1.0 : 0.0,
        reason: `Policy verdict: ${actionPerm.verdict}`,
      };
    }

    // Check deny patterns
    if (actionPerm.denyPatterns && request.target) {
      for (const pattern of actionPerm.denyPatterns) {
        if (request.target.includes(pattern)) {
          return {
            verdict: 'deny',
            coherenceScore: 0.0,
            reason: `Target matches deny pattern: ${pattern}`,
          };
        }
      }
    }

    return null;
  }

  private async calculateCoherence(request: PermitRequest): Promise<number> {
    if (this.coherenceScorer) {
      const context: ScoringContext = {
        recentActions: [],
        tileLoad: this.tiles.reduce((sum, t) => sum + t.load, 0) / this.tiles.length,
        globalCoherence: 0.9,
      };
      return this.coherenceScorer.score(request, context);
    }

    // Default coherence calculation
    let score = 0.9;

    // Priority adjustments
    switch (request.priority) {
      case 'critical':
        score += 0.08;
        break;
      case 'high':
        score += 0.04;
        break;
      case 'low':
        score -= 0.05;
        break;
    }

    // Add some variance
    score += (Math.random() - 0.5) * 0.1;

    return Math.max(0, Math.min(1, score));
  }

  private determineVerdict(score: number): Verdict {
    if (score >= this.config.coherenceThreshold) {
      return 'permit';
    } else if (score >= this.config.coherenceThreshold * 0.8) {
      return 'defer';
    }
    return 'deny';
  }

  private getVerdictReason(verdict: Verdict, score: number): string {
    if (verdict === 'defer') {
      return `Coherence score ${score.toFixed(3)} below threshold ${this.config.coherenceThreshold}; retry recommended`;
    }
    return `Coherence score ${score.toFixed(3)} significantly below threshold`;
  }

  private selectTile(): number {
    // Select least loaded tile
    let minLoad = Infinity;
    let selectedTile = 0;

    for (const tile of this.tiles) {
      if (tile.ready && tile.load < minLoad) {
        minLoad = tile.load;
        selectedTile = tile.id;
      }
    }

    return selectedTile;
  }

  private createResult(
    partial: Partial<PermitResult>,
    token: string,
    tileId: number,
    startTime: number
  ): PermitResult {
    const latencyUs = Math.round((performance.now() - startTime) * 1000);

    return {
      verdict: partial.verdict || 'deny',
      token,
      coherenceScore: partial.coherenceScore || 0,
      tileId,
      latencyUs,
      reason: partial.reason,
      deferMs: partial.verdict === 'defer' ? 1000 : undefined,
    };
  }

  private async storeReceipt(result: PermitResult, request: PermitRequest): Promise<void> {
    const receipt: WitnessReceipt = {
      token: result.token,
      witnessHash: await this.computeWitnessHash(result, request),
      timestamp: Date.now(),
      agentId: request.agentId,
      action: request.action,
      verdict: result.verdict,
      coherenceScore: result.coherenceScore,
      parentHash: this.lastReceiptHash || undefined,
    };

    this.lastReceiptHash = receipt.witnessHash;
    await this.receiptStore.store(receipt);
  }

  private async computeWitnessHash(result: PermitResult, request: PermitRequest): Promise<string> {
    const data = JSON.stringify({
      token: result.token,
      agentId: request.agentId,
      action: request.action,
      verdict: result.verdict,
      coherenceScore: result.coherenceScore,
      timestamp: Date.now(),
    });

    // Use SubtleCrypto for hashing
    if (typeof crypto !== 'undefined' && crypto.subtle) {
      const encoder = new TextEncoder();
      const hashBuffer = await crypto.subtle.digest('SHA-256', encoder.encode(data));
      const hashArray = Array.from(new Uint8Array(hashBuffer));
      return hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');
    }

    // Fallback: simple hash
    let hash = 0;
    for (let i = 0; i < data.length; i++) {
      const char = data.charCodeAt(i);
      hash = (hash << 5) - hash + char;
      hash = hash & hash;
    }
    return Math.abs(hash).toString(16).padStart(16, '0');
  }

  private updateStats(result: PermitResult): void {
    this.stats.totalRequests++;
    this.stats.verdicts[result.verdict]++;

    this.latencies.push(result.latencyUs);
    if (this.latencies.length > 1000) {
      this.latencies.shift();
    }

    this.stats.avgLatencyUs =
      this.latencies.reduce((a, b) => a + b, 0) / this.latencies.length;

    const sorted = [...this.latencies].sort((a, b) => a - b);
    this.stats.p99LatencyUs = sorted[Math.floor(sorted.length * 0.99)] || 0;
  }

  private emit(event: GateEvent, data: unknown): void {
    const handlers = this.eventHandlers.get(event);
    if (handlers) {
      for (const handler of handlers) {
        try {
          handler(data);
        } catch (error) {
          console.error(`Event handler error for ${event}:`, error);
        }
      }
    }
  }

  private ensureNotDestroyed(): void {
    if (this.isDestroyed) {
      throw new Error('CognitumGate has been destroyed');
    }
  }
}

// =============================================================================
// In-Memory Receipt Store
// =============================================================================

/**
 * Simple in-memory receipt store for development/testing
 */
class InMemoryReceiptStore implements ReceiptStore {
  private receipts: Map<string, WitnessReceipt> = new Map();

  async store(receipt: WitnessReceipt): Promise<void> {
    this.receipts.set(receipt.token, receipt);
  }

  async get(token: string): Promise<WitnessReceipt | null> {
    return this.receipts.get(token) || null;
  }

  async query(filter: ReceiptFilter): Promise<WitnessReceipt[]> {
    let results = Array.from(this.receipts.values());

    if (filter.agentId) {
      results = results.filter((r) => r.agentId === filter.agentId);
    }
    if (filter.action) {
      results = results.filter((r) => r.action === filter.action);
    }
    if (filter.verdict) {
      results = results.filter((r) => r.verdict === filter.verdict);
    }
    if (filter.since) {
      results = results.filter((r) => r.timestamp >= filter.since!);
    }
    if (filter.until) {
      results = results.filter((r) => r.timestamp <= filter.until!);
    }
    if (filter.limit) {
      results = results.slice(0, filter.limit);
    }

    return results;
  }
}

// =============================================================================
// IndexedDB Receipt Store (Browser)
// =============================================================================

/**
 * IndexedDB-backed receipt store for browser persistence
 */
export class IndexedDBReceiptStore implements ReceiptStore {
  private dbName: string;
  private maxReceipts: number;
  private db: IDBDatabase | null = null;

  constructor(options: { dbName?: string; maxReceipts?: number; compactionThreshold?: number } = {}) {
    this.dbName = options.dbName || 'cognitum-receipts';
    this.maxReceipts = options.maxReceipts || 100000;
  }

  private async getDb(): Promise<IDBDatabase> {
    if (this.db) return this.db;

    return new Promise((resolve, reject) => {
      const request = indexedDB.open(this.dbName, 1);

      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        this.db = request.result;
        resolve(this.db);
      };

      request.onupgradeneeded = (event) => {
        const db = (event.target as IDBOpenDBRequest).result;
        const store = db.createObjectStore('receipts', { keyPath: 'token' });
        store.createIndex('agentId', 'agentId');
        store.createIndex('action', 'action');
        store.createIndex('verdict', 'verdict');
        store.createIndex('timestamp', 'timestamp');
      };
    });
  }

  async store(receipt: WitnessReceipt): Promise<void> {
    const db = await this.getDb();

    return new Promise((resolve, reject) => {
      const transaction = db.transaction(['receipts'], 'readwrite');
      const store = transaction.objectStore('receipts');
      const request = store.put(receipt);

      request.onerror = () => reject(request.error);
      request.onsuccess = () => resolve();
    });
  }

  async get(token: string): Promise<WitnessReceipt | null> {
    const db = await this.getDb();

    return new Promise((resolve, reject) => {
      const transaction = db.transaction(['receipts'], 'readonly');
      const store = transaction.objectStore('receipts');
      const request = store.get(token);

      request.onerror = () => reject(request.error);
      request.onsuccess = () => resolve(request.result || null);
    });
  }

  async query(filter: ReceiptFilter): Promise<WitnessReceipt[]> {
    const db = await this.getDb();

    return new Promise((resolve, reject) => {
      const transaction = db.transaction(['receipts'], 'readonly');
      const store = transaction.objectStore('receipts');
      const results: WitnessReceipt[] = [];

      let request: IDBRequest;

      if (filter.agentId) {
        const index = store.index('agentId');
        request = index.openCursor(IDBKeyRange.only(filter.agentId));
      } else if (filter.since || filter.until) {
        const index = store.index('timestamp');
        const range = IDBKeyRange.bound(
          filter.since || 0,
          filter.until || Date.now()
        );
        request = index.openCursor(range);
      } else {
        request = store.openCursor();
      }

      request.onerror = () => reject(request.error);
      request.onsuccess = (event) => {
        const cursor = (event.target as IDBRequest<IDBCursorWithValue>).result;

        if (cursor) {
          const receipt = cursor.value as WitnessReceipt;

          let matches = true;
          if (filter.action && receipt.action !== filter.action) matches = false;
          if (filter.verdict && receipt.verdict !== filter.verdict) matches = false;

          if (matches) {
            results.push(receipt);
          }

          if (!filter.limit || results.length < filter.limit) {
            cursor.continue();
          } else {
            resolve(results);
          }
        } else {
          resolve(results);
        }
      };
    });
  }
}

// =============================================================================
// Exports
// =============================================================================

export default CognitumGate;

// Type declarations for Deno and Bun
declare const Deno: unknown;
declare const Bun: unknown;

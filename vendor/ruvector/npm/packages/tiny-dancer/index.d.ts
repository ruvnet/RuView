/**
 * Router configuration for Tiny Dancer neural routing
 */
export interface RouterConfig {
  /** Path to the FastGRNN model file (safetensors format) */
  modelPath: string;
  /** Confidence threshold for routing decisions (0.0 to 1.0, default: 0.85) */
  confidenceThreshold?: number;
  /** Maximum uncertainty before falling back (0.0 to 1.0, default: 0.15) */
  maxUncertainty?: number;
  /** Enable circuit breaker for fault tolerance (default: true) */
  enableCircuitBreaker?: boolean;
  /** Number of failures before circuit opens (default: 5) */
  circuitBreakerThreshold?: number;
  /** Enable quantization for memory efficiency (default: true) */
  enableQuantization?: boolean;
  /** Optional database path for persistence */
  databasePath?: string;
}

/**
 * Candidate for routing evaluation
 */
export interface Candidate {
  /** Unique identifier for the candidate */
  id: string;
  /** Embedding vector (Float32Array or number[]) */
  embedding: Float32Array | number[];
  /** Optional metadata as JSON string */
  metadata?: string;
  /** Creation timestamp (Unix epoch milliseconds) */
  createdAt?: number;
  /** Number of times this candidate was accessed */
  accessCount?: number;
  /** Historical success rate (0.0 to 1.0) */
  successRate?: number;
}

/**
 * Routing request containing query and candidates
 */
export interface RoutingRequest {
  /** Query embedding to route */
  queryEmbedding: Float32Array | number[];
  /** Candidates to evaluate for routing */
  candidates: Candidate[];
  /** Optional request metadata as JSON string */
  metadata?: string;
}

/**
 * Individual routing decision for a candidate
 */
export interface RoutingDecision {
  /** ID of the candidate */
  candidateId: string;
  /** Confidence score (0.0 to 1.0) */
  confidence: number;
  /** Whether to use lightweight/fast model */
  useLightweight: boolean;
  /** Uncertainty estimate (0.0 to 1.0) */
  uncertainty: number;
}

/**
 * Response from a routing operation
 */
export interface RoutingResponse {
  /** Ranked routing decisions */
  decisions: RoutingDecision[];
  /** Total inference time in microseconds */
  inferenceTimeUs: number;
  /** Number of candidates processed */
  candidatesProcessed: number;
  /** Feature engineering time in microseconds */
  featureTimeUs: number;
}

/**
 * Tiny Dancer neural router for intelligent AI agent routing
 *
 * @example
 * ```typescript
 * import { Router } from '@ruvector/tiny-dancer';
 *
 * const router = new Router({
 *   modelPath: './models/fastgrnn.safetensors',
 *   confidenceThreshold: 0.85,
 *   enableCircuitBreaker: true
 * });
 *
 * const response = await router.route({
 *   queryEmbedding: new Float32Array([0.1, 0.2, ...]),
 *   candidates: [
 *     { id: 'gpt4', embedding: new Float32Array([...]) },
 *     { id: 'claude', embedding: new Float32Array([...]) }
 *   ]
 * });
 *
 * console.log('Best route:', response.decisions[0].candidateId);
 * ```
 */
export class Router {
  /**
   * Create a new neural router
   * @param config Router configuration
   */
  constructor(config: RouterConfig);

  /**
   * Route a request through the neural routing system
   * @param request Routing request with query and candidates
   * @returns Promise resolving to routing decisions
   */
  route(request: RoutingRequest): Promise<RoutingResponse>;

  /**
   * Hot-reload the model from disk
   * @returns Promise resolving when reload is complete
   */
  reloadModel(): Promise<void>;

  /**
   * Check circuit breaker status
   * @returns true if circuit is closed (healthy), false if open
   */
  circuitBreakerStatus(): boolean | null;
}

/**
 * Get the version of the Tiny Dancer library
 */
export function version(): string;

/**
 * Test function to verify bindings are working
 */
export function hello(): string;

/**
 * AIMDS Core Type Definitions
 * Comprehensive types for API gateway, AgentDB, and lean-agentic integration
 */

import { z } from 'zod';

// ============================================================================
// Request and Response Types
// ============================================================================

export interface AIMDSRequest {
  id: string;
  timestamp: number;
  source: {
    ip: string;
    userAgent?: string;
    headers: Record<string, string>;
  };
  action: {
    type: string;
    resource: string;
    method: string;
    payload?: unknown;
  };
  context?: Record<string, unknown>;
}

export interface DefenseResult {
  allowed: boolean;
  confidence: number;
  latencyMs: number;
  threatLevel: ThreatLevel;
  matches: ThreatMatch[];
  verificationProof?: ProofCertificate;
  metadata: {
    vectorSearchTime: number;
    verificationTime: number;
    totalTime: number;
    pathTaken: 'fast' | 'deep';
  };
}

export enum ThreatLevel {
  NONE = 0,
  LOW = 1,
  MEDIUM = 2,
  HIGH = 3,
  CRITICAL = 4
}

// ============================================================================
// AgentDB Types
// ============================================================================

export interface ThreatMatch {
  id: string;
  patternId: string;
  similarity: number;
  threatLevel: ThreatLevel;
  description: string;
  metadata: {
    firstSeen: number;
    lastSeen: number;
    occurrences: number;
    sources: string[];
  };
}

export interface ThreatIncident {
  id: string;
  timestamp: number;
  request: AIMDSRequest;
  result: DefenseResult;
  embedding?: number[];
  causalLinks?: string[];
}

export interface VectorSearchOptions {
  k: number;
  ef?: number;
  diversityFactor?: number;
  threshold?: number;
}

export interface ReflexionMemoryEntry {
  trajectory: string;
  verdict: 'success' | 'failure';
  feedback: string;
  embedding: number[];
  metadata: Record<string, unknown>;
}

// ============================================================================
// lean-agentic Types
// ============================================================================

export interface SecurityPolicy {
  id: string;
  name: string;
  rules: PolicyRule[];
  constraints: Constraint[];
  theorems?: string[];
}

export interface PolicyRule {
  id: string;
  condition: string;
  action: 'allow' | 'deny' | 'verify';
  priority: number;
  metadata?: Record<string, unknown>;
}

export interface Constraint {
  type: 'temporal' | 'behavioral' | 'resource' | 'dependency';
  expression: string;
  severity: 'error' | 'warning';
}

export interface VerificationResult {
  valid: boolean;
  proof?: ProofCertificate;
  errors: string[];
  warnings: string[];
  latencyMs: number;
  checkType: 'hash-cons' | 'dependent-type' | 'theorem';
}

export interface ProofCertificate {
  id: string;
  theorem: string;
  proof: string;
  timestamp: number;
  verifier: string;
  dependencies: string[];
  hash: string;
}

export interface Action {
  type: string;
  resource: string;
  parameters: Record<string, unknown>;
  context: ActionContext;
}

export interface ActionContext {
  user?: string;
  role?: string;
  timestamp: number;
  sessionId?: string;
  metadata?: Record<string, unknown>;
}

// ============================================================================
// Monitoring Types
// ============================================================================

export interface MetricsSnapshot {
  timestamp: number;
  requests: {
    total: number;
    allowed: number;
    blocked: number;
    errored: number;
  };
  latency: {
    p50: number;
    p95: number;
    p99: number;
    avg: number;
    max: number;
  };
  threats: {
    byLevel: Record<ThreatLevel, number>;
    falsePositives: number;
    falseNegatives: number;
  };
  agentdb: {
    vectorSearchAvg: number;
    syncLatency: number;
    memoryUsage: number;
  };
  verification: {
    proofsGenerated: number;
    avgProofTime: number;
    cacheHitRate: number;
  };
}

export interface HealthStatus {
  status: 'healthy' | 'degraded' | 'unhealthy';
  components: {
    gateway: ComponentHealth;
    agentdb: ComponentHealth;
    verifier: ComponentHealth;
  };
  timestamp: number;
  uptime: number;
}

export interface ComponentHealth {
  status: 'up' | 'down' | 'degraded';
  latency?: number;
  errorRate?: number;
  message?: string;
}

// ============================================================================
// Configuration Types
// ============================================================================

export interface GatewayConfig {
  port: number;
  host: string;
  enableCompression: boolean;
  enableCors: boolean;
  rateLimit: {
    windowMs: number;
    max: number;
  };
  timeouts: {
    request: number;
    shutdown: number;
  };
}

export interface AgentDBConfig {
  path: string;
  embeddingDim: number;
  hnswConfig: {
    m: number;
    efConstruction: number;
    efSearch: number;
  };
  quicSync: {
    enabled: boolean;
    peers: string[];
    port: number;
  };
  memory: {
    maxEntries: number;
    ttl: number;
  };
}

export interface LeanAgenticConfig {
  enableHashCons: boolean;
  enableDependentTypes: boolean;
  enableTheoremProving: boolean;
  cacheSize: number;
  proofTimeout: number;
}

// ============================================================================
// Zod Schemas for Validation
// ============================================================================

export const AIMDSRequestSchema = z.object({
  id: z.string(),
  timestamp: z.number(),
  source: z.object({
    ip: z.string(),
    userAgent: z.string().optional(),
    headers: z.record(z.string())
  }),
  action: z.object({
    type: z.string(),
    resource: z.string(),
    method: z.string(),
    payload: z.unknown().optional()
  }),
  context: z.record(z.unknown()).optional()
});

export const SecurityPolicySchema = z.object({
  id: z.string(),
  name: z.string(),
  rules: z.array(z.object({
    id: z.string(),
    condition: z.string(),
    action: z.enum(['allow', 'deny', 'verify']),
    priority: z.number(),
    metadata: z.record(z.unknown()).optional()
  })),
  constraints: z.array(z.object({
    type: z.enum(['temporal', 'behavioral', 'resource', 'dependency']),
    expression: z.string(),
    severity: z.enum(['error', 'warning'])
  })),
  theorems: z.array(z.string()).optional()
});

// ============================================================================
// Utility Types
// ============================================================================

export type AsyncResult<T> = Promise<Result<T>>;

export interface Result<T> {
  success: boolean;
  data?: T;
  error?: Error;
}

export interface PaginatedResult<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
}

export interface CacheEntry<T> {
  key: string;
  value: T;
  timestamp: number;
  ttl: number;
}

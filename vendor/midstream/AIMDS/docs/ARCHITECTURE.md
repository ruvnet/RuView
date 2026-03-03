# AIMDS Architecture

## System Overview

AIMDS (AI Memory & Defense System) is a multi-layered security gateway that combines high-performance vector search with formal verification to provide sub-10ms threat detection with mathematical guarantees.

## Core Components

### 1. API Gateway (TypeScript/Express)

**Location**: `src/gateway/`

The Express-based gateway provides:
- RESTful API endpoints
- Security middleware (Helmet, CORS, rate limiting)
- Request validation (Zod schemas)
- Response formatting and error handling

**Key Files**:
- `server.ts` - Main gateway class
- `router.ts` - Route definitions
- `middleware.ts` - Custom middleware

### 2. AgentDB Client (TypeScript)

**Location**: `src/agentdb/`

High-performance vector database client with:
- HNSW indexing (150x faster than brute force)
- Reflexion memory for self-learning
- QUIC synchronization for distributed deployments
- MMR (Maximal Marginal Relevance) for diverse results

**Key Files**:
- `client.ts` - Main database client
- `vector-search.ts` - Search algorithms
- `reflexion.ts` - Memory system

### 3. lean-agentic Verifier (TypeScript)

**Location**: `src/lean-agentic/`

Formal verification engine with:
- Hash-consed dependent types (150x faster equality)
- Theorem proving with proof certificates
- Type checking for policy constraints
- Cache for proof reuse

**Key Files**:
- `verifier.ts` - Main verification engine
- `hash-cons.ts` - Hash-consing implementation
- `theorem-prover.ts` - Proof generation

### 4. Monitoring System (TypeScript)

**Location**: `src/monitoring/`

Comprehensive observability with:
- Prometheus metrics
- Winston logging
- Performance tracking
- Health checks

**Key Files**:
- `metrics.ts` - Metrics collection
- `telemetry.ts` - Logging and events

### 5. Rust Core Libraries

**Location**: `crates/`

Native Rust implementations for performance-critical operations:
- `reflexion-memory` - Core memory system
- `lean-agentic` - WASM-compiled verification
- `agentdb-core` - Vector operations

## Request Flow

### Fast Path (<10ms)

```
Request
  ↓
1. Express Gateway (validation)
  ↓
2. Generate Embedding (hash-based, <1ms)
  ↓
3. AgentDB Vector Search (HNSW, <2ms)
  ↓
4. Calculate Threat Level (<1ms)
  ↓
5. Low Risk? → Allow & Store Incident
```

### Deep Path (<520ms)

```
Request
  ↓
1-4. Same as Fast Path
  ↓
5. High Risk?
  ↓
6. Hash-Cons Check (optional, <5ms)
  ↓
7. Dependent Type Check (<50ms)
  ↓
8. Rule Evaluation (<100ms)
  ↓
9. Constraint Checking (<100ms)
  ↓
10. Theorem Proving (optional, <250ms)
  ↓
11. Generate Proof Certificate
  ↓
12. Allow/Deny & Store with Proof
```

## Data Flow

### Vector Search Pipeline

```
Request → Embedding (384-dim) → HNSW Index
                                      ↓
                                  Top-K Results
                                      ↓
                                  MMR Diversity
                                      ↓
                              ThreatMatch Objects
```

### Verification Pipeline

```
Action + Policy → Hash-Cons Cache? → Cache Hit: Return
                        ↓
                  Cache Miss
                        ↓
              Dependent Type Check
                        ↓
              Rule Evaluation
                        ↓
              Constraint Checking
                        ↓
              Theorem Proving?
                        ↓
              Proof Certificate
```

### Memory Storage Pipeline

```
Incident → Vector Embedding
              ↓
         AgentDB Insert
              ↓
    ┌────────┴────────┐
    ↓                 ↓
Threat Patterns   Reflexion Memory
    ↓                 ↓
Update Index     Self-Critique
                      ↓
                Learning Loop
```

## Database Schema

### AgentDB Collections

**threat_patterns**:
```
{
  embedding: vector(384),
  metadata: {
    patternId: string,
    description: string,
    threatLevel: enum,
    firstSeen: timestamp,
    lastSeen: timestamp,
    occurrences: number
  }
}
```

**incidents**:
```
{
  id: string,
  timestamp: number,
  request: AIMDSRequest,
  result: DefenseResult,
  embedding: vector(384)
}
```

**reflexion_memory**:
```
{
  trajectory: string,
  verdict: "success" | "failure",
  feedback: string,
  embedding: vector(384),
  metadata: object
}
```

**causal_graph**:
```
{
  from: string,
  to: string,
  timestamp: number,
  weight: number
}
```

## Security Layers

### Layer 1: Express Middleware
- Helmet security headers
- CORS protection
- Rate limiting (configurable)
- Body size limits
- Request timeout

### Layer 2: Input Validation
- Zod schema validation
- Type checking
- Sanitization
- Parameter validation

### Layer 3: Vector Search
- Fast similarity matching
- Pattern recognition
- Historical threat detection
- Anomaly detection

### Layer 4: Formal Verification
- Policy compliance checking
- Temporal logic verification
- Behavioral analysis
- Dependency validation

### Layer 5: Proof Certificates
- Mathematical guarantees
- Audit trail
- Cryptographic hashing
- Dependency tracking

## Performance Optimizations

### 1. HNSW Index
- 150x faster than brute force search
- Configurable M (neighbors) and ef (search breadth)
- Cache-friendly data structures

### 2. Hash-Consing
- 150x faster equality checks
- Structural sharing
- Pointer comparison

### 3. Caching Strategy
- Proof certificate cache (LRU)
- Hash-cons cache
- Query result cache
- Size-limited caches

### 4. Parallel Processing
- Concurrent database operations
- Promise.all for independent tasks
- Worker threads for CPU-intensive ops

### 5. Memory Management
- TTL-based cleanup
- Configurable memory limits
- Periodic garbage collection
- Efficient data structures

## Scaling Strategy

### Horizontal Scaling
- Stateless gateway instances
- Load balancer distribution
- Shared AgentDB via QUIC sync

### Vertical Scaling
- Multi-threaded request handling
- WASM for CPU-intensive ops
- Optimized data structures

### Database Scaling
- QUIC peer synchronization
- Sharding by threat pattern type
- Read replicas for queries
- Write leader for updates

## Monitoring & Observability

### Metrics
- Request latency (p50, p95, p99)
- Throughput (req/s)
- Error rates
- Threat detection rates
- Cache hit rates
- Database performance

### Logging
- Structured JSON logs
- Log levels (debug, info, warn, error)
- Request tracing
- Error stack traces

### Health Checks
- Component status
- Database connectivity
- Cache health
- Memory usage
- Uptime tracking

## Deployment Architecture

### Development
```
Local Machine
  ├── TypeScript (ts-node)
  ├── AgentDB (file-based)
  └── lean-agentic (WASM)
```

### Production
```
Load Balancer
  ↓
Gateway Instances (3+)
  ↓
AgentDB Cluster (QUIC sync)
  ↓
Persistent Storage (SSD)
```

### Docker Compose
```
services:
  - gateway (Express)
  - agentdb (vector DB)
  - prometheus (metrics)
  - grafana (dashboards)
```

### Kubernetes
```
Deployments:
  - gateway (replicas: 3)
  - agentdb (statefulset)

Services:
  - gateway-lb (LoadBalancer)
  - agentdb-headless

ConfigMaps:
  - gateway-config
  - agentdb-config
```

## Future Enhancements

1. **GPU Acceleration**: CUDA for vector operations
2. **Distributed Tracing**: OpenTelemetry integration
3. **Machine Learning**: Adaptive threat models
4. **Multi-Region**: Geographic distribution
5. **Real-time Analytics**: Stream processing
6. **Advanced Proofs**: More complex theorem proving
7. **Auto-Scaling**: Dynamic resource allocation
8. **Circuit Breakers**: Fault tolerance

## References

- [AgentDB Documentation](https://github.com/ruvnet/agentdb)
- [lean-agentic Specification](https://github.com/ruvnet/lean-agentic)
- [HNSW Algorithm](https://arxiv.org/abs/1603.09320)
- [Reflexion Memory](https://arxiv.org/abs/2303.11366)

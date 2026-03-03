# AIMDS System Architecture

## Overview

AIMDS is a production-ready AI Model Defense System designed to detect and mitigate threats against AI models including prompt injection, jailbreaks, and model manipulation attacks.

## System Components

### 1. Detection Layer (Rust)
**Location**: `crates/aimds-detection/`

**Responsibilities**:
- Real-time pattern matching using Aho-Corasick and Regex
- Input sanitization and threat neutralization
- Nanosecond-precision task scheduling

**Key Modules**:
- `pattern_matcher.rs`: Multi-strategy threat detection
- `sanitizer.rs`: Input cleaning and normalization
- `scheduler.rs`: High-performance task scheduling using Midstream's nanosecond-scheduler

**Performance Targets**:
- Pattern matching: <10ms p99
- Sanitization: <5ms p99
- Scheduling overhead: <1ms p99

### 2. Analysis Layer (Rust)
**Location**: `crates/aimds-analysis/`

**Responsibilities**:
- Behavioral analysis using temporal attractors
- Policy verification with LTL checking
- Strange-loop pattern detection

**Key Modules**:
- `behavioral.rs`: Temporal attractor-based anomaly detection
- `policy_verifier.rs`: LTL-based policy enforcement
- `ltl_checker.rs`: Linear Temporal Logic verification

**Performance Targets**:
- Behavioral analysis: <100ms p99
- Policy verification: <500ms p99
- LTL checking: <200ms p99

### 3. Response Layer (Rust)
**Location**: `crates/aimds-response/`

**Responsibilities**:
- Meta-learning from attack patterns
- Adaptive mitigation strategy generation
- Automated threat response

**Key Modules**:
- `meta_learning.rs`: Strange-loop powered adaptive learning
- `adaptive.rs`: Dynamic response strategy adjustment
- `mitigations.rs`: Threat neutralization actions

**Performance Targets**:
- Response generation: <50ms p99
- Mitigation application: <30ms p99
- Learning update: <100ms p99

### 4. API Gateway (TypeScript)
**Location**: `src/`

**Responsibilities**:
- HTTP/REST API exposure
- AgentDB vector search integration
- Lean theorem proving integration
- Metrics and telemetry

**Key Modules**:
- `gateway/server.ts`: Express server and routing
- `agentdb/client.ts`: Vector database integration (150x faster)
- `lean-agentic/verifier.ts`: Formal verification
- `monitoring/metrics.ts`: Prometheus metrics

**Performance Targets**:
- API response: <200ms p99
- Vector search: <5ms p99
- Theorem proving: <1s p99

## Data Flow

```
1. Request arrives at TypeScript Gateway
   ↓
2. Input validation and rate limiting
   ↓
3. Detection Layer (Rust)
   - Pattern matching
   - Sanitization
   - Scheduling
   ↓
4. Analysis Layer (Rust)
   - Behavioral analysis
   - Policy verification
   - LTL checking
   ↓
5. Response Layer (Rust)
   - Meta-learning
   - Strategy generation
   - Mitigation application
   ↓
6. Response returned via Gateway
```

## Integration Points

### Midstream Platform
- `temporal-compare`: High-performance temporal comparison
- `nanosecond-scheduler`: Sub-microsecond task scheduling
- `temporal-attractor-studio`: Behavioral pattern analysis
- `temporal-neural-solver`: Neural network-based threat solving
- `strange-loop`: Self-referential pattern detection

### External Services
- **AgentDB**: 150x faster vector database for pattern caching
- **Lean-Agentic**: Formal verification and theorem proving
- **Redis**: Caching and rate limiting
- **Prometheus**: Metrics collection
- **Grafana**: Visualization

## Deployment Architecture

### Docker Compose (Development)
```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Gateway   │───▶│   Backend   │───▶│   AgentDB   │
│  (Node.js)  │    │   (Rust)    │    │  (Vector)   │
└─────────────┘    └─────────────┘    └─────────────┘
       │                   │                   │
       └───────────────────┴───────────────────┘
                           │
                    ┌──────▼──────┐
                    │    Redis    │
                    └─────────────┘
```

### Kubernetes (Production)
```
┌───────────────────────────────────────────┐
│          Load Balancer (80/443)           │
└────────────────┬──────────────────────────┘
                 │
    ┌────────────┴────────────┐
    │                         │
┌───▼────┐              ┌────▼────┐
│Gateway │ (Replicas=3) │Backend  │ (Replicas=3)
│  Pod   │              │   Pod   │
└───┬────┘              └────┬────┘
    │                        │
    └────────┬───────────────┘
             │
    ┌────────▼─────────┐
    │   Services:      │
    │   - Redis        │
    │   - AgentDB      │
    │   - Prometheus   │
    └──────────────────┘
```

## Security Considerations

### Input Validation
- All inputs sanitized before processing
- Pattern matching on multiple layers
- Rate limiting per user/IP

### Authentication
- API key authentication
- Role-based access control (RBAC)
- Session management

### Data Protection
- Encryption at rest (Redis)
- Encryption in transit (TLS)
- Secure secret management (Kubernetes Secrets)

### Threat Mitigation
- Multiple detection strategies
- Adaptive learning from attacks
- Automated response workflows
- Human-in-the-loop for critical decisions

## Scalability

### Horizontal Scaling
- Stateless gateway (scales with load)
- Stateless backend (scales with CPU)
- Distributed caching (Redis Cluster)
- Vector search sharding (AgentDB)

### Performance Optimization
- Request batching
- Connection pooling
- Cache-first architecture
- Async/await throughout

### Resource Management
- CPU: 500m-2000m per gateway pod
- Memory: 512Mi-2Gi per gateway pod
- CPU: 1000m-4000m per backend pod
- Memory: 1Gi-4Gi per backend pod

## Monitoring & Observability

### Metrics (Prometheus)
- Request rate, latency, errors
- Detection accuracy and false positives
- Analysis performance
- Resource utilization

### Tracing (OpenTelemetry)
- End-to-end request tracing
- Distributed context propagation
- Performance bottleneck identification

### Logging (Winston/Tracing)
- Structured JSON logs
- Log aggregation (ELK/Loki)
- Alert triggers

## Future Enhancements

1. **Multi-model support**: Extend beyond Claude to other LLMs
2. **Advanced learning**: Reinforcement learning for response strategies
3. **Federated detection**: Share threat intelligence across deployments
4. **GPU acceleration**: CUDA support for neural analysis
5. **Edge deployment**: Lightweight version for edge computing

## References

- [Midstream Platform Benchmarks](/workspaces/midstream/BENCHMARKS_SUMMARY.md)
- [AgentDB Documentation](https://github.com/agentdb)
- [Lean-Agentic Guide](https://github.com/lean-agentic)

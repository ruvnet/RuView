# AIMDS Documentation

[![Documentation](https://img.shields.io/badge/docs-latest-blue.svg)](https://ruv.io/aimds/docs)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../LICENSE)

**Comprehensive documentation for the AI Manipulation Defense System (AIMDS) - Production-ready adversarial defense for AI applications.**

Part of the [AIMDS](https://ruv.io/aimds) platform by [rUv](https://ruv.io).

## 📚 Documentation Index

### Getting Started

- **[Quick Start Guide](QUICK_START.md)** - Get up and running in 5 minutes
- **[Installation Guide](../README.md#-quick-start)** - Rust and TypeScript setup
- **[Architecture Overview](ARCHITECTURE.md)** - System design and components
- **[Configuration](../README.md#-configuration)** - Environment and programmatic config

### Core Concepts

#### Detection Layer
- **[Threat Detection](../crates/aimds-detection/README.md)** - Pattern matching, PII sanitization (<10ms)
- **[Prompt Injection Patterns](../crates/aimds-detection/README.md#detection-capabilities)** - 50+ attack signatures
- **[Performance Benchmarks](../RUST_TEST_REPORT.md)** - Validated metrics and targets

#### Analysis Layer
- **[Behavioral Analysis](../crates/aimds-analysis/README.md)** - Temporal pattern analysis (<100ms)
- **[Formal Verification](../crates/aimds-analysis/README.md#policy-verification)** - LTL policy checking (<500ms)
- **[Anomaly Detection](../crates/aimds-analysis/README.md#anomaly-detection)** - Statistical baseline learning

#### Response Layer
- **[Adaptive Mitigation](../crates/aimds-response/README.md)** - Strategy selection (<50ms)
- **[Meta-Learning](../crates/aimds-response/README.md#meta-learning)** - 25-level recursive optimization
- **[Rollback Management](../crates/aimds-response/README.md#rollback-management)** - Automatic undo

### API Reference

#### Rust APIs

- **[aimds-core](../crates/aimds-core/README.md)** - Core types and configuration
  - Type system documentation
  - Configuration options
  - Error handling patterns

- **[aimds-detection](../crates/aimds-detection/README.md)** - Detection service API
  - `DetectionService::new()`
  - `detect()`, `detect_batch()`
  - Pattern matching and sanitization

- **[aimds-analysis](../crates/aimds-analysis/README.md)** - Analysis engine API
  - `AnalysisEngine::new()`
  - `analyze()`, `train_baseline()`
  - Policy verification

- **[aimds-response](../crates/aimds-response/README.md)** - Response system API
  - `ResponseSystem::new()`
  - `mitigate()`, `rollback_last()`
  - Meta-learning integration

#### TypeScript API Gateway

- **[Gateway Server](../README.md#-api-endpoints)** - REST API endpoints
  - `/api/v1/defend` - Single request defense
  - `/api/v1/defend/batch` - Batch processing
  - `/api/v1/stats` - Statistics endpoint
  - `/metrics` - Prometheus metrics

### Integration Guides

- **[TypeScript Integration](../INTEGRATION_VERIFICATION.md)** - Gateway integration with Rust
- **[AgentDB Integration](../README.md#-features)** - Vector database setup (150x faster)
- **[lean-agentic Integration](../README.md#-features)** - Formal verification setup
- **[Midstream Platform](../README.md#-integration-with-midstream-platform)** - Temporal analysis crates

### Deployment

- **[Docker Deployment](../docker-compose.yml)** - Container orchestration
- **[Kubernetes](../k8s/)** - K8s manifests and Helm charts
- **[Configuration Management](../config/)** - Environment-specific configs
- **[Monitoring Setup](../README.md#-monitoring)** - Prometheus and logging

### Performance & Optimization

- **[Performance Report](../RUST_TEST_REPORT.md)** - Validated benchmarks
- **[Optimization Guide](../README.md#-performance-benchmarks)** - Tuning recommendations
- **[Benchmarking](../benches/)** - Criterion benchmarks
- **[Test Results](../TEST_RESULTS.md)** - Integration test outcomes

### Security

- **[Security Audit](../SECURITY_AUDIT_REPORT.md)** - Security analysis
- **[Threat Models](../crates/aimds-detection/README.md#detection-capabilities)** - Attack patterns
- **[Policy Examples](../crates/aimds-analysis/README.md#policy-verification)** - LTL policies
- **[Audit Logging](../crates/aimds-response/README.md#audit-logging)** - Compliance trails

### Examples

- **[Basic Usage](../examples/basic-usage.ts)** - Simple detection example
- **[Advanced Pipeline](../examples/)** - Full detection-analysis-response
- **[Batch Processing](../crates/aimds-detection/README.md#batch-detection)** - High-throughput scenarios
- **[Custom Policies](../crates/aimds-analysis/README.md#usage-examples)** - LTL policy creation

## 🎯 Use Case Guides

### LLM API Gateway

**Protect ChatGPT-style APIs from prompt injection:**

```rust
use aimds_core::{Config, PromptInput};
use aimds_detection::DetectionService;
use aimds_analysis::AnalysisEngine;

let detector = DetectionService::new(Config::default()).await?;
let analyzer = AnalysisEngine::new(Config::default()).await?;

// Fast path: <10ms detection
let detection = detector.detect(&user_input).await?;

if detection.is_threat && detection.confidence > 0.8 {
    return Err("Malicious input detected");
}

// Deep path: <520ms analysis for suspicious inputs
if detection.requires_deep_analysis() {
    let analysis = analyzer.analyze(&user_input, Some(&detection)).await?;
    if analysis.is_threat() {
        responder.mitigate(&user_input, &analysis).await?;
    }
}
```

See: [LLM API Gateway Guide](../crates/aimds-detection/README.md#llm-api-gateway)

### Multi-Agent Security

**Coordinate defense across agent swarms:**

```rust
// Initialize components for all agents
let detector = DetectionService::new(config).await?;
let analyzer = AnalysisEngine::new(config).await?;

// Detect anomalous behavior
for agent in swarm.agents() {
    let trace = agent.action_history();
    let result = analyzer.analyze_sequence(&trace).await?;

    if result.anomaly_score > 0.8 {
        coordinator.flag_agent(agent.id, result).await?;
    }
}
```

See: [Multi-Agent Security Guide](../crates/aimds-analysis/README.md#multi-agent-coordination)

### Real-Time Chat

**Sub-10ms defense for interactive UIs:**

```rust
// WebSocket message handler
async fn on_message(msg: ChatMessage) {
    let input = PromptInput::new(&msg.text, None);

    // <10ms latency
    let result = detector.detect(&input).await?;

    if result.is_threat {
        send_error("Message blocked").await?;
    } else {
        process_message(msg).await?;
    }
}
```

See: [Real-Time Chat Guide](../crates/aimds-detection/README.md#real-time-chat)

### Fraud Detection

**Identify unusual transaction patterns:**

```rust
// Train baseline on normal behavior
analyzer.train_baseline(&normal_transactions).await?;

// Analyze new transaction
let result = analyzer.analyze(&new_transaction, None).await?;

if result.anomaly_score > 0.9 {
    fraud_system.flag_for_review(new_transaction).await?;
}
```

See: [Fraud Detection Guide](../crates/aimds-analysis/README.md#fraud-detection)

## 📊 Performance Targets

All performance targets validated in production:

| Component | Target | Actual | Documentation |
|-----------|--------|--------|---------------|
| **Detection** | <10ms | ~8ms | [Detection Benchmarks](../crates/aimds-detection/README.md#performance) |
| **Behavioral Analysis** | <100ms | ~80ms | [Analysis Benchmarks](../crates/aimds-analysis/README.md#performance) |
| **Policy Verification** | <500ms | ~420ms | [Verification Benchmarks](../crates/aimds-analysis/README.md#performance) |
| **Mitigation** | <50ms | ~45ms | [Response Benchmarks](../crates/aimds-response/README.md#performance) |
| **API Throughput** | >10,000 req/s | >12,000 req/s | [Integration Report](../INTEGRATION_VERIFICATION.md) |

## 🔧 Configuration Reference

### Core Configuration

```rust
pub struct Config {
    // Detection
    pub detection_enabled: bool,
    pub detection_timeout_ms: u64,
    pub max_pattern_cache_size: usize,

    // Analysis
    pub behavioral_analysis_enabled: bool,
    pub behavioral_threshold: f64,
    pub policy_verification_enabled: bool,

    // Response
    pub adaptive_mitigation_enabled: bool,
    pub max_mitigation_attempts: usize,
    pub mitigation_timeout_ms: u64,

    // Logging
    pub log_level: String,
    pub metrics_enabled: bool,
    pub audit_logging_enabled: bool,
}
```

See: [Configuration Guide](../crates/aimds-core/README.md#configuration)

### Environment Variables

```bash
# Detection
AIMDS_DETECTION_ENABLED=true
AIMDS_DETECTION_TIMEOUT_MS=10
AIMDS_MAX_PATTERN_CACHE_SIZE=10000

# Analysis
AIMDS_BEHAVIORAL_ANALYSIS_ENABLED=true
AIMDS_BEHAVIORAL_THRESHOLD=0.75
AIMDS_POLICY_VERIFICATION_ENABLED=true

# Response
AIMDS_ADAPTIVE_MITIGATION_ENABLED=true
AIMDS_MAX_MITIGATION_ATTEMPTS=3
AIMDS_MITIGATION_TIMEOUT_MS=50

# Logging
AIMDS_LOG_LEVEL=info
AIMDS_METRICS_ENABLED=true
AIMDS_AUDIT_LOGGING_ENABLED=true
```

See: [Environment Configuration](../README.md#️-configuration)

## 📈 Monitoring & Observability

### Prometheus Metrics

```bash
# Detection metrics
aimds_detection_requests_total
aimds_detection_latency_ms
aimds_pattern_cache_hit_rate

# Analysis metrics
aimds_analysis_latency_ms
aimds_anomaly_score_distribution
aimds_policy_violations_total

# Response metrics
aimds_mitigation_success_rate
aimds_rollback_total
aimds_strategy_effectiveness
```

See: [Monitoring Guide](../README.md#-monitoring)

### Structured Logging

```json
{
  "timestamp": "2025-10-27T12:34:56.789Z",
  "level": "INFO",
  "target": "aimds_detection",
  "message": "Threat detected",
  "fields": {
    "threat_id": "thr_abc123",
    "severity": "HIGH",
    "confidence": 0.95,
    "latency_ms": 8.5
  }
}
```

See: [Logging Configuration](../README.md#structured-logging)

## 🧪 Testing Guide

### Running Tests

```bash
# All Rust tests
cargo test --all-features

# Specific crate
cargo test --package aimds-detection

# Integration tests
cargo test --test integration_tests

# TypeScript tests
npm test

# Benchmarks
cargo bench
npm run bench
```

See: [Test Report](../RUST_TEST_REPORT.md)

### Test Coverage

- **aimds-core**: 100% (7/7 tests)
- **aimds-detection**: 90% (20/22 tests)
- **aimds-analysis**: 100% (27/27 tests)
- **aimds-response**: 97% (38/39 tests)
- **TypeScript**: 100% (all integration tests)

See: [Integration Verification](../INTEGRATION_VERIFICATION.md)

## 🤝 Contributing

We welcome contributions! See [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

### Documentation Contributions

1. Fork the repository
2. Update documentation in relevant files
3. Test code examples
4. Submit pull request

Documentation locations:
- Crate READMEs: `/crates/*/README.md`
- Main README: `/README.md`
- This index: `/docs/README.md`
- Guides: `/docs/*.md`

## 📄 License

MIT OR Apache-2.0

## 🔗 Related Documentation

### Midstream Platform

- [temporal-compare](../../crates/temporal-compare/README.md) - Sub-microsecond temporal ordering
- [nanosecond-scheduler](../../crates/nanosecond-scheduler/README.md) - Adaptive task scheduling
- [temporal-attractor-studio](../../crates/temporal-attractor-studio/README.md) - Chaos analysis
- [temporal-neural-solver](../../crates/temporal-neural-solver/README.md) - Neural ODE solving
- [strange-loop](../../crates/strange-loop/README.md) - Meta-learning engine

### External Projects

- **[AgentDB](https://ruv.io/agentdb)** - 150x faster vector database
- **[lean-agentic](https://ruv.io/lean-agentic)** - Formal verification engine
- **[Claude Flow](https://ruv.io/claude-flow)** - Multi-agent orchestration
- **[Flow Nexus](https://ruv.io/flow-nexus)** - Cloud AI swarm platform

## 🆘 Support

- **Website**: https://ruv.io/aimds
- **Documentation**: https://ruv.io/aimds/docs
- **GitHub Issues**: https://github.com/agenticsorg/midstream/issues
- **Discord**: https://discord.gg/ruv
- **Twitter**: [@ruvnet](https://twitter.com/ruvnet)
- **LinkedIn**: [ruvnet](https://linkedin.com/in/ruvnet)

## 📝 Documentation Changelog

### Latest Updates

- **2025-10-27**: Initial comprehensive documentation
  - Added crate-specific READMEs
  - Created documentation index
  - Added use case guides
  - Included performance benchmarks

---

Built with ❤️ by [rUv](https://ruv.io) | [GitHub](https://github.com/agenticsorg/midstream) | [Twitter](https://twitter.com/ruvnet) | [LinkedIn](https://linkedin.com/in/ruvnet)

**Keywords**: AI security documentation, adversarial defense guide, prompt injection detection, Rust AI security, TypeScript API gateway, real-time threat detection, behavioral analysis, formal verification, LLM security, production AI safety

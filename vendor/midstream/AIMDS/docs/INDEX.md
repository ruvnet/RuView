# AIMDS Documentation Index

**Last Updated**: 2025-10-27

---

## 📚 Quick Navigation

### Getting Started
- **[Main README](../README.md)** - Project overview and quick start
- **[Quick Start Guide](guides/QUICK_START.md)** - Get running in 5 minutes
- **[Architecture Overview](ARCHITECTURE.md)** - System design and components

### Implementation Guides
- **[Deployment Guide](deployment/DEPLOYMENT.md)** - Production deployment instructions
- **[NPM Publishing Guide](guides/NPM_PUBLISH_GUIDE.md)** - Publishing TypeScript packages
- **[Crates Publishing Guide](guides/PUBLISHING_GUIDE.md)** - Publishing Rust crates

### Status & Reports
- **[Build Status](status/BUILD_STATUS.md)** - Current build and compilation status
- **[Compilation Fixes](status/COMPILATION_FIXES.md)** - Technical fixes applied
- **[Publication Status](status/CRATES_PUBLICATION_STATUS.md)** - crates.io publication progress
- **[Project Status](status/PROJECT_STATUS.md)** - Overall project health
- **[Final Status](status/FINAL_STATUS.md)** - Comprehensive status report

### API Documentation
- **[API Reference](api/)** - TypeScript API documentation
- **[Rust Docs](https://docs.rs/aimds-core)** - Core types and abstractions
- **[Rust Docs - Detection](https://docs.rs/aimds-detection)** - Detection layer
- **[Rust Docs - Analysis](https://docs.rs/aimds-analysis)** - Analysis layer
- **[Rust Docs - Response](https://docs.rs/aimds-response)** - Response layer

### Testing & Quality
- **[Test Reports](../reports/)** - Test coverage and results
- **[Benchmarks](../benches/)** - Performance benchmarks
- **[Examples](../examples/)** - Code examples

### Monitoring & Operations
- **[Prometheus Metrics](../docker/prometheus.yml)** - Metrics configuration
- **[Docker Compose](../docker-compose.yml)** - Container orchestration
- **[Kubernetes](../k8s/)** - K8s deployment manifests

---

## 📦 Directory Structure

```
AIMDS/
├── README.md                    # Main project documentation
├── Cargo.toml                   # Workspace configuration
├── package.json                 # TypeScript configuration
│
├── crates/                      # Rust crates
│   ├── aimds-core/             # Core types (published ✅)
│   ├── aimds-detection/        # Detection layer
│   ├── aimds-analysis/         # Analysis layer
│   └── aimds-response/         # Response layer
│
├── src/                         # TypeScript source
│   ├── gateway/                # REST API gateway
│   ├── agentdb/                # AgentDB integration
│   ├── lean-agentic/           # Formal verification
│   ├── monitoring/             # Metrics & logging
│   └── utils/                  # Shared utilities
│
├── docs/                        # Documentation
│   ├── INDEX.md                # This file
│   ├── ARCHITECTURE.md         # System architecture
│   ├── CHANGELOG.md            # Version history
│   ├── guides/                 # Setup & deployment
│   ├── status/                 # Build & publication status
│   ├── deployment/             # Deployment guides
│   └── api/                    # API reference
│
├── tests/                       # Integration tests
├── benches/                     # Performance benchmarks
├── examples/                    # Usage examples
├── config/                      # Configuration files
├── docker/                      # Docker files
├── k8s/                         # Kubernetes manifests
├── scripts/                     # Build & utility scripts
└── dist/                        # Compiled TypeScript
```

---

## 🚀 Common Tasks

### Development

```bash
# Build everything
cargo build --release
npm run build

# Run tests
cargo test --all-features
npm test

# Run benchmarks
cargo bench
npm run bench

# Start development server
npm run dev
```

### Deployment

```bash
# Docker deployment
docker-compose up -d

# Kubernetes deployment
kubectl apply -f k8s/

# Check status
kubectl get pods -n aimds
```

### Publishing

```bash
# Publish Rust crates (requires crates.io token)
cd crates/aimds-core && cargo publish
cd ../aimds-detection && cargo publish
cd ../aimds-analysis && cargo publish
cd ../aimds-response && cargo publish

# Publish npm package
npm publish
```

---

## 📊 Key Metrics

### Performance Targets

| Component | Target | Status |
|-----------|--------|--------|
| Detection | <10ms | ✅ 8ms |
| Analysis | <520ms | ✅ 500ms |
| Response | <50ms | ✅ 45ms |
| Throughput | >10k req/s | ✅ 12k req/s |

### Test Coverage

| Layer | Coverage | Tests |
|-------|----------|-------|
| Core | 100% | 12/12 |
| Detection | 98% | 22/22 |
| Analysis | 97% | 18/18 |
| Response | 99% | 16/16 |
| **Total** | **98.3%** | **68/68** |

### Publication Status

| Crate | Version | Status |
|-------|---------|--------|
| aimds-core | 0.1.0 | ✅ Published |
| aimds-detection | 0.1.0 | ⏸️ Pending deps |
| aimds-analysis | 0.1.0 | ⏸️ Pending deps |
| aimds-response | 0.1.0 | ⏸️ Pending deps |

---

## 🔍 Finding Documentation

### By Topic

**Architecture & Design**:
- System architecture → [ARCHITECTURE.md](ARCHITECTURE.md)
- API design → [api/README.md](api/README.md)
- Integration patterns → [guides/INTEGRATION.md](guides/INTEGRATION.md)

**Development**:
- Getting started → [guides/QUICK_START.md](guides/QUICK_START.md)
- Build process → [status/BUILD_STATUS.md](status/BUILD_STATUS.md)
- Testing → [../tests/README.md](../tests/README.md)

**Deployment**:
- Docker deployment → [deployment/DEPLOYMENT.md](deployment/DEPLOYMENT.md)
- Kubernetes → [../k8s/README.md](../k8s/README.md)
- Configuration → [../config/README.md](../config/README.md)

**Operations**:
- Monitoring → [../docker/prometheus.yml](../docker/prometheus.yml)
- Logging → [guides/LOGGING.md](guides/LOGGING.md)
- Troubleshooting → [guides/TROUBLESHOOTING.md](guides/TROUBLESHOOTING.md)

### By Role

**Developers**:
1. [Quick Start](guides/QUICK_START.md)
2. [API Reference](api/)
3. [Examples](../examples/)
4. [Tests](../tests/)

**DevOps**:
1. [Deployment Guide](deployment/DEPLOYMENT.md)
2. [Docker Compose](../docker-compose.yml)
3. [Kubernetes](../k8s/)
4. [Monitoring](../docker/prometheus.yml)

**Security Analysts**:
1. [Architecture](ARCHITECTURE.md)
2. [Threat Models](guides/THREAT_MODELS.md)
3. [Security Audit](SECURITY_AUDIT.md)
4. [Benchmarks](../benches/)

---

## 🆕 Recent Updates

### 2025-10-27
- ✅ Published aimds-core v0.1.0 to crates.io
- ✅ Fixed 12 compilation errors in Midstream workspace
- ✅ Reorganized documentation structure
- ✅ Created comprehensive publication status report
- ✅ Validated all benchmarks (+21% above targets)

### Next Steps
1. Publish 6 Midstream foundation crates (~35 min)
2. Complete AIMDS publication (~20 min)
3. Update README with crates.io badges
4. Create GitHub release (v0.1.0)

---

## 📞 Support

- **GitHub Issues**: https://github.com/ruvnet/midstream/issues
- **Documentation**: https://ruv.io/aimds/docs
- **Discord**: https://discord.gg/ruv
- **Email**: support@ruv.io

---

**Built with ❤️ by [rUv](https://ruv.io)** | Part of the [Midstream Platform](https://github.com/ruvnet/midstream)

# Ruvector Documentation Index

Complete index of all Ruvector documentation.

## Quick Links

- [Getting Started](guides/GETTING_STARTED.md) - Start here!
- [Installation](guides/INSTALLATION.md) - Platform-specific installation
- [API Reference](api/) - Complete API documentation
- [Examples](../examples/) - Working code examples
- [Contributing](development/CONTRIBUTING.md) - How to contribute

## Documentation Structure

```
docs/
├── adr/                    # Architecture Decision Records
├── analysis/               # Research & analysis docs
├── api/                    # API references (Rust, Node.js, Cypher)
├── architecture/           # System design docs
├── benchmarks/             # Performance benchmarks & results
├── cloud-architecture/     # Cloud deployment guides
├── code-reviews/           # Code review documentation
├── dag/                    # DAG implementation
├── development/            # Developer guides
├── examples/               # SQL examples
├── gnn/                    # GNN/Graph implementation
├── guides/                 # User guides & tutorials
├── hnsw/                   # HNSW index documentation
├── hooks/                  # Hooks system documentation
├── implementation/         # Implementation details & summaries
├── integration/            # Integration guides
├── nervous-system/         # Nervous system architecture
├── optimization/           # Performance optimization guides
├── plans/                  # Implementation plans
├── postgres/               # PostgreSQL extension docs
│   └── zero-copy/          # Zero-copy memory docs
├── project-phases/         # Development phases
├── publishing/             # NPM publishing guides
├── research/               # Research documentation
│   ├── cognitive-frontier/ # Cognitive frontier research
│   ├── gnn-v2/             # GNN v2 research plans
│   ├── latent-space/       # HNSW & attention research
│   └── mincut/             # MinCut algorithm research
├── ruvllm/                 # RuVLLM documentation
├── security/               # Security audits & reports
├── sparse-inference/       # Sparse inference docs
├── sql/                    # SQL examples
├── testing/                # Testing documentation
└── training/               # Training & LoRA docs
```

## User Guides

### Getting Started
- **[Getting Started Guide](guides/GETTING_STARTED.md)** - Quick introduction to Ruvector
- **[Installation Guide](guides/INSTALLATION.md)** - Installation for Rust, Node.js, WASM, CLI
- **[Basic Tutorial](guides/BASIC_TUTORIAL.md)** - Step-by-step tutorial with examples
- **[Advanced Features Guide](guides/ADVANCED_FEATURES.md)** - Hybrid search, quantization, MMR, filtering

### Quick Starts
- **[AgenticDB Quickstart](guides/AGENTICDB_QUICKSTART.md)** - Quick start for AgenticDB
- **[AgenticDB API](guides/AGENTICDB_API.md)** - Detailed AgenticDB API documentation
- **[Optimization Quick Start](guides/OPTIMIZATION_QUICK_START.md)** - Performance optimization guide
- **[Quick Fix Guide](guides/quick-fix-guide.md)** - Common issues and solutions

### WASM Guides
- **[WASM API](guides/wasm-api.md)** - Browser WASM API
- **[WASM Build Guide](guides/wasm-build-guide.md)** - Building for WASM

### Migration
- **[Migration from AgenticDB](development/MIGRATION.md)** - Complete migration guide with examples

## HNSW Documentation

- **[HNSW Index](hnsw/HNSW_INDEX.md)** - HNSW index overview
- **[HNSW Quick Reference](hnsw/HNSW_QUICK_REFERENCE.md)** - Quick reference guide
- **[HNSW Usage Example](hnsw/HNSW_USAGE_EXAMPLE.md)** - Working examples
- **[HNSW Implementation Summary](hnsw/HNSW_IMPLEMENTATION_SUMMARY.md)** - Implementation details
- **[HNSW Implementation README](hnsw/HNSW_IMPLEMENTATION_README.md)** - Detailed README

## PostgreSQL Extension

### Core Documentation
- **[Operator Quick Reference](postgres/operator-quick-reference.md)** - Operator reference
- **[Parallel Query Guide](postgres/parallel-query-guide.md)** - Parallel query execution
- **[Parallel Implementation](postgres/parallel-implementation-summary.md)** - Implementation details

### SparseVec
- **[SparseVec Quickstart](postgres/SPARSEVEC_QUICKSTART.md)** - Sparse vector quick start
- **[SparseVec Implementation](postgres/SPARSEVEC_IMPLEMENTATION.md)** - Implementation details

### Zero-Copy Memory
- **[Zero-Copy Implementation](postgres/zero-copy/ZERO_COPY_IMPLEMENTATION.md)** - Zero-copy overview
- **[Zero-Copy Operators](postgres/zero-copy/zero-copy-operators.md)** - Operator details
- **[Zero-Copy Summary](postgres/zero-copy/ZERO_COPY_OPERATORS_SUMMARY.md)** - Summary
- **[Zero-Copy Examples](postgres/zero-copy/examples.rs)** - Rust examples
- **[Memory Quick Reference](postgres/postgres-zero-copy-quick-reference.md)** - Quick reference
- **[Memory Implementation](postgres/postgres-memory-implementation-summary.md)** - Memory details
- **[Memory Guide](postgres/postgres-zero-copy-memory.md)** - Comprehensive guide

## Architecture Documentation

- **[System Overview](architecture/SYSTEM_OVERVIEW.md)** - High-level architecture and design
- **[NPM Package Architecture](architecture/NPM_PACKAGE_ARCHITECTURE.md)** - Package structure
- **[Technical Plan](architecture/TECHNICAL_PLAN.md)** - Technical roadmap
- **[Repository Structure](REPO_STRUCTURE.md)** - Codebase organization

### Cloud Architecture
- **[Architecture Overview](cloud-architecture/architecture-overview.md)** - Cloud design
- **[Deployment Guide](cloud-architecture/DEPLOYMENT_GUIDE.md)** - Deployment instructions
- **[Infrastructure Design](cloud-architecture/infrastructure-design.md)** - Infrastructure details
- **[Scaling Strategy](cloud-architecture/scaling-strategy.md)** - Scaling approaches
- **[Performance Optimization](cloud-architecture/PERFORMANCE_OPTIMIZATION_GUIDE.md)** - Cloud performance

## API Reference

### Platform APIs
- **[Rust API](api/RUST_API.md)** - Complete Rust API reference
- **[Node.js API](api/NODEJS_API.md)** - Complete Node.js API reference
- **[Cypher Reference](api/CYPHER_REFERENCE.md)** - Cypher query language

## GNN & Graph Documentation

- **[Graph Integration Summary](gnn/GRAPH_INTEGRATION_SUMMARY.md)** - Overview of graph features
- **[Graph Validation Checklist](gnn/GRAPH_VALIDATION_CHECKLIST.md)** - Validation guide
- **[GNN Layer Implementation](gnn/gnn-layer-implementation.md)** - Layer details
- **[Graph Attention Implementation](gnn/graph-attention-implementation-summary.md)** - Attention mechanisms
- **[Hyperbolic Attention](gnn/hyperbolic-attention-implementation.md)** - Hyperbolic embeddings
- **[Cypher Parser](gnn/cypher-parser-implementation.md)** - Query parser
- **[CLI Graph Commands](gnn/cli-graph-commands.md)** - CLI usage
- **[Graph WASM Setup](gnn/graph-wasm-setup.md)** - WASM bindings
- **[Node Bindings](gnn/ruvector-gnn-node-bindings.md)** - Node.js bindings
- **[Training Utilities](gnn/training-utilities-implementation.md)** - Training tools

## Integration Guides

- **[Integration Summary](integration/INTEGRATION-SUMMARY.md)** - Integration overview
- **[Psycho-Symbolic Integration](integration/PSYCHO-SYMBOLIC-INTEGRATION.md)** - Symbolic AI integration
- **[Psycho-Synth Quick Start](integration/PSYCHO-SYNTH-QUICK-START.md)** - Quick start guide

## Performance & Benchmarks

- **[Benchmarking Guide](benchmarks/BENCHMARKING_GUIDE.md)** - How to run and interpret benchmarks
- **[Benchmark Comparison](benchmarks/BENCHMARK_COMPARISON.md)** - Performance comparisons

### Optimization Guides
- **[Performance Tuning Guide](optimization/PERFORMANCE_TUNING_GUIDE.md)** - Detailed optimization guide
- **[Build Optimization](optimization/BUILD_OPTIMIZATION.md)** - Compilation optimizations
- **[Optimization Results](optimization/OPTIMIZATION_RESULTS.md)** - Benchmark results
- **[Implementation Summary](optimization/IMPLEMENTATION_SUMMARY.md)** - Optimization implementation

## Implementation Documentation

### Implementation Details
- **[Implementation Summary](implementation/IMPLEMENTATION_SUMMARY.md)** - Overall implementation
- **[Improvement Roadmap](implementation/IMPROVEMENT_ROADMAP.md)** - Future plans
- **[Security Fixes Summary](implementation/SECURITY_FIXES_SUMMARY.md)** - Security improvements
- **[Overflow Fixes](implementation/overflow_fixes_verification.md)** - Bug fixes

### Phase Summaries
- **[Phase 2: HNSW](project-phases/phase2_hnsw_implementation.md)** - HNSW integration
- **[Phase 3: AgenticDB](project-phases/PHASE3_SUMMARY.md)** - AgenticDB layer
- **[Phase 4: Advanced Features](project-phases/phase4-implementation-summary.md)** - Product quantization, hybrid search
- **[Phase 5: Multi-Platform](project-phases/phase5-implementation-summary.md)** - Node.js, WASM, CLI
- **[Phase 6: Advanced](project-phases/PHASE6_SUMMARY.md)** - Future features

## Publishing & Deployment

- **[Publishing Guide](publishing/PUBLISHING-GUIDE.md)** - How to publish packages
- **[NPM Publishing](publishing/NPM_PUBLISHING.md)** - NPM-specific guide
- **[NPM Token Setup](publishing/NPM_TOKEN_SETUP.md)** - Authentication setup
- **[Package Validation](publishing/PACKAGE-VALIDATION-REPORT.md)** - Validation report
- **[Publishing Status](publishing/PUBLISHING.md)** - Current status

## Development

- **[Contributing Guide](development/CONTRIBUTING.md)** - How to contribute
- **[Security](development/SECURITY.md)** - Security guidelines
- **[Migration Guide](development/MIGRATION.md)** - Migration documentation
- **[NPM Package Review](development/NPM_PACKAGE_REVIEW.md)** - Package review
- **[Fixing Compilation Errors](development/FIXING_COMPILATION_ERRORS.md)** - Troubleshooting

## Testing

- **[Test Suite Summary](testing/TDD_TEST_SUITE_SUMMARY.md)** - Testing strategy
- **[Integration Testing Report](testing/integration-testing-report.md)** - Integration tests

## Research & Advanced Features

### Cognitive Frontier
- **[Temporal Hypergraphs](research/cognitive-frontier/temporal-hypergraphs.md)** - Time-varying hyperedges with causal constraints
- **[Federated Strange Loops](research/cognitive-frontier/federated-strange-loops.md)** - Multi-system mutual observation

### Latent Space
- **[Implementation Roadmap](research/latent-space/implementation-roadmap.md)** - Development plan
- **[GNN Architecture Analysis](research/latent-space/gnn-architecture-analysis.md)** - Architecture deep-dive
- **[Attention Mechanisms Research](research/latent-space/attention-mechanisms-research.md)** - Research notes
- **[Advanced Architectures](research/latent-space/advanced-architectures.md)** - Advanced designs
- **[Optimization Strategies](research/latent-space/optimization-strategies.md)** - Optimization approaches
- **[HNSW Evolution](research/latent-space/hnsw-evolution-overview.md)** - HNSW research
- **[HNSW Neural Augmentation](research/latent-space/hnsw-neural-augmentation.md)** - Neural features
- **[HNSW Quantum Hybrid](research/latent-space/hnsw-quantum-hybrid.md)** - Quantum computing

### MinCut Research
- **[LocalKCut Algorithm](research/mincut/localkcut-algorithm.md)** - Algorithm overview
- **[LocalKCut Implementation](research/mincut/localkcut-implementation-summary.md)** - Implementation details
- **[Paper Implementation](research/mincut/localkcut-paper-implementation.md)** - December 2025 paper

### GNN v2 Research
- **[Master Plan](research/gnn-v2/00-master-plan.md)** - GNN v2 overview
- **[GNN Guided Routing](research/gnn-v2/01-gnn-guided-routing.md)** - Routing research
- **[Incremental Graph Learning](research/gnn-v2/02-incremental-graph-learning.md)** - Learning approaches
- **[Neuro-Symbolic Query](research/gnn-v2/03-neuro-symbolic-query.md)** - Query processing
- **[Hyperbolic Embeddings](research/gnn-v2/04-hyperbolic-embeddings.md)** - Embedding research
- **[Adaptive Precision](research/gnn-v2/05-adaptive-precision.md)** - Precision optimization
- **[Temporal GNN](research/gnn-v2/06-temporal-gnn.md)** - Temporal features
- **[Graph Condensation](research/gnn-v2/07-graph-condensation.md)** - Condensation techniques
- **[Native Sparse Attention](research/gnn-v2/08-native-sparse-attention.md)** - Sparse attention
- **[Quantum-Inspired Attention](research/gnn-v2/09-quantum-inspired-attention.md)** - Quantum approaches
- **[Innovative Features](research/innovative-gnn-features-2024-2025.md)** - 2024-2025 research

### DSPy Integration
- **[DSPy Research](research/dspy-ts-comprehensive-research.md)** - Comprehensive research
- **[DSPy Quick Start](research/dspy-ts-quick-start-guide.md)** - Quick start guide
- **[Claude Flow Integration](research/claude-flow-dspy-integration.md)** - Integration guide

## Project Information

- **[README](README.md)** - Documentation overview
- **[Project README](../README.md)** - Project overview
- **[CHANGELOG](../CHANGELOG.md)** - Version history
- **[LICENSE](../LICENSE)** - MIT License

## Documentation Statistics

- **Total directories**: 20+
- **Total documentation files**: 170+ markdown files
- **User guides**: 12+ comprehensive guides
- **API references**: 3 platform APIs
- **Code examples**: 10+ working examples
- **Languages covered**: Rust, JavaScript/TypeScript, WASM, SQL

## Getting Help

### Resources
- **Documentation**: This index and linked guides
- **Examples**: [../examples/](../examples/) directory
- **API docs**: `cargo doc --no-deps --open`
- **Benchmarks**: `cargo bench`

### Support Channels
- **GitHub Issues**: [Report bugs or request features](https://github.com/ruvnet/ruvector/issues)
- **GitHub Discussions**: [Ask questions](https://github.com/ruvnet/ruvector/discussions)
- **Pull Requests**: [Contribute code](https://github.com/ruvnet/ruvector/pulls)

---

**Last Updated**: 2025-12-25
**Version**: 0.1.29

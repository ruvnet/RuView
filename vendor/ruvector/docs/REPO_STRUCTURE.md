# Repository Structure

Clean and organized structure for the RuVector project.

## Root Directory

```
ruvector/
├── README.md                 # Main project README
├── CHANGELOG.md             # Version history and changes
├── CLAUDE.md                # Claude Code configuration
├── LICENSE                  # MIT License
├── Cargo.toml              # Rust workspace configuration
├── Cargo.lock              # Rust dependency lock
├── package.json            # NPM workspace configuration
├── .gitignore              # Git ignore rules
│
├── crates/                 # Rust crates
│   ├── ruvector-core/      # Core vector database
│   ├── ruvector-node/      # Node.js bindings
│   ├── ruvector-wasm/      # WebAssembly bindings
│   ├── ruvector-cli/       # Command-line interface
│   ├── ruvector-bench/     # Benchmarking suite
│   ├── ruvllm/             # LLM inference engine
│   ├── sona/               # Self-Optimizing Neural Architecture
│   ├── router-core/        # Neural routing
│   └── ...                 # Additional crates
│
├── npm/                    # NPM packages
│   └── packages/
│       ├── ruvector/       # Core bindings
│       ├── ruvllm/         # LLM package
│       ├── raft/           # Consensus implementation
│       ├── replication/    # Data replication
│       └── scipix/         # OCR client
│
├── docs/                   # 📚 Documentation (organized)
│   ├── README.md           # Documentation index
│   ├── INDEX.md            # Complete file index
│   ├── REPO_STRUCTURE.md   # This file
│   ├── adr/                # Architecture Decision Records
│   ├── analysis/           # Research & analysis
│   ├── api/                # API documentation
│   ├── architecture/       # System architecture
│   ├── benchmarks/         # Performance benchmarks
│   ├── cloud-architecture/ # Cloud deployment
│   ├── code-reviews/       # Code reviews
│   ├── development/        # Contributing guides
│   ├── gnn/                # GNN documentation
│   ├── guides/             # User guides
│   ├── hnsw/               # HNSW documentation
│   ├── hooks/              # Hooks system
│   ├── implementation/     # Implementation details
│   ├── integration/        # Integration guides
│   ├── nervous-system/     # Nervous system arch
│   ├── optimization/       # Performance tuning
│   ├── postgres/           # PostgreSQL extension
│   ├── project-phases/     # Historical phases
│   ├── publishing/         # NPM publishing
│   ├── research/           # Research documentation
│   ├── ruvllm/             # RuVLLM docs
│   ├── security/           # Security audits
│   ├── testing/            # Testing docs
│   └── training/           # Training & LoRA
│
├── src/                    # 🚀 Cloud deployment source
│   ├── cloud-run/         # Cloud Run services
│   ├── agentic-integration/ # Agent coordination
│   └── burst-scaling/     # Auto-scaling system
│
├── benchmarks/            # Load testing and benchmarks
├── tests/                 # Rust integration tests
├── examples/             # Example code
│   ├── rust/            # Rust examples
│   ├── nodejs/          # Node.js examples
│   └── wasm-*/         # WASM examples
│
└── .claude/             # Claude Code helpers
```

## Documentation Organization

All documentation is organized in `/docs` with clear categories:

### 📖 Guides & Tutorials
- **guides/** - Getting started, tutorials, installation
- **api/** - Rust, Node.js, Cypher API references

### 🏗️ Architecture & Design
- **adr/** - Architecture Decision Records
- **architecture/** - System design documents
- **cloud-architecture/** - Global cloud deployment
- **nervous-system/** - Nervous system architecture

### ⚡ Performance
- **benchmarks/** - Performance benchmarks & results
- **optimization/** - Performance tuning guides
- **analysis/** - Research & analysis documents

### 🔐 Security
- **security/** - Security audits & reports

### 💻 Implementation
- **implementation/** - Implementation details & summaries
- **integration/** - Integration guides
- **code-reviews/** - Code review documentation

### 🔬 Specialized Topics
- **gnn/** - Graph Neural Networks
- **hnsw/** - HNSW index documentation
- **postgres/** - PostgreSQL extension
- **ruvllm/** - RuVLLM documentation
- **training/** - Training & LoRA guides

### 👨‍💻 Development
- **development/** - Contributing, migration, troubleshooting
- **testing/** - Testing documentation
- **publishing/** - NPM publishing guides
- **hooks/** - Hooks system documentation

### 🔬 Research
- **research/** - Research documentation
  - cognitive-frontier/ - Advanced AI research
  - gnn-v2/ - GNN v2 plans
  - latent-space/ - HNSW & attention research
  - mincut/ - MinCut algorithm research

### 📜 Historical
- **project-phases/** - Project phase documentation

## Source Code Organization

### `/crates` - Rust Crates
Core Rust implementation organized as workspace:
- `ruvector-core` - Core vector database
- `ruvllm` - LLM inference engine
- `sona` - Self-Optimizing Neural Architecture
- Platform bindings (Node.js, WASM, FFI)
- CLI and benchmarking tools

### `/npm/packages` - NPM Packages
TypeScript packages for Node.js:
- `@ruvector/ruvector` - Core bindings
- `@ruvector/ruvllm` - LLM inference
- `@ruvector/raft` - Consensus implementation
- `@ruvector/replication` - Data replication
- `@ruvector/scipix` - OCR client

### `/src` - Cloud Deployment Code
Global streaming implementation:
- `cloud-run/` - Cloud Run services
- `agentic-integration/` - Distributed agent coordination
- `burst-scaling/` - Auto-scaling and capacity management

### `/benchmarks` - Load Testing
Comprehensive benchmarking suite for performance testing

## File Counts

- **Documentation**: 170+ markdown files (organized in 25+ directories)
- **Rust Crates**: 15+ crates
- **NPM Packages**: 5 packages
- **Root Files**: 8 essential files only

## Clean Root Directory

Only essential files in root:
- ✅ README.md - Project overview
- ✅ CHANGELOG.md - Version history
- ✅ CLAUDE.md - Development configuration
- ✅ LICENSE - MIT license
- ✅ Cargo.toml - Rust workspace
- ✅ Cargo.lock - Dependencies
- ✅ package.json - NPM workspace
- ✅ .gitignore - Git rules

**No test files, temporary files, or duplicate docs in root!**

## Navigation Tips

1. **New users**: Start at [docs/README.md](./README.md)
2. **Quick start**: See [docs/guides/](./guides/)
3. **Cloud deployment**: Check [docs/cloud-architecture/](./cloud-architecture/)
4. **Contributing**: Read [docs/development/CONTRIBUTING.md](./development/CONTRIBUTING.md)
5. **API docs**: Browse [docs/api/](./api/)
6. **Architecture decisions**: Review [docs/adr/](./adr/)

---

**Last Updated**: 2026-01-21
**Status**: ✅ Clean and Organized
**Total Documentation**: 170+ files properly categorized

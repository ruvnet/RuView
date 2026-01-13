# WiFi-DensePose Rust Port - 15-Agent Swarm Configuration

## Mission Statement
Port the WiFi-DensePose Python system to Rust using ruvnet/ruvector patterns, with modular crates, WASM support, and comprehensive documentation following ADR/DDD principles.

## Agent Swarm Architecture

### Tier 1: Orchestration (1 Agent)
1. **Orchestrator Agent** - Coordinates all agents, manages dependencies, tracks progress

### Tier 2: Architecture & Documentation (3 Agents)
2. **ADR Agent** - Creates Architecture Decision Records for all major decisions
3. **DDD Agent** - Designs Domain-Driven Design models and bounded contexts
4. **Documentation Agent** - Maintains comprehensive documentation, README, API docs

### Tier 3: Core Implementation (5 Agents)
5. **Signal Processing Agent** - Ports CSI processing, phase sanitization, FFT algorithms
6. **Neural Network Agent** - Ports DensePose head, modality translation using tch-rs/onnx
7. **API Agent** - Implements Axum/Actix REST API and WebSocket handlers
8. **Database Agent** - Implements SQLx PostgreSQL/SQLite with migrations
9. **Config Agent** - Implements configuration management, environment handling

### Tier 4: Platform & Integration (3 Agents)
10. **WASM Agent** - Implements wasm-bindgen, browser compatibility, wasm-pack builds
11. **Hardware Agent** - Ports CSI extraction, router interfaces, hardware abstraction
12. **Integration Agent** - Integrates ruvector crates, vector search, GNN layers

### Tier 5: Quality Assurance (3 Agents)
13. **Test Agent** - Writes unit, integration, and benchmark tests
14. **Validation Agent** - Validates against Python implementation, accuracy checks
15. **Optimization Agent** - Profiles, benchmarks, and optimizes hot paths

## Crate Workspace Structure

```
wifi-densepose-rs/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── wifi-densepose-core/      # Core types, traits, errors
│   ├── wifi-densepose-signal/    # Signal processing (CSI, phase, FFT)
│   ├── wifi-densepose-nn/        # Neural networks (DensePose, translation)
│   ├── wifi-densepose-api/       # REST/WebSocket API (Axum)
│   ├── wifi-densepose-db/        # Database layer (SQLx)
│   ├── wifi-densepose-config/    # Configuration management
│   ├── wifi-densepose-hardware/  # Hardware abstraction
│   ├── wifi-densepose-wasm/      # WASM bindings
│   └── wifi-densepose-cli/       # CLI application
├── docs/
│   ├── adr/                      # Architecture Decision Records
│   ├── ddd/                      # Domain-Driven Design docs
│   └── api/                      # API documentation
├── benches/                      # Benchmarks
└── tests/                        # Integration tests
```

## Domain Model (DDD)

### Bounded Contexts
1. **Signal Domain** - CSI data, phase processing, feature extraction
2. **Pose Domain** - DensePose inference, keypoints, segmentation
3. **Streaming Domain** - WebSocket, real-time updates, connection management
4. **Storage Domain** - Persistence, caching, retrieval
5. **Hardware Domain** - Router interfaces, device management

### Core Aggregates
- `CsiFrame` - Raw CSI data aggregate
- `ProcessedSignal` - Cleaned and extracted features
- `PoseEstimate` - DensePose inference result
- `Session` - Client session with history
- `Device` - Hardware device state

## ADR Topics to Document
- ADR-001: Rust Workspace Structure
- ADR-002: Signal Processing Library Selection
- ADR-003: Neural Network Inference Strategy
- ADR-004: API Framework Selection (Axum vs Actix)
- ADR-005: Database Layer Strategy (SQLx)
- ADR-006: WASM Compilation Strategy
- ADR-007: Error Handling Approach
- ADR-008: Async Runtime Selection (Tokio)
- ADR-009: ruvector Integration Strategy
- ADR-010: Configuration Management

## Phase Execution Plan

### Phase 1: Foundation
- Set up Cargo workspace
- Create all crate scaffolding
- Write ADR-001 through ADR-005
- Define core traits and types

### Phase 2: Core Implementation
- Port signal processing algorithms
- Implement neural network inference
- Build API layer
- Database integration

### Phase 3: Platform
- WASM compilation
- Hardware abstraction
- ruvector integration

### Phase 4: Quality
- Comprehensive testing
- Python validation
- Benchmarking
- Optimization

## Success Metrics
- Feature parity with Python implementation
- < 10ms latency improvement over Python
- WASM bundle < 5MB
- 100% test coverage
- All ADRs documented

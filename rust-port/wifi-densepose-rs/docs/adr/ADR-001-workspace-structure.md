# ADR-001: Rust Workspace Structure

## Status
Accepted

## Context
We need to port the WiFi-DensePose Python application to Rust for improved performance, memory safety, and cross-platform deployment including WASM. The architecture must be modular, maintainable, and support multiple deployment targets.

## Decision
We will use a Cargo workspace with 9 modular crates:

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
```

### Crate Responsibilities

1. **wifi-densepose-core**: Foundation types, traits, and error handling shared across all crates
2. **wifi-densepose-signal**: CSI data processing, phase sanitization, FFT, feature extraction
3. **wifi-densepose-nn**: Neural network inference using ONNX Runtime, Candle, or tch-rs
4. **wifi-densepose-api**: HTTP/WebSocket server using Axum
5. **wifi-densepose-db**: Database operations with SQLx
6. **wifi-densepose-config**: Configuration loading and validation
7. **wifi-densepose-hardware**: Router and hardware interfaces
8. **wifi-densepose-wasm**: WebAssembly bindings for browser deployment
9. **wifi-densepose-cli**: Command-line interface

## Consequences

### Positive
- Clear separation of concerns
- Independent crate versioning
- Parallel compilation
- Selective feature inclusion
- Easier testing and maintenance
- WASM target isolation

### Negative
- More complex dependency management
- Initial setup overhead
- Cross-crate refactoring complexity

## References
- [Cargo Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [ruvector crate structure](https://github.com/ruvnet/ruvector)

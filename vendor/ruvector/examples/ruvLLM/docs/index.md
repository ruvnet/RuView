# RuvLLM Documentation

## Overview

This directory contains documentation for the RuvLLM self-learning LLM architecture.

## Quick Links

- [Main README](../README.md) - Getting started, API reference, benchmarks
- [SPARC Documentation](./sparc/) - Design methodology documentation

## SPARC Methodology

The project was designed using the SPARC methodology:

| Phase | Document | Description |
|-------|----------|-------------|
| 1 | [Specification](./sparc/01-specification.md) | Requirements and acceptance criteria |
| 2 | [Pseudocode](./sparc/02-pseudocode.md) | Algorithm design and data flows |
| 3 | [Architecture](./sparc/03-architecture.md) | System design and component interactions |
| 4 | [Refinement](./sparc/04-refinement.md) | TDD implementation and iterative improvement |
| 5 | [Completion](./sparc/05-completion.md) | Integration, testing, and deployment |

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         RuvLLM System                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │  Embedding  │  │   Memory    │  │   Router    │             │
│  │  Service    │  │   (HNSW)    │  │  (FastGRNN) │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         │                │                │                     │
│         └────────────────┼────────────────┘                     │
│                          │                                      │
│                   ┌──────┴──────┐                               │
│                   │ Orchestrator │                              │
│                   └──────┬──────┘                               │
│                          │                                      │
│  ┌─────────────┐  ┌──────┴──────┐  ┌─────────────┐             │
│  │  Attention  │  │  Inference  │  │  Learning   │             │
│  │  Engine     │  │  Pool       │  │  Service    │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Module Documentation

### Core Modules

| Module | File | Description |
|--------|------|-------------|
| `orchestrator` | `src/orchestrator.rs` | Main coordinator, request processing pipeline |
| `memory` | `src/memory.rs` | HNSW-based semantic memory with graph expansion |
| `router` | `src/router.rs` | FastGRNN routing with EWC learning |
| `attention` | `src/attention.rs` | Multi-head graph attention with edge features |
| `embedding` | `src/embedding.rs` | Tokenization, embedding, and caching |
| `inference` | `src/inference.rs` | LFM2 model pool management |
| `learning` | `src/learning.rs` | Self-learning feedback loops |
| `compression` | `src/compression.rs` | Memory compression and clustering |

### Supporting Modules

| Module | File | Description |
|--------|------|-------------|
| `config` | `src/config.rs` | Configuration system with builder pattern |
| `error` | `src/error.rs` | Error types and result aliases |
| `types` | `src/types.rs` | Core domain types and structs |

## API Examples

### Basic Query

```rust
use ruvllm::{Config, RuvLLM};

let config = Config::builder().build()?;
let llm = RuvLLM::new(config).await?;
let response = llm.query("What is Rust?").await?;
```

### Session Management

```rust
let session = llm.new_session();
let r1 = llm.query_session(&session, "Tell me about vectors").await?;
let r2 = llm.query_session(&session, "How are they used in ML?").await?;
```

### Feedback Loop

```rust
use ruvllm::Feedback;

llm.feedback(Feedback {
    request_id: response.request_id,
    rating: Some(5),
    correction: None,
    task_success: Some(true),
}).await?;
```

## Performance Tuning

### Memory Configuration

```rust
Config::builder()
    .hnsw_params(
        32,   // M: connections per node (higher = better recall, more memory)
        200,  // ef_construction: build quality (higher = slower build, better index)
        64,   // ef_search: search quality (higher = slower search, better recall)
    )
```

### Router Configuration

```rust
Config::builder()
    .router_hidden_dim(128)  // Hidden state size (higher = more capacity)
```

### Learning Configuration

```rust
Config::builder()
    .learning_enabled(true)  // Enable self-learning
```

## Further Reading

- [LFM2 Paper](https://arxiv.org/abs/2511.23404v1) - Liquid Foundation Models
- [FastGRNN Paper](https://arxiv.org/abs/1901.02358) - Fast RNN architecture
- [HNSW Paper](https://arxiv.org/abs/1603.09320) - Approximate nearest neighbor search
- [EWC Paper](https://arxiv.org/abs/1612.00796) - Continual learning

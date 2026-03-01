# ADR-001: RuvBot Architecture Overview

## Status
Accepted

## Date
2026-01-27

## Context

We need to build **RuvBot**, a Clawdbot-style personal AI assistant with a RuVector backend. The system must:

1. Provide a self-hosted, extensible AI assistant framework
2. Integrate with RuVector's WASM-based vector operations for SOTA learning
3. Support multi-tenancy for enterprise deployments
4. Enable long-running tasks via background workers
5. Integrate with messaging platforms (Slack, Discord, webhooks)
6. Distribute as an `npx` package with local/remote deployment options

## Decision

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           RuvBot System                              │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐│
│  │   REST API  │  │  GraphQL    │  │   Slack     │  │  Webhooks   ││
│  │  Endpoints  │  │  Gateway    │  │  Adapter    │  │  Handler    ││
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘│
│         │                │                │                │        │
│  ┌──────┴────────────────┴────────────────┴────────────────┴──────┐│
│  │                      Message Router                             ││
│  └─────────────────────────────┬───────────────────────────────────┘│
│                                │                                     │
│  ┌─────────────────────────────┴───────────────────────────────────┐│
│  │                     Core Application Layer                       ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           ││
│  │  │ AgentManager │  │SessionStore  │  │ SkillRegistry│           ││
│  │  └──────────────┘  └──────────────┘  └──────────────┘           ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           ││
│  │  │MemoryManager │  │WorkerPool   │  │ EventBus     │           ││
│  │  └──────────────┘  └──────────────┘  └──────────────┘           ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                │                                     │
│  ┌─────────────────────────────┴───────────────────────────────────┐│
│  │                    Infrastructure Layer                          ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           ││
│  │  │ RuVector     │  │ PostgreSQL   │  │ RuvLLM       │           ││
│  │  │ WASM Engine  │  │ + pgvector   │  │ Inference    │           ││
│  │  └──────────────┘  └──────────────┘  └──────────────┘           ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           ││
│  │  │ agentic-flow │  │ SONA Learning│  │ HNSW Index   │           ││
│  │  │ Workers      │  │ System       │  │ Memory       │           ││
│  │  └──────────────┘  └──────────────┘  └──────────────┘           ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

### DDD Bounded Contexts

#### 1. Core Context
- **Agent**: The AI agent entity with identity, capabilities, and state
- **Session**: Conversation context with message history and metadata
- **Memory**: Vector-based memory with HNSW indexing
- **Skill**: Extensible capabilities (tools, commands, integrations)

#### 2. Infrastructure Context
- **Persistence**: PostgreSQL with RuVector extensions, pgvector
- **Messaging**: Event-driven message bus (Redis/in-memory)
- **Workers**: Background task processing via agentic-flow

#### 3. Integration Context
- **Slack**: Slack Bot API adapter
- **Webhooks**: Generic webhook handler
- **Providers**: LLM provider abstraction (Anthropic, OpenAI, etc.)

#### 4. Learning Context
- **Embeddings**: RuVector WASM vector operations
- **Training**: Trajectory learning, LoRA fine-tuning
- **Patterns**: Neural pattern storage and retrieval

### Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Runtime | Node.js 18+ | Primary runtime |
| Language | TypeScript (ESM) | Type-safe development |
| Vector Engine | @ruvector/wasm-unified | SIMD-optimized vectors |
| LLM Layer | @ruvector/ruvllm | SONA, LoRA, inference |
| Database | PostgreSQL + pgvector | Persistence + vectors |
| Workers | agentic-flow | Background processing |
| Testing | Vitest | Unit/Integration/E2E |
| CLI | Commander.js | npx distribution |

### Package Structure

```
npm/packages/ruvbot/
├── bin/                      # CLI entry points
│   └── ruvbot.ts            # npx ruvbot entry
├── src/
│   ├── core/                # Domain layer
│   │   ├── entities/        # Agent, Session, Memory, Skill
│   │   ├── services/        # AgentManager, SessionStore, etc.
│   │   └── events/          # Domain events
│   ├── infrastructure/      # Infrastructure layer
│   │   ├── persistence/     # PostgreSQL, SQLite adapters
│   │   ├── messaging/       # Event bus, message queue
│   │   └── workers/         # agentic-flow integration
│   ├── integrations/        # External integrations
│   │   ├── slack/           # Slack adapter
│   │   ├── webhooks/        # Webhook handlers
│   │   └── providers/       # LLM providers
│   ├── learning/            # Learning system
│   │   ├── embeddings/      # WASM vector ops
│   │   ├── training/        # LoRA, SONA
│   │   └── patterns/        # Pattern storage
│   └── api/                 # API layer
│       ├── rest/            # REST endpoints
│       └── graphql/         # GraphQL schema
├── tests/
│   ├── unit/
│   ├── integration/
│   └── e2e/
├── docs/
│   └── adr/                 # Architecture Decision Records
└── scripts/                 # Build/deploy scripts
```

### Multi-Tenancy Strategy

1. **Database Level**: Row-Level Security (RLS) with tenant_id
2. **Application Level**: Tenant context middleware
3. **Memory Level**: Namespace isolation in vector storage
4. **Worker Level**: Tenant-scoped job queues

### Key Design Principles

1. **Self-Learning**: Every interaction improves the system via SONA
2. **WASM-First**: Use RuVector WASM for portable, fast vector ops
3. **Event-Driven**: Loose coupling via event bus
4. **Extensible**: Plugin architecture for skills and integrations
5. **Observable**: Built-in metrics and tracing

## Consequences

### Positive
- Modular architecture enables independent scaling
- WASM integration provides consistent cross-platform performance
- Multi-tenancy from day one avoids later refactoring
- Self-learning improves over time with usage

### Negative
- Initial complexity is higher than monolithic approach
- WASM has some interop overhead
- Multi-tenancy adds complexity to all data operations

### Risks
- WASM performance in Node.js may vary by platform
- PostgreSQL dependency limits serverless options
- Background workers need careful monitoring

## Related ADRs
- ADR-002: Multi-tenancy Design
- ADR-003: Persistence Layer
- ADR-004: Background Workers
- ADR-005: Integration Layer
- ADR-006: WASM Integration
- ADR-007: Learning System
- ADR-008: Security Architecture

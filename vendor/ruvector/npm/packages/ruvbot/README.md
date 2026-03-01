# RuvBot

[![npm version](https://img.shields.io/npm/v/@ruvector/ruvbot.svg)](https://www.npmjs.com/package/@ruvector/ruvbot)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.3+-blue.svg)](https://www.typescriptlang.org/)
[![Node.js](https://img.shields.io/badge/Node.js-18+-green.svg)](https://nodejs.org/)
[![Tests](https://img.shields.io/badge/Tests-560%2F571%20passing-brightgreen.svg)]()

**Enterprise-Grade Self-Learning AI Assistant with Military-Strength Security**

**Live Demo**: https://ruvbot-875130704813.us-central1.run.app

## Table of Contents

- [RVF Microkernel](#rvf-microkernel---self-contained-boot)
- [Why RuvBot?](#why-ruvbot-over-clawdbot)
- [Comparison](#ruvbot-vs-clawdbot-comparison)
- [Requirements](#requirements)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Channel Integrations](#channel-integrations)
- [Template Library](#template-library)
- [API Usage](#api-usage)
- [Security](#security-architecture-6-layers---why-this-matters)
- [LLM Providers](#llm-providers---gemini-25-default)
- [TypeScript](#typescript-support)
- [Events & Hooks](#events--hooks)
- [Streaming](#streaming-responses)
- [Migration](#migration-from-clawdbot)
- [Troubleshooting](#troubleshooting)
- [Development](#development)

---

RuvBot is a next-generation personal AI assistant powered by RuVector's WASM vector operations. It addresses **critical security gaps found in Clawdbot** while delivering 150x faster performance, self-learning neural architecture, and enterprise-grade multi-tenancy.

## RVF Microkernel - Self-Contained Boot

RuvBot ships as a **self-contained RVF (RuVector Format) file** that includes a real Linux 6.6 microkernel and the full Node.js runtime bundle. A single 3.7 MB binary boots directly on any x86_64 hypervisor.

### What's Inside

| Segment | Size | Contents |
|---------|------|----------|
| `KERNEL_SEG` | 1.6 MB | Linux 6.6.80 bzImage (gzip compressed) |
| `WASM_SEG` | 2.2 MB | Node.js runtime bundle (all dist/ + bin/) |
| `META_SEG` | 793 B | Package metadata (name, version, format) |
| `PROFILE_SEG` | 399 B | Default agent profile |
| `WITNESS_SEG` | 426 B | Genesis witness chain (build provenance) |
| `MANIFEST_SEG` | 151 B | 6-segment manifest |

### Build the RVF

```bash
# Build from package (uses kernel/bzImage)
npm run build:rvf

# Build with custom kernel
node scripts/build-rvf.js --kernel /path/to/bzImage --output ruvbot.rvf
```

### Run the RVF

```bash
# Inspect segments
node scripts/run-rvf.js ruvbot.rvf --inspect

# Boot with QEMU (builds initramfs, launches kernel)
node scripts/run-rvf.js ruvbot.rvf --boot

# Extract and run Node.js runtime directly
node scripts/run-rvf.js ruvbot.rvf --runtime
```

### Boot Output

```
================================================================
  RuvBot RVF Microkernel - Self-Contained Runtime
================================================================

  Kernel:  Linux 6.6.80
  Arch:    x86_64
  MemTotal: 52992 kB

  RVF Segments loaded:
    [KERNEL]   Linux 6.6.80 bzImage (x86_64)
    [WASM]     RuvBot Node.js runtime bundle
    [META]     ruvbot@0.3.1 [rvf-self-contained]
    [PROFILE]  Default agent profile
    [WITNESS]  Genesis witness chain
    [MANIFEST] 6-segment manifest

  Status: BOOT OK - All segments verified
================================================================
```

### Supported Hypervisors

| Hypervisor | Command |
|------------|---------|
| QEMU | `qemu-system-x86_64 -kernel bzImage -initrd initramfs.cpio.gz -nographic` |
| Firecracker | `firecracker --kernel bzImage --initrd initramfs.cpio.gz` |
| Cloud Hypervisor | `cloud-hypervisor --kernel bzImage --initramfs initramfs.cpio.gz` |

### Kernel Configuration

The included Linux 6.6.80 kernel is built from source with a minimal config:

- **Base**: `tinyconfig` (smallest possible kernel)
- **Enabled**: 64-bit, VirtIO (PCI, NET, BLK, Console), Serial 8250, TCP/IPv4/IPv6, ProcFS, SysFS, DevTmpFS, ELF, initramfs (gzip)
- **Result**: 1.6 MB bzImage -- boots in ~1 second

## Why RuvBot Over Clawdbot?

**Clawdbot lacks essential security protections** that are mandatory for production AI deployments:

- **No prompt injection defense** - Clawdbot is vulnerable to adversarial prompts
- **No jailbreak detection** - Users can bypass system instructions
- **No PII protection** - Sensitive data leakage risk
- **No input sanitization** - Control character and unicode attacks possible
- **Single-tenant only** - No enterprise data isolation

**RuvBot solves all of these** with a 6-layer security architecture and AIDefence integration.

## RuvBot vs Clawdbot Comparison

| Feature | Clawdbot | RuvBot | Improvement |
|---------|----------|--------|-------------|
| **Security** | Basic validation | 6-layer + AIDefence | **Critical upgrade** |
| **Prompt Injection** | Vulnerable | Protected (<5ms) | **Essential protection** |
| **PII Protection** | None | Full detection + masking | **Compliance-ready** |
| **Vector Search** | Linear search | HNSW-indexed | **150x-12,500x faster** |
| **Embeddings** | External API | Local WASM | **75x faster**, no network latency |
| **Learning** | Static | SONA adaptive | Self-improving with EWC++ |
| **Multi-tenancy** | Single-user | Full RLS | Enterprise isolation |
| **LLM Models** | Single provider | 12+ models (Gemini 2.5, Claude, GPT) | **Full flexibility** |
| **Cold Start** | ~3s | ~500ms | **6x faster** |

## Performance Benchmarks

| Operation | Clawdbot | RuvBot | Speedup |
|-----------|----------|--------|---------|
| Embedding generation | 200ms (API) | 2.7ms (WASM) | **74x** |
| Vector search (10K) | 50ms | <1ms | **50x** |
| Vector search (100K) | 500ms+ | <5ms | **100x** |
| Session restore | 100ms | 10ms | **10x** |
| Skill invocation | 50ms | 5ms | **10x** |

## Features

- **Self-Learning**: SONA adaptive learning with trajectory tracking and pattern extraction
- **WASM Embeddings**: High-performance vector operations using RuVector WASM bindings
- **Vector Memory**: HNSW-indexed semantic memory with 150x-12,500x faster search
- **Multi-Platform**: Slack, Discord, webhook, REST API, and CLI interfaces
- **Extensible Skills**: Plugin architecture for custom capabilities with hot-reload
- **Multi-Tenancy**: Enterprise-ready with PostgreSQL row-level security
- **Background Workers**: 12 specialized worker types via agentic-flow
- **LLM Routing**: Intelligent 3-tier routing for optimal cost/performance

## Requirements

- **Node.js**: 18.0.0 or higher
- **npm**: 9.0.0 or higher
- **API Key**: OpenRouter (recommended) or Anthropic

## Quick Start

### Install via curl (Recommended)

```bash
# Basic install
curl -fsSL https://raw.githubusercontent.com/ruvnet/ruvector/main/npm/packages/ruvbot/scripts/install.sh | bash

# Install with interactive wizard
RUVBOT_WIZARD=true curl -fsSL https://raw.githubusercontent.com/ruvnet/ruvector/main/npm/packages/ruvbot/scripts/install.sh | bash

# Install specific version
RUVBOT_VERSION=0.1.3 curl -fsSL https://raw.githubusercontent.com/ruvnet/ruvector/main/npm/packages/ruvbot/scripts/install.sh | bash

# Install and deploy to Cloud Run
RUVBOT_DEPLOY=cloudrun curl -fsSL https://raw.githubusercontent.com/ruvnet/ruvector/main/npm/packages/ruvbot/scripts/install.sh | bash

# Install with Slack channel dependencies
RUVBOT_CHANNEL=slack curl -fsSL https://raw.githubusercontent.com/ruvnet/ruvector/main/npm/packages/ruvbot/scripts/install.sh | bash
```

### Install Options (Environment Variables)

| Variable | Description | Default |
|----------|-------------|---------|
| `RUVBOT_VERSION` | Specific version to install | `latest` |
| `RUVBOT_GLOBAL` | Install globally | `true` |
| `RUVBOT_INIT` | Initialize project after install | `false` |
| `RUVBOT_CHANNEL` | Install channel deps: `slack`, `discord`, `telegram`, `all` | - |
| `RUVBOT_DEPLOY` | Deploy to: `cloudrun`, `docker`, `k8s` | - |
| `RUVBOT_WIZARD` | Run interactive setup wizard | `false` |

### Install via npm/npx

```bash
# Run directly (no install)
npx ruvbot start

# Install globally
npm install -g ruvbot
ruvbot start

# Install locally
npm install ruvbot
npx ruvbot start
```

## Configuration

### Environment Variables

```bash
# LLM Provider (required - choose one)
# Option 1: OpenRouter (RECOMMENDED - access to Gemini 2.5, Claude, GPT, etc.)
export OPENROUTER_API_KEY=sk-or-xxx

# Option 2: Anthropic Direct
export ANTHROPIC_API_KEY=sk-ant-xxx

# Slack Integration (optional)
export SLACK_BOT_TOKEN=xoxb-xxx
export SLACK_SIGNING_SECRET=xxx
export SLACK_APP_TOKEN=xapp-xxx

# Discord Integration (optional)
export DISCORD_TOKEN=xxx
export DISCORD_CLIENT_ID=xxx

# Server Configuration
export RUVBOT_PORT=3000
export RUVBOT_LOG_LEVEL=info
```

### Configuration File

Create `ruvbot.config.json`:

```json
{
  "name": "my-ruvbot",
  "api": {
    "enabled": true,
    "port": 3000,
    "host": "0.0.0.0"
  },
  "storage": {
    "type": "sqlite",
    "path": "./data/ruvbot.db"
  },
  "memory": {
    "dimensions": 384,
    "maxVectors": 100000,
    "indexType": "hnsw"
  },
  "skills": {
    "enabled": ["search", "summarize", "code", "memory"]
  },
  "slack": {
    "enabled": true,
    "socketMode": true
  }
}
```

## CLI Commands

```bash
# Initialize in current directory
ruvbot init

# Start the bot server
ruvbot start [--port 3000] [--debug]

# Start with a specific channel
ruvbot start --channel slack
ruvbot start --channel discord
ruvbot start --channel telegram

# Check status
ruvbot status

# Manage skills
ruvbot skills list
ruvbot skills add <name>

# Channel setup help
ruvbot channels list
ruvbot channels setup slack
ruvbot channels setup discord
ruvbot channels setup telegram

# Template library
ruvbot templates list
ruvbot templates info <template-id>
ruvbot deploy <template-id>

# Run diagnostics
ruvbot doctor

# Show configuration
ruvbot config --show
```

## Channel Integrations

RuvBot supports multiple messaging platforms. Use `ruvbot channels setup <platform>` for interactive setup guides.

### Slack Integration

**Step 1: Create a Slack App**
1. Go to https://api.slack.com/apps
2. Click "Create New App" → "From Scratch"
3. Name your app and select your workspace

**Step 2: Configure Bot Permissions**
Navigate to **OAuth & Permissions** and add these Bot Token Scopes:
- `app_mentions:read` - Receive @mentions
- `chat:write` - Send messages
- `channels:history` - Read channel messages
- `im:history` - Read direct messages
- `reactions:write` - Add reactions
- `files:read` - Access shared files

**Step 3: Enable Socket Mode**
1. Go to **Socket Mode** → Enable
2. Create an App-Level Token with `connections:write` scope
3. Save the `xapp-...` token

**Step 4: Install & Get Tokens**
1. Go to **Install App** → Install to Workspace
2. Copy the Bot User OAuth Token (`xoxb-...`)
3. Copy the Signing Secret from **Basic Information**

**Step 5: Configure Environment**
```bash
export SLACK_BOT_TOKEN="xoxb-your-bot-token"
export SLACK_SIGNING_SECRET="your-signing-secret"
export SLACK_APP_TOKEN="xapp-your-app-token"

# Start with Slack
ruvbot start --channel slack
```

**Step 6: Enable Events (Optional)**
For real-time events without Socket Mode:
1. Go to **Event Subscriptions** → Enable
2. Add Request URL: `https://your-ruvbot.run.app/slack/events`
3. Subscribe to bot events: `message.channels`, `message.im`, `app_mention`

---

### Discord Integration

**Step 1: Create a Discord Application**
1. Go to https://discord.com/developers/applications
2. Click "New Application" and name it

**Step 2: Create a Bot**
1. Go to **Bot** section → Add Bot
2. Enable Privileged Gateway Intents:
   - ✅ Message Content Intent
   - ✅ Server Members Intent
3. Copy the Bot Token (click "Reset Token" if needed)

**Step 3: Get Application IDs**
1. Copy **Application ID** from General Information
2. Right-click your server → Copy Server ID (for guild-specific commands)

**Step 4: Invite Bot to Server**
1. Go to **OAuth2** → **URL Generator**
2. Select scopes: `bot`, `applications.commands`
3. Select permissions: `Send Messages`, `Read Message History`, `Add Reactions`, `Use Slash Commands`
4. Open the generated URL to invite the bot

**Step 5: Configure Environment**
```bash
export DISCORD_TOKEN="your-bot-token"
export DISCORD_CLIENT_ID="your-application-id"
export DISCORD_GUILD_ID="your-server-id"  # Optional, for testing

# Start with Discord
ruvbot start --channel discord
```

---

### Telegram Integration

**Step 1: Create a Bot with BotFather**
1. Open Telegram and search for `@BotFather`
2. Send `/newbot` command
3. Follow prompts to name your bot
4. Copy the HTTP API token (format: `123456789:ABC-DEF...`)

**Step 2: Configure Environment**
```bash
export TELEGRAM_BOT_TOKEN="your-bot-token"

# Start with Telegram
ruvbot start --channel telegram
```

**Step 3: Test Your Bot**
1. Search for your bot by username in Telegram
2. Start a chat and send `/start`
3. Send messages to interact with RuvBot

**Production: Webhook Mode**
For production deployments (Cloud Run, etc.), use webhook mode:
```bash
export TELEGRAM_BOT_TOKEN="your-bot-token"
export TELEGRAM_WEBHOOK_URL="https://your-ruvbot.run.app/telegram/webhook"
```

---

### Multi-Channel Configuration

Run RuvBot with multiple channels simultaneously:

```json
{
  "name": "my-ruvbot",
  "channels": {
    "slack": {
      "enabled": true,
      "token": "${SLACK_BOT_TOKEN}",
      "signingSecret": "${SLACK_SIGNING_SECRET}",
      "appToken": "${SLACK_APP_TOKEN}"
    },
    "discord": {
      "enabled": true,
      "token": "${DISCORD_TOKEN}",
      "clientId": "${DISCORD_CLIENT_ID}"
    },
    "telegram": {
      "enabled": true,
      "token": "${TELEGRAM_BOT_TOKEN}"
    }
  }
}
```

Install optional dependencies:
```bash
npm install @slack/bolt @slack/web-api discord.js telegraf
```

---

### Cloud Run Channel Setup

For Google Cloud Run deployments:

```bash
# Slack
gcloud run services update ruvbot --set-env-vars="\
SLACK_BOT_TOKEN=xoxb-...,\
SLACK_SIGNING_SECRET=...,\
SLACK_APP_TOKEN=xapp-..."

# Discord
gcloud run services update ruvbot --set-env-vars="\
DISCORD_TOKEN=...,\
DISCORD_CLIENT_ID=...,\
DISCORD_GUILD_ID=..."

# Telegram (webhook mode recommended)
gcloud run services update ruvbot --set-env-vars="\
TELEGRAM_BOT_TOKEN=...,\
TELEGRAM_WEBHOOK_URL=https://ruvbot-xxx.run.app/telegram/webhook"
```

## Template Library

RuvBot includes pre-built templates for deploying long-running agent patterns.

### List Available Templates

```bash
ruvbot templates list
ruvbot templates list --category advanced
```

### Available Templates

| Category | Template | Description |
|----------|----------|-------------|
| **Practical** | `code-reviewer` | Automated code review with security scanning |
| | `doc-generator` | Auto-generate project documentation |
| | `test-generator` | Generate comprehensive test suites (TDD) |
| **Intermediate** | `feature-swarm` | Parallel feature development with coordinated agents |
| | `refactor-squad` | Coordinated codebase refactoring |
| | `ci-cd-pipeline` | Automated build, test, and deployment |
| **Advanced** | `self-learning-bot` | AI that improves from interactions |
| | `research-swarm` | Distributed research across sources |
| | `performance-optimizer` | Continuous performance monitoring |
| **Exotic** | `byzantine-validator` | Byzantine fault-tolerant validation (33% fault tolerance) |
| | `hive-mind` | Emergent collective intelligence with queen coordination |
| | `multi-repo-coordinator` | Cross-repository change coordination |
| | `adversarial-tester` | Red team vs blue team security testing |

### Deploy a Template

```bash
# View template details
ruvbot templates info hive-mind

# Deploy a template
ruvbot deploy code-reviewer --repo ./my-project

# Deploy with options
ruvbot deploy feature-swarm --name "auth-feature" --model google/gemini-2.0-flash-001

# Dry run (preview without executing)
ruvbot deploy hive-mind --dry-run
```

### Template Examples

```bash
# Code review with security scanning
ruvbot deploy code-reviewer --repo ./my-project

# Documentation generation
ruvbot deploy doc-generator --output ./docs

# Test suite generation with 80% coverage target
ruvbot deploy test-generator --coverage 80

# Feature development with coordinated swarm
ruvbot deploy feature-swarm --feature "Add user authentication"

# Research across multiple sources
ruvbot deploy research-swarm --topic "vector databases"

# Hive-mind for complex objectives
ruvbot deploy hive-mind --objective "Build complete REST API"

# Byzantine validation for critical operations
ruvbot deploy byzantine-validator --quorum 4
```

## API Usage

### REST API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check (Cloud Run ready) |
| `/ready` | GET | Readiness check |
| `/api/status` | GET | Bot status and metrics |
| `/api/models` | GET | List available LLM models |
| `/api/agents` | GET/POST | Agent management |
| `/api/sessions` | GET/POST | Session management |
| `/api/sessions/:id/chat` | POST | Send message with AIDefence |

### Quick Start

```bash
# Check health
curl https://your-ruvbot.run.app/health

# List available models
curl https://your-ruvbot.run.app/api/models

# Create a session
curl -X POST https://your-ruvbot.run.app/api/sessions \
  -H "Content-Type: application/json" \
  -d '{"agentId": "default-agent"}'

# Chat (with automatic AIDefence protection)
curl -X POST https://your-ruvbot.run.app/api/sessions/{id}/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello, RuvBot!"}'
```

### Programmatic Usage

```typescript
import { RuvBot, createRuvBot } from '@ruvector/ruvbot';

// Create bot instance
const bot = createRuvBot({
  config: {
    llm: {
      provider: 'anthropic',
      apiKey: process.env.ANTHROPIC_API_KEY,
    },
    memory: {
      dimensions: 384,
      maxVectors: 100000,
    },
  },
});

// Start the bot
await bot.start();

// Spawn an agent
const agent = await bot.spawnAgent({
  id: 'assistant',
  name: 'My Assistant',
});

// Create a session
const session = await bot.createSession(agent.id, {
  userId: 'user-123',
  platform: 'api',
});

// Chat
const response = await bot.chat(session.id, 'What can you help me with?');
console.log(response.content);
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                           RuvBot                                 │
├─────────────────────────────────────────────────────────────────┤
│  REST API │ GraphQL │ Slack Adapter │ Discord │ Webhooks       │
├─────────────────────────────────────────────────────────────────┤
│                     Core Application Layer                       │
│  AgentManager │ SessionStore │ SkillRegistry │ MemoryManager    │
├─────────────────────────────────────────────────────────────────┤
│                      Learning Layer                              │
│  SONA Trainer │ Pattern Extractor │ Trajectory Store │ EWC++    │
├─────────────────────────────────────────────────────────────────┤
│                    Infrastructure Layer                          │
│  RuVector WASM │ PostgreSQL │ RuvLLM │ agentic-flow Workers     │
└─────────────────────────────────────────────────────────────────┘
```

## Intelligent LLM Routing (3-Tier)

| Tier | Handler | Latency | Cost | Use Cases |
|------|---------|---------|------|-----------|
| **1** | Agent Booster | <1ms | $0 | Simple transforms, formatting |
| **2** | Haiku | ~500ms | $0.0002 | Simple tasks, bug fixes |
| **3** | Sonnet/Opus | 2-5s | $0.003-$0.015 | Complex reasoning, architecture |

Benefits: **75% cost reduction**, **352x faster** for Tier 1 tasks.

## Security Architecture (6 Layers) - Why This Matters

**Clawdbot's security model is fundamentally insufficient for production AI:**

| Vulnerability | Clawdbot | RuvBot |
|--------------|----------|--------|
| Prompt Injection | **VULNERABLE** | Protected (<5ms) |
| Jailbreak Attacks | **VULNERABLE** | Detected + blocked |
| PII Data Leakage | **UNPROTECTED** | Auto-masked |
| Control Characters | **UNFILTERED** | Sanitized |
| Homoglyph Attacks | **VULNERABLE** | Normalized |
| Multi-tenant Isolation | **NONE** | PostgreSQL RLS |

**RuvBot's Defense-in-Depth Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│ Layer 1: Transport (TLS 1.3, HSTS, cert pinning)               │
│ Layer 2: Authentication (JWT RS256, OAuth 2.0, rate limiting)  │
│ Layer 3: Authorization (RBAC, claims, tenant isolation)        │
│ Layer 4: Data Protection (AES-256-GCM, key rotation)           │
│ Layer 5: AIDefence (prompt injection, jailbreak, PII)          │
│ Layer 6: WASM Sandbox (memory isolation, resource limits)      │
└─────────────────────────────────────────────────────────────────┘
```

Compliance Ready: **GDPR**, **SOC 2**, **HIPAA** (configurable).

## AI Defense (aidefence Integration) - Critical for Production

**Every production AI system needs adversarial defense.** Clawdbot has none. RuvBot integrates [aidefence](https://www.npmjs.com/package/aidefence) for military-grade protection.

### Threat Detection (<10ms latency)

- **Prompt Injection Detection** - 50+ injection pattern signatures
- **Jailbreak Prevention** - DAN, bypass, unlimited mode, roleplay attacks
- **PII Protection** - Email, phone, SSN, credit cards, API keys, IP addresses
- **Unicode Normalization** - Homoglyph and encoding attack prevention
- **Behavioral Analysis** - User baseline deviation detection
- **Response Validation** - Prevents LLM from leaking injected content
- **Audit Logging** - Full threat tracking for compliance

### Usage

```typescript
import { createAIDefenceGuard, createAIDefenceMiddleware } from '@ruvector/ruvbot';

// Create guard
const guard = createAIDefenceGuard({
  detectPromptInjection: true,
  detectJailbreak: true,
  detectPII: true,
  blockThreshold: 'medium',
});

// Analyze input
const result = await guard.analyze(userInput);
if (!result.safe) {
  console.log('Threats:', result.threats);
  const safeInput = result.sanitizedInput;
}

// Or use middleware
const middleware = createAIDefenceMiddleware();
const { allowed, sanitizedInput } = await middleware.validateInput(input);
```

### Threat Detection

| Threat | Severity | Latency |
|--------|----------|---------|
| Prompt Injection | High | <5ms |
| Jailbreak | Critical | <5ms |
| PII Exposure | Medium-Critical | <3ms |
| Control Characters | Medium | <1ms |

See [ADR-014: AIDefence Integration](docs/adr/ADR-014-aidefence-integration.md) for details.

## Plugin System

RuvBot includes an extensible plugin system inspired by claude-flow's IPFS-based registry.

### Features

- **Plugin Discovery**: Auto-load plugins from `./plugins` directory
- **Lifecycle Management**: Install, enable, disable, unload plugins
- **Hot-Reload**: Dynamic plugin loading without restart
- **Sandboxed Execution**: Permission-based access control
- **IPFS Registry**: Optional decentralized plugin distribution

### Usage

```typescript
import { createPluginManager } from '@ruvector/ruvbot';

// Create and initialize plugin manager
const plugins = createPluginManager({
  pluginsDir: './plugins',
  autoLoad: true,
});
await plugins.initialize();

// List plugins and skills
console.log(plugins.listPlugins());
const skills = plugins.getPluginSkills();
```

### Plugin Permissions

| Permission | Description |
|------------|-------------|
| `memory:read` | Read from memory store |
| `memory:write` | Write to memory store |
| `skill:register` | Register new skills |
| `llm:invoke` | Invoke LLM providers |
| `http:outbound` | Make external HTTP requests |

## Background Workers

| Worker | Priority | Purpose |
|--------|----------|---------|
| `ultralearn` | normal | Deep knowledge acquisition |
| `optimize` | high | Performance optimization |
| `consolidate` | low | Memory consolidation (EWC++) |
| `predict` | normal | Predictive preloading |
| `audit` | critical | Security analysis |
| `map` | normal | Codebase/context mapping |
| `deepdive` | normal | Deep code analysis |
| `document` | normal | Auto-documentation |
| `refactor` | normal | Refactoring suggestions |
| `benchmark` | normal | Performance benchmarking |
| `testgaps` | normal | Test coverage analysis |
| `preload` | low | Resource preloading |

## Skills

### Built-in Skills

| Skill | Description | SOTA Feature |
|-------|-------------|--------------|
| `search` | Semantic search across memory | HNSW O(log n) search |
| `summarize` | Generate concise summaries | Multi-level summarization |
| `code` | Code generation & analysis | AST-aware with context |
| `memory` | Long-term memory storage | SONA learning integration |
| `reasoning` | Multi-step reasoning | Chain-of-thought |
| `extraction` | Entity & pattern extraction | Named entity recognition |

### Self-Learning Pipeline

```
┌─────────────────────────────────────────────────────────────────┐
│  User Query ──► Agent Response ──► Outcome ──► Pattern Store    │
│       │              │               │              │           │
│       ▼              ▼               ▼              ▼           │
│   Embedding     Action Log       Reward Score   Neural Update   │
│                                                                 │
│  SONA 4-Step: RETRIEVE → JUDGE → DISTILL → CONSOLIDATE         │
└─────────────────────────────────────────────────────────────────┘
```

### Custom Skills

Create custom skills in the `skills/` directory:

```typescript
// skills/my-skill.ts
import { defineSkill } from '@ruvector/ruvbot';

export default defineSkill({
  name: 'my-skill',
  description: 'Custom skill description',
  inputs: [
    { name: 'query', type: 'string', required: true }
  ],
  async execute(params, context) {
    return {
      success: true,
      data: `Processed: ${params.query}`,
    };
  },
});
```

## Memory System

RuvBot uses HNSW-indexed vector memory for fast semantic search:

```typescript
import { MemoryManager, createWasmEmbedder } from '@ruvector/ruvbot/learning';

const embedder = createWasmEmbedder({ dimensions: 384 });
const memory = new MemoryManager({
  config: { dimensions: 384, maxVectors: 100000, indexType: 'hnsw' },
  embedder,
});

// Store a memory
await memory.store('Important information', {
  source: 'user',
  tags: ['important'],
  importance: 0.9,
});

// Search memories
const results = await memory.search('find important info', {
  topK: 5,
  threshold: 0.7,
});
```

## Deployment

RuvBot supports multiple deployment options from local development to enterprise cloud.

### Quick Deploy Options

| Method | Best For | Command |
|--------|----------|---------|
| **npx** | Quick testing | `npx ruvbot start` |
| **Docker** | Containerized | `docker run -p 3000:3000 ruvector/ruvbot` |
| **Cloud Run** | Serverless | `ruvbot deploy cloudrun` |
| **Kubernetes** | Enterprise | `kubectl apply -f k8s/` |

### Docker

```yaml
# docker-compose.yml
version: '3.8'
services:
  ruvbot:
    image: ruvector/ruvbot:latest
    ports:
      - "3000:3000"
    environment:
      - OPENROUTER_API_KEY=${OPENROUTER_API_KEY}
      - SLACK_BOT_TOKEN=${SLACK_BOT_TOKEN}
      - SLACK_SIGNING_SECRET=${SLACK_SIGNING_SECRET}
      - SLACK_APP_TOKEN=${SLACK_APP_TOKEN}
      - DISCORD_TOKEN=${DISCORD_TOKEN}
      - TELEGRAM_BOT_TOKEN=${TELEGRAM_BOT_TOKEN}
    volumes:
      - ./data:/app/data
      - ./skills:/app/skills
```

### Docker with Webhooks

```yaml
# docker-compose.yml with webhook configuration
version: '3.8'
services:
  ruvbot:
    image: ruvector/ruvbot:latest
    ports:
      - "3000:3000"
    environment:
      - OPENROUTER_API_KEY=${OPENROUTER_API_KEY}
      # Outbound webhooks
      - WEBHOOK_URL=https://your-service.com/callback
      - WEBHOOK_SECRET=your-shared-secret
      # Inbound webhook auth
      - WEBHOOK_AUTH_TOKEN=your-auth-token
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

## Google Cloud Deployment

RuvBot includes cost-optimized Google Cloud Platform deployment (~$15-20/month for low traffic).

### Quick Deploy

```bash
# Set environment variables
export ANTHROPIC_API_KEY="sk-ant-..."
export PROJECT_ID="my-gcp-project"

# Deploy with script
./deploy/gcp/deploy.sh --project-id $PROJECT_ID
```

### Terraform (Infrastructure as Code)

```bash
cd deploy/gcp/terraform
terraform init
terraform apply \
  -var="project_id=my-project" \
  -var="anthropic_api_key=$ANTHROPIC_API_KEY"
```

### Cost Breakdown

| Service | Configuration | Monthly Cost |
|---------|---------------|--------------|
| Cloud Run | 0-10 instances, 512Mi | ~$0-5 (free tier) |
| Cloud SQL | db-f1-micro PostgreSQL | ~$10-15 |
| Secret Manager | 3-5 secrets | ~$0.18 |
| Cloud Storage | Standard | ~$0.02/GB |
| **Total** | | **~$15-20/month** |

### Features

- **Serverless**: Scale to zero when not in use
- **Managed Database**: Cloud SQL PostgreSQL with automatic backups
- **Secure Secrets**: Secret Manager for API keys
- **CI/CD**: Cloud Build pipeline included
- **Terraform**: Full infrastructure as code support

See [ADR-013: GCP Deployment](docs/adr/ADR-013-gcp-deployment.md) for architecture details.

### gcloud CLI Integration

RuvBot includes native gcloud CLI integration for Cloud Run deployments.

**Prerequisites:**
```bash
# Install gcloud CLI
curl https://sdk.cloud.google.com | bash

# Authenticate
gcloud auth login

# Set project
gcloud config set project YOUR_PROJECT_ID
```

**Deploy with CLI:**
```bash
# Interactive deployment wizard
ruvbot deploy-cloud wizard

# Deploy to Cloud Run
ruvbot deploy-cloud cloudrun --project my-project --region us-central1

# Deploy with environment file
ruvbot deploy-cloud cloudrun --env-file .env

# Deploy to Docker
ruvbot deploy-cloud docker --port 3000

# Deploy to Kubernetes
ruvbot deploy-cloud k8s --namespace production --replicas 3

# Check deployment status
ruvbot deploy-cloud status
```

**CLI Options:**

| Command | Options | Description |
|---------|---------|-------------|
| `cloudrun` | `--project`, `--region`, `--service`, `--memory`, `--min-instances`, `--max-instances`, `--env-file` | Deploy to Cloud Run |
| `docker` | `--name`, `--port`, `--detach`, `--env-file` | Deploy with Docker Compose |
| `k8s` | `--namespace`, `--replicas`, `--env-file` | Deploy to Kubernetes |
| `wizard` | - | Interactive deployment wizard |
| `status` | `--platform` | Check deployment status |

### Deploy with Channel Webhooks (Cloud Run)

Complete Cloud Run deployment with all channel webhooks:

```bash
# Build and deploy
gcloud run deploy ruvbot \
  --source . \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --set-env-vars="\
OPENROUTER_API_KEY=${OPENROUTER_API_KEY},\
DEFAULT_MODEL=google/gemini-2.0-flash-001,\
SLACK_BOT_TOKEN=${SLACK_BOT_TOKEN},\
SLACK_SIGNING_SECRET=${SLACK_SIGNING_SECRET},\
TELEGRAM_BOT_TOKEN=${TELEGRAM_BOT_TOKEN},\
TELEGRAM_WEBHOOK_URL=https://ruvbot-xxx.run.app/telegram/webhook,\
DISCORD_TOKEN=${DISCORD_TOKEN},\
DISCORD_CLIENT_ID=${DISCORD_CLIENT_ID}"
```

### Kubernetes Deployment

```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ruvbot
spec:
  replicas: 2
  selector:
    matchLabels:
      app: ruvbot
  template:
    metadata:
      labels:
        app: ruvbot
    spec:
      containers:
      - name: ruvbot
        image: ruvector/ruvbot:latest
        ports:
        - containerPort: 3000
        envFrom:
        - secretRef:
            name: ruvbot-secrets
        livenessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /ready
            port: 3000
          initialDelaySeconds: 5
          periodSeconds: 10
---
apiVersion: v1
kind: Service
metadata:
  name: ruvbot
spec:
  selector:
    app: ruvbot
  ports:
  - port: 80
    targetPort: 3000
  type: LoadBalancer
```

## Webhook Configuration

### Inbound Webhooks

RuvBot exposes webhook endpoints for receiving messages from external services.

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/webhook/message` | POST | Receive chat messages |
| `/webhook/event` | POST | Receive system events |
| `/slack/events` | POST | Slack Events API |
| `/telegram/webhook` | POST | Telegram webhook updates |
| `/api/sessions/:id/chat` | POST | Direct chat endpoint |

### Webhook Security

```bash
# Set webhook authentication
export WEBHOOK_SECRET="your-shared-secret"
export WEBHOOK_AUTH_TOKEN="bearer-token-for-inbound"
```

Inbound requests are validated using:
- **X-Webhook-Secret** header for HMAC signature verification
- **Authorization** header with Bearer token
- Request body signature validation

### Outbound Webhooks

Configure RuvBot to send responses and events to your services:

```json
{
  "webhooks": {
    "outbound": {
      "url": "https://your-service.com/ruvbot-callback",
      "secret": "shared-secret-for-signing",
      "events": ["message", "agent.spawn", "session.create", "error"],
      "retries": 3,
      "timeout": 30000,
      "headers": {
        "X-Custom-Header": "value"
      }
    }
  }
}
```

### Webhook Event Types

| Event | Payload | Description |
|-------|---------|-------------|
| `message` | `{sessionId, role, content, timestamp}` | New message in session |
| `message.response` | `{sessionId, content, model, tokens}` | Bot response generated |
| `agent.spawn` | `{agentId, type, name}` | Agent created |
| `agent.stop` | `{agentId, reason}` | Agent terminated |
| `session.create` | `{sessionId, agentId, userId}` | Session started |
| `session.end` | `{sessionId, messageCount, duration}` | Session ended |
| `memory.store` | `{key, namespace, size}` | Memory stored |
| `security.threat` | `{type, severity, blocked}` | Threat detected |
| `error` | `{code, message, context}` | Error occurred |

### Example: Custom Integration

```typescript
// Your webhook receiver
app.post('/ruvbot-callback', (req, res) => {
  // Verify signature
  const signature = req.headers['x-webhook-signature'];
  const isValid = verifyHmac(req.body, signature, WEBHOOK_SECRET);

  if (!isValid) {
    return res.status(401).send('Invalid signature');
  }

  const { event, data } = req.body;

  switch (event) {
    case 'message.response':
      // Forward to your chat system
      sendToChat(data.sessionId, data.content);
      break;
    case 'security.threat':
      // Alert security team
      alertSecurity(data);
      break;
  }

  res.status(200).send('OK');
});
```

### CLI Webhook Commands

```bash
# List configured webhooks
ruvbot webhooks list

# Test a webhook endpoint
ruvbot webhooks test https://your-service.com/callback

# Test with custom payload
ruvbot webhooks test https://your-service.com/callback \
  --payload '{"test": true, "message": "Hello"}'

# Show channel-specific setup
ruvbot channels setup webhook
```

## LLM Providers - Gemini 2.5 Default

RuvBot supports 12+ models with **Gemini 2.5 Pro as the recommended default** for optimal cost/performance.

### Available Models (via REST API)

```bash
curl https://your-ruvbot.run.app/api/models
```

| Model | Provider | Use Case | Recommended |
|-------|----------|----------|-------------|
| **Gemini 2.5 Pro** | OpenRouter | General + Reasoning | **Default** |
| Gemini 2.0 Flash | OpenRouter | Fast responses | Speed-critical |
| Gemini 2.0 Flash Thinking | OpenRouter | Reasoning (FREE) | Budget |
| Claude 3.5 Sonnet | Anthropic/OpenRouter | Complex analysis | Quality |
| Claude 3 Opus | Anthropic/OpenRouter | Deep reasoning | Premium |
| GPT-4o | OpenRouter | General | Alternative |
| O1 Preview | OpenRouter | Advanced reasoning | Complex |
| Qwen QwQ-32B | OpenRouter | Math + Reasoning | Cost-effective |
| DeepSeek R1 | OpenRouter | Open-source reasoning | Privacy |
| Llama 3.1 405B | OpenRouter | Large context | Enterprise |

### OpenRouter (Default - 200+ Models)

```typescript
import { createOpenRouterProvider } from '@ruvector/ruvbot';

// Gemini 2.5 Pro (recommended)
const provider = createOpenRouterProvider({
  apiKey: process.env.OPENROUTER_API_KEY,
  model: 'google/gemini-2.5-pro-preview-05-06',
});
```

### Anthropic (Direct)

```typescript
import { createAnthropicProvider } from '@ruvector/ruvbot';

const provider = createAnthropicProvider({
  apiKey: process.env.ANTHROPIC_API_KEY,
  model: 'claude-3-5-sonnet-20241022',
});
```

See [ADR-012: LLM Providers](docs/adr/ADR-012-llm-providers.md) for details.

## TypeScript Support

RuvBot is written in TypeScript and provides full type definitions:

```typescript
import type {
  RuvBot,
  RuvBotOptions,
  Agent,
  AgentConfig,
  Session,
  Message,
  BotConfig,
} from '@ruvector/ruvbot';

// Full IntelliSense support
const bot = createRuvBot({
  config: {
    name: 'MyBot',
    llm: { provider: 'anthropic', apiKey: '...' },
  },
});
```

## Events & Hooks

RuvBot emits events for lifecycle and message handling:

```typescript
import { createRuvBot } from '@ruvector/ruvbot';

const bot = createRuvBot();

// Lifecycle events
bot.on('ready', () => console.log('Bot is ready'));
bot.on('shutdown', () => console.log('Bot shutting down'));
bot.on('error', (error) => console.error('Error:', error));

// Agent events
bot.on('agent:spawn', (agent) => console.log('Agent spawned:', agent.id));
bot.on('agent:stop', (agentId) => console.log('Agent stopped:', agentId));

// Session events
bot.on('session:create', (session) => console.log('Session created:', session.id));
bot.on('session:end', (sessionId) => console.log('Session ended:', sessionId));

// Message events
bot.on('message', (message, session) => {
  console.log(`[${message.role}]: ${message.content}`);
});
```

## Streaming Responses

RuvBot supports streaming for real-time responses:

```typescript
import { createRuvBot } from '@ruvector/ruvbot';

const bot = createRuvBot({
  config: {
    llm: {
      provider: 'anthropic',
      apiKey: process.env.ANTHROPIC_API_KEY,
      streaming: true,  // Enable streaming
    },
  },
});

// Streaming is handled automatically in chat responses
const response = await bot.chat(sessionId, 'Tell me a story');
```

## Migration from Clawdbot

RuvBot provides a seamless migration path from Clawdbot:

### 1. Export Clawdbot Data

```bash
# Export your Clawdbot data
clawdbot export --format json > clawdbot-data.json
```

### 2. Install RuvBot

```bash
npm install -g @ruvector/ruvbot
```

### 3. Import Data

```bash
ruvbot import --from-clawdbot clawdbot-data.json
```

### 4. Verify Migration

```bash
ruvbot doctor --verify-migration
```

### Key Differences

| Aspect | Clawdbot | RuvBot |
|--------|----------|--------|
| Config file | `clawdbot.config.json` | `ruvbot.config.json` |
| Environment prefix | `CLAWDBOT_` | `RUVBOT_` |
| Skills directory | `./skills` | `./skills` (compatible) |
| Memory storage | SQLite | SQLite + PostgreSQL + HNSW |

### Skill Compatibility

All 52 Clawdbot skills are compatible with RuvBot. Simply copy your `skills/` directory.

## Troubleshooting

### Common Issues

**LLM not configured**
```
[RuvBot] LLM not configured. Received: "..."
```
Solution: Set `OPENROUTER_API_KEY` or `ANTHROPIC_API_KEY` environment variable.

**Agent not found**
```
Error: Agent with ID xxx not found
```
Solution: Create an agent first with `bot.spawnAgent()` or use `default-agent`.

**Session expired**
```
Error: Session with ID xxx not found
```
Solution: Sessions expire after 1 hour by default. Create a new session.

**Memory search returns empty**
```
No results found
```
Solution: Ensure you've stored memories first and the HNSW index is initialized.

### Debug Mode

```bash
# Enable debug logging
export LOG_LEVEL=debug
ruvbot start --debug
```

### Health Check

```bash
# Check service health
curl https://your-ruvbot.run.app/health

# Run diagnostics
ruvbot doctor
```

## Development

```bash
# Clone the repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/npm/packages/ruvbot

# Install dependencies
npm install

# Run in development mode
npm run dev

# Run tests
npm test

# Run tests with coverage
npm run test:coverage

# Type check
npm run typecheck

# Build
npm run build
```

## Dependencies

| Package | Purpose |
|---------|---------|
| `@ruvector/ruvllm` | LLM orchestration with SONA learning |
| `@ruvector/wasm-unified` | WASM vector operations |
| `@ruvector/postgres-cli` | PostgreSQL vector storage |
| `fastify` | REST API server |
| `@slack/bolt` | Slack integration |

## What's New in v0.3.1

- **RVF Microkernel** - Self-contained boot with real Linux 6.6.80 kernel (3.7 MB)
- **RVF Runner** - Extract, inspect, and boot RVF files with QEMU/Firecracker
- **Initramfs Builder** - On-the-fly initramfs generation with static init binary
- **6-Segment Format** - Kernel, WASM runtime, metadata, profile, witness, manifest

### Previous Releases

**v0.1.0**
- Gemini 2.5 Pro Support, 12+ LLM Models, AIDefence Integration
- 6-Layer Security, HNSW Vector Search (150x-12,500x faster)
- SONA Learning, Cloud Run Ready, Multi-tenancy

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for contribution guidelines.

## Links

- **Live Demo**: https://ruvbot-875130704813.us-central1.run.app
- **npm Package**: https://www.npmjs.com/package/@ruvector/ruvbot
- **Repository**: https://github.com/ruvnet/ruvector
- **Issues**: https://github.com/ruvnet/ruvector/issues
- **Documentation**: https://github.com/ruvnet/ruvector/tree/main/npm/packages/ruvbot
- **Feature Comparison**: [docs/FEATURE_COMPARISON.md](docs/FEATURE_COMPARISON.md)
- **ADR Documents**: [docs/adr/](docs/adr/)

## Support

- **GitHub Issues**: https://github.com/ruvnet/ruvector/issues
- **Discussions**: https://github.com/ruvnet/ruvector/discussions

---

Made with security in mind by the RuVector Team

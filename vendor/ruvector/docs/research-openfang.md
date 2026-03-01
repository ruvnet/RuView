# OpenFang Project Research

**Date**: 2026-02-26
**Scope**: GitHub projects using the "OpenFang" name

---

## Summary

There are three distinct projects on GitHub that share the "OpenFang" name:

| Project | Domain | Language | License | Stars | Status |
|---------|--------|----------|---------|-------|--------|
| [RightNow-AI/openfang](https://github.com/RightNow-AI/openfang) | Agent Operating System | Rust | MIT / Apache 2.0 | ~979 | Active (v0.1.0, Feb 2026) |
| [anmaped/openfang](https://github.com/anmaped/openfang) | Camera firmware (Ingenic T20) | PHP/Shell | GPL-3.0 | ~188 | Dormant (last release 2018) |
| [danshorstein/OpenFang](https://github.com/danshorstein/OpenFang) | Python AI assistant | Python | Unknown | Low | Fork of OpenClaw |

---

## 1. RightNow-AI/openfang — Agent Operating System (Primary)

**Website**: [openfang.sh](https://www.openfang.sh/)
**Repo**: [github.com/RightNow-AI/openfang](https://github.com/RightNow-AI/openfang)
**Built by**: Jaber (RightNow AI)

### What It Is

OpenFang is a **production-grade Agent Operating System** built from scratch in Rust. It is not a chatbot framework or a Python wrapper around an LLM — it is a full operating system for autonomous agents that run 24/7, building knowledge graphs, monitoring targets, generating leads, and managing social media.

The entire system compiles to a **single ~32 MB binary**.

### Key Numbers

- **137,728** lines of Rust code across **14 crates**
- **1,767+** passing tests, **0** clippy warnings
- **30** pre-built agents across 4 performance tiers
- **40** channel adapters (Telegram, Discord, Slack, WhatsApp, Signal, Matrix, etc.)
- **38** built-in tools + MCP integration
- **27** LLM providers supporting **123+** models
- **16** security systems
- **v0.1.0** — first public release (February 2026)

### Performance Benchmarks

| Metric | OpenFang | OpenClaw | CrewAI | AutoGen |
|--------|----------|----------|--------|---------|
| Cold Start | 180ms | 5.98s | 3s | — |
| Idle Memory | 40MB | 394MB | 250MB | — |
| Install Size | 32MB | 500MB | — | 200MB |

### The 7 "Hands" (Autonomous Agents)

1. **Clip** — Video processing: downloads YouTube content, creates vertical shorts with captions
2. **Lead** — Daily prospect discovery with ICP matching and qualification scoring
3. **Collector** — OSINT intelligence with continuous monitoring and change detection
4. **Predictor** — Superforecasting engine with calibrated reasoning and accuracy tracking
5. **Researcher** — Cross-references sources using CRAAP criteria with APA citations
6. **Twitter** — Account management across 7 content formats with approval gates
7. **Browser** — Web automation with mandatory purchase approval safeguards

### 14 Core Rust Crates

| Crate | Purpose |
|-------|---------|
| `openfang-kernel` | Orchestration, workflows, RBAC |
| `openfang-runtime` | Agent loop, 53 tools, WASM sandbox |
| `openfang-api` | 140+ REST/WS/SSE endpoints |
| `openfang-channels` | 40 messaging adapters |
| `openfang-memory` | SQLite persistence, vector embeddings |
| `openfang-skills` | 60 bundled skills |
| `openfang-hands` | Lifecycle management for autonomous agents |
| `openfang-extensions` | 25 MCP templates |
| `openfang-wire` | P2P protocol |
| `openfang-cli` | Daemon management |
| `openfang-desktop` | Tauri 2.0 native app |
| `openfang-migrate` | OpenClaw/LangChain migration |

### 16 Security Systems

1. WASM dual-metered sandbox (fuel + epoch interruption)
2. Merkle hash-chain audit trails
3. Information flow taint tracking
4. Ed25519 signed agent manifests
5. SSRF protection
6. Secret zeroization
7. OFP mutual authentication (HMAC-SHA256)
8. Capability gates (role-based access)
9. Security headers (CSP, HSTS, X-Frame-Options)
10. Health endpoint redaction
11. Subprocess sandbox with environment isolation
12. Prompt injection scanner
13. Loop guard with circuit breaker
14. Session repair (7-phase validation)
15. Path traversal prevention
16. GCRA rate limiter

### Protocol Support

- **MCP** (Model Context Protocol)
- **A2A** (Agent-to-Agent)
- **OFP** (OpenFang Protocol — proprietary P2P with HMAC-SHA256 mutual auth)

### LLM Providers (27)

Anthropic, OpenAI, Google Gemini, Groq, DeepSeek, Mistral, xAI, Ollama, AWS Bedrock, and 18+ others — supporting 123+ models total.

### Installation

```bash
# macOS/Linux
curl -fsSL https://openfang.sh/install | sh
openfang init
openfang start
# Dashboard: http://localhost:4200

# Windows
irm https://openfang.sh/install.ps1 | iex
```

### Key Differentiators

- Single binary deployment — no Python, no Node, no Docker required
- OpenAI-compatible API — drop-in replacement capability
- Migration engine — imports from OpenClaw, LangChain, AutoGPT
- Dashboard-first — web UI at localhost:4200
- Desktop app — native Tauri 2.0 application with system tray

---

## 2. anmaped/openfang — Camera Firmware

**Repo**: [github.com/anmaped/openfang](https://github.com/anmaped/openfang)

### What It Is

An open-source bootloader, kernel, and toolchain for IP cameras using **Ingenic T10 and T20 SoCs**. This was one of the early community firmware projects for cheap Chinese IP cameras.

### Supported Devices

| SoC | RAM | Cameras |
|-----|-----|---------|
| Ingenic T20L | 64MB DDR | Xiaomi Mijia 2018, Xiaomi Xiaofang 1S |
| Ingenic T20N | 64MB DDR + SIMD128 | DIGOO DG W30 |
| Ingenic T20X | 128MB DDR | Wyze Cam V2, Xiaomi Dafang, Wyze Cam Pan |

### Technical Details

- Kernel version 3.10.14
- U-Boot bootloader v2013.07
- Buildroot-based toolchain
- Docker support for compilation
- GPL-3.0 license

### Status

- **Last release**: RC5 (November 2018) — **dormant**
- 188 stars, 43 forks, 10 contributors
- Largely superseded by [OpenMiko](https://github.com/openmiko/openmiko), [OpenIPC](https://github.com/OpenIPC), and [Thingino](https://thingino.com/)

---

## 3. danshorstein/OpenFang — Python AI Assistant

**Repo**: [github.com/danshorstein/OpenFang](https://github.com/danshorstein/OpenFang)

### What It Is

An open-source fork of **OpenClaw** that rethinks personal AI agents. Built on the principle that "LLMs should write automations, not be automations."

### Key Claims

- 90%+ reduction in token costs
- Faster execution and more reliable automations
- System gets cheaper over time as workflows graduate from LLM-orchestrated to Python-automated

### Status

Low activity, small community. Positioned as a philosophical alternative to the mainstream agent frameworks.

---

## Analysis & Relevance

### Most Notable: RightNow-AI/openfang

The **RightNow-AI** variant is by far the most significant project:

- **Active development** with a February 2026 v0.1.0 release
- **Rust-based architecture** — high performance, single binary, low memory
- **Comprehensive agent ecosystem** — 30 agents, 40 channels, 38 tools
- **Strong security posture** — 16 dedicated security systems
- **Production-oriented** — not a research project or toy framework

### Potential Relevance to RuVector

- The Rust architecture and WASM sandbox approach could inform solver optimization strategies
- The 14-crate modular design demonstrates a scalable Rust workspace pattern
- The security systems (especially taint tracking, prompt injection scanning) are relevant to any AI-adjacent system
- The performance benchmarks (180ms cold start, 40MB idle) set a useful reference point

---

## Sources

- [RightNow-AI/openfang (GitHub)](https://github.com/RightNow-AI/openfang)
- [OpenFang.sh (Website)](https://www.openfang.sh/)
- [anmaped/openfang (GitHub)](https://github.com/anmaped/openfang)
- [danshorstein/OpenFang (GitHub)](https://github.com/danshorstein/OpenFang)
- [OpenMiko (GitHub)](https://github.com/openmiko/openmiko)
- [OpenIPC (GitHub)](https://github.com/OpenIPC)

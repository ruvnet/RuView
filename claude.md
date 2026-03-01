# Claude Code Configuration — WiFi-DensePose + Claude Flow V3

## Project: wifi-densepose

WiFi-based human pose estimation using Channel State Information (CSI).
Dual codebase: Python v1 (`v1/`) and Rust port (`rust-port/wifi-densepose-rs/`).

### Key Rust Crates
- `wifi-densepose-signal` — SOTA signal processing (conjugate mult, Hampel, Fresnel, BVP, spectrogram)
- `wifi-densepose-train` — Training pipeline with ruvector integration (ADR-016)
- `wifi-densepose-mat` — Disaster detection module (MAT, multi-AP, triage)
- `wifi-densepose-nn` — Neural network inference (DensePose head, RCNN)
- `wifi-densepose-hardware` — ESP32 aggregator, hardware interfaces

### RuVector v2.0.4 Integration (ADR-016 complete, ADR-017 proposed)
All 5 ruvector crates integrated in workspace:
- `ruvector-mincut` → `metrics.rs` (DynamicPersonMatcher) + `subcarrier_selection.rs`
- `ruvector-attn-mincut` → `model.rs` (apply_antenna_attention) + `spectrogram.rs`
- `ruvector-temporal-tensor` → `dataset.rs` (CompressedCsiBuffer) + `breathing.rs`
- `ruvector-solver` → `subcarrier.rs` (sparse interpolation 114→56) + `triangulation.rs`
- `ruvector-attention` → `model.rs` (apply_spatial_attention) + `bvp.rs`

### Architecture Decisions
28 ADRs in `docs/adr/` (ADR-001 through ADR-028). Key ones:
- ADR-014: SOTA signal processing (Accepted)
- ADR-015: MM-Fi + Wi-Pose training datasets (Accepted)
- ADR-016: RuVector training pipeline integration (Accepted — complete)
- ADR-017: RuVector signal + MAT integration (Proposed — next target)
- ADR-024: Contrastive CSI embedding / AETHER (Accepted)
- ADR-027: Cross-environment domain generalization / MERIDIAN (Accepted)
- ADR-028: ESP32 capability audit + witness verification (Accepted)

### Build & Test Commands (this repo)
```bash
# Rust — full workspace tests (1,031 tests, ~2 min)
cd rust-port/wifi-densepose-rs
cargo test --workspace --no-default-features

# Rust — single crate check (no GPU needed)
cargo check -p wifi-densepose-train --no-default-features

# Python — deterministic proof verification (SHA-256)
python v1/data/proof/verify.py

# Python — test suite
cd v1 && python -m pytest tests/ -x -q
```

### Validation & Witness Verification (ADR-028)

**After any significant code change, run the full validation:**

```bash
# 1. Rust tests — must be 1,031+ passed, 0 failed
cd rust-port/wifi-densepose-rs
cargo test --workspace --no-default-features

# 2. Python proof — must print VERDICT: PASS
cd ../..
python v1/data/proof/verify.py

# 3. Generate witness bundle (includes both above + firmware hashes)
bash scripts/generate-witness-bundle.sh

# 4. Self-verify the bundle — must be 7/7 PASS
cd dist/witness-bundle-ADR028-*/
bash VERIFY.sh
```

**If the Python proof hash changes** (e.g., numpy/scipy version update):
```bash
# Regenerate the expected hash, then verify it passes
python v1/data/proof/verify.py --generate-hash
python v1/data/proof/verify.py
```

**Witness bundle contents** (`dist/witness-bundle-ADR028-<sha>.tar.gz`):
- `WITNESS-LOG-028.md` — 33-row attestation matrix with evidence per capability
- `ADR-028-esp32-capability-audit.md` — Full audit findings
- `proof/verify.py` + `expected_features.sha256` — Deterministic pipeline proof
- `test-results/rust-workspace-tests.log` — Full cargo test output
- `firmware-manifest/source-hashes.txt` — SHA-256 of all 7 ESP32 firmware files
- `crate-manifest/versions.txt` — All 15 crates with versions
- `VERIFY.sh` — One-command self-verification for recipients

**Key proof artifacts:**
- `v1/data/proof/verify.py` — Trust Kill Switch: feeds reference signal through production pipeline, hashes output
- `v1/data/proof/expected_features.sha256` — Published expected hash
- `v1/data/proof/sample_csi_data.json` — 1,000 synthetic CSI frames (seed=42)
- `docs/WITNESS-LOG-028.md` — 11-step reproducible verification procedure
- `docs/adr/ADR-028-esp32-capability-audit.md` — Complete audit record

### Branch
Default branch: `main`

---

## Behavioral Rules (Always Enforced)

- Do what has been asked; nothing more, nothing less
- NEVER create files unless they're absolutely necessary for achieving your goal
- ALWAYS prefer editing an existing file to creating a new one
- NEVER proactively create documentation files (*.md) or README files unless explicitly requested
- NEVER save working files, text/mds, or tests to the root folder
- Never continuously check status after spawning a swarm — wait for results
- ALWAYS read a file before editing it
- NEVER commit secrets, credentials, or .env files

## File Organization

- NEVER save to root folder — use the directories below
- `docs/adr/` — Architecture Decision Records
- `rust-port/wifi-densepose-rs/crates/` — Rust workspace crates (signal, train, mat, nn, hardware)
- `v1/src/` — Python source (core, hardware, services, api)
- `v1/data/proof/` — Deterministic CSI proof bundles
- `.claude-flow/` — Claude Flow coordination state (committed for team sharing)
- `.claude/` — Claude Code settings, agents, memory (committed for team sharing)

## Project Architecture

- Follow Domain-Driven Design with bounded contexts
- Keep files under 500 lines
- Use typed interfaces for all public APIs
- Prefer TDD London School (mock-first) for new code
- Use event sourcing for state changes
- Ensure input validation at system boundaries

### Project Config

- **Topology**: hierarchical-mesh
- **Max Agents**: 15
- **Memory**: hybrid
- **HNSW**: Enabled
- **Neural**: Enabled

## Pre-Merge Checklist

Before merging any PR, verify each item applies and is addressed:

1. **Rust tests pass** — `cargo test --workspace --no-default-features` (1,031+ passed, 0 failed)
2. **Python proof passes** — `python v1/data/proof/verify.py` (VERDICT: PASS)
3. **README.md** — Update platform tables, crate descriptions, hardware tables, feature summaries if scope changed
4. **CHANGELOG.md** — Add entry under `[Unreleased]` with what was added/fixed/changed
5. **User guide** (`docs/user-guide.md`) — Update if new data sources, CLI flags, or setup steps were added
6. **ADR index** — Update ADR count in README docs table if a new ADR was created
7. **Witness bundle** — Regenerate if tests or proof hash changed: `bash scripts/generate-witness-bundle.sh`
8. **Docker Hub image** — Only rebuild if Dockerfile, dependencies, or runtime behavior changed
9. **Crate publishing** — Only needed if a crate is published to crates.io and its public API changed
10. **`.gitignore`** — Add any new build artifacts or binaries

## Build & Test

```bash
# Build
npm run build

# Test
npm test

# Lint
npm run lint
```

- ALWAYS run tests after making code changes
- ALWAYS verify build succeeds before committing

## Security Rules

- NEVER hardcode API keys, secrets, or credentials in source files
- NEVER commit .env files or any file containing secrets
- Always validate user input at system boundaries
- Always sanitize file paths to prevent directory traversal
- Run `npx @claude-flow/cli@latest security scan` after security-related changes

## Concurrency: 1 MESSAGE = ALL RELATED OPERATIONS

- All operations MUST be concurrent/parallel in a single message
- Use Claude Code's Task tool for spawning agents, not just MCP
- ALWAYS batch ALL todos in ONE TodoWrite call (5-10+ minimum)
- ALWAYS spawn ALL agents in ONE message with full instructions via Task tool
- ALWAYS batch ALL file reads/writes/edits in ONE message
- ALWAYS batch ALL Bash commands in ONE message

## Swarm Orchestration

- MUST initialize the swarm using CLI tools when starting complex tasks
- MUST spawn concurrent agents using Claude Code's Task tool
- Never use CLI tools alone for execution — Task tool agents do the actual work
- MUST call CLI tools AND Task tool in ONE message for complex work

### 3-Tier Model Routing (ADR-026)

| Tier | Handler | Latency | Cost | Use Cases |
|------|---------|---------|------|-----------|
| **1** | Agent Booster (WASM) | <1ms | $0 | Simple transforms (var→const, add types) — Skip LLM |
| **2** | Haiku | ~500ms | $0.0002 | Simple tasks, low complexity (<30%) |
| **3** | Sonnet/Opus | 2-5s | $0.003-0.015 | Complex reasoning, architecture, security (>30%) |

- Always check for `[AGENT_BOOSTER_AVAILABLE]` or `[TASK_MODEL_RECOMMENDATION]` before spawning agents
- Use Edit tool directly when `[AGENT_BOOSTER_AVAILABLE]`

## Swarm Configuration & Anti-Drift

- ALWAYS use hierarchical topology for coding swarms
- Keep maxAgents at 6-8 for tight coordination
- Use specialized strategy for clear role boundaries
- Use `raft` consensus for hive-mind (leader maintains authoritative state)
- Run frequent checkpoints via `post-task` hooks
- Keep shared memory namespace for all agents

```bash
npx @claude-flow/cli@latest swarm init --topology hierarchical --max-agents 8 --strategy specialized
```

## Swarm Execution Rules

- ALWAYS use `run_in_background: true` for all agent Task calls
- ALWAYS put ALL agent Task calls in ONE message for parallel execution
- After spawning, STOP — do NOT add more tool calls or check status
- Never poll TaskOutput or check swarm status — trust agents to return
- When agent results arrive, review ALL results before proceeding

## V3 CLI Commands

### Core Commands

| Command | Subcommands | Description |
|---------|-------------|-------------|
| `init` | 4 | Project initialization |
| `agent` | 8 | Agent lifecycle management |
| `swarm` | 6 | Multi-agent swarm coordination |
| `memory` | 11 | AgentDB memory with HNSW search |
| `task` | 6 | Task creation and lifecycle |
| `session` | 7 | Session state management |
| `hooks` | 17 | Self-learning hooks + 12 workers |
| `hive-mind` | 6 | Byzantine fault-tolerant consensus |

### Quick CLI Examples

```bash
npx @claude-flow/cli@latest init --wizard
npx @claude-flow/cli@latest agent spawn -t coder --name my-coder
npx @claude-flow/cli@latest swarm init --v3-mode
npx @claude-flow/cli@latest memory search --query "authentication patterns"
npx @claude-flow/cli@latest doctor --fix
```

## Available Agents (60+ Types)

### Core Development
`coder`, `reviewer`, `tester`, `planner`, `researcher`

### Specialized
`security-architect`, `security-auditor`, `memory-specialist`, `performance-engineer`

### Swarm Coordination
`hierarchical-coordinator`, `mesh-coordinator`, `adaptive-coordinator`

### GitHub & Repository
`pr-manager`, `code-review-swarm`, `issue-tracker`, `release-manager`

### SPARC Methodology
`sparc-coord`, `sparc-coder`, `specification`, `pseudocode`, `architecture`

## Memory Commands Reference

```bash
# Store (REQUIRED: --key, --value; OPTIONAL: --namespace, --ttl, --tags)
npx @claude-flow/cli@latest memory store --key "pattern-auth" --value "JWT with refresh" --namespace patterns

# Search (REQUIRED: --query; OPTIONAL: --namespace, --limit, --threshold)
npx @claude-flow/cli@latest memory search --query "authentication patterns"

# List (OPTIONAL: --namespace, --limit)
npx @claude-flow/cli@latest memory list --namespace patterns --limit 10

# Retrieve (REQUIRED: --key; OPTIONAL: --namespace)
npx @claude-flow/cli@latest memory retrieve --key "pattern-auth" --namespace patterns
```

## Quick Setup

```bash
claude mcp add claude-flow -- npx -y @claude-flow/cli@latest
npx @claude-flow/cli@latest daemon start
npx @claude-flow/cli@latest doctor --fix
```

## Claude Code vs CLI Tools

- Claude Code's Task tool handles ALL execution: agents, file ops, code generation, git
- CLI tools handle coordination via Bash: swarm init, memory, hooks, routing
- NEVER use CLI tools as a substitute for Task tool agents

## Support

- Documentation: https://github.com/ruvnet/claude-flow
- Issues: https://github.com/ruvnet/claude-flow/issues

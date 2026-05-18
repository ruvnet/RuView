# Claude Code Configuration — WiFi-DensePose

## Project: wifi-densepose

WiFi-based human pose estimation from Channel State Information (CSI).
Dual codebase: Python v1 (`archive/v1/`) and Rust port (`v2/`).

See **[`CHECKLIST.md`](CHECKLIST.md)** for current implementation status
(50 Done / 0 Open in-scope; ADR-100..120 are this operational session).

### Detailed handbooks (extracted to keep this file ≤200 lines)

| File | Covers |
|---|---|
| [`docs/dev-handbook.md`](docs/dev-handbook.md) | Rust crate map (15 crates), RuvSense modules (14), Cross-Viewpoint fusion (5), Architecture Decisions list, supported hardware, build & test commands, ESP32 firmware build + provision, release process, crate publishing order, witness verification (ADR-028) |
| [`docs/claude-swarm.md`](docs/claude-swarm.md) | V3 CLI commands, available agents (60+ types), memory commands reference, Claude Code vs CLI tools |
| [`docs/architecture.md`](docs/architecture.md) | End-to-end pipeline diagram from CSI capture to pose / vital signs / room fingerprint |
| [`docs/use-cases.md`](docs/use-cases.md) | Full deployment-tier catalogue (Everyday / Specialized / Robotics / Extreme) + all 60 ADR-041 edge modules + self-learning system (ADR-024) |
| [`docs/adr/`](docs/adr/) | All 120 ADRs (ADR-111 intentionally absent); session-specific records ADR-100..120 |

---

## Behavioral Rules (Always Enforced)

- Do what has been asked; nothing more, nothing less
- NEVER create files unless absolutely necessary for the goal
- ALWAYS prefer editing an existing file to creating a new one
- NEVER proactively create documentation files (*.md) or README files unless explicitly requested
- NEVER save working files, tests or markdown to the root folder
- ALWAYS read a file before editing it
- NEVER commit secrets, credentials, or .env files
- Never continuously check status after spawning a swarm — wait for results
- README.md and CLAUDE.md must each stay ≤ 200 lines; move detail to `docs/` and link

## File Organization

- NEVER save to root folder — use the directories below
- `docs/adr/` — Architecture Decision Records (currently 120, ADR-111 absent)
- `docs/ddd/` — Domain-Driven Design models
- `v2/crates/` — Rust workspace crates (15+ crates)
- `v2/crates/wifi-densepose-signal/src/ruvsense/` — RuvSense multistatic modules
- `v2/crates/wifi-densepose-ruvector/src/viewpoint/` — Cross-viewpoint fusion
- `v2/crates/wifi-densepose-hardware/src/esp32/` — ESP32 TDM protocol
- `firmware/esp32-csi-node/main/` — ESP32 C firmware (CSI capture, NVS config, OTA, channel hopping)
- `archive/v1/src/` — Python source (core, hardware, services, api)
- `archive/v1/data/proof/` — Deterministic CSI proof bundles
- `.claude-flow/` — Claude Flow coordination state (committed for team sharing)
- `.claude/` — Claude Code settings, agents, memory (committed for team sharing)

## Project Architecture

- Follow Domain-Driven Design with bounded contexts
- Keep files under 500 lines; ADRs ≤ 200 lines; README.md and CLAUDE.md ≤ 200 lines
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

1. **Rust tests pass** — `cargo test --workspace --no-default-features`
2. **Python proof passes** — `python archive/v1/data/proof/verify.py` (VERDICT: PASS)
3. **README.md** — Update if scope changed; verify ≤ 200 lines
4. **CLAUDE.md** — Update if scope changed; verify ≤ 200 lines; move detail into `docs/`
5. **CHANGELOG.md** — Add entry under `[Unreleased]`
6. **User guide** (`docs/user-guide.md`) — Update if new data sources, CLI flags, or setup steps
7. **ADR index** — Update ADR count + range in CHECKLIST and reference tables when a new ADR is created
8. **Witness bundle** — Regenerate if tests or proof hash changed: `bash scripts/generate-witness-bundle.sh`
9. **Docker Hub image** — Rebuild only if Dockerfile / dependencies / runtime behavior changed
10. **Crate publishing** — Only needed if a crate is published to crates.io and its public API changed
11. **`.gitignore`** — Add any new build artifacts or large deployment-specific data files
12. **Security audit** — Run a security review for new modules touching hardware/network boundaries

## Build & Test

```bash
# Rust — full workspace tests
cargo test --workspace --no-default-features

# Python — deterministic proof
python archive/v1/data/proof/verify.py
```

- ALWAYS run tests after code changes
- ALWAYS verify build succeeds before committing

Full per-crate commands and firmware flash recipe: **[`docs/dev-handbook.md`](docs/dev-handbook.md)**.

## Security Rules

- NEVER hardcode API keys, secrets, or credentials in source files
- NEVER commit `.env` files or any file containing secrets
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
- Never use CLI tools alone for execution — Task-tool agents do the actual work
- MUST call CLI tools AND Task tool in ONE message for complex work

Full CLI command reference, agent type catalogue, memory operations and
3-tier model routing: **[`docs/claude-swarm.md`](docs/claude-swarm.md)**.

## Swarm Execution Rules

1. **Spawn in background** — use `run_in_background: true` for all agent Task calls
2. **Spawn all at once** — put ALL agent Task calls in ONE message for parallel execution
3. **Tell the user** — after spawning, list what each agent is doing
4. **Stop and wait** — after spawning, STOP; do NOT add more tool calls or check status
5. **No polling** — never poll TaskOutput or swarm status; trust agents to return
6. **Synthesize** — when agent results arrive, review ALL before proceeding
7. **No confirmation** — don't ask "should I check?"; just wait for results

---

## Branch

Default branch: `main`.
Current operator branch (this session series): `feat/ota-rssi-mobile` —
PR [#596](https://github.com/ruvnet/RuView/pull/596) on the upstream fork.

## Support

- GitHub Issues: <https://github.com/ruvnet/RuView/issues>
- ADR index: [`docs/adr/`](docs/adr/)
- Implementation status: [`CHECKLIST.md`](CHECKLIST.md)
- Detailed dev handbook: [`docs/dev-handbook.md`](docs/dev-handbook.md)

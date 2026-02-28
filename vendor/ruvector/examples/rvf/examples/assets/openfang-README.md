# OpenFang Agent OS — RVF Full Surface Demo

Exercises **every major RVF capability** against a realistic agent-OS registry, using [OpenFang](https://github.com/RightNow-AI/openfang) as the domain model. A single ~35 KB RVF file holds 65 vector components plus embedded WASM, kernel, eBPF, and dashboard segments.

## Run

```bash
cd examples/rvf
cargo run --example openfang
```

## 24 Capabilities Demonstrated

| # | Capability | RVF API | What It Shows |
|--:|-----------|---------|---------------|
| 1 | **Store creation** | `RvfStore::create` | 128-dim L2 store with file identity |
| 2-4 | **Batch ingestion** | `ingest_batch` | Multi-type metadata (String, U64) with per-category vector biasing |
| 5 | **Nearest-neighbor search** | `query` | Unfiltered + type-filtered task routing |
| 6 | **Quality envelope** | `query_with_envelope` | ResponseQuality, safety-net activation, budget reporting |
| 7 | **Audited query** | `query_audited` | Auto-appends witness entry per search (compliance) |
| 8 | **Security filter** | `FilterExpr::Ge` | Hands with security >= 80 |
| 9 | **Tier filter** | `FilterExpr::Eq` | Tier-4 autonomous agents only |
| 10 | **Category filter** | `FilterExpr::And` | Security tools by category |
| 11 | **Membership filter** | `MembershipFilter` | Tenant isolation — tools-only view via bitmap |
| 12 | **DoS hardening** | `BudgetTokenBucket`, `NegativeCache`, `ProofOfWork` | Rate limiting, degenerate query blacklisting, PoW challenge |
| 13 | **Adversarial detection** | `is_degenerate_distribution`, `centroid_distance_cv` | CV analysis to detect uniform (attack) distance distributions |
| 14 | **Embed WASM** | `embed_wasm` / `extract_wasm` | Microkernel role, self-bootstrapping check |
| 15 | **Embed kernel** | `embed_kernel` / `extract_kernel` | Linux image with cmdline, API port binding |
| 16 | **Embed eBPF** | `embed_ebpf` / `extract_ebpf` | Socket filter program (2 instructions) |
| 17 | **Embed dashboard** | `embed_dashboard` / `extract_dashboard` | HTML registry dashboard bundle |
| 18 | **Delete + compact** | `delete` + `compact` | Decommission 'twitter', reclaim 512 bytes |
| 19 | **Derive (lineage)** | `derive` | Snapshot child with parent provenance, depth 0→1 |
| 20 | **COW branch + freeze** | `freeze` + `branch` | Staging env with experimental 'sentinel' agent |
| 21 | **AGI container** | `AgiContainerBuilder` + `ParsedAgiManifest` | Full manifest: model, orchestrator, tools, eval, policy |
| 22 | **Segment directory** | `segment_dir` | Raw segment inventory (VEC, WASM, KERN, EBPF, DASH) |
| 23 | **Witness chain** | `create_witness_chain` + `verify` | 17-entry cryptographic audit trail |
| 24 | **Persistence** | `close` + `open_readonly` | Round-trip with file ID, WASM, kernel, eBPF, dashboard preservation |

## Registry Contents

| Component | Count | Description |
|-----------|------:|-------------|
| **Hands** | 7 | Autonomous agents (Clip, Lead, Collector, Predictor, Researcher, Twitter, Browser) |
| **Tools** | 38 | Built-in capabilities across 13 categories |
| **Channels** | 20 | Messaging adapters (Telegram, Discord, Slack, WhatsApp, etc.) |
| **Total** | 65 | All searchable in one vector space |

## Metadata Schema

| Field ID | Constant | Name | Type | Applies To |
|:--------:|----------|------|------|------------|
| 0 | `F_TYPE` | component_type | String | All (`"hand"`, `"tool"`, `"channel"`) |
| 1 | `F_NAME` | name | String | All |
| 2 | `F_DOMAIN` | domain / category / protocol | String | All |
| 3 | `F_TIER` | tier | U64 (1-4) | Hands only |
| 4 | `F_SEC` | security_level | U64 (0-100) | Hands only |

## Hands

| Hand | Domain | Tier | Security |
|------|--------|:----:|:--------:|
| clip | video-processing | 3 | 60 |
| lead | sales-automation | 2 | 70 |
| collector | osint-intelligence | 4 | 90 |
| predictor | forecasting | 3 | 80 |
| researcher | fact-checking | 3 | 75 |
| twitter | social-media | 2 | 65 |
| browser | web-automation | 4 | 95 |

## Tool Categories (13)

`browser`, `communication`, `database`, `document`, `filesystem`, `inference`, `integration`, `memory`, `network`, `scheduling`, `security`, `system`, `transform`

## Channel Adapters (20)

Telegram, Discord, Slack, WhatsApp, Signal, Matrix, Email (SMTP/IMAP), Teams, Google Chat, LinkedIn, Twitter/X, Mastodon, Bluesky, Reddit, IRC, XMPP, Webhooks (in/out), gRPC

## Architecture Notes

### Vector Biasing

Tools and channels use `category_bias()` — a hash-based offset on the first 16 dimensions — so same-category items cluster in vector space. Hands use tier-proportional bias (`tier * 0.1`).

### Quality Envelope (Step 6)

`query_with_envelope` returns a `QualityEnvelope` containing:
- `ResponseQuality` — Verified, Approximate, Degraded, or Unreliable
- `SearchEvidenceSummary` — HNSW vs. safety-net candidate counts
- `BudgetReport` — time budget consumption in microseconds
- Optional `DegradationReport` if quality falls below threshold

### Audited Queries (Step 7)

`query_audited` works like `query` but auto-appends a `COMPUTATION` witness entry to the store's on-disk witness chain. Used for compliance-grade audit trails where every search must be recorded.

### Membership Filter (Step 11)

A dense bitmap that controls vector visibility:
- **Include mode**: only IDs in the bitmap are visible (tenant isolation)
- **Exclude mode**: IDs in the bitmap are hidden (access revocation)
- Serializes to compact bytes for network transfer between nodes

### DoS Hardening (Step 12)

Three-layer defense:
1. **BudgetTokenBucket** — rate-limits distance ops per time window
2. **NegativeCache** — blacklists query signatures that trigger degenerate search >N times
3. **ProofOfWork** — optional computational challenge (FNV-1a hash with leading-zero difficulty)

### Adversarial Detection (Step 13)

Detects attack vectors where all centroid distances are nearly uniform (CV < 0.05), indicating the query is designed to force exhaustive search. The `adaptive_n_probe` function widens search when degenerate distributions are detected.

### Segment Embedding (Steps 14-17)

Four segment types can be embedded into the RVF file:
- **WASM** — query engine microkernel or interpreter (enables self-bootstrapping)
- **Kernel** — Linux image with cmdline and API port binding
- **eBPF** — socket filter or XDP programs for kernel-level acceleration
- **Dashboard** — HTML/JS bundle for browser-based registry visualization

All survive close/reopen and can be extracted with `extract_*` methods.

### AGI Container (Step 21)

`AgiContainerBuilder` packages the entire agent OS into a self-describing manifest:
- Model pinning (`claude-opus-4-6`)
- Orchestrator config (Claude Code + Claude Flow)
- Tool registry, agent prompts, eval suite, grading rules
- Policy, skill library, project instructions
- Segment inventory (kernel, WASM, vectors, witnesses)
- Offline capability flag

`ParsedAgiManifest` provides zero-copy parsing and `is_autonomous_capable()` validation.

### Delete + Compact Lifecycle (Step 18)

1. `delete(&[id])` — soft-delete (tombstone), dead_ratio increases
2. `compact()` — rewrites store, reclaims dead space
3. Post-delete queries confirm the vector is gone

### COW Branching (Step 20)

1. `freeze()` — immutable baseline
2. `branch()` — COW child inheriting all parent vectors
3. Writes to child allocate local clusters only
4. `cow_stats()` reports cluster-level copy-on-write telemetry

### Lineage (Step 19)

`derive()` creates a child with:
- New `file_id`, parent's `file_id` as `parent_id`
- `lineage_depth` incremented (0 → 1)
- Provenance chain cryptographically verifiable

## About OpenFang

[OpenFang](https://openfang.sh) by RightNow AI is a Rust-based Agent Operating System — 137K lines of code across 14 crates, compiling to a single ~32 MB binary. It runs autonomous agents 24/7 with 16 security systems, 27 LLM providers, and 40 channel adapters.

- GitHub: [RightNow-AI/openfang](https://github.com/RightNow-AI/openfang)
- License: MIT / Apache 2.0

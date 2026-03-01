# ADR-057: Federated RVF Format for Real-Time Transfer Learning

**Status**: Proposed
**Date**: 2026-02-26
**Authors**: ruv.io, RuVector Architecture Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow
**Supersedes**: None
**Related**: ADR-029 (RVF Canonical Format), ADR-030 (Cognitive Containers), ADR-056 (Knowledge Export)

## Context

### The Federation Problem

RuVector users independently develop modules and crates, each accumulating valuable learning patterns: SONA weight trajectories, policy kernel configurations, domain expansion priors, HNSW tuning parameters, and convergence data. Today, this learning is siloed. User A discovers that a specific LoRA rank and EWC lambda combination works well for code review tasks, but User B must rediscover this independently.

The existing infrastructure already supports local federated learning within a single deployment:

1. **SONA `FederatedCoordinator`** (`crates/sona/src/training/federated.rs`) aggregates `AgentExport` from `EphemeralAgent` instances, replaying trajectories above a quality threshold into a master engine. Supports `Star`, `Hierarchical`, and `PeerToPeer` topologies.

2. **Domain Expansion Engine** (`crates/ruvector-domain-expansion/`) implements cross-domain transfer via `MetaThompsonEngine` with `TransferPrior` (compact Beta posteriors), `PolicyKernel` (population-based policy search), and `CostCurve` (acceleration scoreboard). The `rvf_bridge` module already serializes these into RVF segments `0x30`, `0x31`, `0x32`.

3. **RVF Format** (`crates/rvf/`) provides 25 segment types with 64-byte headers, SHAKE-256 hashing, Ed25519 signing, WITNESS_SEG audit trails, and forward-compatible unknown-segment passthrough. Segments `TransferPrior (0x30)`, `PolicyKernel (0x31)`, and `CostCurve (0x32)` already exist.

4. **Google Cloud example** (`examples/google-cloud/`) demonstrates Cloud Run deployment with axum HTTP server, GPU benchmarking, and self-learning models.

What is missing is the **inter-user federation layer**: the ability to strip PII, package transferable learning as RVF segments, publish them to a shared registry, and merge incoming learning with differential privacy guarantees.

### Why Now

- The RVF segment model is stable with 25 types and a clear allocation map
- The `rvf_bridge` proves that `TransferPrior`/`PolicyKernel`/`CostCurve` round-trip cleanly through RVF segments
- SONA's `FederatedCoordinator` demonstrates that trajectory aggregation with quality gating works
- The Google Cloud example provides the deployment foundation
- Users are building domain-specific crates and would benefit from shared learning

### Design Principles

1. **Optional**: Core RuVector works without federation. All new crates are feature-gated.
2. **Privacy-First**: PII stripping happens before any data leaves the local system. Differential privacy noise is injected at the export boundary.
3. **RVF-Native**: Learning is exchanged as RVF segments, not custom wire formats. Unknown segments pass through unchanged.
4. **Cryptographically Verifiable**: Every export carries a WITNESS_SEG chain and Ed25519/ML-DSA-65 signatures.
5. **Incremental**: Users can share only what they choose. No all-or-nothing.

## Decision

### 1. New Segment Types

Add four new segment types to the `0x33-0x36` range in `rvf-types`:

| Code | Name | Purpose |
|------|------|---------|
| `0x33` | `FederatedManifest` | Describes a federated learning export: contributor pseudonym, export timestamp, included segment IDs, privacy budget spent, format version |
| `0x34` | `DiffPrivacyProof` | Differential privacy attestation: epsilon/delta values, noise mechanism used, sensitivity bounds, clipping parameters |
| `0x35` | `RedactionLog` | PII stripping attestation: which fields were redacted, which rules fired, hash of pre-redaction content (for audit without revealing content) |
| `0x36` | `AggregateWeights` | Federated-averaged SONA weights: aggregated LoRA deltas, participation count, round number, convergence metrics |

The existing `TransferPrior (0x30)`, `PolicyKernel (0x31)`, `CostCurve (0x32)`, `Witness (0x0A)`, `Crypto (0x0C)`, and `Meta (0x07)` segments are reused as-is.

### 2. New Crates

Nine new crates (seven within `crates/rvf/`, two interface crates):

| Crate | Path | no_std | Purpose |
|-------|------|--------|---------|
| `rvf-federation` | `crates/rvf/rvf-federation` | no (std-only) | Core federation protocol: export builder, import merger, version-aware conflict resolution, selective sharing |
| `rvf-pii-strip` | `crates/rvf/rvf-pii-strip` | core: yes, full: no | PII detection and stripping pipeline: regex patterns, path normalization, credential detection, configurable rules, REDACTION_LOG segment generation |
| `rvf-diff-privacy` | `crates/rvf/rvf-diff-privacy` | core: yes, full: no | Differential privacy primitives: Gaussian/Laplace mechanisms, privacy accountant (RDP), gradient clipping, per-parameter noise calibration |
| `rvf-gcloud` | `crates/rvf/rvf-gcloud` | no (std-only) | Google Cloud integration: Pub/Sub publisher/subscriber, GCS object store, Firestore metadata registry, Cloud IAM auth |
| `rvf-fed-aggregate` | `crates/rvf/rvf-fed-aggregate` | no (std-only) | Federated aggregation server: FedAvg, FedProx, weighted averaging, Byzantine-tolerant aggregation, round management |
| `rvf-fed-wasm` | `crates/rvf/rvf-fed-wasm` | no (wasm32) | WASM-compatible export path: browser-side PII stripping and export packaging |
| `mcp-federation` | `crates/mcp-federation` | no (std-only) | MCP server for AI agent access: 6 tools + 4 resources over JSON-RPC 2.0 stdio |
| `rvf-fed-server` | `crates/rvf/rvf-fed-server` | no (std-only) | REST API server (axum): export/import/aggregate endpoints, SSE events, Prometheus metrics |
| `rvf-adapters/federation` | `crates/rvf/rvf-adapters/federation` | no (std-only) | Adapter connecting SONA's `FederatedCoordinator` and domain expansion's `MetaThompsonEngine` to the federation protocol |

### 3. PII Stripping Pipeline

The `rvf-pii-strip` crate implements a three-stage pipeline:

**Stage 1: Detection** -- Scan all string fields in RVF segment payloads for PII patterns:
- File paths (`/home/user/...`, `C:\Users\...`)
- IP addresses (IPv4, IPv6, loopback)
- Email addresses
- API keys (common patterns: `sk-...`, `AKIA...`, `ghp_...`, Bearer tokens)
- Usernames and hostnames
- Environment variable references (`$HOME`, `%USERPROFILE%`)
- Custom regex rules from configuration

**Stage 2: Redaction** -- Replace detected PII with deterministic pseudonyms:
- Paths become `<PATH_N>` where N is a per-export incrementing counter
- IPs become `<IP_N>`
- Keys become `<REDACTED_KEY>`
- Usernames become `<USER_N>`
- Preserves structural relationships (same path always maps to same pseudonym within one export)

**Stage 3: Attestation** -- Generate a `RedactionLog (0x35)` segment containing:
- Count of each redaction type
- SHAKE-256 hash of the pre-redaction content (proves content was scanned without revealing it)
- Rules that fired
- Timestamp

### 4. Differential Privacy

The `rvf-diff-privacy` crate provides mathematical privacy guarantees:

- **Gradient Clipping**: Before aggregation, clip per-user gradient norms to bound sensitivity
- **Noise Injection**: Add calibrated Gaussian noise (for (epsilon, delta)-DP) to aggregated weights
- **Privacy Accountant**: Track cumulative privacy loss using Renyi Differential Privacy (RDP) composition
- **Per-Export Budget**: Each federated export consumes a portion of the user's privacy budget. The `DiffPrivacyProof (0x34)` segment records the spent budget.
- **Configurable Epsilon**: Users set their comfort level. Default: epsilon=1.0, delta=1e-5 (strong privacy)

### 5. Google Cloud Architecture

The `rvf-gcloud` crate integrates with Google Cloud Platform:

**Pub/Sub**: Real-time learning event propagation
- Topic: `ruvector-federation-events`
- Messages: serialized `FederatedManifest` headers (small, <1KB)
- Subscribers filter by domain, version, and contributor reputation

**Cloud Storage (GCS)**: RVF file exchange
- Bucket: `ruvector-federation-{region}`
- Object naming: `{domain}/{version}/{contributor_pseudonym}/{timestamp}.rvf`
- Lifecycle: auto-archive after 90 days, delete after 365 days
- Server-side encryption with CMEK

**Firestore**: Metadata registry
- Collection: `federation_manifests`
- Documents: manifest metadata, contributor reputation scores, merge history
- Real-time listeners for new contribution notifications

**Cloud Run**: Aggregation service
- Extends the existing `examples/google-cloud/` server
- New endpoints: `POST /federation/submit`, `GET /federation/pull`, `POST /federation/aggregate`
- Rate limiting: 100 submissions/hour per contributor
- IAM-based access control

### 6. Transfer Learning Protocol

**Export Flow**:
1. User triggers export (CLI: `rvf federation export --domain <id> --epsilon 1.0`)
2. `rvf-adapters/federation` extracts `TransferPrior`, `PolicyKernel`, `CostCurve`, and SONA weights from local engines
3. `rvf-pii-strip` scans and redacts all payloads, generating `RedactionLog` segment
4. `rvf-diff-privacy` adds calibrated noise to numerical parameters, generating `DiffPrivacyProof` segment
5. `rvf-federation` assembles the export: `FederatedManifest` + learning segments + `RedactionLog` + `DiffPrivacyProof` + `Witness` chain + `Crypto` signature
6. `rvf-gcloud` uploads to GCS and publishes notification to Pub/Sub

**Import Flow**:
1. User subscribes to federation updates (CLI: `rvf federation subscribe --domains <ids>`)
2. `rvf-gcloud` receives Pub/Sub notification, downloads RVF file from GCS
3. `rvf-federation` validates: signature check, witness chain verification, privacy proof verification, version compatibility check
4. `rvf-federation` merges: version-aware prior dampening (same sqrt-scaling as `MetaThompsonEngine::init_domain_with_transfer`), conflict resolution for competing patterns
5. `rvf-adapters/federation` imports merged learning into local SONA and domain expansion engines

**Federated Averaging**:
1. Aggregation server collects N exports for a given domain/version
2. `rvf-fed-aggregate` computes weighted average (weight = contributor reputation * trajectory count * quality score)
3. Byzantine tolerance: exclude outliers beyond 2 standard deviations from the mean
4. Generate aggregate `AggregateWeights (0x36)` segment
5. Publish aggregate back to GCS for all subscribers

### 7. Version-Aware Merging

Learning from different RVF versions must be handled:
- **Same version**: Direct merge using federated averaging
- **Newer to older**: Newer learning carries a version tag; older clients skip segments they cannot parse (RVF forward compatibility)
- **Older to newer**: Accepted with dampened confidence (lower weight in averaging)
- **Conflict resolution**: When two priors disagree on a bucket/arm, merge using `BetaParams::merge()` (sum parameters minus uniform prior)

### 8. MCP Server Interface

A dedicated `mcp-federation` crate provides AI agent access to federation through MCP (JSON-RPC 2.0 over stdio), following the same pattern as the existing `mcp-gate` crate:

| Tool | Purpose |
|------|---------|
| `federation_export` | Extract learning, strip PII, apply DP noise, sign, and upload |
| `federation_import` | Pull, validate, and merge federated learning into local engines |
| `federation_status` | Read privacy budget, recent activity, contributor reputation |
| `federation_search` | Query the registry for available learning by domain/quality |
| `federation_budget` | Check remaining privacy budget and export history |
| `federation_aggregate` | Trigger server-side aggregation round |

Resources (read-only): `federation://domains`, `federation://contributors`, `federation://rounds/{id}`, `federation://budget`

Registration: `claude mcp add mcp-federation -- cargo run -p mcp-federation`

### 9. REST API Interface

The `rvf-fed-server` crate provides a REST API (axum-based, deployed on Cloud Run) for programmatic access:

- **Export/Import**: `POST /v1/exports`, `GET /v1/exports/{id}`, `DELETE /v1/exports/{id}`
- **Aggregation**: `POST /v1/aggregates`, `GET /v1/aggregates/{round_id}`, `GET /v1/aggregates/latest`
- **Registry**: `GET /v1/domains`, `GET /v1/contributors/{pseudonym}`, `GET /v1/contributors/{pseudonym}/budget`
- **Events**: `GET /v1/events?domain=X` (Server-Sent Events for real-time notifications)
- **Health**: `GET /v1/health`, `GET /v1/metrics` (Prometheus)

Authentication: API key (Bearer token) or Ed25519 signed requests. Rate-limited per contributor.

SDKs: Rust (`rvf_federation::client::FederationClient`) and TypeScript (`@ruvector/rvf-federation`).

### 10. Selective Sharing

Users control what they share via a `FederationPolicy`:
- **Allowlist/Denylist**: Specific segment types or domains to include/exclude
- **Quality Gate**: Only export learning from trajectories above a quality threshold (reuses SONA's `quality_threshold`)
- **Minimum Evidence**: Only export priors with sufficient observations (reuses `TransferPrior::extract_summary()`'s >12 observation filter)
- **Rate Limit**: Maximum exports per time period
- **Privacy Budget**: Cumulative epsilon limit before exports are blocked

## Consequences

### Benefits

1. **Knowledge acceleration**: New users bootstrap from community learning instead of starting cold
2. **Privacy-preserving**: PII stripping + differential privacy ensure no sensitive data leaks
3. **RVF-native**: No new wire formats; everything is standard RVF segments
4. **Cryptographically auditable**: Witness chains prove provenance without revealing content
5. **Incremental adoption**: Feature-gated, optional, selective sharing
6. **Cloud-native**: Google Cloud Pub/Sub + GCS + Firestore provide scalable infrastructure
7. **WASM-compatible**: Browser-based exports via `rvf-fed-wasm`
8. **MCP-integrated**: AI agents access federation through standard MCP tools (JSON-RPC 2.0)
9. **API-first**: REST API with SSE events for programmatic access, Rust and TypeScript SDKs

### Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Poisoning attacks (malicious learning) | High | Byzantine-tolerant aggregation, reputation system, signature verification |
| Privacy budget exhaustion | Medium | Configurable epsilon, budget tracking per-export, admin alerts at 80% budget |
| Version skew causing merge failures | Medium | RVF forward compatibility, version-tagged manifests, graceful skip of unknown segments |
| GCS cost escalation | Low | Lifecycle policies, per-contributor quotas, compression (ZSTD segment compression) |
| Latency of federated averaging | Low | Async aggregation, Pub/Sub decoupling, local-first operation |
| Regulatory compliance (GDPR, CCPA) | High | PII stripping attestation, data retention policies, right-to-deletion via contributor pseudonym revocation |

### Segment Allocation Map (Updated)

```
0x00       Invalid
0x01-0x0F  Core segments (Vec, Index, Overlay, Journal, Manifest, Quant, Meta, Hot, Sketch, Witness, Profile, Crypto, MetaIdx, Kernel, Ebpf)
0x10-0x11  Extension segments (Wasm, Dashboard)
0x12-0x1F  RESERVED (12 slots available)
0x20-0x23  Storage segments (CowMap, Refcount, Membership, Delta)
0x24-0x2F  RESERVED (12 slots available)
0x30-0x32  Domain expansion (TransferPrior, PolicyKernel, CostCurve)
0x33-0x36  Federation (FederatedManifest, DiffPrivacyProof, RedactionLog, AggregateWeights)  <-- NEW
0x37-0xEF  RESERVED (future use)
0xF0-0xFF  RESERVED (system)
```

## Compliance

- **GDPR Article 25**: Privacy by design -- PII stripping is mandatory before export, not optional
- **GDPR Article 17**: Right to erasure -- contributor pseudonym revocation removes all associated exports from GCS
- **CCPA Section 1798.105**: Deletion requests honored via pseudonym revocation
- **NIST SP 800-188**: De-identification via differential privacy with formal epsilon guarantees

## References

- McMahan et al., "Communication-Efficient Learning of Deep Networks from Decentralized Data" (FedAvg)
- Abadi et al., "Deep Learning with Differential Privacy" (DP-SGD)
- Mironov, "Renyi Differential Privacy" (RDP composition)
- Blanchard et al., "Machine Learning with Adversaries: Byzantine Tolerant Gradient Descent" (Byzantine tolerance)
- RVF Format Specification (ADR-029)
- SONA Architecture (crates/sona)
- Domain Expansion Engine (crates/ruvector-domain-expansion)

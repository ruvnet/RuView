# Federated RVF Transfer Learning -- GOAP Implementation Plan

**ADR**: ADR-057
**Date**: 2026-02-26
**Methodology**: Goal-Oriented Action Planning (GOAP)

---

## 1. World State Assessment

### 1.1 Current State (What Is True Now)

| State Variable | Value | Evidence |
|---|---|---|
| `rvf_segment_types_defined` | 25 types (0x00-0x32) | `crates/rvf/rvf-types/src/segment_type.rs` |
| `transfer_prior_segment_exists` | true (0x30) | `SegmentType::TransferPrior` |
| `policy_kernel_segment_exists` | true (0x31) | `SegmentType::PolicyKernel` |
| `cost_curve_segment_exists` | true (0x32) | `SegmentType::CostCurve` |
| `rvf_bridge_serialization_works` | true | `ruvector-domain-expansion/src/rvf_bridge.rs` -- 11 passing tests |
| `witness_chain_support` | true | `rvf-crypto/src/witness.rs` -- `create_witness_chain`, `verify_witness_chain` |
| `ed25519_signing_support` | true | `rvf-crypto/src/sign.rs` -- feature-gated `ed25519` |
| `shake256_hashing_support` | true | `rvf-crypto/src/hash.rs` -- `shake256_128`, `shake256_256` |
| `sona_federated_coordinator_exists` | true | `crates/sona/src/training/federated.rs` -- `FederatedCoordinator`, `EphemeralAgent` |
| `sona_agent_export_works` | true | `AgentExport`, `TrajectoryExport` with quality gating |
| `sona_lora_weights_accessible` | true | `SonaEngine::apply_micro_lora`, `MicroLoRA`, `BaseLoRA` |
| `sona_ewc_support` | true | `crates/sona/src/ewc.rs` -- `EwcPlusPlus`, `TaskFisher` |
| `domain_expansion_engine_exists` | true | `crates/ruvector-domain-expansion/` -- 3 domains, Meta-TS, population search |
| `domain_expansion_transfer_works` | true | `MetaThompsonEngine::init_domain_with_transfer` with sqrt dampening |
| `beta_params_merge_exists` | true | `BetaParams::merge()` in `transfer.rs` |
| `gcloud_example_exists` | true | `examples/google-cloud/` -- Cloud Run, axum server |
| `rvf_workspace_defined` | true | `crates/rvf/Cargo.toml` -- 25 workspace members |
| `no_std_types_core` | true | `rvf-types` is `no_std` by default |
| `pii_stripping_exists` | false | No PII detection or redaction crate |
| `differential_privacy_exists` | false | No DP primitives in codebase |
| `federation_protocol_exists` | false | No inter-user export/import protocol |
| `gcloud_pubsub_integration` | false | No Pub/Sub client code |
| `gcloud_gcs_integration` | false | No GCS object store client code |
| `gcloud_firestore_integration` | false | No Firestore client code |
| `federated_manifest_segment` | false | No 0x33 segment type |
| `diff_privacy_proof_segment` | false | No 0x34 segment type |
| `redaction_log_segment` | false | No 0x35 segment type |
| `aggregate_weights_segment` | false | No 0x36 segment type |
| `wasm_export_path` | false | No browser-side federation |
| `aggregation_server` | false | No multi-user aggregation service |
| `mcp_federation_server` | false | No MCP server for AI agent access |
| `rest_api_server` | false | No REST API server for programmatic access |

### 1.2 Goal State (What Should Be True)

| State Variable | Required Value |
|---|---|
| `federated_manifest_segment` | true -- 0x33 defined and wire-coded |
| `diff_privacy_proof_segment` | true -- 0x34 defined and wire-coded |
| `redaction_log_segment` | true -- 0x35 defined and wire-coded |
| `aggregate_weights_segment` | true -- 0x36 defined and wire-coded |
| `pii_stripping_exists` | true -- `rvf-pii-strip` crate with detection, redaction, attestation |
| `differential_privacy_exists` | true -- `rvf-diff-privacy` crate with Gaussian mechanism, RDP accountant |
| `federation_protocol_exists` | true -- `rvf-federation` crate with export builder, import validator, merger |
| `gcloud_pubsub_integration` | true -- `rvf-gcloud` with Pub/Sub publish/subscribe |
| `gcloud_gcs_integration` | true -- `rvf-gcloud` with GCS upload/download |
| `gcloud_firestore_integration` | true -- `rvf-gcloud` with Firestore registry |
| `aggregation_server` | true -- `rvf-fed-aggregate` with FedAvg, Byzantine tolerance |
| `wasm_export_path` | true -- `rvf-fed-wasm` with browser PII strip + export |
| `federation_adapter` | true -- `rvf-adapters/federation` connecting SONA + domain expansion |
| `mcp_federation_server` | true -- `mcp-federation` crate with 6 tools + 4 resources over JSON-RPC 2.0 |
| `rest_api_server` | true -- `rvf-fed-server` crate with REST API, SSE events, Prometheus metrics |
| `all_tests_pass` | true |
| `feature_gated` | true -- all federation is behind `federation` feature flag |

---

## 2. Action Inventory

Each action has: preconditions, effects, estimated cost (story points, 1-13), and dependencies.

### Phase 0: Foundation -- Segment Types and Core Types

#### Action 0.1: Add Federation Segment Types to rvf-types

- **Preconditions**: `rvf_segment_types_defined == true`
- **Effects**: `federated_manifest_segment = true`, `diff_privacy_proof_segment = true`, `redaction_log_segment = true`, `aggregate_weights_segment = true`
- **Cost**: 3 SP
- **Dependencies**: None
- **Files Modified**:
  - `crates/rvf/rvf-types/src/segment_type.rs` -- Add `FederatedManifest = 0x33`, `DiffPrivacyProof = 0x34`, `RedactionLog = 0x35`, `AggregateWeights = 0x36`
  - `crates/rvf/rvf-types/src/federation.rs` -- New module with header structs (`FederatedManifestHeader`, `DiffPrivacyProofHeader`, `RedactionLogHeader`, `AggregateWeightsHeader`)
  - `crates/rvf/rvf-types/src/lib.rs` -- Add `pub mod federation;` (feature-gated behind `federation`)
  - Tests: round-trip for all 4 new segment types, discriminant values

#### Action 0.2: Add Federation Segment Wire Codecs to rvf-wire

- **Preconditions**: `federated_manifest_segment == true`
- **Effects**: `federation_wire_codecs = true`
- **Cost**: 5 SP
- **Dependencies**: [0.1]
- **Files Modified**:
  - `crates/rvf/rvf-wire/src/federation_codec.rs` -- New module: `encode_federated_manifest`, `decode_federated_manifest`, and equivalents for 0x34-0x36
  - `crates/rvf/rvf-wire/src/lib.rs` -- Add `pub mod federation_codec;` (feature-gated)
  - Tests: encode-decode round-trip for each new segment type, fuzz edge cases (truncated payloads, wrong magic)

### Phase 1: PII Stripping

#### Action 1.1: Create rvf-pii-strip Crate

- **Preconditions**: `rvf_workspace_defined == true`
- **Effects**: `pii_detection_exists = true`
- **Cost**: 8 SP
- **Dependencies**: [0.1]
- **New Files**:
  - `crates/rvf/rvf-pii-strip/Cargo.toml` -- deps: `rvf-types`, `regex` (std feature), `serde`
  - `crates/rvf/rvf-pii-strip/src/lib.rs` -- Module structure
  - `crates/rvf/rvf-pii-strip/src/detect.rs` -- `PiiDetector` with regex patterns for paths, IPs, emails, API keys, usernames, env refs
  - `crates/rvf/rvf-pii-strip/src/redact.rs` -- `PiiRedactor` with pseudonymization (deterministic per-export)
  - `crates/rvf/rvf-pii-strip/src/attest.rs` -- `RedactionAttestor` generating `RedactionLog` segment payload
  - `crates/rvf/rvf-pii-strip/src/rules.rs` -- `RedactionRule` config, `RuleSet` with default + custom rules
  - `crates/rvf/rvf-pii-strip/src/pipeline.rs` -- `StripPipeline::new(rules).detect(payload).redact().attest()` fluent API
  - Tests: detection accuracy for each PII type, pseudonym determinism, attest hash correctness, empty input, binary content (should pass through)

#### Action 1.2: Create rvf-pii-strip no_std Core

- **Preconditions**: `pii_detection_exists == true`
- **Effects**: `pii_strip_nostd_core = true`
- **Cost**: 3 SP
- **Dependencies**: [1.1]
- **Details**: Extract regex-free pattern matching into `no_std` core that works in WASM. Uses simple byte-scanning for path separators, IP octets, `sk-` prefixes. Full regex detection remains in `std` feature.

### Phase 2: Differential Privacy

#### Action 2.1: Create rvf-diff-privacy Crate

- **Preconditions**: `rvf_workspace_defined == true`, `diff_privacy_proof_segment == true`
- **Effects**: `differential_privacy_exists = true`
- **Cost**: 8 SP
- **Dependencies**: [0.1]
- **New Files**:
  - `crates/rvf/rvf-diff-privacy/Cargo.toml` -- deps: `rvf-types`, `rand`, `serde`
  - `crates/rvf/rvf-diff-privacy/src/lib.rs` -- Module structure
  - `crates/rvf/rvf-diff-privacy/src/mechanism.rs` -- `GaussianMechanism`, `LaplaceMechanism`, `ExponentialMechanism` with calibrated noise
  - `crates/rvf/rvf-diff-privacy/src/clipping.rs` -- `GradientClipper` with L2 norm clipping, per-parameter and global
  - `crates/rvf/rvf-diff-privacy/src/accountant.rs` -- `PrivacyAccountant` using Renyi Differential Privacy (RDP) composition
  - `crates/rvf/rvf-diff-privacy/src/budget.rs` -- `PrivacyBudget` tracking cumulative epsilon/delta spend per contributor
  - `crates/rvf/rvf-diff-privacy/src/proof.rs` -- `DiffPrivacyProofBuilder` generating 0x34 segment payload
  - `crates/rvf/rvf-diff-privacy/src/config.rs` -- `DiffPrivacyConfig { epsilon, delta, clipping_norm, noise_multiplier, mechanism }`
  - Tests: noise calibration matches theoretical bounds, RDP composition is monotonically increasing, budget tracking, proof generation

#### Action 2.2: Create rvf-diff-privacy no_std Core

- **Preconditions**: `differential_privacy_exists == true`
- **Effects**: `diff_privacy_nostd_core = true`
- **Cost**: 3 SP
- **Dependencies**: [2.1]
- **Details**: Core noise generation and clipping in `no_std` (uses `rand` which supports `no_std`). RDP accountant requires `f64` math but can be `no_std` with `libm`.

### Phase 3: Federation Protocol

#### Action 3.1: Create rvf-federation Crate

- **Preconditions**: `federation_wire_codecs == true`, `pii_detection_exists == true`, `differential_privacy_exists == true`
- **Effects**: `federation_protocol_exists = true`
- **Cost**: 13 SP
- **Dependencies**: [0.2, 1.1, 2.1]
- **New Files**:
  - `crates/rvf/rvf-federation/Cargo.toml` -- deps: `rvf-types`, `rvf-wire`, `rvf-crypto`, `rvf-pii-strip`, `rvf-diff-privacy`, `serde`, `serde_json`
  - `crates/rvf/rvf-federation/src/lib.rs` -- Module structure
  - `crates/rvf/rvf-federation/src/export.rs` -- `ExportBuilder`:
    - `.add_transfer_prior(prior)` -- adds 0x30 segment
    - `.add_policy_kernel(kernel)` -- adds 0x31 segment
    - `.add_cost_curve(curve)` -- adds 0x32 segment
    - `.add_sona_weights(weights)` -- adds 0x36 segment
    - `.set_contributor(pseudonym)` -- sets contributor ID
    - `.set_privacy_config(config)` -- sets epsilon/delta
    - `.set_pii_rules(rules)` -- sets redaction rules
    - `.build()` -- runs PII strip pipeline, noise injection, generates manifest + redaction log + proof + witness + signature, returns `Vec<u8>`
  - `crates/rvf/rvf-federation/src/import.rs` -- `ImportValidator`:
    - `.validate(data: &[u8])` -- parses segments, verifies signature, witness chain, privacy proof, redaction log
    - `.extract_priors()`, `.extract_kernels()`, `.extract_curves()`, `.extract_weights()`
    - Returns `ValidatedImport` with all segments + metadata
  - `crates/rvf/rvf-federation/src/merge.rs` -- `VersionMerger`:
    - `.merge_transfer_prior(local, remote, weight)` -- version-aware Beta parameter merging with dampening
    - `.merge_policy_kernel(local_population, remote_kernel)` -- inject remote kernel into population
    - `.merge_sona_weights(local, remote, ewc_fisher)` -- weighted average with EWC regularization
    - `.merge_cost_curve(local_scoreboard, remote_curve)` -- add as reference curve
  - `crates/rvf/rvf-federation/src/policy.rs` -- `FederationPolicy`:
    - Allowlist/denylist for segment types
    - Quality gate threshold
    - Minimum evidence threshold
    - Rate limit configuration
    - Privacy budget limit
  - `crates/rvf/rvf-federation/src/manifest.rs` -- `FederatedManifestBuilder` for 0x33 segment
  - `crates/rvf/rvf-federation/src/version.rs` -- Version compatibility checking and negotiation
  - Tests: full export/import round-trip, merge correctness, policy enforcement, version compatibility, signature verification, error cases

#### Action 3.2: Create rvf-adapters/federation

- **Preconditions**: `federation_protocol_exists == true`, `sona_federated_coordinator_exists == true`, `domain_expansion_engine_exists == true`
- **Effects**: `federation_adapter = true`
- **Cost**: 8 SP
- **Dependencies**: [3.1]
- **New Files**:
  - `crates/rvf/rvf-adapters/federation/Cargo.toml` -- deps: `rvf-federation`, `sona`, `ruvector-domain-expansion`, `rvf-adapter-sona`
  - `crates/rvf/rvf-adapters/federation/src/lib.rs` -- Module structure
  - `crates/rvf/rvf-adapters/federation/src/export_coordinator.rs` -- `FederationExportCoordinator`:
    - Takes `&SonaEngine` and `&DomainExpansionEngine`
    - Extracts `TransferPrior` from `MetaThompsonEngine`
    - Extracts best `PolicyKernel` from `PopulationSearch`
    - Extracts `CostCurve` from `AccelerationScoreboard`
    - Extracts SONA LoRA weights for `AggregateWeights`
    - Applies quality gate and minimum evidence filter
    - Passes to `rvf-federation::ExportBuilder`
  - `crates/rvf/rvf-adapters/federation/src/import_coordinator.rs` -- `FederationImportCoordinator`:
    - Takes `&mut SonaEngine` and `&mut DomainExpansionEngine`
    - Uses `rvf-federation::ImportValidator` to validate
    - Uses `rvf-federation::VersionMerger` to merge
    - Updates local `MetaThompsonEngine` with merged priors
    - Injects kernels into `PopulationSearch`
    - Merges SONA weights with EWC protection
  - Tests: end-to-end export from real engines, import into fresh engines, verify acceleration after import

### Phase 4: Google Cloud Integration

#### Action 4.1: Create rvf-gcloud Crate

- **Preconditions**: `federation_protocol_exists == true`
- **Effects**: `gcloud_pubsub_integration = true`, `gcloud_gcs_integration = true`, `gcloud_firestore_integration = true`
- **Cost**: 13 SP
- **Dependencies**: [3.1]
- **New Files**:
  - `crates/rvf/rvf-gcloud/Cargo.toml` -- deps: `google-cloud-pubsub`, `google-cloud-storage`, `google-cloud-firestore` (or `gcloud-sdk`), `tokio`, `serde`, `rvf-federation`
  - `crates/rvf/rvf-gcloud/src/lib.rs` -- Module structure
  - `crates/rvf/rvf-gcloud/src/pubsub.rs` -- `FederationPubSub`:
    - `publish_export_notification(manifest)` -- publish FederatedManifest header to topic
    - `subscribe_federation_events(filter)` -- subscribe with domain/version filter
    - `acknowledge(message_id)` -- ack after successful import
    - Topic/subscription management
  - `crates/rvf/rvf-gcloud/src/gcs.rs` -- `FederationStorage`:
    - `upload_export(domain, contributor, data)` -- upload RVF to GCS with proper naming
    - `download_export(path)` -- download RVF from GCS
    - `list_exports(domain, since)` -- list available exports
    - `delete_by_contributor(pseudonym)` -- right-to-deletion support
    - Lifecycle policy configuration
  - `crates/rvf/rvf-gcloud/src/firestore.rs` -- `FederationRegistry`:
    - `register_manifest(manifest)` -- store manifest metadata
    - `get_contributor_reputation(pseudonym)` -- read reputation score
    - `update_reputation(pseudonym, delta)` -- update reputation
    - `get_privacy_budget(pseudonym)` -- read remaining budget
    - `record_budget_spend(pseudonym, epsilon)` -- deduct from budget
    - `list_manifests(domain, limit)` -- query manifest history
  - `crates/rvf/rvf-gcloud/src/auth.rs` -- IAM authentication and service account management
  - `crates/rvf/rvf-gcloud/src/config.rs` -- `GCloudConfig { project_id, region, bucket, topic, collection }`
  - Tests: mock-based tests for all GCloud operations (no real GCloud calls in unit tests), integration test behind `gcloud-integration` feature flag

#### Action 4.2: Extend Google Cloud Example

- **Preconditions**: `gcloud_pubsub_integration == true`
- **Effects**: `gcloud_example_updated = true`
- **Cost**: 5 SP
- **Dependencies**: [4.1, 3.2]
- **Files Modified**:
  - `examples/google-cloud/src/server.rs` -- Add federation endpoints: `POST /federation/submit`, `GET /federation/pull`, `POST /federation/aggregate`, `GET /federation/status`
  - `examples/google-cloud/src/federation.rs` -- New module: handler implementations using `rvf-gcloud` and `rvf-federation`
  - `examples/google-cloud/Cargo.toml` -- Add `rvf-federation`, `rvf-gcloud`, `rvf-adapters/federation` deps
  - `examples/google-cloud/cloudrun.yaml` -- Add environment variables for federation config

### Phase 5: Federated Aggregation

#### Action 5.1: Create rvf-fed-aggregate Crate

- **Preconditions**: `federation_protocol_exists == true`, `differential_privacy_exists == true`
- **Effects**: `aggregation_server = true`
- **Cost**: 8 SP
- **Dependencies**: [3.1, 2.1]
- **New Files**:
  - `crates/rvf/rvf-fed-aggregate/Cargo.toml` -- deps: `rvf-federation`, `rvf-diff-privacy`, `rvf-types`, `serde`, `tokio`
  - `crates/rvf/rvf-fed-aggregate/src/lib.rs` -- Module structure
  - `crates/rvf/rvf-fed-aggregate/src/round.rs` -- `AggregationRound`:
    - Round lifecycle: `Open -> Collecting -> Aggregating -> Published`
    - `submit(validated_import)` -- add contributor
    - `is_ready()` -- check if min_participants reached or timeout
    - `aggregate()` -- trigger aggregation
  - `crates/rvf/rvf-fed-aggregate/src/fedavg.rs` -- `FedAvgAggregator`:
    - Weighted average of `TransferPrior` Beta parameters
    - Weighted average of `PolicyKnobs` numeric fields
    - Weighted average of SONA LoRA deltas
    - Weight = contributor_reputation * trajectory_count * quality_score
  - `crates/rvf/rvf-fed-aggregate/src/fedprox.rs` -- `FedProxAggregator`:
    - FedAvg + proximal term `mu/2 * ||w_k - w_global||^2`
    - For heterogeneous contributor distributions
  - `crates/rvf/rvf-fed-aggregate/src/byzantine.rs` -- `ByzantineFilter`:
    - IQR-based outlier detection on parameter vectors
    - Krum aggregation: select contributor closest to peers
    - Configurable tolerance threshold
  - `crates/rvf/rvf-fed-aggregate/src/reputation.rs` -- `ReputationManager`:
    - Score = f(avg_quality, trajectory_count, age, acceptance_rate)
    - Decay over time
    - Penalty for rejected submissions
  - Tests: FedAvg correctness (average of known inputs), Byzantine tolerance (inject outlier, verify exclusion), reputation scoring, round lifecycle

### Phase 5B: MCP and API Interfaces

#### Action 5B.1: Create mcp-federation Crate (MCP Server)

- **Preconditions**: `federation_protocol_exists == true`, `gcloud_pubsub_integration == true`
- **Effects**: `mcp_federation_server = true`
- **Cost**: 8 SP
- **Dependencies**: [3.1, 4.1]
- **New Files**:
  - `crates/mcp-federation/Cargo.toml` -- deps: `rvf-federation`, `rvf-gcloud`, `rvf-pii-strip`, `rvf-diff-privacy`, `rvf-adapters/federation`, `serde`, `serde_json`, `tokio`
  - `crates/mcp-federation/src/lib.rs` -- Module structure, `McpFederationServer`
  - `crates/mcp-federation/src/server.rs` -- JSON-RPC 2.0 stdio transport (same pattern as `mcp-gate/src/server.rs`):
    - `McpFederationServer::new(config)` -- initialize with federation config
    - `McpFederationServer::run()` -- main event loop: read stdin, dispatch, write stdout
    - Handles `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`
  - `crates/mcp-federation/src/tools.rs` -- `McpFederationTools`:
    - `federation_export` -- extracts, strips PII, applies noise, signs, uploads
    - `federation_import` -- pulls, validates, merges into local engines
    - `federation_status` -- reads budget, recent activity, reputation
    - `federation_search` -- queries Firestore manifest registry
    - `federation_budget` -- reads privacy budget details
    - `federation_aggregate` -- triggers server-side aggregation round
  - `crates/mcp-federation/src/resources.rs` -- `McpFederationResources`:
    - `federation://domains` -- list of federated domains with stats
    - `federation://contributors` -- pseudonymized contributor list + reputation
    - `federation://rounds/{round_id}` -- aggregation round details
    - `federation://budget` -- privacy budget for current contributor
  - `crates/mcp-federation/src/schemas.rs` -- JSON Schema definitions for all tool inputs/outputs
  - Tests: tool dispatch, resource resolution, schema validation, error handling

#### Action 5B.2: Create rvf-fed-server Crate (REST API)

- **Preconditions**: `federation_protocol_exists == true`, `gcloud_pubsub_integration == true`, `aggregation_server == true`
- **Effects**: `rest_api_server = true`
- **Cost**: 8 SP
- **Dependencies**: [3.1, 4.1, 5.1]
- **New Files**:
  - `crates/rvf/rvf-fed-server/Cargo.toml` -- deps: `rvf-federation`, `rvf-gcloud`, `rvf-fed-aggregate`, `axum`, `tower`, `tower-http`, `tokio`, `serde`, `serde_json`, `tracing`, `metrics`, `metrics-exporter-prometheus`
  - `crates/rvf/rvf-fed-server/src/lib.rs` -- Module structure, `FederationServer`
  - `crates/rvf/rvf-fed-server/src/routes.rs` -- axum Router:
    - `POST /v1/exports` -- accept RVF bytes, validate, store in GCS, publish event
    - `GET /v1/exports/{id}` -- download RVF export by ID
    - `GET /v1/exports?domain=&since=&limit=` -- list exports
    - `DELETE /v1/exports/{id}` -- contributor deletes own export
    - `POST /v1/aggregates` -- trigger aggregation round
    - `GET /v1/aggregates/{round_id}` -- round status
    - `GET /v1/aggregates/latest?domain=` -- latest aggregate RVF
    - `GET /v1/domains` -- list federated domains
    - `GET /v1/contributors/{pseudonym}` -- contributor profile
    - `GET /v1/contributors/{pseudonym}/budget` -- privacy budget
    - `GET /v1/health` -- health check
    - `GET /v1/metrics` -- Prometheus metrics
    - `GET /v1/events?domain=` -- SSE stream
  - `crates/rvf/rvf-fed-server/src/auth.rs` -- Authentication middleware:
    - Bearer token validation (SHAKE-256 hash lookup in Firestore)
    - Ed25519 signature verification (X-Federation-Signature, X-Federation-PublicKey)
  - `crates/rvf/rvf-fed-server/src/rate_limit.rs` -- Tower rate limiting middleware:
    - Per-contributor, per-endpoint configurable limits
    - Token bucket algorithm
  - `crates/rvf/rvf-fed-server/src/sse.rs` -- Server-Sent Events:
    - `new_export`, `aggregation_complete`, `import_available` event types
    - Domain-filtered subscriptions
  - `crates/rvf/rvf-fed-server/src/metrics.rs` -- Prometheus metrics registration and export
  - Tests: route handler tests with mock backends, auth middleware, rate limiting, SSE stream

### Phase 6: WASM Export Path

#### Action 6.1: Create rvf-fed-wasm Crate

- **Preconditions**: `pii_strip_nostd_core == true`, `diff_privacy_nostd_core == true`, `federation_protocol_exists == true`
- **Effects**: `wasm_export_path = true`
- **Cost**: 5 SP
- **Dependencies**: [1.2, 2.2, 3.1]
- **New Files**:
  - `crates/rvf/rvf-fed-wasm/Cargo.toml` -- deps: `rvf-types`, `rvf-wire`, `rvf-crypto`, `rvf-pii-strip` (no_std), `rvf-diff-privacy` (no_std), `wasm-bindgen`, `js-sys`
  - `crates/rvf/rvf-fed-wasm/src/lib.rs` -- `wasm-bindgen` exports:
    - `FederationExporter::new(config)` -- create exporter with epsilon/delta/rules
    - `FederationExporter::add_transfer_prior(bytes)` -- add prior segment
    - `FederationExporter::add_policy_kernel(bytes)` -- add kernel segment
    - `FederationExporter::add_cost_curve(bytes)` -- add curve segment
    - `FederationExporter::build()` -- strip PII, add noise, sign, return `Uint8Array`
  - `crates/rvf/rvf-fed-wasm/src/js_types.rs` -- JavaScript-friendly type wrappers
  - npm package config for `@ruvector/rvf-fed-wasm`
  - Tests: build with `wasm-pack test --headless --chrome`

### Phase 7: Integration and Testing

#### Action 7.1: Integration Tests

- **Preconditions**: All previous actions complete
- **Effects**: `all_tests_pass = true`
- **Cost**: 8 SP
- **Dependencies**: [All above]
- **New Files**:
  - `crates/rvf/tests/rvf-integration/src/federation.rs` -- Integration tests:
    - Full export/import round-trip with real SONA and DomainExpansion engines
    - PII stripping verification (inject known PII, verify redaction)
    - Differential privacy verification (noise bounds check)
    - Version compatibility matrix (v1 export, v1 import; future v2 considerations)
    - Byzantine tolerance verification (inject poisoned export, verify exclusion)
    - Privacy budget exhaustion (export until budget depleted, verify rejection)
    - Signature verification (tamper with segment, verify rejection)
    - Witness chain verification (reorder segments, verify rejection)
    - Federated averaging correctness (known inputs, verify output)
    - End-to-end acceleration test (import learning, verify faster convergence)

#### Action 7.2: Update Workspace Configuration

- **Preconditions**: All new crates created
- **Effects**: `feature_gated = true`
- **Cost**: 2 SP
- **Dependencies**: [All new crate creations]
- **Files Modified**:
  - `crates/rvf/Cargo.toml` -- Add new members to workspace, add workspace dependencies
  - Each existing crate that gains federation feature gates

#### Action 7.3: CLI Extension

- **Preconditions**: `federation_adapter == true`, `gcloud_pubsub_integration == true`
- **Effects**: `cli_federation_commands = true`
- **Cost**: 5 SP
- **Dependencies**: [3.2, 4.1]
- **Files Modified**:
  - `crates/rvf/rvf-cli/` -- Add subcommands:
    - `rvf federation export --domain <id> --epsilon <val> --output <path>`
    - `rvf federation import --input <path>`
    - `rvf federation subscribe --domains <ids> --gcloud-config <path>`
    - `rvf federation status` -- show privacy budget, contribution history
    - `rvf federation revoke --contributor <pseudonym>` -- right-to-deletion

---

## 3. GOAP Plan: Optimal Action Sequence

Using A* search through the action dependency graph, the optimal implementation order is:

```
MILESTONE 1: FOUNDATION (Week 1-2)
===================================
Sprint 1 (Week 1):
  [0.1] Add Federation Segment Types        (3 SP)
  [0.2] Add Federation Wire Codecs          (5 SP)
                                      Total: 8 SP

Sprint 2 (Week 2):
  [1.1] Create rvf-pii-strip               (8 SP)
  [2.1] Create rvf-diff-privacy            (8 SP)  -- parallel with 1.1
                                      Total: 16 SP


MILESTONE 2: CORE PROTOCOL (Week 3-4)
======================================
Sprint 3 (Week 3):
  [1.2] PII Strip no_std Core              (3 SP)
  [2.2] Diff Privacy no_std Core           (3 SP)  -- parallel with 1.2
  [3.1] Create rvf-federation (start)      (8 SP of 13)
                                      Total: 14 SP

Sprint 4 (Week 4):
  [3.1] Create rvf-federation (complete)   (5 SP remaining)
  [3.2] Create rvf-adapters/federation     (8 SP)
                                      Total: 13 SP


MILESTONE 3: CLOUD + AGGREGATION (Week 5-6)
=============================================
Sprint 5 (Week 5):
  [4.1] Create rvf-gcloud                  (13 SP)
                                      Total: 13 SP

Sprint 6 (Week 6):
  [5.1] Create rvf-fed-aggregate           (8 SP)
  [4.2] Extend Google Cloud Example        (5 SP)  -- parallel with 5.1
                                      Total: 13 SP


MILESTONE 4: INTERFACES + WASM (Week 7-8)
==========================================
Sprint 7 (Week 7):
  [5B.1] Create mcp-federation (MCP)       (8 SP)
  [5B.2] Create rvf-fed-server (REST API)  (8 SP)  -- parallel with 5B.1
                                      Total: 16 SP

Sprint 8 (Week 8):
  [6.1] Create rvf-fed-wasm               (5 SP)
  [7.2] Update Workspace Configuration    (2 SP)
  [7.3] CLI Extension                      (5 SP)  -- parallel with 6.1
                                      Total: 12 SP


MILESTONE 5: INTEGRATION (Week 9)
==================================
Sprint 9 (Week 9):
  [7.1] Integration Tests                  (8 SP)
                                      Total: 8 SP


TOTAL: 113 SP across 9 weeks (5 milestones)
```

### Dependency Graph (Topological Order)

```
[0.1] Segment Types
  |
  +---> [0.2] Wire Codecs
  |       |
  |       +---> [3.1] rvf-federation ──────────────────────┐
  |       |       |                                         |
  +---> [1.1] rvf-pii-strip ──> [1.2] no_std core ──> [6.1] rvf-fed-wasm
  |       |                                                 |
  +---> [2.1] rvf-diff-privacy ──> [2.2] no_std core ──────┘
          |                           |
          +---> [5.1] rvf-fed-aggregate ──> [5B.2] rvf-fed-server (REST API)
          |                                    |
          +---> [3.1] ──> [3.2] rvf-adapters/federation
          |                  |
          +---> [4.1] rvf-gcloud ──> [4.2] Example update
          |       |            |
          |       |            +---> [5B.1] mcp-federation (MCP Server)
          |       |            |
          +-------+---> [7.3] CLI Extension
                  |
                  +---> [7.2] Workspace Config
                  |
                  +---> [7.1] Integration Tests
```

### Critical Path

```
[0.1] -> [0.2] -> [3.1] -> [3.2] -> [7.1]
                     ^
                     |
[1.1] -> [1.2] -----+
                     |
[2.1] -> [2.2] -----+

Interface crates (off critical path, parallel):
[3.1] + [4.1] -> [5B.1] mcp-federation
[3.1] + [4.1] + [5.1] -> [5B.2] rvf-fed-server
```

The critical path runs through the segment types, wire codecs, and federation protocol. PII stripping and differential privacy can proceed in parallel but must complete before `rvf-federation` begins its final integration. The MCP server and REST API crates are off the critical path — they depend on `rvf-federation` and `rvf-gcloud` but can be built in parallel with WASM and CLI work.

---

## 4. Detailed Implementation Notes

### 4.1 Segment Type Registration

In `crates/rvf/rvf-types/src/segment_type.rs`, add to the enum:

```rust
/// Federated learning manifest (contributor, privacy budget, segment list).
FederatedManifest = 0x33,
/// Differential privacy proof (epsilon, delta, mechanism, noise proof).
DiffPrivacyProof = 0x34,
/// PII redaction attestation (counts, hashes, rules fired).
RedactionLog = 0x35,
/// Federated-averaged weights (LoRA deltas, participation, convergence).
AggregateWeights = 0x36,
```

Add to `TryFrom<u8>`:
```rust
0x33 => Ok(Self::FederatedManifest),
0x34 => Ok(Self::DiffPrivacyProof),
0x35 => Ok(Self::RedactionLog),
0x36 => Ok(Self::AggregateWeights),
```

### 4.2 PII Detection Patterns

Core regex patterns for `rvf-pii-strip`:

```rust
const PATH_UNIX: &str = r"(?:/(?:home|Users|tmp|var|etc|opt)/[^\s\x00-\x1f]+)";
const PATH_WINDOWS: &str = r"(?:[A-Za-z]:\\(?:Users|Windows|Program Files)[^\s\x00-\x1f]*)";
const IPV4: &str = r"\b(?:\d{1,3}\.){3}\d{1,3}\b";
const IPV6: &str = r"\b(?:[0-9a-fA-F]{1,4}:){2,7}[0-9a-fA-F]{1,4}\b";
const EMAIL: &str = r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b";
const API_KEY_OPENAI: &str = r"\bsk-(?:proj-)?[A-Za-z0-9]{20,}\b";
const API_KEY_AWS: &str = r"\bAKIA[A-Z0-9]{16}\b";
const API_KEY_GITHUB: &str = r"\bgh[ps]_[A-Za-z0-9]{36,}\b";
const BEARER_TOKEN: &str = r"\bBearer\s+[A-Za-z0-9\-._~+/]+=*\b";
const ENV_VAR_UNIX: &str = r"\$(?:HOME|USER|PATH|SHELL|TMPDIR|HOSTNAME)\b";
const ENV_VAR_WIN: &str = r"%(?:USERPROFILE|USERNAME|COMPUTERNAME|TEMP|TMP)%";
```

### 4.3 Gaussian Mechanism Calibration

For `rvf-diff-privacy`, the Gaussian mechanism adds noise:

```rust
/// Calibrate noise for (epsilon, delta)-differential privacy.
///
/// sigma = sensitivity * sqrt(2 * ln(1.25 / delta)) / epsilon
pub fn calibrate_gaussian(sensitivity: f64, epsilon: f64, delta: f64) -> f64 {
    sensitivity * (2.0 * (1.25 / delta).ln()).sqrt() / epsilon
}

/// Add calibrated Gaussian noise to a parameter vector.
pub fn add_gaussian_noise(
    params: &mut [f32],
    sensitivity: f64,
    epsilon: f64,
    delta: f64,
    rng: &mut impl Rng,
) {
    let sigma = calibrate_gaussian(sensitivity, epsilon, delta) as f32;
    let dist = rand_distr::Normal::new(0.0, sigma).unwrap();
    for p in params.iter_mut() {
        *p += rng.sample(dist);
    }
}
```

### 4.4 Renyi Differential Privacy Accountant

```rust
/// RDP accountant for privacy budget tracking.
///
/// For the Gaussian mechanism with noise multiplier sigma:
///   RDP(alpha) = alpha / (2 * sigma^2)
///
/// Convert RDP to (epsilon, delta)-DP:
///   epsilon = RDP(alpha) + ln(1/delta) / (alpha - 1) - ln(alpha) / (alpha - 1)
///
/// Composition: RDP values add across multiple queries.
pub struct RdpAccountant {
    /// Accumulated RDP values at each alpha order.
    rdp_values: Vec<(f64, f64)>,  // (alpha, accumulated_rdp)
    /// Alpha orders to track.
    alpha_orders: Vec<f64>,
}

impl RdpAccountant {
    pub fn new() -> Self {
        let alpha_orders: Vec<f64> = (2..=256).map(|a| a as f64).collect();
        let rdp_values = alpha_orders.iter().map(|&a| (a, 0.0)).collect();
        Self { rdp_values, alpha_orders }
    }

    pub fn add_gaussian_query(&mut self, sigma: f64) {
        for (alpha, rdp) in self.rdp_values.iter_mut() {
            *rdp += *alpha / (2.0 * sigma * sigma);
        }
    }

    pub fn get_epsilon(&self, delta: f64) -> f64 {
        self.rdp_values.iter()
            .map(|(alpha, rdp)| rdp + (1.0 / delta).ln() / (alpha - 1.0))
            .fold(f64::INFINITY, f64::min)
    }
}
```

### 4.5 Version-Aware Prior Merging

```rust
/// Merge a remote TransferPrior into a local one.
///
/// Uses evidence-weighted blending: the source with more training cycles
/// gets higher weight. Then applies sqrt-dampening to prevent over-confidence.
pub fn merge_transfer_priors(
    local: &TransferPrior,
    remote: &TransferPrior,
) -> TransferPrior {
    let total_cycles = local.training_cycles + remote.training_cycles;
    let remote_weight = if total_cycles > 0 {
        remote.training_cycles as f32 / total_cycles as f32
    } else {
        0.5
    };
    let local_weight = 1.0 - remote_weight;

    let mut merged = TransferPrior::uniform(local.source_domain.clone());
    merged.training_cycles = total_cycles;

    // Collect all bucket/arm combinations from both
    let all_buckets: HashSet<_> = local.bucket_priors.keys()
        .chain(remote.bucket_priors.keys())
        .collect();

    for bucket in all_buckets {
        let local_arms = local.bucket_priors.get(bucket);
        let remote_arms = remote.bucket_priors.get(bucket);

        let all_arms: HashSet<_> = local_arms.iter()
            .flat_map(|m| m.keys())
            .chain(remote_arms.iter().flat_map(|m| m.keys()))
            .collect();

        let mut merged_arms = HashMap::new();
        for arm in all_arms {
            let l = local_arms
                .and_then(|m| m.get(arm))
                .unwrap_or(&BetaParams::uniform());
            let r = remote_arms
                .and_then(|m| m.get(arm))
                .unwrap_or(&BetaParams::uniform());

            // Weighted blend
            let alpha = l.alpha * local_weight + r.alpha * remote_weight;
            let beta = l.beta * local_weight + r.beta * remote_weight;

            // Sqrt-dampening (same as MetaThompsonEngine::init_domain_with_transfer)
            let dampened = BetaParams {
                alpha: 1.0 + (alpha - 1.0).sqrt(),
                beta: 1.0 + (beta - 1.0).sqrt(),
            };

            merged_arms.insert(arm.clone(), dampened);
        }
        merged.bucket_priors.insert(bucket.clone(), merged_arms);
    }

    merged
}
```

### 4.6 FedAvg Implementation

```rust
/// Federated averaging of TransferPriors.
///
/// weight_k = reputation_k * trajectory_count_k * avg_quality_k
/// w_avg = sum(weight_k * prior_k) / sum(weight_k)
pub fn fedavg_priors(
    contributions: &[(TransferPrior, f32)],  // (prior, weight)
) -> TransferPrior {
    let total_weight: f32 = contributions.iter().map(|(_, w)| w).sum();
    if total_weight < 1e-10 || contributions.is_empty() {
        return TransferPrior::uniform(DomainId("aggregate".into()));
    }

    let mut result = TransferPrior::uniform(DomainId("aggregate".into()));

    // Collect all unique bucket/arm combinations
    let all_buckets: HashSet<_> = contributions.iter()
        .flat_map(|(p, _)| p.bucket_priors.keys())
        .collect();

    for bucket in &all_buckets {
        let all_arms: HashSet<_> = contributions.iter()
            .flat_map(|(p, _)| {
                p.bucket_priors.get(*bucket)
                    .map(|m| m.keys().collect::<Vec<_>>())
                    .unwrap_or_default()
            })
            .collect();

        let mut merged_arms = HashMap::new();
        for arm in &all_arms {
            let mut alpha_sum = 0.0;
            let mut beta_sum = 0.0;

            for (prior, weight) in contributions {
                let params = prior.get_prior(bucket, arm);
                let normalized_weight = weight / total_weight;
                alpha_sum += params.alpha * normalized_weight;
                beta_sum += params.beta * normalized_weight;
            }

            merged_arms.insert((*arm).clone(), BetaParams {
                alpha: alpha_sum,
                beta: beta_sum,
            });
        }
        result.bucket_priors.insert((*bucket).clone(), merged_arms);
    }

    result.training_cycles = contributions.iter()
        .map(|(p, _)| p.training_cycles)
        .sum();

    result
}
```

### 4.7 Byzantine-Tolerant Aggregation (Krum)

```rust
/// Krum aggregation: select the contribution closest to its peers.
///
/// For each contribution k, compute:
///   score_k = sum of distances to (N - f - 1) nearest neighbors
/// where f = ceil(N/3) - 1 (Byzantine tolerance).
///
/// Select the contribution with the minimum score.
pub fn krum_select(
    contributions: &[(TransferPrior, f32)],
) -> Option<usize> {
    let n = contributions.len();
    if n < 4 { return Some(0); }  // Need at least 4 for Byzantine tolerance

    let f = (n as f32 / 3.0).ceil() as usize - 1;
    let neighbors_to_check = n - f - 1;

    // Flatten each prior into a parameter vector for distance computation
    let vectors: Vec<Vec<f32>> = contributions.iter()
        .map(|(p, _)| flatten_prior(p))
        .collect();

    // Compute pairwise distances
    let mut scores = vec![0.0f32; n];
    for i in 0..n {
        let mut distances: Vec<f32> = (0..n)
            .filter(|&j| j != i)
            .map(|j| l2_distance(&vectors[i], &vectors[j]))
            .collect();
        distances.sort_by(|a, b| a.partial_cmp(b).unwrap());
        scores[i] = distances.iter().take(neighbors_to_check).sum();
    }

    scores.iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(idx, _)| idx)
}
```

---

## 5. Risk Mitigations

### 5.1 Poisoning Attack Prevention

**Threat**: Malicious contributor submits crafted learning data to degrade other users' models.

**Mitigations** (defense in depth):
1. Byzantine-tolerant aggregation (Krum) excludes outlier contributions
2. Reputation system: new contributors have low weight; weight grows with successful contributions
3. Signature verification: every export is signed, attributable to a pseudonym
4. Quality gate: only learning from high-quality trajectories is exported
5. Differential privacy noise limits the impact of any single contribution
6. Dampened priors: imports are sqrt-dampened before integration

### 5.2 Privacy Budget Management

**Threat**: User exports too frequently, accumulating enough epsilon to allow reconstruction.

**Mitigations**:
1. `PrivacyBudget` tracked per-contributor in Firestore
2. Each export's `DiffPrivacyProof` records epsilon spent
3. Server rejects exports when cumulative epsilon exceeds configurable limit (default: 10.0)
4. Alert at 80% budget usage
5. Budget resets annually (configurable)

### 5.3 Backward Compatibility

**Threat**: Adding new segment types breaks existing RVF readers.

**Mitigations**:
1. RVF's forward compatibility: unknown segment types are skipped by readers that do not recognize them (existing behavior)
2. New segments use `0x33-0x36` range, which existing `TryFrom<u8>` returns `Err(_)` for
3. All federation code is behind `federation` Cargo feature flag
4. `FederatedManifest` header includes a format version field for future evolution

### 5.4 Regulatory Compliance

**Threat**: Federation data subject to GDPR/CCPA despite PII stripping.

**Mitigations**:
1. PII stripping is **mandatory** at the export boundary, not optional
2. `RedactionLog` provides auditable proof that stripping occurred
3. Contributor pseudonym (SHAKE-256 hash) is the only identifier in cloud
4. Right-to-deletion: revoke pseudonym -> delete all GCS objects -> Firestore cleanup
5. Differential privacy provides mathematical guarantee that individual contributions cannot be reconstructed

---

## 6. Monitoring and Observability

### 6.1 Metrics

| Metric | Type | Description |
|---|---|---|
| `federation.exports.total` | Counter | Total exports submitted |
| `federation.imports.total` | Counter | Total imports processed |
| `federation.rejections.total{reason}` | Counter | Imports rejected, labeled by reason |
| `federation.pii.detections{type}` | Counter | PII detections by type |
| `federation.privacy.budget.used{contributor}` | Gauge | Epsilon spent per contributor |
| `federation.aggregate.rounds` | Counter | Aggregation rounds completed |
| `federation.aggregate.participants` | Histogram | Participants per round |
| `federation.acceleration.factor` | Gauge | Last measured acceleration from imports |
| `federation.latency.export_ms` | Histogram | Export build time |
| `federation.latency.import_ms` | Histogram | Import + merge time |

### 6.2 Structured Logging

All federation operations emit structured log events:
- `event=federation_export contributor=<pseudonym> domain=<id> segments=<count> epsilon=<val>`
- `event=federation_import source=<pseudonym> domain=<id> valid=<bool> reason=<str>`
- `event=federation_aggregate round=<id> participants=<count> method=<fedavg|krum>`
- `event=pii_detection type=<path|ip|key> count=<n>`
- `event=privacy_budget contributor=<pseudonym> remaining=<epsilon>`

---

## 7. Testing Strategy

### 7.1 Unit Tests (per crate)

| Crate | Test Focus | Est. Count |
|---|---|---|
| rvf-types (federation) | Segment type discriminants, header struct sizes | ~10 |
| rvf-wire (federation) | Codec round-trips, malformed input handling | ~15 |
| rvf-pii-strip | Detection patterns, redaction determinism, attestation hashes | ~30 |
| rvf-diff-privacy | Noise calibration, RDP composition, budget tracking | ~25 |
| rvf-federation | Export/import round-trip, policy enforcement, version compat | ~30 |
| rvf-fed-aggregate | FedAvg math, Krum selection, reputation scoring | ~20 |
| rvf-gcloud | Mock-based GCS/PubSub/Firestore operations | ~25 |
| rvf-adapters/federation | Coordinator export/import, engine integration | ~15 |

### 7.2 Integration Tests

- End-to-end export from real SONA + DomainExpansion -> PII strip -> noise -> sign -> validate -> import -> verify acceleration
- Multi-contributor aggregation round with FedAvg
- Byzantine tolerance with injected outlier
- Privacy budget exhaustion and rejection
- WASM export path (headless browser test)

### 7.3 Property-Based Tests

- PII detector: any string matching a PII pattern must be redacted
- Differential privacy: output distribution must satisfy (epsilon, delta) bounds
- Witness chain: reordering segments must fail verification
- FedAvg: result is a convex combination of inputs

---

## 8. Open Questions

1. **ML-DSA-65 vs Ed25519**: ADR-057 mentions both. Ed25519 is available now in `rvf-crypto`. ML-DSA-65 (post-quantum) would require adding `pqcrypto-dilithium` dependency. Recommendation: start with Ed25519, add ML-DSA-65 as a future optional feature.

2. **Reputation bootstrapping**: New contributors start with no reputation. How much weight should their first contribution receive? Recommendation: fixed minimum weight of 0.1 for first 5 contributions, then reputation-based.

3. **Cross-region replication**: Should GCS buckets be multi-region? Recommendation: start single-region (us-central1), add multi-region when >100 contributors.

4. **Aggregation trigger**: Time-based (hourly) or participant-based (every N submissions)? Recommendation: participant-based with timeout fallback. `min_participants=5, max_wait=3600s`.

5. **SONA weight granularity**: Export full LoRA matrices or just the rank-1/rank-2 deltas? Recommendation: export rank-matched deltas only (typically 2*hidden_dim*rank floats = 1024 floats for rank=2, dim=256). Full matrices are unnecessary for transfer.

# Federated RVF Transfer Learning -- Architecture

**ADR**: ADR-057
**Date**: 2026-02-26

## System Architecture Overview

```
                              GOOGLE CLOUD
    ┌────────────────────────────────────────────────────────────────────┐
    │                                                                    │
    │  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐             │
    │  │  Pub/Sub     │  │  Cloud       │  │  Firestore    │             │
    │  │  (events)    │  │  Storage     │  │  (registry)   │             │
    │  │              │  │  (RVF files) │  │               │             │
    │  └──────┬───────┘  └──────┬───────┘  └──────┬────────┘             │
    │         │                 │                  │                      │
    │  ┌──────┴─────────────────┴──────────────────┴──────────────────┐  │
    │  │     Cloud Run: rvf-fed-server (axum REST API)                │  │
    │  │                                                              │  │
    │  │  REST API (https://federation.ruvector.dev/v1)               │  │
    │  │  POST /v1/exports          GET /v1/domains                   │  │
    │  │  GET  /v1/exports/{id}     GET /v1/contributors/{id}        │  │
    │  │  POST /v1/aggregates       GET /v1/events (SSE)             │  │
    │  │  GET  /v1/health           GET /v1/metrics (Prometheus)     │  │
    │  └──────────────────────────────────────────────────────────────┘  │
    │                                                                    │
    └──────────┬───────────────────────┬───────────────────┬─────────────┘
               │                       │                   │
               │  REST/SSE             │  stdio JSON-RPC   │  REST/SSE
    ┌──────────┴────────┐    ┌─────────┴──────────┐    ┌───┴──────────────┐
    │   USER A          │    │   MCP-FEDERATION   │    │   USER B          │
    │                   │    │   (MCP Server)      │    │                   │
    │  ┌──────────┐     │    │                     │    │  ┌──────────┐     │
    │  │  SONA    │     │    │  federation_export  │    │  │  SONA    │     │
    │  │  Engine  │     │    │  federation_import  │    │  │  Engine  │     │
    │  └────┬─────┘     │    │  federation_status  │    │  └────┬─────┘     │
    │       │           │    │  federation_search  │    │       │           │
    │  ┌────┴─────┐     │    │  federation_budget  │    │  ┌────┴─────┐     │
    │  │  Domain  │     │    │  federation_aggr.   │    │  │  Domain  │     │
    │  │  Expand  │     │    │                     │    │  │  Expand  │     │
    │  └────┬─────┘     │    │  Resources:         │    │  └────┬─────┘     │
    │       │           │    │  federation://...   │    │       │           │
    │  ┌────┴────────┐  │    └─────────────────────┘    │  ┌────┴────────┐  │
    │  │rvf-adapters │  │                               │  │rvf-adapters │  │
    │  │/federation  │  │                               │  │/federation  │  │
    │  └────┬────────┘  │                               │  └────┬────────┘  │
    │       │           │                               │       │           │
    │  ┌────┴──────┐    │                               │  ┌────┴──────┐    │
    │  │PII Strip +│    │                               │  │ Import    │    │
    │  │Diff Priv. │    │                               │  │ Merger    │    │
    │  └────┬──────┘    │                               │  └───────────┘    │
    │       │           │                               │                   │
    │  ┌────┴────────┐  │                               │                   │
    │  │rvf-gcloud   │  │                               │                   │
    │  └─────────────┘  │                               │                   │
    │                   │                               │                   │
    └───────────────────┘                               └───────────────────┘
```

## Crate Dependency Graph

```
rvf-types (no_std)
    |
    v
rvf-wire
    |
    v
rvf-crypto (ed25519, shake-256)
    |
    +---> rvf-pii-strip (core: no_std, full: std)
    |         |
    |         +---> rvf-diff-privacy (core: no_std, full: std)
    |         |         |
    |         |         v
    |         +-------> rvf-federation (std-only)
    |                       |
    |                       +---> rvf-fed-aggregate (std-only)
    |                       |         |
    |                       |         v
    |                       +---> rvf-gcloud (std-only)
    |                       |
    |                       +---> rvf-fed-wasm (wasm32)
    |                       |
    |                       +---> mcp-federation (MCP server, std-only)  <-- NEW
    |                       |         depends on: rvf-federation, rvf-gcloud
    |                       |
    |                       +---> rvf-fed-server (axum REST API, std-only)  <-- NEW
    |                       |         depends on: rvf-federation, rvf-gcloud,
    |                       |                     rvf-fed-aggregate
    |                       |
    |                       v
    |                   rvf-adapters/federation (std-only)
    |                       |
    +--- rvf-runtime -------+---> sona (via rvf-adapter-sona)
    |                       |
    +--- rvf-adapter-sona --+
    |                       |
    +--- domain-expansion --+  (via rvf_bridge)
```

**Note**: `mcp-federation` and `rvf-fed-server` are interface crates — they expose
federation functionality through MCP (JSON-RPC 2.0 over stdio) and REST API (axum HTTP)
respectively. Both delegate to `rvf-federation` for core logic and `rvf-gcloud` for
cloud operations. The old crate dependency graph (without these two) is shown below
for reference.

### Original Dependency Graph (pre-MCP/API)

```
rvf-types (no_std)
    |
    v
rvf-wire
    |
    v
rvf-crypto (ed25519, shake-256)
    |
    +---> rvf-pii-strip (core: no_std, full: std)
    |         |
    |         +---> rvf-diff-privacy (core: no_std, full: std)
    |         |         |
    |         |         v
    |         +-------> rvf-federation (std-only)
    |                       |
    |                       +---> rvf-fed-aggregate (std-only)
    |                       |         |
    |                       |         v
    |                       +---> rvf-gcloud (std-only)
    |                       |
    |                       +---> rvf-fed-wasm (wasm32)
    |                       |
    |                       v
    |                   rvf-adapters/federation (std-only)
    |                       |
    +--- rvf-runtime -------+---> sona (via rvf-adapter-sona)
    |                       |
    +--- rvf-adapter-sona --+
    |                       |
    +--- domain-expansion --+  (via rvf_bridge)
```

## Export Data Flow

```
Step 1: EXTRACTION
=================

SONA Engine ─────────────────┐
  - Micro-LoRA weights       │
  - Base-LoRA weights        │
  - EWC Fisher information   │
  - Trajectory buffer stats  │      rvf-adapters/federation
  - ReasoningBank patterns   ├────> ExportCoordinator
                              │      - Collects all sources
Domain Expansion ────────────┤      - Applies quality gate
  - TransferPrior (Beta)     │      - Applies min-evidence filter
  - PolicyKernel (knobs)     │      - Respects FederationPolicy
  - CostCurve (convergence)  │
  - Scoreboard (accel)       │
                              │
RVF Store ───────────────────┘
  - HNSW tuning params
  - Quantization codebooks


Step 2: PII STRIPPING
=====================

             rvf-pii-strip Pipeline
             ┌─────────────────────────────────────────┐
             │                                         │
   Raw       │  ┌──────────┐  ┌──────────┐  ┌──────┐  │  Clean
   Segments ─┤─>│ Detection │─>│ Redaction│─>│Attest│──├──> Segments
             │  │ (regex)   │  │ (pseudo) │  │(hash)│  │  + RedactionLog
             │  └──────────┘  └──────────┘  └──────┘  │    (0x35)
             │                                         │
             └─────────────────────────────────────────┘

Detection Patterns:
  /home/*/...           -> <PATH_1>
  192.168.*.*           -> <IP_1>
  user@domain.com       -> <EMAIL_1>
  sk-proj-...           -> <REDACTED_KEY>
  ghp_...               -> <REDACTED_KEY>
  AKIA...               -> <REDACTED_KEY>
  $HOME, %USERPROFILE%  -> <ENV_REF>


Step 3: DIFFERENTIAL PRIVACY
=============================

               rvf-diff-privacy Pipeline
               ┌───────────────────────────────────────┐
               │                                       │
   Clean       │  ┌─────────┐  ┌──────────┐  ┌─────┐  │  Private
   Segments ───├─>│  Clip   │─>│  Noise   │─>│Proof│──├──> Segments
               │  │ (norms) │  │ (Gauss)  │  │(RDP)│  │  + DiffPrivacyProof
               │  └─────────┘  └──────────┘  └─────┘  │    (0x34)
               │                                       │
               └───────────────────────────────────────┘

Parameters:
  epsilon = 1.0 (default, configurable)
  delta = 1e-5
  clipping_norm = 1.0
  noise_multiplier = calibrated to (epsilon, delta, sensitivity)

Applied to:
  - BetaParams.alpha, BetaParams.beta (TransferPrior)
  - PolicyKnobs numeric fields (PolicyKernel)
  - CostCurvePoint values (CostCurve)
  - LoRA weight deltas (AggregateWeights)
  - NOT applied to: structure, types, segment headers


Step 4: ASSEMBLY
================

   rvf-federation ExportBuilder
   ┌──────────────────────────────────────────────────────────┐
   │                                                          │
   │  ┌────────────────────┐                                  │
   │  │ FederatedManifest  │  (0x33) -- always first          │
   │  │ - contributor_id   │                                  │
   │  │ - export_version   │                                  │
   │  │ - domain_ids       │                                  │
   │  │ - segment_count    │                                  │
   │  │ - privacy_budget   │                                  │
   │  └────────────────────┘                                  │
   │                                                          │
   │  ┌────────────────────┐  (0x30) -- one per domain        │
   │  │ TransferPrior      │  stripped + noised                │
   │  └────────────────────┘                                  │
   │                                                          │
   │  ┌────────────────────┐  (0x31) -- best kernel only      │
   │  │ PolicyKernel       │  stripped + noised                │
   │  └────────────────────┘                                  │
   │                                                          │
   │  ┌────────────────────┐  (0x32) -- convergence summary   │
   │  │ CostCurve          │  stripped + noised                │
   │  └────────────────────┘                                  │
   │                                                          │
   │  ┌────────────────────┐  (0x36) -- if SONA weights       │
   │  │ AggregateWeights   │  clipped + noised LoRA deltas    │
   │  └────────────────────┘                                  │
   │                                                          │
   │  ┌────────────────────┐  (0x35) -- PII attestation       │
   │  │ RedactionLog       │                                  │
   │  └────────────────────┘                                  │
   │                                                          │
   │  ┌────────────────────┐  (0x34) -- privacy attestation   │
   │  │ DiffPrivacyProof   │                                  │
   │  └────────────────────┘                                  │
   │                                                          │
   │  ┌────────────────────┐  (0x0A) -- audit chain           │
   │  │ Witness            │  SHAKE-256 chain over all above   │
   │  └────────────────────┘                                  │
   │                                                          │
   │  ┌────────────────────┐  (0x0C) -- Ed25519 signature     │
   │  │ Crypto             │  signs entire export              │
   │  └────────────────────┘                                  │
   │                                                          │
   └──────────────────────────────────────────────────────────┘

   Output: Single .rvf file, 64-byte-aligned, crash-safe
```

## Import and Merge Flow

```
Step 1: VALIDATION
==================

  Downloaded .rvf ──> rvf-federation ImportValidator
                      ┌──────────────────────────────────┐
                      │  1. Verify Crypto signature       │
                      │  2. Verify Witness chain          │
                      │  3. Check FederatedManifest       │
                      │     - version compatibility       │
                      │     - contributor not blacklisted  │
                      │     - segments not expired         │
                      │  4. Verify DiffPrivacyProof       │
                      │     - epsilon within bounds        │
                      │     - mechanism is approved        │
                      │  5. Verify RedactionLog            │
                      │     - all required rules ran       │
                      └──────────────┬───────────────────┘
                                     │
                                  VALID? ──no──> REJECT + log
                                     │
                                    yes
                                     │
Step 2: VERSION-AWARE MERGE
============================

  rvf-federation VersionMerger
  ┌───────────────────────────────────────────────────────────┐
  │                                                           │
  │  For each TransferPrior:                                  │
  │    local = engine.extract_prior(domain)                   │
  │    remote = decoded TransferPrior from import             │
  │                                                           │
  │    if local.training_cycles > remote.training_cycles:     │
  │      weight_remote = 0.3  (local has more evidence)       │
  │    else:                                                  │
  │      weight_remote = 0.7  (remote has more evidence)      │
  │                                                           │
  │    for each (bucket, arm):                                │
  │      merged.alpha = local.alpha * (1 - w) + remote.alpha  │
  │                     * w                                   │
  │      merged.beta  = local.beta  * (1 - w) + remote.beta   │
  │                     * w                                   │
  │                                                           │
  │    Apply dampening (sqrt-scaling, same as                 │
  │    init_domain_with_transfer)                             │
  │                                                           │
  │  For each PolicyKernel:                                   │
  │    if remote.fitness() > local_best.fitness():            │
  │      inject into population as elite candidate            │
  │    else:                                                  │
  │      inject as mutant (non-elite) for diversity           │
  │                                                           │
  │  For each AggregateWeights:                               │
  │    Weighted average with local LoRA weights               │
  │    Apply EWC penalty to prevent catastrophic forgetting   │
  │                                                           │
  │  For each CostCurve:                                      │
  │    Merge into acceleration scoreboard as reference curve   │
  │                                                           │
  └───────────────────────────────────────────────────────────┘
```

## Federated Aggregation Architecture

```
                    AGGREGATION SERVER (Cloud Run)
    ┌──────────────────────────────────────────────────────────┐
    │                                                          │
    │  ┌──────────────────────┐                                │
    │  │  Round Manager       │                                │
    │  │  - round_id: u64     │                                │
    │  │  - min_participants  │                                │
    │  │  - max_wait_seconds  │                                │
    │  │  - domain_filter     │                                │
    │  └──────────┬───────────┘                                │
    │             │                                            │
    │             v                                            │
    │  ┌──────────────────────┐   ┌────────────────────────┐   │
    │  │  Submission Queue    │   │  Byzantine Filter      │   │
    │  │  (per round)         │──>│  - IQR outlier detect  │   │
    │  │  [export_1, ...]     │   │  - Krum aggregation    │   │
    │  └──────────────────────┘   │  - reputation weight   │   │
    │                              └────────────┬───────────┘   │
    │                                           │              │
    │                                           v              │
    │                              ┌────────────────────────┐   │
    │                              │  FedAvg / FedProx      │   │
    │                              │  - weighted_average()  │   │
    │                              │  - proximal_term()     │   │
    │                              │  - momentum()          │   │
    │                              └────────────┬───────────┘   │
    │                                           │              │
    │                                           v              │
    │                              ┌────────────────────────┐   │
    │                              │  Aggregate Builder     │   │
    │                              │  - AggregateWeights    │   │
    │                              │  - WitnessChain        │   │
    │                              │  - Signature           │   │
    │                              └────────────┬───────────┘   │
    │                                           │              │
    │                                           v              │
    │                              ┌────────────────────────┐   │
    │                              │  Publish to GCS        │   │
    │                              │  + Pub/Sub notify      │   │
    │                              └────────────────────────┘   │
    │                                                          │
    └──────────────────────────────────────────────────────────┘


    AGGREGATION ALGORITHMS
    ======================

    FedAvg (default):
      w_agg = (1/N) * SUM(n_k / n_total * w_k)
      where n_k = trajectory count from contributor k
            n_total = sum of all trajectory counts

    FedProx (for heterogeneous contributors):
      w_agg = FedAvg + mu/2 * ||w_k - w_global||^2
      mu = proximal term (default 0.01)

    Byzantine-Tolerant (Krum):
      For each contributor k, compute:
        score_k = SUM over nearest f neighbors of ||w_k - w_j||^2
      Select w_k with minimum score
      (f = ceil(N/3) - 1 for Byzantine tolerance)

    Weighted by Reputation:
      reputation_k = avg_quality_k * trajectory_count_k * age_factor_k
      w_agg = SUM(reputation_k * w_k) / SUM(reputation_k)
```

## Google Cloud Infrastructure

```
    PROJECT: ruvector-federation
    REGION: us-central1 (primary), europe-west1 (secondary)

    ┌─────────────────────────────────────────────────────────┐
    │                    VPC Network                           │
    │                                                         │
    │  ┌─────────────────────┐    ┌────────────────────────┐  │
    │  │  Cloud Run           │    │  Cloud Storage          │  │
    │  │  federation-server   │    │  ruvector-federation-*  │  │
    │  │                      │    │                          │  │
    │  │  CPU: 2 vCPU         │    │  Buckets:               │  │
    │  │  Memory: 4 GiB       │    │  - /exports/{domain}/   │  │
    │  │  Min: 0, Max: 10     │    │  - /aggregates/{round}/ │  │
    │  │  Concurrency: 80     │    │  - /manifests/           │  │
    │  │  Timeout: 300s       │    │                          │  │
    │  │                      │    │  Lifecycle:              │  │
    │  │  Endpoints:           │    │  - Archive: 90 days     │  │
    │  │  /federation/submit  │    │  - Delete: 365 days     │  │
    │  │  /federation/pull    │    │                          │  │
    │  │  /federation/agg     │    │  Encryption: CMEK       │  │
    │  │  /federation/status  │    │                          │  │
    │  └─────────┬───────────┘    └────────────────────────┘  │
    │            │                                             │
    │  ┌─────────┴───────────┐    ┌────────────────────────┐  │
    │  │  Pub/Sub             │    │  Firestore              │  │
    │  │                      │    │                          │  │
    │  │  Topics:             │    │  Collections:            │  │
    │  │  - federation-events │    │  - manifests             │  │
    │  │  - aggregation-ready │    │  - contributors          │  │
    │  │  - import-available  │    │  - rounds                │  │
    │  │                      │    │  - reputation_scores     │  │
    │  │  Subscriptions:      │    │  - privacy_budgets       │  │
    │  │  - per-contributor   │    │                          │  │
    │  │  - per-domain filter │    │  Indexes:                │  │
    │  │                      │    │  - domain + timestamp    │  │
    │  │  Retention: 7 days   │    │  - contributor + domain  │  │
    │  │  Ack deadline: 60s   │    │                          │  │
    │  └──────────────────────┘    └────────────────────────┘  │
    │                                                         │
    │  ┌──────────────────────┐    ┌────────────────────────┐  │
    │  │  Cloud IAM           │    │  Cloud Monitoring       │  │
    │  │                      │    │                          │  │
    │  │  Roles:              │    │  Metrics:                │  │
    │  │  - federation.submit │    │  - exports/min           │  │
    │  │  - federation.pull   │    │  - imports/min           │  │
    │  │  - federation.admin  │    │  - aggregate_latency     │  │
    │  │                      │    │  - privacy_budget_usage  │  │
    │  │  Service Accounts:   │    │  - rejection_rate        │  │
    │  │  - server-sa         │    │                          │  │
    │  │  - contributor-sa    │    │  Alerts:                 │  │
    │  └──────────────────────┘    │  - budget > 80%          │  │
    │                              │  - rejection > 50%       │  │
    │                              └────────────────────────┘  │
    │                                                         │
    └─────────────────────────────────────────────────────────┘
```

## Segment Wire Format Details

### FederatedManifest (0x33)

```
Offset  Size    Field
0x00    4       magic: 0x46454430 ("FED0")
0x04    2       version: u16 (currently 1)
0x06    2       flags: u16
                  bit 0: has_diff_privacy
                  bit 1: has_redaction_log
                  bit 2: has_aggregate_weights
                  bit 3: has_sona_weights
0x08    8       export_timestamp_ns: u64
0x10    32      contributor_pseudonym: [u8; 32] (SHAKE-256 of real identity)
0x30    4       segment_count: u32
0x34    4       domain_count: u32
0x38    8       total_training_cycles: u64
0x40    4       epsilon_millis: u32 (epsilon * 1000, for integer representation)
0x44    4       delta_exp: u32 (negative exponent, e.g., 5 for 1e-5)
0x48    24      reserved: [u8; 24]
--- variable-length section ---
0x60    N       domain_ids: length-prefixed UTF-8 strings
        M       segment_id_list: u64[] (IDs of included segments)
```

### DiffPrivacyProof (0x34)

```
Offset  Size    Field
0x00    4       magic: 0x44505246 ("DPRF")
0x04    1       mechanism: u8 (0=Gaussian, 1=Laplace, 2=Exponential)
0x05    1       composition: u8 (0=Basic, 1=Advanced, 2=RDP)
0x06    2       reserved: u16
0x08    4       epsilon_millis: u32
0x0C    4       delta_exp: u32
0x10    4       noise_multiplier_millis: u32
0x14    4       clipping_norm_millis: u32
0x18    4       parameters_clipped: u32 (count)
0x1C    4       total_parameters: u32 (count)
0x20    8       cumulative_epsilon_millis: u64 (lifetime budget spent)
0x28    8       remaining_budget_millis: u64
0x30    32      proof_hash: [u8; 32] (SHAKE-256 of noise-added parameters)
```

### RedactionLog (0x35)

```
Offset  Size    Field
0x00    4       magic: 0x52444354 ("RDCT")
0x04    2       version: u16
0x06    2       rule_count: u16
0x08    4       paths_redacted: u32
0x0C    4       ips_redacted: u32
0x10    4       emails_redacted: u32
0x14    4       keys_redacted: u32
0x18    4       env_refs_redacted: u32
0x1C    4       custom_redacted: u32
0x20    32      pre_redaction_hash: [u8; 32] (SHAKE-256 of original content)
0x40    32      post_redaction_hash: [u8; 32] (SHAKE-256 of redacted content)
--- variable-length section ---
0x60    N       rules_fired: length-prefixed rule name strings
```

### AggregateWeights (0x36)

```
Offset  Size    Field
0x00    4       magic: 0x41475754 ("AGWT")
0x04    2       version: u16
0x06    2       flags: u16
                  bit 0: is_lora_delta (vs. full weights)
                  bit 1: is_ewc_regularized
                  bit 2: is_quantized
0x08    4       participant_count: u32
0x0C    4       aggregation_round: u32
0x10    4       hidden_dim: u32
0x14    4       lora_rank: u32
0x18    4       weight_count: u32
0x1C    4       quantization: u32 (0=f32, 1=f16, 2=bf16, 3=int8)
0x20    8       convergence_metric_millis: u64
0x28    8       timestamp_ns: u64
0x30    16      reserved: [u8; 16]
--- variable-length section ---
0x40    N       weights: [f32/f16/...] (weight_count elements)
        M       ewc_fisher: [f32] (optional, if flag bit 1)
```

## Security Model

```
    TRUST BOUNDARIES
    ================

    ┌──────────────────────────────────────────────────────┐
    │  LOCAL TRUST ZONE (user's machine)                    │
    │                                                      │
    │  - Raw SONA weights (unredacted, unnoised)           │
    │  - Raw TransferPriors (with real bucket names)        │
    │  - File paths, API keys, credentials                 │
    │  - Contributor's real identity                        │
    │                                                      │
    │  ┌────────────────────────────────────────────────┐   │
    │  │  EXPORT BOUNDARY (rvf-pii-strip + diff-priv)  │   │
    │  │  ============================================  │   │
    │  │  PII stripped, noise injected, signed          │   │
    │  └────────────────────────────────────────────────┘   │
    │                                                      │
    └──────────────────────────┬───────────────────────────┘
                               │
                               │ HTTPS + TLS 1.3
                               │ + Ed25519 signature
                               │
    ┌──────────────────────────┴───────────────────────────┐
    │  CLOUD TRUST ZONE (Google Cloud)                      │
    │                                                      │
    │  - Pseudonymized contributor IDs                      │
    │  - Noised parameters only                            │
    │  - Encrypted at rest (CMEK)                          │
    │  - IAM-controlled access                             │
    │  - No raw PII ever reaches cloud                     │
    │                                                      │
    │  Verification at cloud:                              │
    │  1. Ed25519 signature valid?                         │
    │  2. Witness chain intact?                            │
    │  3. DiffPrivacyProof epsilon within policy?          │
    │  4. RedactionLog present and complete?               │
    │  5. Contributor not revoked?                         │
    │                                                      │
    └──────────────────────────────────────────────────────┘

    SIGNATURE CHAIN
    ===============

    Export segments:
      seg_1 (FederatedManifest) ──hash──┐
      seg_2 (TransferPrior)     ──hash──┤
      seg_3 (PolicyKernel)      ──hash──┤
      ...                               │
      seg_N (DiffPrivacyProof)  ──hash──┤
                                         v
      WITNESS_SEG: SHAKE-256 chain ─────┐
                                         v
      CRYPTO_SEG: Ed25519.sign(         │
        SHAKE-256(all_segment_hashes)   │
      ) ────────────────────────────────┘

    Import verification:
      1. Verify CRYPTO_SEG signature with contributor's public key
      2. Recompute WITNESS_SEG chain from segments
      3. Compare computed chain with embedded chain
      4. Verify DiffPrivacyProof.proof_hash matches noised content
      5. Accept or reject
```

## WASM Export Path

```
    BROWSER / EDGE DEVICE
    ┌────────────────────────────────────────────────────────┐
    │                                                        │
    │  JavaScript Application                                │
    │  ┌──────────────────────────────────────────────────┐   │
    │  │                                                  │   │
    │  │  import { FederationExporter } from              │   │
    │  │    '@ruvector/rvf-fed-wasm';                     │   │
    │  │                                                  │   │
    │  │  const exporter = new FederationExporter({       │   │
    │  │    epsilon: 1.0,                                 │   │
    │  │    pii_rules: ['paths', 'ips', 'keys'],         │   │
    │  │  });                                             │   │
    │  │                                                  │   │
    │  │  // Add learning data                           │   │
    │  │  exporter.add_transfer_prior(priorBytes);        │   │
    │  │  exporter.add_policy_kernel(kernelBytes);        │   │
    │  │                                                  │   │
    │  │  // Build export (PII strip + noise + sign)     │   │
    │  │  const rvfBytes = exporter.build();              │   │
    │  │                                                  │   │
    │  │  // Upload                                      │   │
    │  │  await fetch('/federation/submit', {             │   │
    │  │    method: 'POST',                               │   │
    │  │    body: rvfBytes,                               │   │
    │  │  });                                             │   │
    │  │                                                  │   │
    │  └──────────────────────────────────────────────────┘   │
    │                                                        │
    │  rvf-fed-wasm (wasm32-unknown-unknown)                 │
    │  ┌──────────────────────────────────────────────────┐   │
    │  │  - rvf-pii-strip (no_std core)                   │   │
    │  │  - rvf-diff-privacy (no_std core)                │   │
    │  │  - rvf-types (no_std)                            │   │
    │  │  - rvf-wire                                      │   │
    │  │  - rvf-crypto (ed25519)                          │   │
    │  │  - wasm-bindgen FFI layer                        │   │
    │  │                                                  │   │
    │  │  Size target: < 500 KB gzipped                   │   │
    │  └──────────────────────────────────────────────────┘   │
    │                                                        │
    └────────────────────────────────────────────────────────┘
```

## MCP Server Interface (mcp-federation)

AI agents (Claude Code, claude-flow, agentic-flow) interact with federation
through MCP tools over stdio JSON-RPC 2.0 — same pattern as `mcp-gate`.

```
    AI AGENT (Claude Code, claude-flow, etc.)
    ┌────────────────────────────────────────────────────────┐
    │                                                        │
    │  tools/call: "federation_export"                       │
    │  tools/call: "federation_import"                       │
    │  tools/call: "federation_status"                       │
    │  tools/call: "federation_search"                       │
    │  tools/call: "federation_budget"                       │
    │  resources/read: "federation://domains"                │
    │  resources/read: "federation://contributors"           │
    │                                                        │
    └──────────────────────┬─────────────────────────────────┘
                           │ stdio (JSON-RPC 2.0)
                           │
    ┌──────────────────────┴─────────────────────────────────┐
    │  mcp-federation (MCP Server)                            │
    │                                                        │
    │  ┌──────────────────────────────────────────────────┐   │
    │  │  TOOLS                                           │   │
    │  │                                                  │   │
    │  │  federation_export                               │   │
    │  │    Input:  { domains?, epsilon?, pii_rules? }    │   │
    │  │    Output: { export_id, segments, bytes, uri }   │   │
    │  │    Action: Extract → PII strip → noise → sign    │   │
    │  │            → upload to GCS → publish event       │   │
    │  │                                                  │   │
    │  │  federation_import                               │   │
    │  │    Input:  { source?, domain?, auto_merge? }     │   │
    │  │    Output: { imported, domains, acceleration }   │   │
    │  │    Action: Pull from GCS → validate → merge      │   │
    │  │                                                  │   │
    │  │  federation_status                               │   │
    │  │    Input:  {}                                    │   │
    │  │    Output: { budget, exports, imports, domains } │   │
    │  │    Action: Read local state + Firestore          │   │
    │  │                                                  │   │
    │  │  federation_search                               │   │
    │  │    Input:  { domain?, min_quality?, limit? }     │   │
    │  │    Output: { results: [{contributor, quality}] } │   │
    │  │    Action: Query Firestore manifest registry     │   │
    │  │                                                  │   │
    │  │  federation_budget                               │   │
    │  │    Input:  {}                                    │   │
    │  │    Output: { epsilon_used, epsilon_remaining,    │   │
    │  │             exports_count, reset_date }          │   │
    │  │    Action: Read PrivacyBudget from Firestore     │   │
    │  │                                                  │   │
    │  │  federation_aggregate                            │   │
    │  │    Input:  { domain, method?, min_participants? }│   │
    │  │    Output: { round_id, participants, result_uri }│   │
    │  │    Action: Trigger server-side aggregation round  │   │
    │  │                                                  │   │
    │  └──────────────────────────────────────────────────┘   │
    │                                                        │
    │  ┌──────────────────────────────────────────────────┐   │
    │  │  RESOURCES (read-only)                           │   │
    │  │                                                  │   │
    │  │  federation://domains                            │   │
    │  │    → List of federated domains with stats        │   │
    │  │                                                  │   │
    │  │  federation://contributors                       │   │
    │  │    → Pseudonymized contributor list + reputation  │   │
    │  │                                                  │   │
    │  │  federation://rounds/{round_id}                  │   │
    │  │    → Aggregation round details                   │   │
    │  │                                                  │   │
    │  │  federation://budget                             │   │
    │  │    → Privacy budget for current contributor      │   │
    │  │                                                  │   │
    │  └──────────────────────────────────────────────────┘   │
    │                                                        │
    │  Internals:                                            │
    │  ┌────────────────┐ ┌───────────┐ ┌─────────────────┐  │
    │  │ rvf-federation │ │rvf-gcloud │ │rvf-pii-strip    │  │
    │  │ ExportBuilder  │ │ GcsClient │ │ + rvf-diff-priv │  │
    │  │ ImportValidator│ │ PubSubCli │ │                  │  │
    │  │ VersionMerger  │ │ Firestore │ │                  │  │
    │  └────────────────┘ └───────────┘ └─────────────────┘  │
    │                                                        │
    └────────────────────────────────────────────────────────┘

    REGISTRATION:
    $ claude mcp add mcp-federation -- cargo run -p mcp-federation
    OR
    $ claude mcp add mcp-federation -- npx @ruvector/mcp-federation
```

### MCP Tool Schemas

```json
// federation_export
{
  "name": "federation_export",
  "description": "Export local learning as a federated RVF package. Strips PII, applies differential privacy, signs with Ed25519, and uploads to the federation hub.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "domains": {
        "type": "array", "items": { "type": "string" },
        "description": "Domains to export (all if omitted)"
      },
      "epsilon": {
        "type": "number", "default": 1.0,
        "description": "Differential privacy epsilon (lower = more private)"
      },
      "pii_rules": {
        "type": "array", "items": { "type": "string" },
        "default": ["paths", "ips", "emails", "keys", "env"],
        "description": "PII detection rule sets to apply"
      },
      "min_quality": {
        "type": "number", "default": 0.5,
        "description": "Minimum trajectory quality threshold (0-1)"
      },
      "dry_run": {
        "type": "boolean", "default": false,
        "description": "If true, build export but don't upload"
      }
    }
  }
}

// federation_import
{
  "name": "federation_import",
  "description": "Import federated learning from the hub. Validates signatures, checks privacy proofs, and merges into local SONA/DomainExpansion engines.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "domain": {
        "type": "string",
        "description": "Domain to import (required)"
      },
      "source": {
        "type": "string",
        "description": "Specific contributor pseudonym (latest aggregate if omitted)"
      },
      "auto_merge": {
        "type": "boolean", "default": true,
        "description": "Automatically merge into local engines"
      },
      "max_epsilon": {
        "type": "number", "default": 5.0,
        "description": "Reject imports with epsilon higher than this"
      }
    },
    "required": ["domain"]
  }
}

// federation_status
{
  "name": "federation_status",
  "description": "Get federation status: privacy budget, recent exports/imports, available domains, and contributor reputation.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}

// federation_search
{
  "name": "federation_search",
  "description": "Search the federation registry for available learning packages by domain, quality, or recency.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "domain": { "type": "string" },
      "min_quality": { "type": "number", "default": 0.5 },
      "limit": { "type": "integer", "default": 10 },
      "since_hours": { "type": "integer", "description": "Only results newer than N hours" }
    }
  }
}
```

## REST API Interface (rvf-fed-server)

The federation server exposes a REST API for programmatic access beyond MCP.
Built on axum (same as `ruvector-server`), deployed as the Cloud Run service.

```
    REST API ENDPOINTS
    ==================

    Base URL: https://federation.ruvector.dev/v1

    ┌──────────────────────────────────────────────────────────────┐
    │                                                              │
    │  EXPORT / IMPORT                                             │
    │  ─────────────                                               │
    │  POST   /v1/exports                                          │
    │         Content-Type: application/x-ruvector-format           │
    │         Authorization: Bearer <token>                        │
    │         Body: raw RVF bytes (federated export)               │
    │         → 201 { export_id, uri, segments }                   │
    │                                                              │
    │  GET    /v1/exports/{export_id}                               │
    │         → 200 RVF bytes                                      │
    │                                                              │
    │  GET    /v1/exports?domain=X&since=ISO&limit=N               │
    │         → 200 [{ export_id, contributor, domain, quality }]  │
    │                                                              │
    │  DELETE /v1/exports/{export_id}                               │
    │         → 204 (contributor can delete own exports)            │
    │                                                              │
    │  AGGREGATION                                                 │
    │  ───────────                                                 │
    │  POST   /v1/aggregates                                       │
    │         Body: { domain, method, min_participants }           │
    │         → 202 { round_id, status: "pending" }                │
    │                                                              │
    │  GET    /v1/aggregates/{round_id}                             │
    │         → 200 { round_id, status, participants, result_uri } │
    │                                                              │
    │  GET    /v1/aggregates/latest?domain=X                        │
    │         → 200 RVF bytes (latest aggregate for domain)        │
    │                                                              │
    │  REGISTRY                                                    │
    │  ────────                                                    │
    │  GET    /v1/domains                                           │
    │         → 200 [{ domain_id, contributors, exports, latest }] │
    │                                                              │
    │  GET    /v1/contributors/{pseudonym}                          │
    │         → 200 { reputation, exports, domains, budget }       │
    │                                                              │
    │  GET    /v1/contributors/{pseudonym}/budget                   │
    │         → 200 { epsilon_used, epsilon_remaining, reset }     │
    │                                                              │
    │  HEALTH / META                                               │
    │  ────────────                                                │
    │  GET    /v1/health                                            │
    │         → 200 { status, version, uptime }                    │
    │                                                              │
    │  GET    /v1/metrics                                           │
    │         → 200 Prometheus text format                         │
    │                                                              │
    │  EVENTS (Server-Sent Events)                                 │
    │  ───────────────────────────                                 │
    │  GET    /v1/events?domain=X                                   │
    │         → SSE stream: new_export, aggregation_complete,      │
    │           import_available                                   │
    │                                                              │
    └──────────────────────────────────────────────────────────────┘

    AUTHENTICATION
    ==============

    Two modes:
    1. API Key: Authorization: Bearer <api-key>
       - Issued via CLI: rvf-cli federation register
       - Stored in Firestore contributors collection
       - Hashed (SHAKE-256) server-side, never stored in plaintext

    2. Ed25519 Signed Request:
       - X-Federation-Signature: <base64(Ed25519.sign(body))>
       - X-Federation-PublicKey: <base64(pubkey)>
       - Used for export submission (proves contributor identity)
       - Preferred for production deployments

    RATE LIMITS
    ===========

    | Endpoint           | Rate Limit         | Burst |
    |--------------------|--------------------|-------|
    | POST /exports      | 10/hour/contributor| 3     |
    | GET  /exports      | 100/min            | 20    |
    | POST /aggregates   | 1/hour/domain      | 1     |
    | GET  /events       | 5 concurrent       | -     |
    | GET  /domains      | 60/min             | 10    |
```

### API Data Flow

```
    CLI / SDK USAGE
    ===============

    # Export via CLI
    $ rvf-cli federation export --domain learning --epsilon 1.0
    Extracting learning from SONA + DomainExpansion...
    PII stripping: 3 paths, 1 email redacted
    Differential privacy: epsilon=1.0, delta=1e-5
    Signing with Ed25519...
    Uploading to federation hub...
    ✓ Export complete: export_id=fed-a7b3c9

    # Import via CLI
    $ rvf-cli federation import --domain learning
    Fetching latest aggregate for domain 'learning'...
    Validating: signature ✓, witness chain ✓, privacy proof ✓
    Merging into local engines...
    ✓ Import complete: +12% acceleration on domain 'learning'

    # Status
    $ rvf-cli federation status
    Privacy Budget:  3.2 / 10.0 epsilon (68% remaining)
    Exports:         7 (last: 2h ago)
    Imports:         12
    Reputation:      0.82
    Domains:         [learning, optimization, inference]

    # Search
    $ rvf-cli federation search --domain inference --min-quality 0.7
    ┌─────────────┬──────────┬─────────┬──────────┐
    │ Contributor  │ Quality  │ Epsilon │ Age      │
    ├─────────────┼──────────┼─────────┼──────────┤
    │ a7b3...c9f2 │ 0.91     │ 1.0     │ 3h       │
    │ d4e5...8a1b │ 0.85     │ 0.5     │ 12h      │
    │ f6g7...2c3d │ 0.73     │ 2.0     │ 1d       │
    └─────────────┴──────────┴─────────┴──────────┘
```

### SDK Usage (Rust)

```rust
use rvf_federation::client::FederationClient;

// Programmatic export
let client = FederationClient::new("https://federation.ruvector.dev/v1", api_key)?;

let export = client.export()
    .domains(&["learning", "optimization"])
    .epsilon(1.0)
    .pii_rules(&["paths", "ips", "keys"])
    .build_and_upload()
    .await?;

println!("Exported: {}", export.export_id);

// Programmatic import
let import = client.import("learning")
    .auto_merge(true)
    .max_epsilon(5.0)
    .execute()
    .await?;

println!("Acceleration: {}%", import.acceleration_percent);

// SSE event stream
let mut stream = client.events("learning").await?;
while let Some(event) = stream.next().await {
    match event {
        FederationEvent::NewExport(e) => println!("New: {}", e.export_id),
        FederationEvent::AggregateReady(a) => println!("Aggregate: {}", a.round_id),
    }
}
```

### SDK Usage (TypeScript / npm)

```typescript
import { FederationClient } from '@ruvector/rvf-federation';

const client = new FederationClient({
  endpoint: 'https://federation.ruvector.dev/v1',
  apiKey: process.env.RVF_FEDERATION_KEY,
});

// Export
const result = await client.export({
  domains: ['learning'],
  epsilon: 1.0,
});

// Import
const imported = await client.import({
  domain: 'learning',
  autoMerge: true,
});

// Stream events
for await (const event of client.events('learning')) {
  console.log(event.type, event.data);
}
```

## Monitoring and Observability

```
    METRICS (exported to Cloud Monitoring)
    ======================================

    federation_exports_total          counter    Total exports submitted
    federation_imports_total          counter    Total imports processed
    federation_rejections_total       counter    Imports rejected (by reason)
    federation_aggregate_rounds       counter    Aggregation rounds completed
    federation_aggregate_latency_ms   histogram  Aggregation round latency
    federation_privacy_budget_used    gauge      Per-contributor epsilon spent
    federation_pii_detections_total   counter    PII detections by type
    federation_transfer_acceleration  gauge      Acceleration factor from imports
    federation_contributor_reputation gauge      Per-contributor reputation score
    federation_gcs_bytes_stored       gauge      Total bytes in GCS
    federation_pubsub_messages_total  counter    Pub/Sub messages sent/received

    ALERTS
    ======

    privacy_budget_warning:
      condition: federation_privacy_budget_used > 0.8 * budget_limit
      severity: warning
      action: notify contributor, suggest reducing export frequency

    high_rejection_rate:
      condition: rate(federation_rejections_total[5m]) > 0.5 * rate(federation_imports_total[5m])
      severity: warning
      action: investigate potential poisoning attack

    aggregation_stall:
      condition: time_since_last_aggregation > 1 hour AND pending_submissions > min_participants
      severity: critical
      action: force aggregation round

    gcs_cost_alert:
      condition: federation_gcs_bytes_stored > 10 GiB
      severity: warning
      action: trigger lifecycle cleanup
```

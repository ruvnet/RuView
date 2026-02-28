# ADR-044: ruvector-postgres v0.3 Extension Upgrade

## Status

Accepted — Implementation in progress

## Context

ruvector-postgres v2.0.4 has 101 SQL functions across 20+ modules. The workspace contains 5 mature crates (`ruvector-solver`, `ruvector-math`, `ruvector-attention`, `sona`, `ruvector-domain-expansion`) with production-quality algorithms not yet exposed as SQL functions. v0.3 integrates these crates without performance regression. All new functionality is feature-gated.

**Current Docker build features**: `pg17,graph-complete,gated-transformer`

## Decision

Add ~42 new SQL functions in 6 new feature-gated modules, integrating 4 workspace crates. Bump extension version to `0.3.0`. Update Docker build to include Tier 1+2 features.

## New Feature Flags

```toml
solver = ["dep:ruvector-solver"]
math-distances = ["dep:ruvector-math"]
tda = ["dep:ruvector-math"]
attention-extended = ["attention", "dep:ruvector-attention"]
sona-learning = ["dep:ruvector-sona"]
domain-expansion = ["dep:ruvector-domain-expansion"]
analytics-complete = ["solver", "math-distances", "tda"]
ai-complete-v3 = ["ai-complete", "attention-extended", "sona-learning"]
all-features-v3 = ["all-features", "analytics-complete", "ai-complete-v3", "domain-expansion"]
```

## New Modules

| Phase | Module | Feature Flag | Functions | Dependency |
|-------|--------|-------------|-----------|------------|
| 1 | `solver` | `solver` | 11 | `ruvector-solver` |
| 2 | `math` | `math-distances` | 12 | `ruvector-math` |
| 3 | `tda` | `tda` | 7 | `ruvector-math` |
| 4 | `attention` (extended) | `attention-extended` | 7 | `ruvector-attention` |
| 5 | `sona` | `sona-learning` | 4 | `sona` |
| 5 | `domain_expansion` | `domain-expansion` | 1 | `ruvector-domain-expansion` |

## New Functions Summary

### Solver (11)
- `ruvector_pagerank`, `ruvector_pagerank_personalized`, `ruvector_pagerank_multi_seed`
- `ruvector_solve_sparse`, `ruvector_solve_laplacian`, `ruvector_effective_resistance`
- `ruvector_graph_pagerank`, `ruvector_solver_info`, `ruvector_matrix_analyze`
- `ruvector_conjugate_gradient`, `ruvector_graph_centrality`

### Math Distances & Spectral (12)
- `ruvector_wasserstein_distance`, `ruvector_sinkhorn_distance`, `ruvector_sliced_wasserstein`
- `ruvector_kl_divergence`, `ruvector_jensen_shannon`, `ruvector_fisher_information`
- `ruvector_spectral_cluster`, `ruvector_chebyshev_filter`, `ruvector_graph_diffusion`
- `ruvector_product_manifold_distance`, `ruvector_spherical_distance`, `ruvector_gromov_wasserstein`

### TDA (7)
- `ruvector_persistent_homology`, `ruvector_betti_numbers`, `ruvector_bottleneck_distance`
- `ruvector_persistence_wasserstein`, `ruvector_topological_summary`
- `ruvector_embedding_drift`, `ruvector_vietoris_rips`

### Extended Attention (7)
- `ruvector_linear_attention`, `ruvector_sliding_window_attention`, `ruvector_cross_attention`
- `ruvector_sparse_attention`, `ruvector_moe_attention`, `ruvector_hyperbolic_attention`
- `ruvector_attention_benchmark`

### Sona & Domain Expansion (5)
- `ruvector_sona_learn`, `ruvector_sona_apply`, `ruvector_sona_ewc_status`, `ruvector_sona_stats`
- `ruvector_domain_transfer`

## Performance Targets

| Metric | Target | Method |
|--------|--------|--------|
| PageRank 10K nodes | < 50ms | Forward Push O(1/epsilon) |
| Wasserstein 1K dims | < 10ms | Sinkhorn |
| Spectral clustering 10K | < 200ms | Chebyshev K=20 |
| Persistent homology 500 pts | < 100ms | Vietoris-Rips |
| Linear attention 4K seq | < 2ms | O(n) complexity |
| Existing functions | No regression | Feature-gated isolation |

## Docker Build Change

```dockerfile
# Before:
--features pg${PG_VERSION},graph-complete,gated-transformer
# After:
--features pg${PG_VERSION},graph-complete,gated-transformer,analytics-complete,attention-extended
```

## Compatibility

- `ruvector-solver` and `ruvector-math` use workspace `thiserror = "2.0"` while ruvector-postgres uses `thiserror = "1.0"`. Errors are mapped at the boundary via `pgrx::error!()`. Both versions coexist via Cargo semver.
- All new functions are feature-gated, ensuring zero impact on existing builds.

## Verification

```sql
SELECT ruvector_version();
SELECT ruvector_pagerank('{"edges":[[0,1],[1,2],[2,0]]}'::jsonb);
SELECT ruvector_wasserstein_distance(ARRAY[0.5,0.5]::real[], ARRAY[0.3,0.7]::real[]);
SELECT ruvector_persistent_homology('[[1,0],[0,1],[-1,0],[0,-1]]'::jsonb, 1, 3.0);
SELECT ruvector_linear_attention(ARRAY[1,0,0,0]::real[], '[[1,0,0,0]]'::jsonb, '[[5,10]]'::jsonb);
SELECT ruvector_solver_info();
```

## Consequences

- Extension grows from ~101 to ~143 SQL functions
- Docker image size increases by ~5-10MB due to additional crate dependencies
- Build time increases by ~30-60s for full feature builds
- All new functionality is opt-in via feature flags

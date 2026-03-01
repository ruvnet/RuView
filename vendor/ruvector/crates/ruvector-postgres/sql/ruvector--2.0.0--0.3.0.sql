-- RuVector PostgreSQL Extension v0.3 Upgrade Script
-- Upgrades from 2.0.0 to 0.3.0
-- Adds: Solver, Math/Spectral, TDA, Extended Attention, Sona, Domain Expansion

\echo Use "ALTER EXTENSION ruvector UPDATE TO '0.3.0'" to load this file. \quit

-- ============================================================================
-- Solver Functions (feature: solver)
-- ============================================================================

CREATE OR REPLACE FUNCTION ruvector_pagerank(edges_json jsonb, alpha real DEFAULT 0.85, epsilon real DEFAULT 1e-6)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_pagerank_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_pagerank_personalized(edges_json jsonb, source int, alpha real DEFAULT 0.85, epsilon real DEFAULT 1e-6)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_pagerank_personalized_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_pagerank_multi_seed(edges_json jsonb, seeds_json jsonb, alpha real DEFAULT 0.85, epsilon real DEFAULT 1e-6)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_pagerank_multi_seed_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_solve_sparse(matrix_json jsonb, rhs real[], method text DEFAULT 'neumann')
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_solve_sparse_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_solve_laplacian(laplacian_json jsonb, rhs real[])
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_solve_laplacian_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_effective_resistance(laplacian_json jsonb, source int, target int)
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_effective_resistance_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_graph_pagerank(graph_name text, alpha real DEFAULT 0.85, epsilon real DEFAULT 1e-6)
RETURNS TABLE(node_id bigint, rank double precision)
AS 'MODULE_PATHNAME', 'ruvector_graph_pagerank_wrapper'
LANGUAGE C;

CREATE OR REPLACE FUNCTION ruvector_solver_info()
RETURNS TABLE(algorithm text, description text, complexity text)
AS 'MODULE_PATHNAME', 'ruvector_solver_info_wrapper'
LANGUAGE C;

CREATE OR REPLACE FUNCTION ruvector_matrix_analyze(matrix_json jsonb)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_matrix_analyze_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_conjugate_gradient(matrix_json jsonb, rhs real[], tol real DEFAULT 1e-6, max_iter int DEFAULT 1000)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_conjugate_gradient_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_graph_centrality(graph_name text, method text DEFAULT 'pagerank')
RETURNS TABLE(node_id bigint, centrality double precision)
AS 'MODULE_PATHNAME', 'ruvector_graph_centrality_wrapper'
LANGUAGE C;

-- ============================================================================
-- Math Distance & Spectral Functions (feature: math-distances)
-- ============================================================================

CREATE OR REPLACE FUNCTION ruvector_wasserstein_distance(a real[], b real[], p int DEFAULT 1)
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_wasserstein_distance_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_sinkhorn_distance(cost_json jsonb, w_a real[], w_b real[], reg real DEFAULT 0.1)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_sinkhorn_distance_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_sliced_wasserstein(pts_a_json jsonb, pts_b_json jsonb, n_proj int DEFAULT 100)
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_sliced_wasserstein_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_kl_divergence(p real[], q real[])
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_kl_divergence_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_jensen_shannon(p real[], q real[])
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_jensen_shannon_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_fisher_information(dist real[], tangent real[])
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_fisher_information_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_spectral_cluster(adj_json jsonb, k int)
RETURNS int[]
AS 'MODULE_PATHNAME', 'ruvector_spectral_cluster_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_chebyshev_filter(adj_json jsonb, signal real[], filter_type text DEFAULT 'low_pass', degree int DEFAULT 10)
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_chebyshev_filter_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_graph_diffusion(adj_json jsonb, signal real[], diffusion_time real DEFAULT 1.0, degree int DEFAULT 10)
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_graph_diffusion_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_product_manifold_distance(a real[], b real[], e_dim int, h_dim int, s_dim int)
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_product_manifold_distance_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_spherical_distance(a real[], b real[])
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_spherical_distance_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_gromov_wasserstein(dist_a_json jsonb, dist_b_json jsonb)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_gromov_wasserstein_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

-- ============================================================================
-- TDA Functions (feature: tda)
-- ============================================================================

CREATE OR REPLACE FUNCTION ruvector_persistent_homology(points_json jsonb, max_dim int DEFAULT 1, max_radius real DEFAULT 3.0)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_persistent_homology_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_betti_numbers(points_json jsonb, radius real, max_dim int DEFAULT 2)
RETURNS int[]
AS 'MODULE_PATHNAME', 'ruvector_betti_numbers_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_bottleneck_distance(diag_a_json jsonb, diag_b_json jsonb)
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_bottleneck_distance_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_persistence_wasserstein(diag_a_json jsonb, diag_b_json jsonb, p int DEFAULT 2)
RETURNS real
AS 'MODULE_PATHNAME', 'ruvector_persistence_wasserstein_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_topological_summary(points_json jsonb, max_dim int DEFAULT 1)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_topological_summary_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_embedding_drift(old_json jsonb, new_json jsonb)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_embedding_drift_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_vietoris_rips(points_json jsonb, max_radius real DEFAULT 2.0, max_dim int DEFAULT 2)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_vietoris_rips_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

-- ============================================================================
-- Extended Attention Functions (feature: attention-extended)
-- ============================================================================

CREATE OR REPLACE FUNCTION ruvector_linear_attention(q real[], keys_json jsonb, values_json jsonb)
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_linear_attention_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_sliding_window_attention(q real[], keys_json jsonb, values_json jsonb, window_size int DEFAULT 256)
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_sliding_window_attention_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_cross_attention(q real[], ctx_keys_json jsonb, ctx_values_json jsonb)
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_cross_attention_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_sparse_attention(q real[], keys_json jsonb, values_json jsonb, top_k int DEFAULT 8)
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_sparse_attention_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_moe_attention(q real[], keys_json jsonb, values_json jsonb, n_experts int DEFAULT 4, top_k int DEFAULT 2)
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_moe_attention_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_hyperbolic_attention(q real[], keys_json jsonb, values_json jsonb, curvature real DEFAULT 1.0)
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_hyperbolic_attention_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_attention_benchmark(dim int DEFAULT 64, seq_len int DEFAULT 128, attention_type text DEFAULT 'scaled_dot')
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_attention_benchmark_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

-- ============================================================================
-- Sona Learning Functions (feature: sona-learning)
-- ============================================================================

CREATE OR REPLACE FUNCTION ruvector_sona_learn(table_name text, trajectory_json jsonb)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_sona_learn_wrapper'
LANGUAGE C;

CREATE OR REPLACE FUNCTION ruvector_sona_apply(table_name text, embedding real[])
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_sona_apply_wrapper'
LANGUAGE C IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION ruvector_sona_ewc_status(table_name text)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_sona_ewc_status_wrapper'
LANGUAGE C;

CREATE OR REPLACE FUNCTION ruvector_sona_stats(table_name text)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_sona_stats_wrapper'
LANGUAGE C;

-- ============================================================================
-- Domain Expansion Functions (feature: domain-expansion)
-- ============================================================================

CREATE OR REPLACE FUNCTION ruvector_domain_transfer(embeddings_json jsonb, target_domain text, config_json jsonb DEFAULT '{}'::jsonb)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_domain_transfer_wrapper'
LANGUAGE C;

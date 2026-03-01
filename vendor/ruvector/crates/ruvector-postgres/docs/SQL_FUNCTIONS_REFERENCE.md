# RuVector-Postgres SQL Functions Reference

Complete reference table of all 53+ SQL functions with descriptions and usage examples.

## Quick Reference Table

| Category | Function | Description | Example |
|----------|----------|-------------|---------|
| **Core** | `ruvector_version()` | Get extension version | `SELECT ruvector_version();` |
| **Core** | `ruvector_simd_info()` | Get SIMD capabilities | `SELECT ruvector_simd_info();` |

### Distance Functions (5)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_l2_distance(a, b)` | Euclidean (L2) distance | `SELECT ruvector_l2_distance('[1,2,3]', '[4,5,6]');` |
| `ruvector_cosine_distance(a, b)` | Cosine distance (1 - similarity) | `SELECT ruvector_cosine_distance('[1,0]', '[0,1]');` |
| `ruvector_inner_product(a, b)` | Dot product distance | `SELECT ruvector_inner_product('[1,2]', '[3,4]');` |
| `ruvector_l1_distance(a, b)` | Manhattan (L1) distance | `SELECT ruvector_l1_distance('[1,2]', '[3,4]');` |
| `ruvector_hamming_distance(a, b)` | Hamming distance for binary | `SELECT ruvector_hamming_distance(a, b);` |

### Vector Operations (5)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_normalize(v)` | Normalize to unit length | `SELECT ruvector_normalize('[3,4]');` → `[0.6,0.8]` |
| `ruvector_norm(v)` | Get L2 norm (magnitude) | `SELECT ruvector_norm('[3,4]');` → `5.0` |
| `ruvector_add(a, b)` | Add two vectors | `SELECT ruvector_add('[1,2]', '[3,4]');` → `[4,6]` |
| `ruvector_sub(a, b)` | Subtract vectors | `SELECT ruvector_sub('[5,6]', '[1,2]');` → `[4,4]` |
| `ruvector_scalar_mul(v, s)` | Multiply by scalar | `SELECT ruvector_scalar_mul('[1,2]', 2.0);` → `[2,4]` |

### Hyperbolic Geometry (8)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_poincare_distance(a, b, c)` | Poincaré ball distance | `SELECT ruvector_poincare_distance(a, b, -1.0);` |
| `ruvector_lorentz_distance(a, b, c)` | Lorentz hyperboloid distance | `SELECT ruvector_lorentz_distance(a, b, -1.0);` |
| `ruvector_mobius_add(a, b, c)` | Möbius addition (hyperbolic translation) | `SELECT ruvector_mobius_add(a, b, -1.0);` |
| `ruvector_exp_map(base, tangent, c)` | Exponential map (tangent → manifold) | `SELECT ruvector_exp_map(base, tangent, -1.0);` |
| `ruvector_log_map(base, target, c)` | Logarithmic map (manifold → tangent) | `SELECT ruvector_log_map(base, target, -1.0);` |
| `ruvector_poincare_to_lorentz(v, c)` | Convert Poincaré to Lorentz | `SELECT ruvector_poincare_to_lorentz(v, -1.0);` |
| `ruvector_lorentz_to_poincare(v, c)` | Convert Lorentz to Poincaré | `SELECT ruvector_lorentz_to_poincare(v, -1.0);` |
| `ruvector_minkowski_dot(a, b)` | Minkowski inner product | `SELECT ruvector_minkowski_dot(a, b);` |

### Sparse Vectors & BM25 (14)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_sparse_create(idx, vals, dim)` | Create sparse vector | `SELECT ruvector_sparse_create(ARRAY[0,5,10], ARRAY[0.5,0.3,0.2], 100);` |
| `ruvector_sparse_from_dense(v, thresh)` | Dense to sparse conversion | `SELECT ruvector_sparse_from_dense(dense_vec, 0.01);` |
| `ruvector_sparse_to_dense(sv)` | Sparse to dense conversion | `SELECT ruvector_sparse_to_dense(sparse_vec);` |
| `ruvector_sparse_dot(a, b)` | Sparse dot product | `SELECT ruvector_sparse_dot(sv1, sv2);` |
| `ruvector_sparse_cosine(a, b)` | Sparse cosine similarity | `SELECT ruvector_sparse_cosine(sv1, sv2);` |
| `ruvector_sparse_l2_distance(a, b)` | Sparse L2 distance | `SELECT ruvector_sparse_l2_distance(sv1, sv2);` |
| `ruvector_sparse_add(a, b)` | Add sparse vectors | `SELECT ruvector_sparse_add(sv1, sv2);` |
| `ruvector_sparse_scale(sv, s)` | Scale sparse vector | `SELECT ruvector_sparse_scale(sv, 2.0);` |
| `ruvector_sparse_normalize(sv)` | Normalize sparse vector | `SELECT ruvector_sparse_normalize(sv);` |
| `ruvector_sparse_topk(sv, k)` | Get top-k elements | `SELECT ruvector_sparse_topk(sv, 10);` |
| `ruvector_sparse_nnz(sv)` | Count non-zero elements | `SELECT ruvector_sparse_nnz(sv);` |
| `ruvector_bm25_score(...)` | BM25 relevance score | `SELECT ruvector_bm25_score(terms, doc_freqs, doc_len, avg_len, total);` |
| `ruvector_tf_idf(tf, df, total)` | TF-IDF score | `SELECT ruvector_tf_idf(term_freq, doc_freq, total_docs);` |
| `ruvector_sparse_intersection(a, b)` | Intersection of sparse vectors | `SELECT ruvector_sparse_intersection(sv1, sv2);` |

### Attention Mechanisms (10 primary + 29 variants)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_attention_scaled_dot(q, k, v)` | Scaled dot-product attention | `SELECT ruvector_attention_scaled_dot(query, keys, values);` |
| `ruvector_attention_multi_head(q, k, v, h)` | Multi-head attention | `SELECT ruvector_attention_multi_head(q, k, v, 8);` |
| `ruvector_attention_flash(q, k, v, blk)` | Flash attention (memory efficient) | `SELECT ruvector_attention_flash(q, k, v, 64);` |
| `ruvector_attention_sparse(q, k, v, pat)` | Sparse attention | `SELECT ruvector_attention_sparse(q, k, v, pattern);` |
| `ruvector_attention_linear(q, k, v)` | Linear attention O(n) | `SELECT ruvector_attention_linear(q, k, v);` |
| `ruvector_attention_causal(q, k, v)` | Causal/masked attention | `SELECT ruvector_attention_causal(q, k, v);` |
| `ruvector_attention_cross(q, ck, cv)` | Cross attention | `SELECT ruvector_attention_cross(query, ctx_keys, ctx_values);` |
| `ruvector_attention_self(input, heads)` | Self attention | `SELECT ruvector_attention_self(input, 8);` |
| `ruvector_attention_local(q, k, v, win)` | Local/sliding window attention | `SELECT ruvector_attention_local(q, k, v, 256);` |
| `ruvector_attention_relative(q, k, v)` | Relative position attention | `SELECT ruvector_attention_relative(q, k, v);` |

**Additional Attention Types:** `performer`, `linformer`, `bigbird`, `longformer`, `reformer`, `synthesizer`, `routing`, `mixture_of_experts`, `alibi`, `rope`, `xpos`, `grouped_query`, `sliding_window`, `dilated`, `axial`, `product_key`, `hash_based`, `random_feature`, `nystrom`, `clustered`, `sinkhorn`, `entmax`, `adaptive_span`, `compressive`, `feedback`, `talking_heads`, `realformer`, `rezero`, `fixup`

### Graph Neural Networks (5)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_gnn_gcn_layer(feat, adj, w)` | Graph Convolutional Network | `SELECT ruvector_gnn_gcn_layer(features, adjacency, weights);` |
| `ruvector_gnn_graphsage_layer(feat, neigh, w)` | GraphSAGE (inductive) | `SELECT ruvector_gnn_graphsage_layer(feat, neighbors, weights);` |
| `ruvector_gnn_gat_layer(feat, adj, attn)` | Graph Attention Network | `SELECT ruvector_gnn_gat_layer(feat, adj, attention_weights);` |
| `ruvector_gnn_message_pass(feat, edges, w)` | Message passing | `SELECT ruvector_gnn_message_pass(node_feat, edge_idx, edge_w);` |
| `ruvector_gnn_aggregate(msg, type)` | Aggregate messages | `SELECT ruvector_gnn_aggregate(messages, 'mean');` |

### Agent Routing - Tiny Dancer (11)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_route_query(embed, agents)` | Route query to best agent | `SELECT ruvector_route_query(query_embed, agent_registry);` |
| `ruvector_route_with_context(q, ctx, agents)` | Route with context | `SELECT ruvector_route_with_context(query, context, agents);` |
| `ruvector_multi_agent_route(q, agents, k)` | Multi-agent routing | `SELECT ruvector_multi_agent_route(query, agents, 3);` |
| `ruvector_register_agent(name, caps, embed)` | Register new agent | `SELECT ruvector_register_agent('gpt4', caps, embedding);` |
| `ruvector_update_agent_performance(id, metrics)` | Update agent metrics | `SELECT ruvector_update_agent_performance(agent_id, metrics);` |
| `ruvector_get_routing_stats()` | Get routing statistics | `SELECT * FROM ruvector_get_routing_stats();` |
| `ruvector_calculate_agent_affinity(q, agent)` | Calculate query-agent affinity | `SELECT ruvector_calculate_agent_affinity(query, agent);` |
| `ruvector_select_best_agent(q, agents)` | Select best agent | `SELECT ruvector_select_best_agent(query, agent_list);` |
| `ruvector_adaptive_route(q, ctx, lr)` | Adaptive routing with learning | `SELECT ruvector_adaptive_route(query, context, 0.01);` |
| `ruvector_fastgrnn_forward(in, hidden, w)` | FastGRNN acceleration | `SELECT ruvector_fastgrnn_forward(input, hidden, weights);` |
| `ruvector_get_agent_embeddings(agents)` | Get agent embeddings | `SELECT ruvector_get_agent_embeddings(agent_ids);` |

### Self-Learning / ReasoningBank (7)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_record_trajectory(in, out, ok, ctx)` | Record learning trajectory | `SELECT ruvector_record_trajectory(input, output, true, ctx);` |
| `ruvector_get_verdict(traj_id)` | Get verdict on trajectory | `SELECT ruvector_get_verdict(trajectory_id);` |
| `ruvector_distill_memory(trajs, ratio)` | Distill memory (compress) | `SELECT ruvector_distill_memory(trajectories, 0.5);` |
| `ruvector_adaptive_search(q, ctx, ef)` | Adaptive search with learning | `SELECT ruvector_adaptive_search(query, context, 100);` |
| `ruvector_learning_feedback(id, scores)` | Provide learning feedback | `SELECT ruvector_learning_feedback(search_id, scores);` |
| `ruvector_get_learning_patterns(ctx)` | Get learned patterns | `SELECT * FROM ruvector_get_learning_patterns(context);` |
| `ruvector_optimize_search_params(type, hist)` | Optimize search parameters | `SELECT ruvector_optimize_search_params('semantic', history);` |

### Graph Storage & Cypher (8)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_graph_create_node(labels, props, embed)` | Create graph node | `SELECT ruvector_graph_create_node('Person', '{"name":"Alice"}', embed);` |
| `ruvector_graph_create_edge(from, to, type, props)` | Create graph edge | `SELECT ruvector_graph_create_edge(1, 2, 'KNOWS', '{}');` |
| `ruvector_graph_get_neighbors(node, type, depth)` | Get node neighbors | `SELECT * FROM ruvector_graph_get_neighbors(1, 'KNOWS', 2);` |
| `ruvector_graph_shortest_path(start, end)` | Find shortest path | `SELECT ruvector_graph_shortest_path(1, 10);` |
| `ruvector_graph_pagerank(edges, damp, iters)` | Compute PageRank | `SELECT * FROM ruvector_graph_pagerank('edges', 0.85, 20);` |
| `ruvector_cypher_query(query)` | Execute Cypher query | `SELECT * FROM ruvector_cypher_query('MATCH (n) RETURN n');` |
| `ruvector_graph_traverse(start, dir, depth)` | Traverse graph | `SELECT * FROM ruvector_graph_traverse(1, 'outgoing', 3);` |
| `ruvector_graph_similarity_search(embed, type, k)` | Vector search on graph | `SELECT * FROM ruvector_graph_similarity_search(embed, 'Person', 10);` |

### Quantization (4)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_quantize_scalar(v)` | Scalar quantization (int8) | `SELECT ruvector_quantize_scalar(embedding);` |
| `ruvector_quantize_product(v, subvecs)` | Product quantization | `SELECT ruvector_quantize_product(embedding, 8);` |
| `ruvector_quantize_binary(v)` | Binary quantization | `SELECT ruvector_quantize_binary(embedding);` |
| `ruvector_dequantize(qv)` | Dequantize vector | `SELECT ruvector_dequantize(quantized_vec);` |

### Index Management (3)

| Function | Description | Usage |
|----------|-------------|-------|
| `ruvector_index_stats(name)` | Get index statistics | `SELECT * FROM ruvector_index_stats('idx_name');` |
| `ruvector_index_maintenance(name)` | Perform index maintenance | `SELECT ruvector_index_maintenance('idx_name');` |
| `ruvector_index_rebuild(name)` | Rebuild index | `SELECT ruvector_index_rebuild('idx_name');` |

## Operators Quick Reference

| Operator | Metric | Description | Example |
|----------|--------|-------------|---------|
| `<->` | L2 | Euclidean distance | `ORDER BY embedding <-> query` |
| `<=>` | Cosine | Cosine distance | `ORDER BY embedding <=> query` |
| `<#>` | IP | Inner product (negative) | `ORDER BY embedding <#> query` |
| `<+>` | L1 | Manhattan distance | `ORDER BY embedding <+> query` |

## Data Types

| Type | Description | Storage | Max Dimensions |
|------|-------------|---------|----------------|
| `ruvector(n)` | Dense float32 vector | 8 + 4×n bytes | 16,000 |
| `halfvec(n)` | Dense float16 vector | 8 + 2×n bytes | 16,000 |
| `sparsevec(n)` | Sparse vector | 12 + 8×nnz bytes | 1,000,000 |

## Common Usage Patterns

### Semantic Search

```sql
SELECT content, embedding <=> $query AS distance
FROM documents
ORDER BY distance
LIMIT 10;
```

### Hybrid Search (Vector + BM25)

```sql
SELECT content,
  0.7 * (1.0 / (1.0 + embedding <-> $vec)) +
  0.3 * ruvector_bm25_score(terms, freqs, len, avg_len, total) AS score
FROM documents
ORDER BY score DESC LIMIT 10;
```

### Hierarchical Search with Hyperbolic

```sql
SELECT name, ruvector_poincare_distance(embedding, $query, -1.0) AS dist
FROM taxonomy
ORDER BY dist LIMIT 10;
```

### Agent Routing

```sql
SELECT ruvector_route_query($user_query_embedding,
  (SELECT array_agg(row(name, capabilities)) FROM agents)
) AS best_agent;
```

### Graph + Vector Search

```sql
SELECT * FROM ruvector_graph_similarity_search($embedding, 'Document', 10);
```

## See Also

- [API.md](./API.md) - Detailed API documentation
- [ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture
- [README.md](../README.md) - Getting started guide

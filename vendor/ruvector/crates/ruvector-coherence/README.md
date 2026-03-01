# ruvector-coherence

[![Crates.io](https://img.shields.io/crates/v/ruvector-coherence.svg)](https://crates.io/crates/ruvector-coherence)
[![docs.rs](https://docs.rs/ruvector-coherence/badge.svg)](https://docs.rs/ruvector-coherence)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Quantitative coherence metrics for comparing attention mechanisms — measure what gating costs and what it preserves.**

| Metric | What It Measures | Use Case |
|--------|-----------------|----------|
| `contradiction_rate` | Semantic inversion (negative dot product) | Detect gating failures |
| `entailment_consistency` | Adjacent-output alignment (cosine) | Detect erratic swings |
| `delta_behavior` | Direction + magnitude drift | Full coherence profile |
| `jaccard_similarity` | Mask overlap (intersection/union) | Compare sparsity patterns |
| `quality_check` | Cosine similarity pass/fail gate | CI/CD quality guardrail |
| `evaluate_batch` | Aggregate stats with 95% CI | Statistical significance |

## Overview

When replacing softmax attention with a gated alternative (such as min-cut
gating), the central question is: **does the output stay coherent?** This crate
provides a suite of metrics, comparison utilities, quality guardrails, and
batched evaluation tools to answer that question quantitatively.

"Coherence" here means the degree to which gated attention outputs preserve the
semantic and structural properties of baseline softmax outputs. The crate
measures this through vector similarity, contradiction detection, mask overlap
analysis, and statistical aggregation with confidence intervals.

## Modules

| Module | Purpose |
|--------|---------|
| `metrics` | `contradiction_rate`, `entailment_consistency`, `delta_behavior` |
| `comparison` | `compare_attention_masks`, `edge_flip_count`, `jaccard_similarity` |
| `quality` | `quality_check` with `cosine_similarity` and `l2_distance` |
| `batch` | `evaluate_batch` with mean, std, 95% CI, and pass rate |

## Metrics Explained

### contradiction_rate

Measures the fraction of output pairs where the dot product between prediction
and reference vectors is negative. A high contradiction rate signals that gating
has inverted the semantic direction of outputs.

```rust
use ruvector_coherence::contradiction_rate;

let predictions = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
let references  = vec![vec![1.0, 1.0], vec![-1.0, -1.0]];

let rate = contradiction_rate(&predictions, &references);
// rate = 0.5 (second pair contradicts)
```

### entailment_consistency

Computes mean pairwise cosine similarity between consecutive output vectors.
High values (close to 1.0) indicate that adjacent outputs remain aligned --
useful for detecting whether gating introduces erratic token-to-token swings.

```rust
use ruvector_coherence::entailment_consistency;

let outputs = vec![vec![1.0, 0.0], vec![0.9, 0.1], vec![0.8, 0.2]];
let consistency = entailment_consistency(&outputs);
// consistency close to 1.0 (outputs smoothly evolve)
```

### delta_behavior (DeltaMetric)

Compares baseline and gated attention outputs element-by-element, returning:

| Field | Meaning |
|-------|---------|
| `coherence_delta` | Cosine similarity minus 1.0 (0.0 = identical direction) |
| `decision_flips` | Count of sign disagreements between baseline and gated values |
| `path_length_change` | Relative change in L2 norm (magnitude drift) |

```rust
use ruvector_coherence::delta_behavior;

let baseline = vec![1.0, 2.0, 3.0];
let gated    = vec![1.1, 1.9, 3.1];

let delta = delta_behavior(&baseline, &gated);
println!("Coherence delta: {:.6}", delta.coherence_delta);
println!("Decision flips:  {}", delta.decision_flips);
println!("Path change:     {:.6}", delta.path_length_change);
```

## Mask Comparison

### compare_attention_masks (ComparisonResult)

Provides a full comparison between two boolean attention masks:

| Field | Meaning |
|-------|---------|
| `jaccard` | Jaccard similarity (intersection / union) |
| `edge_flips` | Number of positions where masks disagree |
| `baseline_edges` | Count of `true` entries in baseline mask |
| `gated_edges` | Count of `true` entries in gated mask |
| `sparsity_ratio` | Ratio of gated sparsity to baseline sparsity |

```rust
use ruvector_coherence::compare_attention_masks;

let baseline = vec![true, true, false, false, true];
let gated    = vec![true, false, false, true, true];

let cmp = compare_attention_masks(&baseline, &gated);
println!("Jaccard:      {:.3}", cmp.jaccard);       // 0.500
println!("Edge flips:   {}", cmp.edge_flips);        // 2
println!("Sparsity ratio: {:.3}", cmp.sparsity_ratio);
```

Standalone helpers `jaccard_similarity` and `edge_flip_count` are also available
for use outside of the full comparison struct.

## Quality Guardrails

### quality_check (QualityResult)

A pass/fail gate that checks whether gated output stays close enough to
baseline output. The check passes when cosine similarity meets or exceeds
a configurable threshold.

```rust
use ruvector_coherence::quality_check;

let baseline_out = vec![1.0, 2.0, 3.0];
let gated_out    = vec![1.1, 2.1, 3.1];

let result = quality_check(&baseline_out, &gated_out, 0.99);
println!("Cosine sim:  {:.4}", result.cosine_sim);
println!("L2 distance: {:.4}", result.l2_dist);
println!("Passes:      {}", result.passes_threshold);
```

## Batch Evaluation

### evaluate_batch (BatchResult)

Runs `delta_behavior` and `quality_check` across an array of sample pairs,
aggregating results with standard statistics.

| Field | Meaning |
|-------|---------|
| `mean_coherence_delta` | Average coherence delta across samples |
| `std_coherence_delta` | Standard deviation |
| `ci_95_lower` / `ci_95_upper` | 95% confidence interval (z = 1.96) |
| `n_samples` | Number of evaluated pairs |
| `pass_rate` | Fraction of samples passing the quality threshold |

```rust
use ruvector_coherence::evaluate_batch;

let baselines = vec![vec![1.0, 2.0, 3.0]; 100];
let gated     = vec![vec![1.05, 1.95, 3.05]; 100];

let batch = evaluate_batch(&baselines, &gated, 0.99);

println!("Samples:    {}", batch.n_samples);
println!("Mean delta: {:.6}", batch.mean_coherence_delta);
println!("95% CI:     [{:.6}, {:.6}]", batch.ci_95_lower, batch.ci_95_upper);
println!("Pass rate:  {:.1}%", batch.pass_rate * 100.0);
```

## Typical Workflow

```text
1. Run attn_softmax()  --> baseline outputs
2. Run attn_mincut()   --> gated outputs + keep_mask
3. quality_check()     --> per-sample pass/fail
4. compare_attention_masks() --> mask overlap analysis
5. evaluate_batch()    --> aggregate stats with 95% CI
6. Export via ruvector-profiler CSV emitters
```

<details>
<summary><strong>Tutorial: Full Coherence Evaluation Pipeline</strong></summary>

### Step 1: Run baseline and gated attention

```rust
use ruvector_attn_mincut::{attn_softmax, attn_mincut};

let (seq_len, d) = (32, 64);
let q = vec![0.1f32; seq_len * d];
let k = vec![0.1f32; seq_len * d];
let v = vec![1.0f32; seq_len * d];

let baseline = attn_softmax(&q, &k, &v, d, seq_len);
let gated = attn_mincut(&q, &k, &v, d, seq_len, 0.5, 2, 0.01);
```

### Step 2: Individual metrics

```rust
use ruvector_coherence::*;

let delta = delta_behavior(&baseline.output, &gated.output);
println!("Coherence delta: {:.6}", delta.coherence_delta);
println!("Decision flips:  {}", delta.decision_flips);

let quality = quality_check(&baseline.output, &gated.output, 0.99);
println!("Passes: {} (cosine={:.4})", quality.passes_threshold, quality.cosine_sim);
```

### Step 3: Batch evaluation with confidence intervals

```rust
let baselines = vec![baseline.output.clone(); 100];
let gateds = vec![gated.output.clone(); 100];

let batch = evaluate_batch(&baselines, &gateds, 0.99);
println!("Mean delta: {:.6} +/- {:.6}", batch.mean_coherence_delta, batch.std_coherence_delta);
println!("95% CI: [{:.6}, {:.6}]", batch.ci_95_lower, batch.ci_95_upper);
println!("Pass rate: {:.1}%", batch.pass_rate * 100.0);
```

### Step 4: Success criteria

| Criterion | Threshold | Check |
|-----------|-----------|-------|
| Coherence delta | < 5% | `batch.mean_coherence_delta < 0.05` |
| Accuracy loss | < 1% | `batch.pass_rate > 0.99` |
| Contradiction rate | < 0.1% | `contradiction_rate(...) < 0.001` |

</details>

## Related Crates

| Crate | Role |
|-------|------|
| [`ruvector-attn-mincut`](../ruvector-attn-mincut/README.md) | Provides gated attention operators |
| [`ruvector-profiler`](../ruvector-profiler/README.md) | Exports results to CSV for analysis |
| [`ruvector-solver`](../ruvector-solver/README.md) | Sublinear solvers for graph analytics |

## License

Licensed under the [MIT License](../../LICENSE).

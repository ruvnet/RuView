//! Frozen pre-optimization scalar oracles. These deliberately retain the
//! allocation-heavy paths so stale scratch data, reduction-order changes and
//! finite-difference perturbations cannot silently change trained artifacts.
use super::*;
fn bits(rows: Vec<Vec<f32>>) -> Vec<Vec<u32>> {
    rows.into_iter()
        .map(|row| row.into_iter().map(f32::to_bits).collect())
        .collect()
}
fn matrix(rows: usize, width: usize, seed: usize) -> Vec<Vec<f32>> {
    (0..rows)
        .map(|i| {
            (0..width)
                .map(|j| (((i * 17 + j * 7 + seed) % 31) as f32 - 15.0) / 8.0)
                .collect()
        })
        .collect()
}
#[allow(clippy::needless_range_loop)]
fn reference_attention(
    layer: &CrossAttention,
    query: &[Vec<f32>],
    key: &[Vec<f32>],
    value: &[Vec<f32>],
) -> Vec<Vec<f32>> {
    let (n_q, n_kv) = (query.len(), key.len());
    if n_q == 0 || n_kv == 0 {
        return vec![vec![0.0; layer.d_model]; n_q];
    }

    let q_proj: Vec<Vec<f32>> = query.iter().map(|q| layer.w_q.forward(q)).collect();
    let k_proj: Vec<Vec<f32>> = key.iter().map(|k| layer.w_k.forward(k)).collect();
    let v_proj: Vec<Vec<f32>> = value.iter().map(|v| layer.w_v.forward(v)).collect();

    let scale = (layer.d_k as f32).sqrt();
    let mut output = vec![vec![0.0f32; layer.d_model]; n_q];

    for qi in 0..n_q {
        let mut concat = Vec::with_capacity(layer.d_model);
        for h in 0..layer.n_heads {
            let (start, end) = (h * layer.d_k, (h + 1) * layer.d_k);
            let q_h = &q_proj[qi][start..end];
            let mut scores = vec![0.0f32; n_kv];
            for ki in 0..n_kv {
                let dot: f32 = q_h
                    .iter()
                    .zip(&k_proj[ki][start..end])
                    .map(|(a, b)| a * b)
                    .sum();
                scores[ki] = dot / scale;
            }
            let mut wts = vec![0.0f32; n_kv];
            softmax(&scores, &mut wts);
            let mut head_out = vec![0.0f32; layer.d_k];
            for ki in 0..n_kv {
                for (o, &v) in head_out.iter_mut().zip(&v_proj[ki][start..end]) {
                    *o += wts[ki] * v;
                }
            }
            concat.extend_from_slice(&head_out);
        }
        output[qi] = layer.w_o.forward(&concat);
    }
    output
}
#[allow(clippy::needless_range_loop)]
fn reference_graph(layer: &GraphMessagePassing, node_features: &[Vec<f32>]) -> Vec<Vec<f32>> {
    assert_eq!(
        node_features.len(),
        17,
        "expected 17 nodes, got {}",
        node_features.len()
    );
    let mut agg = vec![vec![0.0f32; layer.in_features]; 17];
    #[allow(clippy::needless_range_loop)]
    for i in 0..17 {
        for j in 0..17 {
            let a = layer.norm_adj[i][j];
            if a.abs() > 1e-10 {
                for (ag, &f) in agg[i].iter_mut().zip(&node_features[j]) {
                    *ag += a * f;
                }
            }
        }
    }
    agg.iter()
        .map(|a| layer.weight.forward(a).into_iter().map(relu).collect())
        .collect()
}
#[test]
fn attention_preserves_bits_across_shapes_and_parameter_probes() {
    for (width, heads) in [(0, 1), (2, 1), (3, 1), (4, 2), (8, 4), (16, 4)] {
        let layer = CrossAttention::new(width, heads);
        let mut parameters = Vec::new();
        layer.flatten_into(&mut parameters);
        for (queries, keys, values) in [(0, 4, 4), (17, 0, 0), (1, 1, 1), (17, 4, 4), (3, 7, 9)] {
            let q = matrix(queries, width, 3);
            let k = matrix(keys, width, 7);
            let v = matrix(values, width, 19);
            assert_eq!(
                bits(layer.forward(&q, &k, &v)),
                bits(reference_attention(&layer, &q, &k, &v))
            );
            for index in 0..parameters.len() {
                for epsilon in [-0.002, 0.002] {
                    let mut changed = parameters.clone();
                    changed[index] += epsilon;
                    let (probe, _) = CrossAttention::unflatten_from(&changed, width, heads);
                    assert_eq!(
                        bits(probe.forward(&q, &k, &v)),
                        bits(reference_attention(&probe, &q, &k, &v)),
                        "width={width} heads={heads} parameter={index} epsilon={epsilon}"
                    );
                }
            }
        }
    }
}
#[test]
fn graph_aggregation_preserves_bits_across_widths_and_parameter_probes() {
    for (input, output) in [(0, 3), (3, 3), (4, 2), (8, 6)] {
        let layer = GraphMessagePassing::new(input, output, &BodyGraph::new());
        let features = matrix(17, input, 11);
        let mut parameters = Vec::new();
        layer.weight.flatten_into(&mut parameters);
        assert_eq!(
            bits(layer.forward(&features)),
            bits(reference_graph(&layer, &features))
        );
        for index in 0..parameters.len() {
            for epsilon in [-0.002, 0.002] {
                let mut changed = parameters.clone();
                changed[index] += epsilon;
                let mut probe = layer.clone();
                probe.weight = Linear::unflatten_from(&changed, input, output).0;
                assert_eq!(
                    bits(probe.forward(&features)),
                    bits(reference_graph(&probe, &features))
                );
            }
        }
    }
}
#[test]
fn linear_reuse_overwrites_every_output_and_matches_allocating_path() {
    for (input, output) in [(0, 3), (3, 3), (4, 2), (8, 6)] {
        let layer = Linear::with_seed(input, output, 29);
        let mut scratch = vec![f32::NAN; output];
        for row in matrix(17, input, 13) {
            layer.forward_into(&row, &mut scratch);
            assert_eq!(
                scratch.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                layer
                    .forward(&row)
                    .iter()
                    .map(|x| x.to_bits())
                    .collect::<Vec<_>>()
            );
        }
    }
}
#[test]
fn inconsistent_restored_attention_shape_still_rejects() {
    let original = CrossAttention::new(3, 1);
    let mut parameters = Vec::new();
    original.flatten_into(&mut parameters);
    let (invalid, _) = CrossAttention::unflatten_from(&parameters, 3, 2);
    let q = matrix(17, 3, 1);
    let k = matrix(4, 3, 2);
    assert!(std::panic::catch_unwind(|| reference_attention(&invalid, &q, &k, &k)).is_err());
    assert!(std::panic::catch_unwind(|| invalid.forward(&q, &k, &k)).is_err());
}

#[test]
fn missing_value_rows_still_reject_including_zero_width() {
    for width in [0, 3] {
        let layer = CrossAttention::new(width, 1);
        let q = matrix(2, width, 1);
        let k = matrix(4, width, 2);
        for values in [0, 3] {
            let v = matrix(values, width, 3);
            assert!(std::panic::catch_unwind(|| reference_attention(&layer, &q, &k, &v)).is_err());
            assert!(std::panic::catch_unwind(|| layer.forward(&q, &k, &v)).is_err());
        }
    }
}

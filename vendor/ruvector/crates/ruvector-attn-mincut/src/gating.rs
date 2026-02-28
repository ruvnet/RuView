use crate::mincut::{dynamic_min_cut, GatingResult};

/// Combined output from min-cut gated attention.
#[derive(Debug, Clone)]
pub struct AttentionOutput {
    pub output: Vec<f32>,
    pub gating: GatingResult,
}

/// Compute raw logits: Q * K^T / sqrt(d). Returns flattened `seq_len x seq_len`.
fn compute_logits(q: &[f32], k: &[f32], d: usize, seq_len: usize) -> Vec<f32> {
    let scale = 1.0 / (d as f32).sqrt();
    let mut logits = vec![0.0f32; seq_len * seq_len];
    for i in 0..seq_len {
        for j in 0..seq_len {
            let mut dot = 0.0f32;
            for h in 0..d {
                dot += q[i * d + h] * k[j * d + h];
            }
            logits[i * seq_len + j] = dot * scale;
        }
    }
    logits
}

/// Row-wise softmax in place on a flattened `rows x cols` matrix.
fn row_softmax(mat: &mut [f32], rows: usize, cols: usize) {
    for i in 0..rows {
        let row = &mut mat[i * cols..(i + 1) * cols];
        let mx = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut sum = 0.0f32;
        for v in row.iter_mut() {
            *v = (*v - mx).exp();
            sum += *v;
        }
        if sum > 0.0 {
            for v in row.iter_mut() {
                *v /= sum;
            }
        }
    }
}

/// Multiply weights (seq_len x seq_len) by V (seq_len x d).
fn matmul_wv(w: &[f32], v: &[f32], seq_len: usize, d: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; seq_len * d];
    for i in 0..seq_len {
        for j in 0..seq_len {
            let wij = w[i * seq_len + j];
            if wij != 0.0 {
                for h in 0..d {
                    out[i * d + h] += wij * v[j * d + h];
                }
            }
        }
    }
    out
}

/// Baseline standard softmax attention. Returns flattened `seq_len x d`.
pub fn attn_softmax(q: &[f32], k: &[f32], v: &[f32], d: usize, seq_len: usize) -> Vec<f32> {
    assert!(q.len() == seq_len * d && k.len() == seq_len * d && v.len() == seq_len * d);
    let mut logits = compute_logits(q, k, d, seq_len);
    row_softmax(&mut logits, seq_len, seq_len);
    matmul_wv(&logits, v, seq_len, d)
}

/// Min-cut gated attention.
/// 1. Compute logits  2. Min-cut gating  3. Mask with -INF  4. Row-softmax  5. Multiply V
pub fn attn_mincut(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    d: usize,
    seq_len: usize,
    lambda: f32,
    tau: usize,
    eps: f32,
) -> AttentionOutput {
    assert!(q.len() == seq_len * d && k.len() == seq_len * d && v.len() == seq_len * d);
    let mut logits = compute_logits(q, k, d, seq_len);
    let gating = dynamic_min_cut(&logits, seq_len, lambda, tau, eps);

    // Gate entries with -INF so softmax zeroes them
    for i in 0..logits.len() {
        if !gating.keep_mask[i] {
            logits[i] = f32::NEG_INFINITY;
        }
    }
    row_softmax(&mut logits, seq_len, seq_len);
    // Replace NaN (fully-gated rows) with 0
    for v in logits.iter_mut() {
        if v.is_nan() {
            *v = 0.0;
        }
    }

    AttentionOutput {
        output: matmul_wv(&logits, v, seq_len, d),
        gating,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_qkv(seq: usize, d: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let mut q = vec![0.0f32; seq * d];
        let mut k = vec![0.0f32; seq * d];
        let v: Vec<f32> = (0..seq * d).map(|i| i as f32).collect();
        for i in 0..seq.min(d) {
            q[i * d + i] = 1.0;
            k[i * d + i] = 1.0;
        }
        (q, k, v)
    }

    #[test]
    fn test_softmax_shape_and_finite() {
        let (q, k, v) = make_qkv(4, 3);
        let out = attn_softmax(&q, &k, &v, 3, 4);
        assert_eq!(out.len(), 12);
        assert!(out.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn test_mincut_shape_and_finite() {
        let (q, k, v) = make_qkv(4, 3);
        let r = attn_mincut(&q, &k, &v, 3, 4, 0.5, 2, 0.01);
        assert_eq!(r.output.len(), 12);
        assert!(r.output.iter().all(|x| x.is_finite()));
        assert_eq!(r.gating.edges_total, 16);
    }

    #[test]
    fn test_logit_scale() {
        let logits = compute_logits(&[1.0; 4], &[1.0; 4], 4, 1);
        assert!((logits[0] - 2.0).abs() < 1e-5); // dot=4, scale=1/2
    }

    #[test]
    fn test_row_softmax_sums_to_one() {
        let mut m = vec![1.0, 2.0, 3.0, 4.0];
        row_softmax(&mut m, 2, 2);
        assert!(((m[0] + m[1]) - 1.0).abs() < 1e-5);
        assert!(((m[2] + m[3]) - 1.0).abs() < 1e-5);
    }
}

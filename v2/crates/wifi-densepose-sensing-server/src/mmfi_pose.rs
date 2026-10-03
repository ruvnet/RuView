//! Native inference for the published MM-Fi `Micro` pose checkpoint.
//!
//! The checkpoint is loaded from a non-executable NPZ containing F32 tensors.
//! Live ESP32 input is an explicit domain adapter (3 nodes x 114 bins x 10
//! frames), so its output must remain labelled experimental until validated in
//! the deployment room.

use ndarray::ArrayD;
use ndarray_npy::NpzReader;
use std::{collections::HashMap, fs::File, path::Path};

const D: usize = 64;
const T: usize = 10;
const SC: usize = 114;
const INPUT: usize = 3 * SC;
const KP: usize = 17;
const HEADS: usize = 2;
const HD: usize = D / HEADS;

#[derive(Debug)]
pub struct MmfiPoseModel {
    w: HashMap<String, Vec<f32>>,
}

impl MmfiPoseModel {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
        let mut npz = NpzReader::new(file).map_err(|e| format!("NPZ header: {e}"))?;
        let required = [
            "pos",
            "proj.weight",
            "proj.bias",
            "tf.layers.0.self_attn.in_proj_weight",
            "tf.layers.0.self_attn.in_proj_bias",
            "tf.layers.0.self_attn.out_proj.weight",
            "tf.layers.0.self_attn.out_proj.bias",
            "tf.layers.0.linear1.weight",
            "tf.layers.0.linear1.bias",
            "tf.layers.0.linear2.weight",
            "tf.layers.0.linear2.bias",
            "tf.layers.0.norm1.weight",
            "tf.layers.0.norm1.bias",
            "tf.layers.0.norm2.weight",
            "tf.layers.0.norm2.bias",
            "att.weight",
            "att.bias",
            "head.0.weight",
            "head.0.bias",
            "head.3.weight",
            "head.3.bias",
            "gr.inp.weight",
            "gr.inp.bias",
            "gr.g1.weight",
            "gr.g1.bias",
            "gr.g2.weight",
            "gr.g2.bias",
            "gr.out.weight",
            "gr.out.bias",
        ];
        let mut w = HashMap::new();
        for name in required {
            let key = format!("f::{name}.npy");
            let a: ArrayD<f32> = npz
                .by_name(&key)
                .map_err(|e| format!("missing/invalid {key}: {e}"))?;
            w.insert(name.to_string(), a.iter().copied().collect());
        }
        let model = Self { w };
        model.validate()?;
        Ok(model)
    }

    fn validate(&self) -> Result<(), String> {
        for (n, len) in [
            ("pos", T * D),
            ("proj.weight", D * INPUT),
            ("proj.bias", D),
            ("tf.layers.0.self_attn.in_proj_weight", 3 * D * D),
            ("tf.layers.0.self_attn.in_proj_bias", 3 * D),
            ("tf.layers.0.self_attn.out_proj.weight", D * D),
            ("tf.layers.0.self_attn.out_proj.bias", D),
            ("tf.layers.0.linear1.weight", 2 * D * D),
            ("tf.layers.0.linear1.bias", 2 * D),
            ("tf.layers.0.linear2.weight", 2 * D * D),
            ("tf.layers.0.linear2.bias", D),
            ("tf.layers.0.norm1.weight", D),
            ("tf.layers.0.norm1.bias", D),
            ("tf.layers.0.norm2.weight", D),
            ("tf.layers.0.norm2.bias", D),
            ("att.weight", D),
            ("att.bias", 1),
            ("head.0.weight", D * D),
            ("head.0.bias", D),
            ("head.3.weight", 34 * D),
            ("head.3.bias", 34),
            ("gr.inp.weight", D * 66),
            ("gr.inp.bias", D),
            ("gr.g1.weight", D * D),
            ("gr.g1.bias", D),
            ("gr.g2.weight", D * D),
            ("gr.g2.bias", D),
            ("gr.out.weight", 2 * D),
            ("gr.out.bias", 2),
        ] {
            let got = self.w.get(n).map_or(0, Vec::len);
            if got != len {
                return Err(format!("tensor {n}: expected {len} values, got {got}"));
            }
        }
        Ok(())
    }

    fn p(&self, n: &str) -> &[f32] {
        &self.w[n]
    }

    /// Infer 17 normalized `(x,y)` keypoints from `[node][time][subcarrier]`.
    pub fn infer(&self, input: &[Vec<Vec<f64>>]) -> Result<[[f32; 2]; KP], String> {
        if input.len() != 3 || input.iter().any(|n| n.len() < T) {
            return Err("need exactly three nodes with ten frames each".into());
        }
        let mut x = vec![0.0f32; T * INPUT];
        for (ni, node) in input.iter().enumerate() {
            for ti in 0..T {
                let frame = &node[node.len() - T + ti];
                if frame.len() < 2 {
                    return Err("CSI frame has fewer than two bins".into());
                }
                for si in 0..SC {
                    let at = si as f64 * (frame.len() - 1) as f64 / (SC - 1) as f64;
                    let lo = at.floor() as usize;
                    let hi = at.ceil() as usize;
                    let f = (at - lo as f64) as f32;
                    x[ti * INPUT + ni * SC + si] =
                        frame[lo] as f32 * (1.0 - f) + frame[hi] as f32 * f;
                }
            }
        }
        let mean = x.iter().sum::<f32>() / x.len() as f32;
        let var = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / x.len() as f32;
        let sd = var.sqrt().max(1e-6);
        for v in &mut x {
            *v = (*v - mean) / sd;
        }

        let mut h = linear_rows(&x, T, INPUT, self.p("proj.weight"), self.p("proj.bias"), D);
        for (v, p) in h.iter_mut().zip(self.p("pos")) {
            *v += p;
        }
        let qkv = linear_rows(
            &h,
            T,
            D,
            self.p("tf.layers.0.self_attn.in_proj_weight"),
            self.p("tf.layers.0.self_attn.in_proj_bias"),
            3 * D,
        );
        let mut attn = vec![0.0; T * D];
        for head in 0..HEADS {
            for qi in 0..T {
                let mut scores = [0.0f32; T];
                for ki in 0..T {
                    let mut s = 0.0;
                    for j in 0..HD {
                        s += qkv[qi * 3 * D + head * HD + j] * qkv[ki * 3 * D + D + head * HD + j];
                    }
                    scores[ki] = s / (HD as f32).sqrt();
                }
                softmax(&mut scores);
                for j in 0..HD {
                    for ki in 0..T {
                        attn[qi * D + head * HD + j] +=
                            scores[ki] * qkv[ki * 3 * D + 2 * D + head * HD + j];
                    }
                }
            }
        }
        let a = linear_rows(
            &attn,
            T,
            D,
            self.p("tf.layers.0.self_attn.out_proj.weight"),
            self.p("tf.layers.0.self_attn.out_proj.bias"),
            D,
        );
        for i in 0..h.len() {
            h[i] += a[i];
        }
        layer_norm_rows(
            &mut h,
            T,
            D,
            self.p("tf.layers.0.norm1.weight"),
            self.p("tf.layers.0.norm1.bias"),
        );
        let mut ff = linear_rows(
            &h,
            T,
            D,
            self.p("tf.layers.0.linear1.weight"),
            self.p("tf.layers.0.linear1.bias"),
            2 * D,
        );
        for v in &mut ff {
            *v = gelu(*v);
        }
        let ff = linear_rows(
            &ff,
            T,
            2 * D,
            self.p("tf.layers.0.linear2.weight"),
            self.p("tf.layers.0.linear2.bias"),
            D,
        );
        for i in 0..h.len() {
            h[i] += ff[i];
        }
        layer_norm_rows(
            &mut h,
            T,
            D,
            self.p("tf.layers.0.norm2.weight"),
            self.p("tf.layers.0.norm2.bias"),
        );

        let mut aw = [0.0f32; T];
        for t in 0..T {
            aw[t] = dot(&h[t * D..(t + 1) * D], self.p("att.weight")) + self.p("att.bias")[0];
        }
        softmax(&mut aw);
        let mut z = [0.0f32; D];
        for t in 0..T {
            for j in 0..D {
                z[j] += aw[t] * h[t * D + j];
            }
        }
        let mut hidden = linear_rows(&z, 1, D, self.p("head.0.weight"), self.p("head.0.bias"), D);
        for v in &mut hidden {
            *v = gelu(*v)
        }
        let base = linear_rows(
            &hidden,
            1,
            D,
            self.p("head.3.weight"),
            self.p("head.3.bias"),
            34,
        );
        let mut kp = [[0.0f32; 2]; KP];
        for k in 0..KP {
            for c in 0..2 {
                kp[k][c] = sigmoid(base[k * 2 + c]);
            }
        }

        let mut g0 = vec![0.0f32; KP * D];
        for k in 0..KP {
            let mut v = Vec::with_capacity(66);
            v.extend_from_slice(&z);
            v.extend_from_slice(&kp[k]);
            let o = linear_rows(&v, 1, 66, self.p("gr.inp.weight"), self.p("gr.inp.bias"), D);
            for j in 0..D {
                g0[k * D + j] = o[j].max(0.0)
            }
        }
        let edges = [
            (0, 1),
            (0, 2),
            (1, 3),
            (2, 4),
            (5, 6),
            (5, 7),
            (7, 9),
            (6, 8),
            (8, 10),
            (5, 11),
            (6, 12),
            (11, 12),
            (11, 13),
            (13, 15),
            (12, 14),
            (14, 16),
        ];
        let aggregate = |src: &[f32]| {
            let mut out = vec![0.0; KP * D];
            for k in 0..KP {
                let mut ns = vec![k];
                for &(a, b) in &edges {
                    if a == k {
                        ns.push(b)
                    } else if b == k {
                        ns.push(a)
                    }
                }
                let den = ns.len() as f32;
                for n in ns {
                    for j in 0..D {
                        out[k * D + j] += src[n * D + j] / den
                    }
                }
            }
            out
        };
        let ag = aggregate(&g0);
        let mut g1 = linear_rows(&ag, KP, D, self.p("gr.g1.weight"), self.p("gr.g1.bias"), D);
        for v in &mut g1 {
            *v = v.max(0.0)
        }
        let ag = aggregate(&g1);
        let mut g2 = linear_rows(&ag, KP, D, self.p("gr.g2.weight"), self.p("gr.g2.bias"), D);
        for v in &mut g2 {
            *v = v.max(0.0)
        }
        let delta = linear_rows(
            &g2,
            KP,
            D,
            self.p("gr.out.weight"),
            self.p("gr.out.bias"),
            2,
        );
        for k in 0..KP {
            for c in 0..2 {
                kp[k][c] = (kp[k][c] + 0.3 * delta[k * 2 + c].tanh()).clamp(-0.3, 1.3);
            }
        }
        Ok(kp)
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn linear_rows(x: &[f32], rows: usize, cols: usize, w: &[f32], b: &[f32], out: usize) -> Vec<f32> {
    let mut y = vec![0.0; rows * out];
    for r in 0..rows {
        for o in 0..out {
            y[r * out + o] = dot(&x[r * cols..(r + 1) * cols], &w[o * cols..(o + 1) * cols]) + b[o];
        }
    }
    y
}
fn softmax(x: &mut [f32]) {
    let m = x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut s = 0.0;
    for v in x.iter_mut() {
        *v = (*v - m).exp();
        s += *v
    }
    for v in x {
        *v /= s.max(1e-12)
    }
}
fn layer_norm_rows(x: &mut [f32], rows: usize, cols: usize, w: &[f32], b: &[f32]) {
    for r in 0..rows {
        let q = &mut x[r * cols..(r + 1) * cols];
        let m = q.iter().sum::<f32>() / cols as f32;
        let v = q.iter().map(|z| (z - m) * (z - m)).sum::<f32>() / cols as f32;
        let d = (v + 1e-5).sqrt();
        for j in 0..cols {
            q[j] = (q[j] - m) / d * w[j] + b[j]
        }
    }
}
fn gelu(x: f32) -> f32 {
    0.5 * x * (1.0 + (0.7978845608 * (x + 0.044715 * x * x * x)).tanh())
}
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn softmax_is_normalized() {
        let mut x = [1.0, 2.0, 3.0];
        softmax(&mut x);
        assert!((x.iter().sum::<f32>() - 1.0).abs() < 1e-6)
    }

    #[test]
    fn published_checkpoint_adapter_smoke_when_supplied() {
        let Ok(path) = std::env::var("RUVIEW_MMFI_TEST_MODEL") else {
            return;
        };
        let model = MmfiPoseModel::load(Path::new(&path)).expect("converted checkpoint loads");
        let input = vec![vec![vec![1.0; 256]; T]; 3];
        let pose = model.infer(&input).expect("inference succeeds");
        assert!(pose.iter().flatten().all(|v| v.is_finite()));
    }
}

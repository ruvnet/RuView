//! ADR-116: WiFlow-v1 supervised pose model loader + inference.
//!
//! Ports `scripts/train-wiflow-supervised.js` inference path to Rust so
//! sensing-server can serve real keypoints on `/api/v1/pose/*` instead of
//! returning empty arrays per ADR-105 gate.
//!
//! The model on HuggingFace (`ruv/ruview/wiflow-v1/wiflow-v1.json`) is the
//! **lite scale** (186,946 params), NOT the `architecture` field that the
//! exporter hardcodes (which describes the `full` scale). We trust
//! `totalParams` to disambiguate.
//!
//! Topology (lite):
//!   * 2 TCN blocks, kernel=3, dilations=[1,2]
//!   * Per block: causal_conv1 → bn1 → relu → causal_conv2 → bn2
//!     + residual (1×1 projection if in_ch ≠ out_ch) → relu
//!   * tcnChannels: 35 → 32 → 32
//!   * Flatten (32 × 20 = 640) → fc1 (640→256) → relu → fc2 (256→34)
//!   * Sigmoid on final 34-dim vector → 17 (x,y) keypoints in [0, 1]
//!
//! Weight order (collectParams in train script):
//!   for each tcn block:
//!     conv1.weight, conv1.bias, bn1.gamma, bn1.beta,
//!     conv2.weight, conv2.bias, bn2.gamma, bn2.beta,
//!     (if in_ch ≠ out_ch: res.weight, res.bias)
//!   fc1.weight, fc1.bias, fc2.weight, fc2.bias
//!
//! All weights are f32 little-endian, base64-encoded in `weightsBase64`.

use std::path::Path;

const TIME_STEPS: usize = 20;
const INPUT_DIM: usize  = 35;
const NUM_KP:    usize  = 17;
const OUT_DIM:   usize  = NUM_KP * 2;     // 34
const TCN_CH:    [usize; 3] = [INPUT_DIM, 32, 32]; // chain: 35 → 32 → 32
const TCN_K:     usize  = 3;
const TCN_DIL:   [usize; 2] = [1, 2];
const HIDDEN:    usize  = 256;
const FLAT_DIM:  usize  = 32 * TIME_STEPS; // 640

/// CausalConv1d weights: `weight[oc*(in_ch*k) + ic*k + tap]`, bias `[oc]`.
#[derive(Debug, Clone)]
struct Conv1d {
    in_ch:    usize,
    out_ch:   usize,
    kernel:   usize,
    dilation: usize,
    weight:   Vec<f32>,
    bias:     Vec<f32>,
}

/// BatchNorm1d: 2 params per channel (gamma, beta). Running stats are NOT
/// serialized — JS impl re-computes mean/var per window at inference time.
#[derive(Debug, Clone)]
struct BatchNorm {
    channels: usize,
    gamma:    Vec<f32>,
    beta:     Vec<f32>,
}

#[derive(Debug, Clone)]
struct TcnBlock {
    conv1: Conv1d,
    bn1:   BatchNorm,
    conv2: Conv1d,
    bn2:   BatchNorm,
    res:   Option<Conv1d>, // 1×1 projection when in_ch ≠ out_ch
}

#[derive(Debug, Clone)]
struct Linear {
    in_dim:  usize,
    out_dim: usize,
    /// Row-major `[in_dim, out_dim]` — matches JS `weight[i*outDim + j]`.
    weight:  Vec<f32>,
    bias:    Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct WiflowModel {
    blocks: [TcnBlock; 2],
    fc1:    Linear,
    fc2:    Linear,
}

#[derive(Debug)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "wiflow_v1 load: {}", self.0)
    }
}

impl std::error::Error for LoadError {}

impl WiflowModel {
    pub fn load_from_json(path: &Path) -> Result<Self, LoadError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| LoadError(format!("read {}: {e}", path.display())))?;
        let v: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| LoadError(format!("json parse: {e}")))?;

        let total = v.get("totalParams").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        if total != 186_946 {
            return Err(LoadError(format!(
                "totalParams={total}, expected 186946 (lite scale). The exporter \
                 hardcodes the `architecture` field to the full scale; \
                 totalParams is the only reliable signal."
            )));
        }

        let b64 = v.get("weightsBase64").and_then(|x| x.as_str())
            .ok_or_else(|| LoadError("missing weightsBase64".into()))?;
        let bytes = base64_decode(b64)
            .map_err(|e| LoadError(format!("base64: {e}")))?;
        if bytes.len() != total * 4 {
            return Err(LoadError(format!(
                "bytes={}, expected {} (totalParams*4)", bytes.len(), total * 4)));
        }
        let floats: Vec<f32> = bytes.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        let mut cur = Cursor::new(&floats);
        let block0 = TcnBlock::take(&mut cur, TCN_CH[0], TCN_CH[1], TCN_K, TCN_DIL[0])?;
        let block1 = TcnBlock::take(&mut cur, TCN_CH[1], TCN_CH[2], TCN_K, TCN_DIL[1])?;
        let fc1 = Linear::take(&mut cur, FLAT_DIM, HIDDEN)?;
        let fc2 = Linear::take(&mut cur, HIDDEN, OUT_DIM)?;
        if cur.remaining() != 0 {
            return Err(LoadError(format!(
                "weight stream has {} unread floats after fc2 — topology mismatch",
                cur.remaining()
            )));
        }

        Ok(Self { blocks: [block0, block1], fc1, fc2 })
    }

    /// Forward pass.
    /// `input` is `[INPUT_DIM × TIME_STEPS]` row-major (channel-major):
    /// `input[c * TIME_STEPS + t]`.
    /// Returns 17 keypoints as (x, y) in [0, 1].
    pub fn forward(&self, input: &[f32]) -> [(f32, f32); NUM_KP] {
        debug_assert_eq!(input.len(), INPUT_DIM * TIME_STEPS);
        let mut x: Vec<f32> = input.to_vec();
        // TCN blocks
        x = self.blocks[0].forward(&x, TIME_STEPS);
        x = self.blocks[1].forward(&x, TIME_STEPS);
        // Flatten — channels-major matches JS `c * T + t` linearisation.
        debug_assert_eq!(x.len(), FLAT_DIM);
        // fc1 + relu
        let mut h = self.fc1.forward(&x);
        for v in h.iter_mut() { if *v < 0.0 { *v = 0.0; } }
        // fc2
        let out = self.fc2.forward(&h);
        // sigmoid → 17 (x, y)
        let mut kp = [(0.0f32, 0.0f32); NUM_KP];
        for i in 0..NUM_KP {
            kp[i].0 = sigmoid(out[i * 2]);
            kp[i].1 = sigmoid(out[i * 2 + 1]);
        }
        kp
    }
}

// ── Internal layer impls ─────────────────────────────────────────────────────

struct Cursor<'a> {
    data:   &'a [f32],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(d: &'a [f32]) -> Self { Self { data: d, offset: 0 } }
    fn take(&mut self, n: usize) -> Result<Vec<f32>, LoadError> {
        if self.offset + n > self.data.len() {
            return Err(LoadError(format!(
                "weight underrun: need {}, have {}", n, self.data.len() - self.offset)));
        }
        let out = self.data[self.offset..self.offset + n].to_vec();
        self.offset += n;
        Ok(out)
    }
    fn remaining(&self) -> usize { self.data.len() - self.offset }
}

impl Conv1d {
    fn take(c: &mut Cursor<'_>, in_ch: usize, out_ch: usize, k: usize, dil: usize)
        -> Result<Self, LoadError>
    {
        let weight = c.take(in_ch * k * out_ch)?;
        let bias   = c.take(out_ch)?;
        Ok(Self { in_ch, out_ch, kernel: k, dilation: dil, weight, bias })
    }

    /// Causal conv with left padding. Input layout: `[in_ch * T]` row-major.
    fn forward(&self, input: &[f32], t_steps: usize) -> Vec<f32> {
        let eff_k = self.kernel + (self.kernel - 1) * (self.dilation - 1);
        let pad_left = eff_k - 1;
        let mut out = vec![0.0f32; self.out_ch * t_steps];
        for oc in 0..self.out_ch {
            for t in 0..t_steps {
                let mut sum = self.bias[oc];
                for ic in 0..self.in_ch {
                    for k in 0..self.kernel {
                        let t_idx_signed = t as isize + pad_left as isize
                            - (k * self.dilation) as isize;
                        // Left-pad with zeros: only contribute when t_idx_signed - pad_left >= 0
                        let t_src = t_idx_signed - pad_left as isize;
                        if t_src < 0 || t_src >= t_steps as isize { continue; }
                        let w_idx = oc * (self.in_ch * self.kernel) + ic * self.kernel + k;
                        sum += self.weight[w_idx] * input[ic * t_steps + t_src as usize];
                    }
                }
                out[oc * t_steps + t] = sum;
            }
        }
        out
    }
}

impl BatchNorm {
    fn take(c: &mut Cursor<'_>, channels: usize) -> Result<Self, LoadError> {
        let gamma = c.take(channels)?;
        let beta  = c.take(channels)?;
        Ok(Self { channels, gamma, beta })
    }

    /// Per-window normalisation matching JS impl: mean/var computed across
    /// the T axis at inference time (not from saved running stats).
    fn forward(&self, x: &mut [f32], t_steps: usize) {
        let eps = 1e-5f32;
        for c in 0..self.channels {
            let base = c * t_steps;
            let mut mean = 0.0f32;
            for t in 0..t_steps { mean += x[base + t]; }
            mean /= t_steps as f32;
            let mut var = 0.0f32;
            for t in 0..t_steps {
                let d = x[base + t] - mean;
                var += d * d;
            }
            var /= t_steps as f32;
            let inv_std = 1.0f32 / (var + eps).sqrt();
            let g = self.gamma[c];
            let b = self.beta[c];
            for t in 0..t_steps {
                x[base + t] = g * (x[base + t] - mean) * inv_std + b;
            }
        }
    }
}

impl TcnBlock {
    fn take(c: &mut Cursor<'_>, in_ch: usize, out_ch: usize, k: usize, dil: usize)
        -> Result<Self, LoadError>
    {
        let conv1 = Conv1d::take(c, in_ch, out_ch, k, dil)?;
        let bn1   = BatchNorm::take(c, out_ch)?;
        let conv2 = Conv1d::take(c, out_ch, out_ch, k, dil)?;
        let bn2   = BatchNorm::take(c, out_ch)?;
        let res = if in_ch != out_ch {
            Some(Conv1d::take(c, in_ch, out_ch, 1, 1)?)
        } else { None };
        Ok(Self { conv1, bn1, conv2, bn2, res })
    }

    fn forward(&self, input: &[f32], t_steps: usize) -> Vec<f32> {
        let mut x = self.conv1.forward(input, t_steps);
        self.bn1.forward(&mut x, t_steps);
        for v in x.iter_mut() { if *v < 0.0 { *v = 0.0; } } // relu

        let mut y = self.conv2.forward(&x, t_steps);
        self.bn2.forward(&mut y, t_steps);

        // Residual
        let res: Vec<f32> = if let Some(r) = &self.res {
            r.forward(input, t_steps)
        } else {
            input.to_vec()
        };
        debug_assert_eq!(y.len(), res.len());
        for (yv, rv) in y.iter_mut().zip(res.iter()) { *yv += *rv; }
        for v in y.iter_mut() { if *v < 0.0 { *v = 0.0; } } // relu after residual
        y
    }
}

impl Linear {
    fn take(c: &mut Cursor<'_>, in_dim: usize, out_dim: usize) -> Result<Self, LoadError> {
        let weight = c.take(in_dim * out_dim)?;
        let bias   = c.take(out_dim)?;
        Ok(Self { in_dim, out_dim, weight, bias })
    }

    fn forward(&self, input: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0f32; self.out_dim];
        for j in 0..self.out_dim {
            let mut s = self.bias[j];
            for i in 0..self.in_dim {
                s += input[i] * self.weight[i * self.out_dim + j];
            }
            out[j] = s;
        }
        out
    }
}

fn sigmoid(x: f32) -> f32 {
    if x >= 0.0 {
        let e = (-x).exp();
        1.0 / (1.0 + e)
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

// ── Inline base64 decoder ────────────────────────────────────────────────────
//
// Standard alphabet (A–Z, a–z, 0–9, +, /). Padding `=` tolerated. Whitespace
// (including newlines) ignored — JSON.stringify can wrap base64 across lines
// in some exporters. Avoids pulling the `base64` crate just for one decode.

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4 + 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for ch in s.bytes() {
        let v: u32 = match ch {
            b'A'..=b'Z' => (ch - b'A') as u32,
            b'a'..=b'z' => (ch - b'a' + 26) as u32,
            b'0'..=b'9' => (ch - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return Err(format!("invalid base64 char {:#x}", ch)),
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

// ── Convenience input helpers ────────────────────────────────────────────────

/// Build the `[INPUT_DIM × TIME_STEPS]` input tensor from the most recent
/// `TIME_STEPS` per-frame amplitude vectors of a single node. Picks the
/// `INPUT_DIM` (35) subcarriers with smallest NBVI score (most useful), using
/// the same per-subcarrier `α·σ/μ² + (1−α)·σ/μ` formula the classifier uses,
/// but with K=35 instead of NBVI_TOP_K=12 — model expects 35 channels.
///
/// Returns `None` if the history has fewer than `TIME_STEPS` frames or all
/// subcarriers are zero / unusable.
pub fn build_input_from_history(
    history: &std::collections::VecDeque<Vec<f64>>,
) -> Option<Vec<f32>> {
    let n = history.len();
    if n < TIME_STEPS { return None; }
    // Take the last 20 frames.
    let recent: Vec<&Vec<f64>> = history.iter().rev().take(TIME_STEPS).collect();
    // recent is reverse-chronological; we want chronological for forward pass.
    let recent: Vec<&Vec<f64>> = recent.into_iter().rev().collect();
    let n_sub = recent[0].len();
    if n_sub == 0 { return None; }

    // Per-subcarrier mean and std over the 20 frames.
    let mut score: Vec<(usize, f64)> = (0..n_sub).map(|k| {
        let mut sum = 0.0f64;
        for f in &recent { sum += f.get(k).copied().unwrap_or(0.0); }
        let mu = sum / TIME_STEPS as f64;
        if mu.abs() < 1e-9 { return (k, f64::INFINITY); }
        let mut var = 0.0f64;
        for f in &recent {
            let d = f.get(k).copied().unwrap_or(0.0) - mu;
            var += d * d;
        }
        let sigma = (var / TIME_STEPS as f64).sqrt();
        // NBVI (α = 0.5): 0.5 * (σ/μ²) + 0.5 * (σ/μ)
        let mu2 = mu * mu;
        let nbvi = 0.5 * (sigma / mu2) + 0.5 * (sigma / mu.abs());
        (k, nbvi)
    }).collect();

    // 25th-percentile dead-zone gate (drop subcarriers with mean amplitude
    // below the lower quartile).
    let mut means: Vec<f64> = (0..n_sub).map(|k| {
        let mut s = 0.0f64;
        for f in &recent { s += f.get(k).copied().unwrap_or(0.0); }
        s / TIME_STEPS as f64
    }).collect();
    means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let q25_idx = (n_sub as f64 * 0.25) as usize;
    let dead_thresh = means.get(q25_idx).copied().unwrap_or(0.0);
    for (k, s) in score.iter_mut() {
        // Re-compute mean for this k to gate (means above is sorted, indices lost).
        let mut sum = 0.0f64;
        for f in &recent { sum += f.get(*k).copied().unwrap_or(0.0); }
        let mu = sum / TIME_STEPS as f64;
        if mu < dead_thresh { *s = f64::INFINITY; }
    }

    score.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    if score.is_empty() || !score[0].1.is_finite() { return None; }

    // Pick top-INPUT_DIM (35) by lowest NBVI. If fewer than 35 are finite,
    // pad with whichever finite ones we have and zero the rest — model still
    // runs, it just has dead channels.
    let mut picks: Vec<usize> = score.iter()
        .filter(|(_, s)| s.is_finite())
        .take(INPUT_DIM)
        .map(|(k, _)| *k)
        .collect();
    if picks.is_empty() { return None; }
    while picks.len() < INPUT_DIM { picks.push(0); } // pad with subcarrier 0

    // Raw amplitudes pass-through. Training script (`scripts/train-wiflow-
    // supervised.js::loadJsonl`) feeds raw values; the two TCN BatchNorm
    // layers normalise per-channel per-window at inference time so absolute
    // scale (5–50 ESP32 amplitude range) is handled by the network itself.
    let mut out = vec![0.0f32; INPUT_DIM * TIME_STEPS];
    for (ci, k) in picks.iter().enumerate() {
        for (t, f) in recent.iter().enumerate() {
            out[ci * TIME_STEPS + t] = f.get(*k).copied().unwrap_or(0.0) as f32;
        }
    }
    Some(out)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip_alphabet() {
        // "Man" -> "TWFu"
        assert_eq!(base64_decode("TWFu").unwrap(), b"Man");
        // padding
        assert_eq!(base64_decode("TWE=").unwrap(), b"Ma");
        assert_eq!(base64_decode("TQ==").unwrap(), b"M");
        // whitespace tolerated
        assert_eq!(base64_decode("T W\nF u").unwrap(), b"Man");
    }

    #[test]
    fn sigmoid_bounds() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(10.0) > 0.999);
        assert!(sigmoid(-10.0) < 0.001);
    }

    #[test]
    fn build_input_zero_history() {
        let h = std::collections::VecDeque::new();
        assert!(build_input_from_history(&h).is_none());
    }
}

//! Sparse inference and weight quantization for edge deployment of WiFi DensePose.
//!
//! Implements ADR-023 Phase 6: activation profiling, sparse matrix-vector multiply,
//! INT8/FP16 quantization, and a full sparse inference engine. Pure Rust, no deps.

use std::time::Instant;

// ── Neuron Profiler ──────────────────────────────────────────────────────────

/// Tracks per-neuron activation frequency to partition hot vs cold neurons.
pub struct NeuronProfiler {
    activation_counts: Vec<u64>,
    samples: usize,
    n_neurons: usize,
}

impl NeuronProfiler {
    pub fn new(n_neurons: usize) -> Self {
        Self { activation_counts: vec![0; n_neurons], samples: 0, n_neurons }
    }

    /// Record an activation; values > 0 count as "active".
    pub fn record_activation(&mut self, neuron_idx: usize, activation: f32) {
        if neuron_idx < self.n_neurons && activation > 0.0 {
            self.activation_counts[neuron_idx] += 1;
        }
    }

    /// Mark end of one profiling sample (call after recording all neurons).
    pub fn end_sample(&mut self) { self.samples += 1; }

    /// Fraction of samples where the neuron fired (activation > 0).
    pub fn activation_frequency(&self, neuron_idx: usize) -> f32 {
        if neuron_idx >= self.n_neurons || self.samples == 0 { return 0.0; }
        self.activation_counts[neuron_idx] as f32 / self.samples as f32
    }

    /// Split neurons into (hot, cold) by activation frequency threshold.
    pub fn partition_hot_cold(&self, hot_threshold: f32) -> (Vec<usize>, Vec<usize>) {
        let mut hot = Vec::new();
        let mut cold = Vec::new();
        for i in 0..self.n_neurons {
            if self.activation_frequency(i) >= hot_threshold { hot.push(i); }
            else { cold.push(i); }
        }
        (hot, cold)
    }

    /// Top-k most frequently activated neuron indices.
    pub fn top_k_neurons(&self, k: usize) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.n_neurons).collect();
        idx.sort_by(|&a, &b| {
            self.activation_frequency(b).partial_cmp(&self.activation_frequency(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        idx.truncate(k);
        idx
    }

    /// Fraction of neurons with activation frequency < 0.1.
    pub fn sparsity_ratio(&self) -> f32 {
        if self.n_neurons == 0 || self.samples == 0 { return 0.0; }
        let cold = (0..self.n_neurons).filter(|&i| self.activation_frequency(i) < 0.1).count();
        cold as f32 / self.n_neurons as f32
    }

    pub fn total_samples(&self) -> usize { self.samples }
}

// ── Sparse Linear Layer ──────────────────────────────────────────────────────

/// Linear layer that only computes output rows for "hot" neurons.
pub struct SparseLinear {
    weights: Vec<Vec<f32>>,
    bias: Vec<f32>,
    hot_neurons: Vec<usize>,
    n_outputs: usize,
    n_inputs: usize,
}

impl SparseLinear {
    pub fn new(weights: Vec<Vec<f32>>, bias: Vec<f32>, hot_neurons: Vec<usize>) -> Self {
        let n_outputs = weights.len();
        let n_inputs = weights.first().map_or(0, |r| r.len());
        Self { weights, bias, hot_neurons, n_outputs, n_inputs }
    }

    /// Sparse forward: only compute hot rows; cold outputs are 0.
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0f32; self.n_outputs];
        for &r in &self.hot_neurons {
            if r < self.n_outputs { out[r] = dot_bias(&self.weights[r], input, self.bias[r]); }
        }
        out
    }

    /// Dense forward: compute all rows.
    pub fn forward_full(&self, input: &[f32]) -> Vec<f32> {
        (0..self.n_outputs).map(|r| dot_bias(&self.weights[r], input, self.bias[r])).collect()
    }

    pub fn set_hot_neurons(&mut self, hot: Vec<usize>) { self.hot_neurons = hot; }

    /// Fraction of neurons in the hot set.
    pub fn density(&self) -> f32 {
        if self.n_outputs == 0 { 0.0 } else { self.hot_neurons.len() as f32 / self.n_outputs as f32 }
    }

    /// Multiply-accumulate ops saved vs dense.
    pub fn n_flops_saved(&self) -> usize {
        self.n_outputs.saturating_sub(self.hot_neurons.len()) * self.n_inputs
    }
}

fn dot_bias(row: &[f32], input: &[f32], bias: f32) -> f32 {
    let len = row.len().min(input.len());
    let mut s = bias;
    for i in 0..len { s += row[i] * input[i]; }
    s
}

// ── Quantization ─────────────────────────────────────────────────────────────

/// Quantization mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuantMode { F32, F16, Int8Symmetric, Int8Asymmetric, Int4 }

/// Quantization configuration.
#[derive(Debug, Clone)]
pub struct QuantConfig { pub mode: QuantMode, pub calibration_samples: usize }

impl Default for QuantConfig {
    fn default() -> Self { Self { mode: QuantMode::Int8Symmetric, calibration_samples: 100 } }
}

/// Quantized weight storage.
#[derive(Debug, Clone)]
pub struct QuantizedWeights {
    pub data: Vec<i8>,
    pub scale: f32,
    pub zero_point: i8,
    pub mode: QuantMode,
}

pub struct Quantizer;

impl Quantizer {
    /// Symmetric INT8: zero maps to 0, scale = max(|w|)/127.
    pub fn quantize_symmetric(weights: &[f32]) -> QuantizedWeights {
        if weights.is_empty() {
            return QuantizedWeights { data: vec![], scale: 1.0, zero_point: 0, mode: QuantMode::Int8Symmetric };
        }
        let max_abs = weights.iter().map(|w| w.abs()).fold(0.0f32, f32::max);
        let scale = if max_abs < f32::EPSILON { 1.0 } else { max_abs / 127.0 };
        let data = weights.iter().map(|&w| (w / scale).round().clamp(-127.0, 127.0) as i8).collect();
        QuantizedWeights { data, scale, zero_point: 0, mode: QuantMode::Int8Symmetric }
    }

    /// Asymmetric INT8: maps [min,max] to [0,255].
    pub fn quantize_asymmetric(weights: &[f32]) -> QuantizedWeights {
        if weights.is_empty() {
            return QuantizedWeights { data: vec![], scale: 1.0, zero_point: 0, mode: QuantMode::Int8Asymmetric };
        }
        let w_min = weights.iter().cloned().fold(f32::INFINITY, f32::min);
        let w_max = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let range = w_max - w_min;
        let scale = if range < f32::EPSILON { 1.0 } else { range / 255.0 };
        let zp = if range < f32::EPSILON { 0u8 } else { (-w_min / scale).round().clamp(0.0, 255.0) as u8 };
        let data = weights.iter().map(|&w| ((w - w_min) / scale).round().clamp(0.0, 255.0) as u8 as i8).collect();
        QuantizedWeights { data, scale, zero_point: zp as i8, mode: QuantMode::Int8Asymmetric }
    }

    /// Reconstruct approximate f32 values from quantized weights.
    pub fn dequantize(qw: &QuantizedWeights) -> Vec<f32> {
        match qw.mode {
            QuantMode::Int8Symmetric => qw.data.iter().map(|&q| q as f32 * qw.scale).collect(),
            QuantMode::Int8Asymmetric => {
                let zp = qw.zero_point as u8;
                qw.data.iter().map(|&q| (q as u8 as f32 - zp as f32) * qw.scale).collect()
            }
            _ => qw.data.iter().map(|&q| q as f32 * qw.scale).collect(),
        }
    }

    /// MSE between original and quantized weights.
    pub fn quantization_error(original: &[f32], quantized: &QuantizedWeights) -> f32 {
        let deq = Self::dequantize(quantized);
        if original.len() != deq.len() || original.is_empty() { return f32::MAX; }
        original.iter().zip(deq.iter()).map(|(o, d)| (o - d).powi(2)).sum::<f32>() / original.len() as f32
    }

    /// Convert f32 to IEEE 754 half-precision (u16).
    pub fn f16_quantize(weights: &[f32]) -> Vec<u16> { weights.iter().map(|&w| f32_to_f16(w)).collect() }

    /// Convert FP16 (u16) back to f32.
    pub fn f16_dequantize(data: &[u16]) -> Vec<f32> { data.iter().map(|&h| f16_to_f32(h)).collect() }
}

// ── FP16 bit manipulation ────────────────────────────────────────────────────

fn f32_to_f16(val: f32) -> u16 {
    let bits = val.to_bits();
    let sign = (bits >> 31) & 1;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let man = bits & 0x007F_FFFF;

    if exp == 0xFF { // Inf or NaN
        let hm = if man != 0 { 0x0200 } else { 0 };
        return ((sign << 15) | 0x7C00 | hm) as u16;
    }
    if exp == 0 { return (sign << 15) as u16; } // zero / subnormal -> zero

    let ne = exp - 127 + 15;
    if ne >= 31 { return ((sign << 15) | 0x7C00) as u16; } // overflow -> Inf
    if ne <= 0 {
        if ne < -10 { return (sign << 15) as u16; }
        let full = man | 0x0080_0000;
        return ((sign << 15) | (full >> (13 + 1 - ne))) as u16;
    }
    ((sign << 15) | ((ne as u32) << 10) | (man >> 13)) as u16
}

fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let man = (h & 0x03FF) as u32;

    if exp == 0x1F {
        let fb = if man != 0 { (sign << 31) | 0x7F80_0000 | (man << 13) } else { (sign << 31) | 0x7F80_0000 };
        return f32::from_bits(fb);
    }
    if exp == 0 {
        if man == 0 { return f32::from_bits(sign << 31); }
        let mut m = man; let mut e: i32 = -14;
        while m & 0x0400 == 0 { m <<= 1; e -= 1; }
        m &= 0x03FF;
        return f32::from_bits((sign << 31) | (((e + 127) as u32) << 23) | (m << 13));
    }
    f32::from_bits((sign << 31) | ((exp as i32 - 15 + 127) as u32) << 23 | (man << 13))
}

// ── Sparse Model ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SparseConfig {
    pub hot_threshold: f32,
    pub quant_mode: QuantMode,
    pub profile_frames: usize,
}

impl Default for SparseConfig {
    fn default() -> Self { Self { hot_threshold: 0.5, quant_mode: QuantMode::Int8Symmetric, profile_frames: 100 } }
}

#[allow(dead_code)]
struct ModelLayer {
    name: String,
    weights: Vec<Vec<f32>>,
    bias: Vec<f32>,
    sparse: Option<SparseLinear>,
    profiler: NeuronProfiler,
    is_sparse: bool,
    /// Quantized weights per row (populated by apply_quantization).
    quantized: Option<Vec<QuantizedWeights>>,
    /// Whether to use quantized weights for forward pass.
    use_quantized: bool,
}

impl ModelLayer {
    fn new(name: &str, weights: Vec<Vec<f32>>, bias: Vec<f32>) -> Self {
        let n = weights.len();
        Self {
            name: name.into(), weights, bias, sparse: None,
            profiler: NeuronProfiler::new(n), is_sparse: false,
            quantized: None, use_quantized: false,
        }
    }
    fn forward_dense(&self, input: &[f32]) -> Vec<f32> {
        if self.use_quantized {
            if let Some(ref qrows) = self.quantized {
                return self.forward_quantized(input, qrows);
            }
        }
        self.weights.iter().enumerate().map(|(r, row)| dot_bias(row, input, self.bias[r])).collect()
    }
    /// Forward using dequantized weights: val = q_val * scale (symmetric).
    fn forward_quantized(&self, input: &[f32], qrows: &[QuantizedWeights]) -> Vec<f32> {
        let n_out = qrows.len().min(self.bias.len());
        let mut out = vec![0.0f32; n_out];
        for r in 0..n_out {
            let qw = &qrows[r];
            let len = qw.data.len().min(input.len());
            let mut s = self.bias[r];
            for i in 0..len {
                let w = (qw.data[i] as f32 - qw.zero_point as f32) * qw.scale;
                s += w * input[i];
            }
            out[r] = s;
        }
        out
    }
    fn forward(&self, input: &[f32]) -> Vec<f32> {
        if self.is_sparse { if let Some(ref s) = self.sparse { return s.forward(input); } }
        self.forward_dense(input)
    }
}

#[derive(Debug, Clone)]
pub struct ModelStats {
    pub total_params: usize,
    pub hot_params: usize,
    pub cold_params: usize,
    pub sparsity: f32,
    pub quant_mode: QuantMode,
    pub est_memory_bytes: usize,
    pub est_flops: usize,
}

/// Full sparse inference engine: profiling + sparsity + quantization.
pub struct SparseModel {
    layers: Vec<ModelLayer>,
    config: SparseConfig,
    profiled: bool,
}

impl SparseModel {
    pub fn new(config: SparseConfig) -> Self { Self { layers: vec![], config, profiled: false } }

    pub fn add_layer(&mut self, name: &str, weights: Vec<Vec<f32>>, bias: Vec<f32>) {
        self.layers.push(ModelLayer::new(name, weights, bias));
    }

    /// Profile activation frequencies over sample inputs.
    pub fn profile(&mut self, inputs: &[Vec<f32>]) {
        let n = inputs.len().min(self.config.profile_frames);
        for sample in inputs.iter().take(n) {
            let mut act = sample.clone();
            for layer in &mut self.layers {
                let out = layer.forward_dense(&act);
                for (i, &v) in out.iter().enumerate() { layer.profiler.record_activation(i, v); }
                layer.profiler.end_sample();
                act = out.iter().map(|&v| v.max(0.0)).collect();
            }
        }
        self.profiled = true;
    }

    /// Convert layers to sparse using profiled hot/cold partition.
    pub fn apply_sparsity(&mut self) {
        if !self.profiled { return; }
        let th = self.config.hot_threshold;
        for layer in &mut self.layers {
            let (hot, _) = layer.profiler.partition_hot_cold(th);
            layer.sparse = Some(SparseLinear::new(layer.weights.clone(), layer.bias.clone(), hot));
            layer.is_sparse = true;
        }
    }

    /// Quantize weights using INT8 codebook per the config. After this call,
    /// forward() uses dequantized weights (val = (q - zero_point) * scale).
    pub fn apply_quantization(&mut self) {
        for layer in &mut self.layers {
            let qrows: Vec<QuantizedWeights> = layer.weights.iter().map(|row| {
                match self.config.quant_mode {
                    QuantMode::Int8Symmetric => Quantizer::quantize_symmetric(row),
                    QuantMode::Int8Asymmetric => Quantizer::quantize_asymmetric(row),
                    _ => Quantizer::quantize_symmetric(row),
                }
            }).collect();
            layer.quantized = Some(qrows);
            layer.use_quantized = true;
        }
    }

    /// Forward pass through all layers with ReLU activation.
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        let mut act = input.to_vec();
        for layer in &self.layers {
            act = layer.forward(&act).iter().map(|&v| v.max(0.0)).collect();
        }
        act
    }

    pub fn n_layers(&self) -> usize { self.layers.len() }

    pub fn stats(&self) -> ModelStats {
        let (mut total, mut hot, mut cold, mut flops) = (0, 0, 0, 0);
        for layer in &self.layers {
            let (no, ni) = (layer.weights.len(), layer.weights.first().map_or(0, |r| r.len()));
            let lp = no * ni + no;
            total += lp;
            if let Some(ref s) = layer.sparse {
                let hc = s.hot_neurons.len();
                hot += hc * ni + hc;
                cold += (no - hc) * ni + (no - hc);
                flops += hc * ni;
            } else { hot += lp; flops += no * ni; }
        }
        let bpp = match self.config.quant_mode {
            QuantMode::F32 => 4, QuantMode::F16 => 2,
            QuantMode::Int8Symmetric | QuantMode::Int8Asymmetric => 1,
            QuantMode::Int4 => 1,
        };
        ModelStats {
            total_params: total, hot_params: hot, cold_params: cold,
            sparsity: if total > 0 { cold as f32 / total as f32 } else { 0.0 },
            quant_mode: self.config.quant_mode, est_memory_bytes: hot * bpp, est_flops: flops,
        }
    }
}

// ── Benchmark Runner ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub mean_latency_us: f64,
    pub p50_us: f64,
    pub p99_us: f64,
    pub throughput_fps: f64,
    pub memory_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub dense_latency_us: f64,
    pub sparse_latency_us: f64,
    pub speedup: f64,
    pub accuracy_loss: f32,
}

pub struct BenchmarkRunner;

impl BenchmarkRunner {
    pub fn benchmark_inference(model: &SparseModel, input: &[f32], n: usize) -> BenchmarkResult {
        let mut lat = Vec::with_capacity(n);
        for _ in 0..n {
            let t = Instant::now();
            let _ = model.forward(input);
            lat.push(t.elapsed().as_micros() as f64);
        }
        lat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let sum: f64 = lat.iter().sum();
        let mean = sum / lat.len().max(1) as f64;
        let total_s = sum / 1e6;
        BenchmarkResult {
            mean_latency_us: mean,
            p50_us: pctl(&lat, 50), p99_us: pctl(&lat, 99),
            throughput_fps: if total_s > 0.0 { n as f64 / total_s } else { f64::INFINITY },
            memory_bytes: model.stats().est_memory_bytes,
        }
    }

    pub fn compare_dense_vs_sparse(
        dw: &[Vec<Vec<f32>>], db: &[Vec<f32>], sparse: &SparseModel, input: &[f32], n: usize,
    ) -> ComparisonResult {
        // Dense timing
        let mut dl = Vec::with_capacity(n);
        let mut d_out = Vec::new();
        for _ in 0..n {
            let t = Instant::now();
            let mut a = input.to_vec();
            for (w, b) in dw.iter().zip(db.iter()) {
                a = w.iter().enumerate().map(|(r, row)| dot_bias(row, &a, b[r])).collect::<Vec<_>>()
                    .iter().map(|&v| v.max(0.0)).collect();
            }
            d_out = a;
            dl.push(t.elapsed().as_micros() as f64);
        }
        // Sparse timing
        let mut sl = Vec::with_capacity(n);
        let mut s_out = Vec::new();
        for _ in 0..n {
            let t = Instant::now();
            s_out = sparse.forward(input);
            sl.push(t.elapsed().as_micros() as f64);
        }
        let dm: f64 = dl.iter().sum::<f64>() / dl.len().max(1) as f64;
        let sm: f64 = sl.iter().sum::<f64>() / sl.len().max(1) as f64;
        let loss = if !d_out.is_empty() && d_out.len() == s_out.len() {
            d_out.iter().zip(s_out.iter()).map(|(d, s)| (d - s).powi(2)).sum::<f32>() / d_out.len() as f32
        } else { 0.0 };
        ComparisonResult {
            dense_latency_us: dm, sparse_latency_us: sm,
            speedup: if sm > 0.0 { dm / sm } else { 1.0 }, accuracy_loss: loss,
        }
    }
}

fn pctl(sorted: &[f64], p: usize) -> f64 {
    if sorted.is_empty() { return 0.0; }
    let i = (p as f64 / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neuron_profiler_initially_empty() {
        let p = NeuronProfiler::new(10);
        assert_eq!(p.total_samples(), 0);
        assert_eq!(p.activation_frequency(0), 0.0);
        assert_eq!(p.sparsity_ratio(), 0.0);
    }

    #[test]
    fn neuron_profiler_records_activations() {
        let mut p = NeuronProfiler::new(4);
        p.record_activation(0, 1.0); p.record_activation(1, 0.5);
        p.record_activation(2, 0.1); p.record_activation(3, 0.0);
        p.end_sample();
        p.record_activation(0, 2.0); p.record_activation(1, 0.0);
        p.record_activation(2, 0.0); p.record_activation(3, 0.0);
        p.end_sample();
        assert_eq!(p.total_samples(), 2);
        assert_eq!(p.activation_frequency(0), 1.0);
        assert_eq!(p.activation_frequency(1), 0.5);
        assert_eq!(p.activation_frequency(3), 0.0);
    }

    #[test]
    fn neuron_profiler_hot_cold_partition() {
        let mut p = NeuronProfiler::new(5);
        for _ in 0..20 {
            p.record_activation(0, 1.0); p.record_activation(1, 1.0);
            p.record_activation(2, 0.0); p.record_activation(3, 0.0);
            p.record_activation(4, 0.0); p.end_sample();
        }
        let (hot, cold) = p.partition_hot_cold(0.5);
        assert!(hot.contains(&0) && hot.contains(&1));
        assert!(cold.contains(&2) && cold.contains(&3) && cold.contains(&4));
    }

    #[test]
    fn neuron_profiler_sparsity_ratio() {
        let mut p = NeuronProfiler::new(10);
        for _ in 0..20 {
            p.record_activation(0, 1.0); p.record_activation(1, 1.0);
            for j in 2..10 { p.record_activation(j, 0.0); }
            p.end_sample();
        }
        assert!((p.sparsity_ratio() - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn sparse_linear_matches_dense() {
        let w = vec![vec![1.0,2.0,3.0], vec![4.0,5.0,6.0], vec![7.0,8.0,9.0]];
        let b = vec![0.1, 0.2, 0.3];
        let layer = SparseLinear::new(w, b, vec![0,1,2]);
        let inp = vec![1.0, 0.5, -1.0];
        let (so, do_) = (layer.forward(&inp), layer.forward_full(&inp));
        for (s, d) in so.iter().zip(do_.iter()) { assert!((s - d).abs() < 1e-6); }
    }

    #[test]
    fn sparse_linear_skips_cold_neurons() {
        let w = vec![vec![1.0,2.0], vec![3.0,4.0], vec![5.0,6.0]];
        let layer = SparseLinear::new(w, vec![0.0;3], vec![1]);
        let out = layer.forward(&[1.0, 1.0]);
        assert_eq!(out[0], 0.0);
        assert_eq!(out[2], 0.0);
        assert!((out[1] - 7.0).abs() < 1e-6);
    }

    #[test]
    fn sparse_linear_flops_saved() {
        let w: Vec<Vec<f32>> = (0..4).map(|_| vec![1.0; 4]).collect();
        let layer = SparseLinear::new(w, vec![0.0;4], vec![0,2]);
        assert_eq!(layer.n_flops_saved(), 8);
        assert!((layer.density() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn quantize_symmetric_range() {
        let qw = Quantizer::quantize_symmetric(&[-1.0, 0.0, 0.5, 1.0]);
        assert!((qw.scale - 1.0/127.0).abs() < 1e-6);
        assert_eq!(qw.zero_point, 0);
        assert_eq!(*qw.data.last().unwrap(), 127);
        assert_eq!(qw.data[0], -127);
    }

    #[test]
    fn quantize_symmetric_zero_is_zero() {
        let qw = Quantizer::quantize_symmetric(&[-5.0, 0.0, 3.0, 5.0]);
        assert_eq!(qw.data[1], 0);
    }

    #[test]
    fn quantize_asymmetric_range() {
        let qw = Quantizer::quantize_asymmetric(&[0.0, 0.5, 1.0]);
        assert!((qw.scale - 1.0/255.0).abs() < 1e-4);
        assert_eq!(qw.zero_point as u8, 0);
    }

    #[test]
    fn dequantize_round_trip_small_error() {
        let w: Vec<f32> = (-50..50).map(|i| i as f32 * 0.02).collect();
        let qw = Quantizer::quantize_symmetric(&w);
        assert!(Quantizer::quantization_error(&w, &qw) < 0.01);
    }

    #[test]
    fn int8_quantization_error_bounded() {
        let w: Vec<f32> = (0..256).map(|i| (i as f32 * 1.7).sin() * 2.0).collect();
        assert!(Quantizer::quantization_error(&w, &Quantizer::quantize_symmetric(&w)) < 0.01);
        assert!(Quantizer::quantization_error(&w, &Quantizer::quantize_asymmetric(&w)) < 0.01);
    }

    #[test]
    fn f16_round_trip_precision() {
        for &v in &[1.0f32, 0.5, -0.5, 3.14, 100.0, 0.001, -42.0, 65504.0] {
            let enc = Quantizer::f16_quantize(&[v]);
            let dec = Quantizer::f16_dequantize(&enc)[0];
            let re = if v.abs() > 1e-6 { ((v - dec) / v).abs() } else { (v - dec).abs() };
            assert!(re < 0.001, "f16 error for {v}: decoded={dec}, rel={re}");
        }
    }

    #[test]
    fn f16_special_values() {
        assert_eq!(Quantizer::f16_dequantize(&Quantizer::f16_quantize(&[0.0]))[0], 0.0);
        let inf = Quantizer::f16_dequantize(&Quantizer::f16_quantize(&[f32::INFINITY]))[0];
        assert!(inf.is_infinite() && inf > 0.0);
        let ninf = Quantizer::f16_dequantize(&Quantizer::f16_quantize(&[f32::NEG_INFINITY]))[0];
        assert!(ninf.is_infinite() && ninf < 0.0);
        assert!(Quantizer::f16_dequantize(&Quantizer::f16_quantize(&[f32::NAN]))[0].is_nan());
    }

    #[test]
    fn sparse_model_add_layers() {
        let mut m = SparseModel::new(SparseConfig::default());
        m.add_layer("l1", vec![vec![1.0,2.0],vec![3.0,4.0]], vec![0.0,0.0]);
        m.add_layer("l2", vec![vec![0.5,-0.5],vec![1.0,1.0]], vec![0.1,0.2]);
        assert_eq!(m.n_layers(), 2);
        let out = m.forward(&[1.0, 1.0]);
        assert!(out[0] < 0.001); // ReLU zeros negative
        assert!((out[1] - 10.2).abs() < 0.01);
    }

    #[test]
    fn sparse_model_profile_and_apply() {
        let mut m = SparseModel::new(SparseConfig { hot_threshold: 0.3, ..Default::default() });
        m.add_layer("h", vec![
            vec![1.0;4], vec![0.5;4], vec![-2.0;4], vec![-1.0;4],
        ], vec![0.0;4]);
        let inp: Vec<Vec<f32>> = (0..50).map(|i| vec![1.0 + i as f32 * 0.01; 4]).collect();
        m.profile(&inp);
        m.apply_sparsity();
        let s = m.stats();
        assert!(s.cold_params > 0);
        assert!(s.sparsity > 0.0);
    }

    #[test]
    fn sparse_model_stats_report() {
        let mut m = SparseModel::new(SparseConfig::default());
        m.add_layer("fc1", vec![vec![1.0;8];16], vec![0.0;16]);
        let s = m.stats();
        assert_eq!(s.total_params, 16*8+16);
        assert_eq!(s.quant_mode, QuantMode::Int8Symmetric);
        assert!(s.est_flops > 0 && s.est_memory_bytes > 0);
    }

    #[test]
    fn benchmark_produces_positive_latency() {
        let mut m = SparseModel::new(SparseConfig::default());
        m.add_layer("fc1", vec![vec![1.0;4];4], vec![0.0;4]);
        let r = BenchmarkRunner::benchmark_inference(&m, &[1.0;4], 10);
        assert!(r.mean_latency_us >= 0.0 && r.throughput_fps > 0.0);
    }

    #[test]
    fn compare_dense_sparse_speedup() {
        let w = vec![vec![1.0f32;8];16];
        let b = vec![0.0f32;16];
        let mut pm = SparseModel::new(SparseConfig { hot_threshold: 0.5, quant_mode: QuantMode::F32, profile_frames: 20 });
        let mut pw: Vec<Vec<f32>> = w.clone();
        for row in pw.iter_mut().skip(8) { for v in row.iter_mut() { *v = -1.0; } }
        pm.add_layer("fc1", pw, b.clone());
        let inp: Vec<Vec<f32>> = (0..20).map(|_| vec![1.0;8]).collect();
        pm.profile(&inp); pm.apply_sparsity();
        let r = BenchmarkRunner::compare_dense_vs_sparse(&[w], &[b], &pm, &[1.0;8], 50);
        assert!(r.dense_latency_us >= 0.0 && r.sparse_latency_us >= 0.0);
        assert!(r.speedup > 0.0);
        assert!(r.accuracy_loss.is_finite());
    }
}

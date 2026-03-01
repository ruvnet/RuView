//! MERIDIAN Phase 3 -- Geometry Encoder with FiLM Conditioning (ADR-027).
//!
//! Permutation-invariant encoding of AP positions into a 64-dim geometry
//! vector, plus FiLM layers for conditioning backbone features on room
//! geometry.  Pure Rust, no external dependencies beyond the workspace.

use serde::{Deserialize, Serialize};

const GEOMETRY_DIM: usize = 64;
const NUM_COORDS: usize = 3;

// ---------------------------------------------------------------------------
// Linear layer (pure Rust)
// ---------------------------------------------------------------------------

/// Fully-connected layer: `y = x W^T + b`.  Row-major weights `[out, in]`.
#[derive(Debug, Clone)]
struct Linear {
    weights: Vec<f32>,
    bias: Vec<f32>,
    in_f: usize,
    out_f: usize,
}

impl Linear {
    /// Kaiming-uniform init: U(-k, k), k = sqrt(1/in_f).
    fn new(in_f: usize, out_f: usize, seed: u64) -> Self {
        let k = (1.0 / in_f as f32).sqrt();
        Linear {
            weights: det_uniform(in_f * out_f, -k, k, seed),
            bias: vec![0.0; out_f],
            in_f,
            out_f,
        }
    }

    fn forward(&self, x: &[f32]) -> Vec<f32> {
        debug_assert_eq!(x.len(), self.in_f);
        let mut y = self.bias.clone();
        for j in 0..self.out_f {
            let off = j * self.in_f;
            let mut s = 0.0f32;
            for i in 0..self.in_f {
                s += x[i] * self.weights[off + i];
            }
            y[j] += s;
        }
        y
    }
}

/// Deterministic xorshift64 uniform in `[lo, hi)`.
/// Uses 24-bit precision (matching f32 mantissa) for uniform distribution.
fn det_uniform(n: usize, lo: f32, hi: f32, seed: u64) -> Vec<f32> {
    let r = hi - lo;
    let mut s = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            lo + (s >> 40) as f32 / (1u64 << 24) as f32 * r
        })
        .collect()
}

fn relu(v: &mut [f32]) {
    for x in v.iter_mut() {
        if *x < 0.0 { *x = 0.0; }
    }
}

// ---------------------------------------------------------------------------
// MeridianGeometryConfig
// ---------------------------------------------------------------------------

/// Configuration for the MERIDIAN geometry encoder and FiLM layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeridianGeometryConfig {
    /// Number of Fourier frequency bands (default 10).
    pub n_frequencies: usize,
    /// Spatial scale factor, 1.0 = metres (default 1.0).
    pub scale: f32,
    /// Output embedding dimension (default 64).
    pub geometry_dim: usize,
    /// Random seed for weight init (default 42).
    pub seed: u64,
}

impl Default for MeridianGeometryConfig {
    fn default() -> Self {
        MeridianGeometryConfig { n_frequencies: 10, scale: 1.0, geometry_dim: GEOMETRY_DIM, seed: 42 }
    }
}

// ---------------------------------------------------------------------------
// FourierPositionalEncoding
// ---------------------------------------------------------------------------

/// Fourier positional encoding for 3-D coordinates.
///
/// Per coordinate: `[sin(2^0*pi*x), cos(2^0*pi*x), ..., sin(2^(L-1)*pi*x),
/// cos(2^(L-1)*pi*x)]`.  Zero-padded to `geometry_dim`.
pub struct FourierPositionalEncoding {
    n_frequencies: usize,
    scale: f32,
    output_dim: usize,
}

impl FourierPositionalEncoding {
    /// Create from config.
    pub fn new(cfg: &MeridianGeometryConfig) -> Self {
        FourierPositionalEncoding { n_frequencies: cfg.n_frequencies, scale: cfg.scale, output_dim: cfg.geometry_dim }
    }

    /// Encode `[x, y, z]` into a fixed-length vector of `geometry_dim` elements.
    pub fn encode(&self, coords: &[f32; 3]) -> Vec<f32> {
        let raw = NUM_COORDS * 2 * self.n_frequencies;
        let mut enc = Vec::with_capacity(raw.max(self.output_dim));
        for &c in coords {
            let sc = c * self.scale;
            for l in 0..self.n_frequencies {
                let f = (2.0f32).powi(l as i32) * std::f32::consts::PI * sc;
                enc.push(f.sin());
                enc.push(f.cos());
            }
        }
        enc.resize(self.output_dim, 0.0);
        enc
    }
}

// ---------------------------------------------------------------------------
// DeepSets
// ---------------------------------------------------------------------------

/// Permutation-invariant set encoder: phi each element, mean-pool, then rho.
pub struct DeepSets {
    phi: Linear,
    rho: Linear,
    dim: usize,
}

impl DeepSets {
    /// Create from config.
    pub fn new(cfg: &MeridianGeometryConfig) -> Self {
        let d = cfg.geometry_dim;
        DeepSets { phi: Linear::new(d, d, cfg.seed.wrapping_add(1)), rho: Linear::new(d, d, cfg.seed.wrapping_add(2)), dim: d }
    }

    /// Encode a set of embeddings (each of length `geometry_dim`) into one vector.
    pub fn encode(&self, ap_embeddings: &[Vec<f32>]) -> Vec<f32> {
        assert!(!ap_embeddings.is_empty(), "DeepSets: input set must be non-empty");
        let n = ap_embeddings.len() as f32;
        let mut pooled = vec![0.0f32; self.dim];
        for emb in ap_embeddings {
            debug_assert_eq!(emb.len(), self.dim);
            let mut t = self.phi.forward(emb);
            relu(&mut t);
            for (p, v) in pooled.iter_mut().zip(t.iter()) { *p += *v; }
        }
        for p in pooled.iter_mut() { *p /= n; }
        let mut out = self.rho.forward(&pooled);
        relu(&mut out);
        out
    }
}

// ---------------------------------------------------------------------------
// GeometryEncoder
// ---------------------------------------------------------------------------

/// End-to-end encoder: AP positions -> 64-dim geometry vector.
pub struct GeometryEncoder {
    pos_embed: FourierPositionalEncoding,
    set_encoder: DeepSets,
}

impl GeometryEncoder {
    /// Build from config.
    pub fn new(cfg: &MeridianGeometryConfig) -> Self {
        GeometryEncoder { pos_embed: FourierPositionalEncoding::new(cfg), set_encoder: DeepSets::new(cfg) }
    }

    /// Encode variable-count AP positions `[x,y,z]` into a fixed-dim vector.
    pub fn encode(&self, ap_positions: &[[f32; 3]]) -> Vec<f32> {
        let embs: Vec<Vec<f32>> = ap_positions.iter().map(|p| self.pos_embed.encode(p)).collect();
        self.set_encoder.encode(&embs)
    }
}

// ---------------------------------------------------------------------------
// FilmLayer
// ---------------------------------------------------------------------------

/// Feature-wise Linear Modulation: `output = gamma(g) * h + beta(g)`.
pub struct FilmLayer {
    gamma_proj: Linear,
    beta_proj: Linear,
}

impl FilmLayer {
    /// Create a FiLM layer.  Gamma bias is initialised to 1.0 (identity).
    pub fn new(cfg: &MeridianGeometryConfig) -> Self {
        let d = cfg.geometry_dim;
        let mut gamma_proj = Linear::new(d, d, cfg.seed.wrapping_add(3));
        for b in gamma_proj.bias.iter_mut() { *b = 1.0; }
        FilmLayer { gamma_proj, beta_proj: Linear::new(d, d, cfg.seed.wrapping_add(4)) }
    }

    /// Modulate `features` by `geometry`: `gamma(geometry) * features + beta(geometry)`.
    pub fn modulate(&self, features: &[f32], geometry: &[f32]) -> Vec<f32> {
        let gamma = self.gamma_proj.forward(geometry);
        let beta = self.beta_proj.forward(geometry);
        features.iter().zip(gamma.iter()).zip(beta.iter()).map(|((&f, &g), &b)| g * f + b).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> MeridianGeometryConfig { MeridianGeometryConfig::default() }

    #[test]
    fn fourier_output_dimension_is_64() {
        let c = cfg();
        let out = FourierPositionalEncoding::new(&c).encode(&[1.0, 2.0, 3.0]);
        assert_eq!(out.len(), c.geometry_dim);
    }

    #[test]
    fn fourier_different_coords_different_outputs() {
        let enc = FourierPositionalEncoding::new(&cfg());
        let a = enc.encode(&[0.0, 0.0, 0.0]);
        let b = enc.encode(&[1.0, 0.0, 0.0]);
        let c = enc.encode(&[0.0, 1.0, 0.0]);
        let d = enc.encode(&[0.0, 0.0, 1.0]);
        assert_ne!(a, b); assert_ne!(a, c); assert_ne!(a, d); assert_ne!(b, c);
    }

    #[test]
    fn fourier_values_bounded() {
        let out = FourierPositionalEncoding::new(&cfg()).encode(&[5.5, -3.2, 0.1]);
        for &v in &out { assert!(v.abs() <= 1.0 + 1e-6, "got {v}"); }
    }

    #[test]
    fn deepsets_permutation_invariant() {
        let c = cfg();
        let enc = FourierPositionalEncoding::new(&c);
        let ds = DeepSets::new(&c);
        let (a, b, d) = (enc.encode(&[1.0,0.0,0.0]), enc.encode(&[0.0,2.0,0.0]), enc.encode(&[0.0,0.0,3.0]));
        let abc = ds.encode(&[a.clone(), b.clone(), d.clone()]);
        let cba = ds.encode(&[d.clone(), b.clone(), a.clone()]);
        let bac = ds.encode(&[b.clone(), a.clone(), d.clone()]);
        for i in 0..c.geometry_dim {
            assert!((abc[i] - cba[i]).abs() < 1e-5, "dim {i}: abc={} cba={}", abc[i], cba[i]);
            assert!((abc[i] - bac[i]).abs() < 1e-5, "dim {i}: abc={} bac={}", abc[i], bac[i]);
        }
    }

    #[test]
    fn deepsets_variable_ap_count() {
        let c = cfg();
        let enc = FourierPositionalEncoding::new(&c);
        let ds = DeepSets::new(&c);
        let one = ds.encode(&[enc.encode(&[1.0,0.0,0.0])]);
        assert_eq!(one.len(), c.geometry_dim);
        let three = ds.encode(&[enc.encode(&[1.0,0.0,0.0]), enc.encode(&[0.0,2.0,0.0]), enc.encode(&[0.0,0.0,3.0])]);
        assert_eq!(three.len(), c.geometry_dim);
        let six = ds.encode(&[
            enc.encode(&[1.0,0.0,0.0]), enc.encode(&[0.0,2.0,0.0]), enc.encode(&[0.0,0.0,3.0]),
            enc.encode(&[-1.0,0.0,0.0]), enc.encode(&[0.0,-2.0,0.0]), enc.encode(&[0.0,0.0,-3.0]),
        ]);
        assert_eq!(six.len(), c.geometry_dim);
        assert_ne!(one, three); assert_ne!(three, six);
    }

    #[test]
    fn geometry_encoder_end_to_end() {
        let c = cfg();
        let g = GeometryEncoder::new(&c).encode(&[[1.0,0.0,2.5],[0.0,3.0,2.5],[-2.0,1.0,2.5]]);
        assert_eq!(g.len(), c.geometry_dim);
        for &v in &g { assert!(v.is_finite()); }
    }

    #[test]
    fn geometry_encoder_single_ap() {
        let c = cfg();
        assert_eq!(GeometryEncoder::new(&c).encode(&[[0.0,0.0,0.0]]).len(), c.geometry_dim);
    }

    #[test]
    fn film_identity_when_geometry_zero() {
        let c = cfg();
        let film = FilmLayer::new(&c);
        let feat = vec![1.0f32; c.geometry_dim];
        let out = film.modulate(&feat, &vec![0.0f32; c.geometry_dim]);
        assert_eq!(out.len(), c.geometry_dim);
        // gamma_proj(0) = bias = [1.0], beta_proj(0) = bias = [0.0] => identity
        for i in 0..c.geometry_dim {
            assert!((out[i] - feat[i]).abs() < 1e-5, "dim {i}: expected {}, got {}", feat[i], out[i]);
        }
    }

    #[test]
    fn film_nontrivial_modulation() {
        let c = cfg();
        let film = FilmLayer::new(&c);
        let feat: Vec<f32> = (0..c.geometry_dim).map(|i| i as f32 * 0.1).collect();
        let geom: Vec<f32> = (0..c.geometry_dim).map(|i| (i as f32 - 32.0) * 0.01).collect();
        let out = film.modulate(&feat, &geom);
        assert_eq!(out.len(), c.geometry_dim);
        assert!(out.iter().zip(feat.iter()).any(|(o, f)| (o - f).abs() > 1e-6));
        for &v in &out { assert!(v.is_finite()); }
    }

    #[test]
    fn film_explicit_gamma_beta() {
        let c = MeridianGeometryConfig { geometry_dim: 4, ..cfg() };
        let mut film = FilmLayer::new(&c);
        film.gamma_proj.weights = vec![0.0; 16];
        film.gamma_proj.bias = vec![2.0, 3.0, 0.5, 1.0];
        film.beta_proj.weights = vec![0.0; 16];
        film.beta_proj.bias = vec![10.0, 20.0, 30.0, 40.0];
        let out = film.modulate(&[1.0, 2.0, 3.0, 4.0], &[999.0; 4]);
        let exp = [12.0, 26.0, 31.5, 44.0];
        for i in 0..4 { assert!((out[i] - exp[i]).abs() < 1e-5, "dim {i}"); }
    }

    #[test]
    fn config_defaults() {
        let c = MeridianGeometryConfig::default();
        assert_eq!(c.n_frequencies, 10);
        assert!((c.scale - 1.0).abs() < 1e-6);
        assert_eq!(c.geometry_dim, 64);
        assert_eq!(c.seed, 42);
    }

    #[test]
    fn config_serde_round_trip() {
        let c = MeridianGeometryConfig { n_frequencies: 8, scale: 0.5, geometry_dim: 32, seed: 123 };
        let j = serde_json::to_string(&c).unwrap();
        let d: MeridianGeometryConfig = serde_json::from_str(&j).unwrap();
        assert_eq!(d.n_frequencies, 8); assert!((d.scale - 0.5).abs() < 1e-6);
        assert_eq!(d.geometry_dim, 32); assert_eq!(d.seed, 123);
    }

    #[test]
    fn linear_forward_dim() {
        assert_eq!(Linear::new(8, 4, 0).forward(&vec![1.0; 8]).len(), 4);
    }

    #[test]
    fn linear_zero_input_gives_bias() {
        let lin = Linear::new(4, 3, 0);
        let out = lin.forward(&[0.0; 4]);
        for i in 0..3 { assert!((out[i] - lin.bias[i]).abs() < 1e-6); }
    }
}

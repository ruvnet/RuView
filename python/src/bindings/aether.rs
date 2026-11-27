//! ADR-185 P1 — PyO3 bindings for AETHER contrastive CSI embeddings.
//!
//! Surfaces the **pure-sync** contrastive-embedding compute from
//! `wifi-densepose-aether::embedding` (ADR-024; the std-only leaf hoisted per
//! ADR-185 §13) into `wifi_densepose.aether`:
//!
//! - `AetherConfig`      — wraps `EmbeddingConfig` (d_model / d_proj /
//!                          temperature / normalize)
//! - `CsiAugmenter`      — SimCLR-style augmentation pair generator
//! - `EmbeddingExtractor`— backbone + projection → 128-dim L2-normed embedding
//! - `info_nce_loss`     — NT-Xent contrastive loss (module function)
//! - `cosine_similarity` — re-ID similarity helper (module function)
//!
//! ## Honest scope vs ADR-185 §3.2
//!
//! ADR-185 §3.2 names an aspirational surface (`aether_loss` returning
//! VICReg components, `alignment_metric`, `uniformity_metric`,
//! `forward_dual`, an `AetherConfig` with `vicreg_*` fields). Those do
//! **not** exist in the backing crate at HEAD — `embedding.rs` exposes
//! `EmbeddingConfig { d_model, d_proj, temperature, normalize }`,
//! `info_nce_loss` (plain `f32`), `CsiAugmenter::augment_pair`, and
//! `EmbeddingExtractor::extract`. This binding surfaces **what actually
//! exists** rather than fabricating the ADR's wished-for API. The
//! VICReg loss / metric surface is a Rust-side gap, not a binding gap.
//!
//! ## GIL release strategy (per ADR-117 §7, matching bindings/vitals.rs)
//!
//! `extract`, `augment_pair`, and `info_nce_loss` are pure-sync matrix
//! ops touching no Python objects, so they run inside
//! `py.detach(|| ...)`.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use wifi_densepose_aether::embedding::{
    info_nce_loss as rust_info_nce_loss, CsiAugmenter, EmbeddingConfig, EmbeddingExtractor,
};
use wifi_densepose_aether::graph_transformer::TransformerConfig;

/// Upper bound on model/CSI dimensions accepted from Python. The transformer
/// allocates weight matrices quadratic in these, so this caps a single
/// construction well under a gigabyte and turns an accidental or malicious
/// `d_model=100_000` into a `ValueError` instead of an allocation that aborts
/// the interpreter. Generous relative to real configs (defaults 64/128); raise
/// deliberately if a workload genuinely needs larger.
const MAX_DIM: usize = 4096;
/// Upper bound on GNN layer count — a sanity cap, not a modelling limit.
const MAX_LAYERS: usize = 64;

// ─── AetherConfig ────────────────────────────────────────────────────

/// Configuration for the contrastive embedding model.
///
/// Python:
/// ```python
/// from wifi_densepose.aether import AetherConfig
/// cfg = AetherConfig(d_model=64, d_proj=128, temperature=0.07, normalize=True)
/// ```
#[pyclass(from_py_object, frozen, name = "AetherConfig")]
#[derive(Clone)]
pub struct PyAetherConfig {
    inner: EmbeddingConfig,
}

#[pymethods]
impl PyAetherConfig {
    #[new]
    #[pyo3(signature = (d_model=64, d_proj=128, temperature=0.07, normalize=true))]
    fn new(d_model: usize, d_proj: usize, temperature: f32, normalize: bool) -> PyResult<Self> {
        // Validate at the boundary and raise ValueError. The native constructor
        // allocates weight matrices quadratic in these dims and (elsewhere)
        // divides by them, so zero or absurd values would otherwise reach Rust
        // as a panic (surfacing to Python as an opaque PanicException) or a
        // multi-gigabyte allocation that aborts the interpreter.
        if d_model == 0 || d_proj == 0 {
            return Err(PyValueError::new_err(
                "d_model and d_proj must be positive",
            ));
        }
        if d_model > MAX_DIM || d_proj > MAX_DIM {
            return Err(PyValueError::new_err(format!(
                "d_model ({d_model}) and d_proj ({d_proj}) must be <= {MAX_DIM}"
            )));
        }
        Ok(Self {
            inner: EmbeddingConfig {
                d_model,
                d_proj,
                temperature,
                normalize,
            },
        })
    }

    #[getter]
    fn d_model(&self) -> usize {
        self.inner.d_model
    }

    #[getter]
    fn d_proj(&self) -> usize {
        self.inner.d_proj
    }

    #[getter]
    fn temperature(&self) -> f32 {
        self.inner.temperature
    }

    #[getter]
    fn normalize(&self) -> bool {
        self.inner.normalize
    }

    fn __repr__(&self) -> String {
        format!(
            "AetherConfig(d_model={}, d_proj={}, temperature={}, normalize={})",
            self.inner.d_model, self.inner.d_proj, self.inner.temperature, self.inner.normalize,
        )
    }
}

// ─── CsiAugmenter ────────────────────────────────────────────────────

/// SimCLR-style CSI augmentation. `augment_pair` returns two distinct
/// augmented views of the same CSI window for contrastive pretraining.
///
/// Python:
/// ```python
/// from wifi_densepose.aether import CsiAugmenter
/// aug = CsiAugmenter()
/// view_a, view_b = aug.augment_pair(window, seed=42)
/// ```
#[pyclass(name = "CsiAugmenter")]
pub struct PyCsiAugmenter {
    inner: CsiAugmenter,
}

#[pymethods]
impl PyCsiAugmenter {
    #[new]
    fn new() -> Self {
        Self {
            inner: CsiAugmenter::new(),
        }
    }

    /// Produce two augmented views `(view_a, view_b)` of `window`
    /// (frames × subcarriers) using the deterministic `seed`. GIL is
    /// released during augmentation.
    fn augment_pair(
        &self,
        py: Python<'_>,
        window: Vec<Vec<f32>>,
        seed: u64,
    ) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
        py.detach(|| self.inner.augment_pair(&window, seed))
    }

    fn __repr__(&self) -> String {
        "CsiAugmenter(SimCLR-style CSI augmentation)".to_string()
    }
}

// ─── EmbeddingExtractor ──────────────────────────────────────────────

/// Full AETHER embedding extractor: CSI→pose transformer backbone +
/// projection head → a `d_proj`-dim (default 128) L2-normalized
/// embedding. Weights are deterministically seeded, so `embed` is a
/// pure function of its input for a fixed config.
///
/// Python:
/// ```python
/// from wifi_densepose.aether import AetherConfig, EmbeddingExtractor
/// ext = EmbeddingExtractor(n_subcarriers=56, config=AetherConfig())
/// emb = ext.embed(window)          # list[float], len == config.d_proj
/// ```
#[pyclass(name = "EmbeddingExtractor")]
pub struct PyEmbeddingExtractor {
    inner: EmbeddingExtractor,
    embedding_dim: usize,
}

#[pymethods]
impl PyEmbeddingExtractor {
    /// Construct an extractor. The transformer backbone is sized from
    /// `n_subcarriers` and `config.d_model`; `config.d_proj` sets the
    /// embedding dimension.
    #[new]
    #[pyo3(signature = (n_subcarriers, config, n_keypoints=17, n_heads=4, n_gnn_layers=2))]
    fn new(
        n_subcarriers: usize,
        config: PyAetherConfig,
        n_keypoints: usize,
        n_heads: usize,
        n_gnn_layers: usize,
    ) -> PyResult<Self> {
        let e_config = config.inner.clone();
        // n_heads == 0 reaches `d_model % n_heads` in the transformer and panics
        // (divide-by-zero); a non-divisor trips the native `assert!`. Both would
        // surface to Python as a PanicException. Reject cleanly instead.
        if n_heads == 0 {
            return Err(PyValueError::new_err("n_heads must be positive"));
        }
        if e_config.d_model % n_heads != 0 {
            return Err(PyValueError::new_err(format!(
                "d_model ({}) must be divisible by n_heads ({n_heads})",
                e_config.d_model
            )));
        }
        if n_subcarriers == 0 || n_keypoints == 0 {
            return Err(PyValueError::new_err(
                "n_subcarriers and n_keypoints must be positive",
            ));
        }
        if n_subcarriers > MAX_DIM || n_keypoints > MAX_DIM || n_gnn_layers > MAX_LAYERS {
            return Err(PyValueError::new_err(format!(
                "n_subcarriers/n_keypoints must be <= {MAX_DIM} and n_gnn_layers <= {MAX_LAYERS}"
            )));
        }
        let t_config = TransformerConfig {
            n_subcarriers,
            n_keypoints,
            d_model: e_config.d_model,
            n_heads,
            n_gnn_layers,
        };
        let embedding_dim = e_config.d_proj;
        Ok(Self {
            inner: EmbeddingExtractor::new(t_config, e_config),
            embedding_dim,
        })
    }

    /// Extract an embedding from a CSI window (frames × subcarriers).
    /// Returns a `d_proj`-length vector (L2-normed when the config's
    /// `normalize` is set). GIL released during the forward pass.
    fn embed(&mut self, py: Python<'_>, csi_features: Vec<Vec<f32>>) -> Vec<f32> {
        py.detach(|| self.inner.extract(&csi_features))
    }

    #[getter]
    fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    /// Total trainable parameter count (transformer + projection). Equals the
    /// number of `f32`s in a weight file for this architecture.
    #[getter]
    fn param_count(&self) -> usize {
        self.inner.param_count()
    }

    /// Load weights from `path` (a file written by `save_weights` or the Rust
    /// `EmbeddingExtractor::save_weights`), replacing the current weights.
    ///
    /// By default an `EmbeddingExtractor` uses deterministic **random** init
    /// (untrained); this is the additive path to load real weights once a
    /// trained checkpoint exists (ADR-185 §13.a). Raises `ValueError` on a
    /// missing/corrupt file or a param-count mismatch with this architecture.
    /// GIL released during file I/O + deserialization.
    fn load_weights(&mut self, py: Python<'_>, path: String) -> PyResult<()> {
        py.detach(|| self.inner.load_weights(&path))
            .map_err(PyValueError::new_err)
    }

    /// Serialize the current weights to `path` (magic `AETHERW1` + `u32` count
    /// + little-endian `f32` payload). GIL released.
    fn save_weights(&self, py: Python<'_>, path: String) -> PyResult<()> {
        py.detach(|| self.inner.save_weights(&path))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!("EmbeddingExtractor(embedding_dim={})", self.embedding_dim)
    }
}

// ─── Module functions ────────────────────────────────────────────────

/// InfoNCE (NT-Xent) contrastive loss between two batches of embeddings.
/// Delegates to the identical Rust implementation. GIL released.
#[pyfunction]
#[pyo3(signature = (embeddings_a, embeddings_b, temperature=0.07))]
fn info_nce_loss(
    py: Python<'_>,
    embeddings_a: Vec<Vec<f32>>,
    embeddings_b: Vec<Vec<f32>>,
    temperature: f32,
) -> f32 {
    py.detach(|| rust_info_nce_loss(&embeddings_a, &embeddings_b, temperature))
}

/// Cosine similarity between two embeddings — the re-ID scoring
/// primitive. Byte-identical to the private `cosine_similarity` in the
/// backing crate (same dot-product / norm formula, `f32`).
#[pyfunction]
fn cosine_similarity(a: Vec<f32>, b: Vec<f32>) -> f32 {
    let n = a.len().min(b.len());
    let dot: f32 = (0..n).map(|i| a[i] * b[i]).sum();
    let na = (0..n).map(|i| a[i] * a[i]).sum::<f32>().sqrt();
    let nb = (0..n).map(|i| b[i] * b[i]).sum::<f32>().sqrt();
    if na > 1e-10 && nb > 1e-10 {
        dot / (na * nb)
    } else {
        0.0
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAetherConfig>()?;
    m.add_class::<PyCsiAugmenter>()?;
    m.add_class::<PyEmbeddingExtractor>()?;
    m.add_function(wrap_pyfunction!(info_nce_loss, m)?)?;
    m.add_function(wrap_pyfunction!(cosine_similarity, m)?)?;
    Ok(())
}

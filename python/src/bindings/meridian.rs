//! ADR-185 P2 — PyO3 bindings for MERIDIAN cross-environment domain
//! generalization (ADR-027).
//!
//! Surfaces the **pure-sync, tch-free** inference/adaptation path into
//! `wifi_densepose.meridian`:
//!
//! - `HardwareType` / `HardwareNormalizer` / `CanonicalCsiFrame`
//!   (from `wifi-densepose-signal::hardware_norm`)
//! - `MeridianGeometryConfig` / `GeometryEncoder`
//! - `RapidAdaptation` / `AdaptationResult`
//! - `CrossDomainEvaluator` + `mpjpe`
//!   (from `wifi-densepose-train`, NO `tch-backend`)
//!
//! ## Honest scope vs ADR-185 §3.3
//!
//! ADR-185 §3.3 names a surface that partly diverges from the code at HEAD;
//! this binding tracks the **real** API and documents each deviation:
//!
//! - `HardwareType.detect(subcarrier_count)` — the real detector is the
//!   static `HardwareNormalizer::detect_hardware`; exposed here as a
//!   `HardwareType.detect` staticmethod delegating to it (no reimpl).
//! - `HardwareNormalizer.normalize(frame: CsiFrame, hw)` — the real method
//!   takes raw `(amplitude, phase)` f64 vectors and returns a `Result`, so
//!   it is bound as `normalize(amplitude, phase, hw)` (raises on error).
//! - `CanonicalCsiFrame.amplitudes/.phases` — the real fields are singular
//!   `amplitude`/`phase`; bound under their real names.
//! - `RapidAdaptation.calibrate(csi_windows) -> AdaptationResult` with a
//!   `converged` field — **does not exist**. The real engine is
//!   `push_frame` + `adapt()`, and `AdaptationResult` carries
//!   `{lora_weights, final_loss, frames_used, adaptation_epochs}` (no
//!   `converged`). Bound as-is; the `calibrate`/`converged` surface is a
//!   Rust-side gap, not fabricated here.
//!
//! Training-time types (`DomainFactorizer`, `GradientReversalLayer`,
//! `VirtualDomainAugmentor`) are out of P6 scope (ADR-185 §3.3 / Open Q
//! §11.2) — inference/adaptation only.
//!
//! ## GIL release (per ADR-117 §7, matching bindings/vitals.rs)
//!
//! `normalize`, `encode`, `adapt`, and `evaluate` are pure-sync numeric
//! ops touching no Python objects, so they run inside `py.detach`.

use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use wifi_densepose_signal::hardware_norm::{
    CanonicalCsiFrame, HardwareNormalizer, HardwareType,
};
use wifi_densepose_train::eval::{mpjpe as rust_mpjpe, CrossDomainEvaluator};
use wifi_densepose_train::geometry::{GeometryEncoder, MeridianGeometryConfig};
use wifi_densepose_train::rapid_adapt::{AdaptationLoss, AdaptationResult, RapidAdaptation};

// ─── HardwareType ────────────────────────────────────────────────────

/// WiFi chipset family, keyed by subcarrier count.
#[pyclass(from_py_object, eq, eq_int, frozen, hash, name = "HardwareType")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyHardwareType {
    Esp32S3 = 0,
    Intel5300 = 1,
    Atheros = 2,
    Generic = 3,
}

impl PyHardwareType {
    fn as_rust(self) -> HardwareType {
        match self {
            Self::Esp32S3 => HardwareType::Esp32S3,
            Self::Intel5300 => HardwareType::Intel5300,
            Self::Atheros => HardwareType::Atheros,
            Self::Generic => HardwareType::Generic,
        }
    }
    fn from_rust(hw: HardwareType) -> Self {
        match hw {
            HardwareType::Esp32S3 => Self::Esp32S3,
            HardwareType::Intel5300 => Self::Intel5300,
            HardwareType::Atheros => Self::Atheros,
            HardwareType::Generic => Self::Generic,
        }
    }
}

#[pymethods]
impl PyHardwareType {
    /// Detect hardware from subcarrier count (64→Esp32S3, 30→Intel5300,
    /// 56→Atheros, else Generic). Delegates to the real
    /// `HardwareNormalizer::detect_hardware`.
    #[staticmethod]
    fn detect(subcarrier_count: usize) -> Self {
        Self::from_rust(HardwareNormalizer::detect_hardware(subcarrier_count))
    }

    #[getter]
    fn subcarrier_count(&self) -> usize {
        self.as_rust().subcarrier_count()
    }

    #[getter]
    fn mimo_streams(&self) -> usize {
        self.as_rust().mimo_streams()
    }

    fn __repr__(&self) -> String {
        format!("HardwareType.{:?}", self.as_rust())
    }
}

// ─── CanonicalCsiFrame ───────────────────────────────────────────────

/// A CSI frame canonicalized to the normalizer's subcarrier grid
/// (default 56): z-scored amplitude + sanitized (unwrapped, detrended)
/// phase.
#[pyclass(frozen, name = "CanonicalCsiFrame")]
pub struct PyCanonicalCsiFrame {
    inner: CanonicalCsiFrame,
}

#[pymethods]
impl PyCanonicalCsiFrame {
    #[getter]
    fn amplitude(&self) -> Vec<f32> {
        self.inner.amplitude.clone()
    }

    #[getter]
    fn phase(&self) -> Vec<f32> {
        self.inner.phase.clone()
    }

    #[getter]
    fn hardware_type(&self) -> PyHardwareType {
        PyHardwareType::from_rust(self.inner.hardware_type)
    }

    fn __repr__(&self) -> String {
        format!(
            "CanonicalCsiFrame(subcarriers={}, hardware_type={:?})",
            self.inner.amplitude.len(),
            self.inner.hardware_type,
        )
    }
}

// ─── HardwareNormalizer ──────────────────────────────────────────────

/// Normalizes CSI frames from heterogeneous chipsets into a canonical
/// representation (cubic resample → z-score amplitude → sanitize phase).
#[pyclass(name = "HardwareNormalizer")]
pub struct PyHardwareNormalizer {
    inner: HardwareNormalizer,
}

#[pymethods]
impl PyHardwareNormalizer {
    /// Create a normalizer. `canonical_subcarriers` defaults to 56.
    #[new]
    #[pyo3(signature = (canonical_subcarriers=56))]
    fn new(canonical_subcarriers: usize) -> PyResult<Self> {
        HardwareNormalizer::with_canonical_subcarriers(canonical_subcarriers)
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Detect hardware from subcarrier count (static).
    #[staticmethod]
    fn detect_hardware(subcarrier_count: usize) -> PyHardwareType {
        PyHardwareType::from_rust(HardwareNormalizer::detect_hardware(subcarrier_count))
    }

    #[getter]
    fn canonical_subcarriers(&self) -> usize {
        self.inner.canonical_subcarriers()
    }

    /// Normalize a raw CSI frame given per-subcarrier `amplitude` and
    /// `phase` (equal length) and its `hardware` type. Raises
    /// `ValueError` on empty/mismatched input. GIL released.
    fn normalize(
        &self,
        py: Python<'_>,
        amplitude: Vec<f64>,
        phase: Vec<f64>,
        hardware: PyHardwareType,
    ) -> PyResult<PyCanonicalCsiFrame> {
        let hw = hardware.as_rust();
        py.detach(|| self.inner.normalize(&amplitude, &phase, hw))
            .map(|inner| PyCanonicalCsiFrame { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!(
            "HardwareNormalizer(canonical_subcarriers={})",
            self.inner.canonical_subcarriers()
        )
    }
}

// ─── MeridianGeometryConfig ──────────────────────────────────────────

/// Config for the geometry encoder (Fourier bands + DeepSets output dim).
#[pyclass(from_py_object, frozen, name = "MeridianGeometryConfig")]
#[derive(Clone)]
pub struct PyMeridianGeometryConfig {
    inner: MeridianGeometryConfig,
}

#[pymethods]
impl PyMeridianGeometryConfig {
    #[new]
    #[pyo3(signature = (n_frequencies=10, scale=1.0, geometry_dim=64, seed=42))]
    fn new(n_frequencies: usize, scale: f32, geometry_dim: usize, seed: u64) -> Self {
        Self {
            inner: MeridianGeometryConfig {
                n_frequencies,
                scale,
                geometry_dim,
                seed,
            },
        }
    }

    #[getter]
    fn n_frequencies(&self) -> usize {
        self.inner.n_frequencies
    }
    #[getter]
    fn scale(&self) -> f32 {
        self.inner.scale
    }
    #[getter]
    fn geometry_dim(&self) -> usize {
        self.inner.geometry_dim
    }
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.seed
    }

    fn __repr__(&self) -> String {
        format!(
            "MeridianGeometryConfig(n_frequencies={}, scale={}, geometry_dim={}, seed={})",
            self.inner.n_frequencies, self.inner.scale, self.inner.geometry_dim, self.inner.seed,
        )
    }
}

// ─── GeometryEncoder ─────────────────────────────────────────────────

/// Permutation-invariant encoder: variable-count AP positions `[x,y,z]`
/// → a fixed `geometry_dim` (default 64) vector.
#[pyclass(name = "GeometryEncoder")]
pub struct PyGeometryEncoder {
    inner: GeometryEncoder,
    geometry_dim: usize,
}

#[pymethods]
impl PyGeometryEncoder {
    #[new]
    #[pyo3(signature = (config=None))]
    fn new(config: Option<PyMeridianGeometryConfig>) -> Self {
        let cfg = config.map(|c| c.inner).unwrap_or_default();
        let geometry_dim = cfg.geometry_dim;
        Self {
            inner: GeometryEncoder::new(&cfg),
            geometry_dim,
        }
    }

    /// Encode AP positions (a non-empty list of `[x, y, z]`) into a
    /// `geometry_dim`-length vector. Raises `ValueError` if the list is
    /// empty or any position is not exactly 3 coordinates. GIL released.
    fn encode(&self, py: Python<'_>, ap_positions: Vec<Vec<f32>>) -> PyResult<Vec<f32>> {
        if ap_positions.is_empty() {
            return Err(PyValueError::new_err(
                "ap_positions must contain at least one [x, y, z] position",
            ));
        }
        let mut coords: Vec<[f32; 3]> = Vec::with_capacity(ap_positions.len());
        for (i, p) in ap_positions.iter().enumerate() {
            if p.len() != 3 {
                return Err(PyValueError::new_err(format!(
                    "ap_positions[{i}] must have exactly 3 coordinates, got {}",
                    p.len()
                )));
            }
            coords.push([p[0], p[1], p[2]]);
        }
        Ok(py.detach(|| self.inner.encode(&coords)))
    }

    #[getter]
    fn geometry_dim(&self) -> usize {
        self.geometry_dim
    }

    fn __repr__(&self) -> String {
        format!("GeometryEncoder(geometry_dim={})", self.geometry_dim)
    }
}

// ─── RapidAdaptation / AdaptationResult ──────────────────────────────

/// Result of `RapidAdaptation.adapt()`.
#[pyclass(frozen, name = "AdaptationResult")]
pub struct PyAdaptationResult {
    inner: AdaptationResult,
}

#[pymethods]
impl PyAdaptationResult {
    #[getter]
    fn lora_weights(&self) -> Vec<f32> {
        self.inner.lora_weights.clone()
    }
    #[getter]
    fn final_loss(&self) -> f32 {
        self.inner.final_loss
    }
    #[getter]
    fn frames_used(&self) -> usize {
        self.inner.frames_used
    }
    #[getter]
    fn adaptation_epochs(&self) -> usize {
        self.inner.adaptation_epochs
    }

    fn __repr__(&self) -> String {
        format!(
            "AdaptationResult(final_loss={:.6}, frames_used={}, adaptation_epochs={})",
            self.inner.final_loss, self.inner.frames_used, self.inner.adaptation_epochs,
        )
    }
}

/// Few-shot test-time adaptation: accumulate unlabeled CSI frames, then
/// `adapt()` to produce LoRA weight deltas that minimize a self-supervised
/// proxy loss.
///
/// Scope caveat (from the Rust module, kept honest): this minimizes a
/// self-supervised proxy over a tiny LoRA bottleneck; it is NOT wired to
/// the pose model and there is no measured end-to-end PCK gain from this
/// path — do not cite a PCK improvement from `adapt()`.
#[pyclass(name = "RapidAdaptation")]
pub struct PyRapidAdaptation {
    inner: RapidAdaptation,
}

#[pymethods]
impl PyRapidAdaptation {
    /// Build an adaptation engine. `loss_kind` is one of
    /// `"contrastive"`, `"entropy"`, `"combined"` (default). `lambda_ent`
    /// is used only by `"combined"`.
    #[new]
    #[pyo3(signature = (
        min_calibration_frames,
        lora_rank,
        loss_kind="combined",
        epochs=5,
        lr=0.001,
        lambda_ent=0.5
    ))]
    fn new(
        min_calibration_frames: usize,
        lora_rank: usize,
        loss_kind: &str,
        epochs: usize,
        lr: f32,
        lambda_ent: f32,
    ) -> PyResult<Self> {
        let loss = match loss_kind {
            "contrastive" => AdaptationLoss::ContrastiveTTT { epochs, lr },
            "entropy" => AdaptationLoss::EntropyMin { epochs, lr },
            "combined" => AdaptationLoss::Combined {
                epochs,
                lr,
                lambda_ent,
            },
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown loss_kind '{other}'; expected 'contrastive', 'entropy', or 'combined'"
                )))
            }
        };
        Ok(Self {
            inner: RapidAdaptation::new(min_calibration_frames, lora_rank, loss),
        })
    }

    /// Push a single unlabeled CSI frame into the calibration buffer.
    fn push_frame(&mut self, frame: Vec<f32>) {
        self.inner.push_frame(&frame);
    }

    /// True once at least `min_calibration_frames` have been buffered.
    fn is_ready(&self) -> bool {
        self.inner.is_ready()
    }

    #[getter]
    fn buffer_len(&self) -> usize {
        self.inner.buffer_len()
    }

    /// Run test-time adaptation over the buffered frames. Raises
    /// `ValueError` if the buffer is empty or `lora_rank == 0`. GIL
    /// released during the finite-difference optimization.
    fn adapt(&self, py: Python<'_>) -> PyResult<PyAdaptationResult> {
        py.detach(|| self.inner.adapt())
            .map(|inner| PyAdaptationResult { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!("RapidAdaptation(buffered={})", self.inner.buffer_len())
    }
}

// ─── CrossDomainEvaluator ────────────────────────────────────────────

/// Cross-domain pose-accuracy evaluator (MPJPE + domain-gap ratio).
#[pyclass(name = "CrossDomainEvaluator")]
pub struct PyCrossDomainEvaluator {
    inner: CrossDomainEvaluator,
}

#[pymethods]
impl PyCrossDomainEvaluator {
    /// Create an evaluator for `n_joints` (e.g. 17 for COCO).
    #[new]
    fn new(n_joints: usize) -> Self {
        Self {
            inner: CrossDomainEvaluator::new(n_joints),
        }
    }

    /// Evaluate `predictions` (a list of `(pred, gt)` flat `n_joints*3`
    /// vectors) grouped by `domain_labels` (0 = in-domain). Returns a
    /// dict of the six cross-domain metrics. Raises `ValueError` on a
    /// length mismatch. GIL released.
    fn evaluate(
        &self,
        py: Python<'_>,
        predictions: Vec<(Vec<f32>, Vec<f32>)>,
        domain_labels: Vec<u32>,
    ) -> PyResult<HashMap<String, f32>> {
        if predictions.len() != domain_labels.len() {
            return Err(PyValueError::new_err(format!(
                "predictions ({}) and domain_labels ({}) must have equal length",
                predictions.len(),
                domain_labels.len()
            )));
        }
        let m = py.detach(|| self.inner.evaluate(&predictions, &domain_labels));
        let mut out = HashMap::with_capacity(6);
        out.insert("in_domain_mpjpe".to_string(), m.in_domain_mpjpe);
        out.insert("cross_domain_mpjpe".to_string(), m.cross_domain_mpjpe);
        out.insert("few_shot_mpjpe".to_string(), m.few_shot_mpjpe);
        out.insert("cross_hardware_mpjpe".to_string(), m.cross_hardware_mpjpe);
        out.insert("domain_gap_ratio".to_string(), m.domain_gap_ratio);
        out.insert("adaptation_speedup".to_string(), m.adaptation_speedup);
        Ok(out)
    }

    fn __repr__(&self) -> String {
        "CrossDomainEvaluator()".to_string()
    }
}

/// Mean Per Joint Position Error between flat `[n_joints*3]` pose vectors.
#[pyfunction]
fn mpjpe(pred: Vec<f32>, gt: Vec<f32>, n_joints: usize) -> f32 {
    rust_mpjpe(&pred, &gt, n_joints)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHardwareType>()?;
    m.add_class::<PyCanonicalCsiFrame>()?;
    m.add_class::<PyHardwareNormalizer>()?;
    m.add_class::<PyMeridianGeometryConfig>()?;
    m.add_class::<PyGeometryEncoder>()?;
    m.add_class::<PyAdaptationResult>()?;
    m.add_class::<PyRapidAdaptation>()?;
    m.add_class::<PyCrossDomainEvaluator>()?;
    m.add_function(wrap_pyfunction!(mpjpe, m)?)?;
    Ok(())
}

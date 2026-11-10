//! ADR-117 P3 — PyO3 bindings for `wifi_densepose_vitals`.
//!
//! Surfaces:
//!
//! - `VitalStatus` enum — clinical-grade / degraded / unreliable / unavailable
//! - `VitalEstimate` — single BPM estimate + confidence + status
//! - `VitalReading` — combined HR + BR + signal quality snapshot
//! - `BreathingExtractor` — bandpass 0.1–0.5 Hz → respiratory rate
//! - `HeartRateExtractor` — bandpass 0.8–2.0 Hz + autocorrelation → HR
//!
//! ## GIL release strategy (per ADR-117 §7 and the Q5 audit on
//! 2026-05-24)
//!
//! `wifi-densepose-vitals` has zero tokio deps and the extract loops
//! are pure-sync DSP. Wrap the `.extract(...)` calls in
//! `py.detach(|| ...)` so Python users can run inference in a
//! tokio-backed web server without GIL contention starving the
//! event loop.

use pyo3::prelude::*;

use wifi_densepose_vitals::{
    BreathingExtractor, HeartRateExtractor, VitalEstimate, VitalReading, VitalStatus,
};

// ─── VitalStatus enum ────────────────────────────────────────────────

/// Status of a vital sign measurement.
///
/// Python:
/// ```python
/// from wifi_densepose import VitalStatus
/// VitalStatus.Valid       # clinical-grade
/// VitalStatus.Degraded    # reduced confidence
/// VitalStatus.Unreliable  # single RSSI source / low quality
/// VitalStatus.Unavailable # no measurement possible
/// ```
#[pyclass(from_py_object, eq, eq_int, hash, frozen, name = "VitalStatus")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyVitalStatus {
    Valid = 0,
    Degraded = 1,
    Unreliable = 2,
    Unavailable = 3,
}

#[pymethods]
impl PyVitalStatus {
    fn __repr__(&self) -> String {
        format!("VitalStatus.{:?}", self.as_rust())
    }
}

impl PyVitalStatus {
    fn as_rust(&self) -> VitalStatus {
        match self {
            Self::Valid => VitalStatus::Valid,
            Self::Degraded => VitalStatus::Degraded,
            Self::Unreliable => VitalStatus::Unreliable,
            Self::Unavailable => VitalStatus::Unavailable,
        }
    }

    fn from_rust(s: VitalStatus) -> Self {
        match s {
            VitalStatus::Valid => Self::Valid,
            VitalStatus::Degraded => Self::Degraded,
            VitalStatus::Unreliable => Self::Unreliable,
            VitalStatus::Unavailable => Self::Unavailable,
        }
    }
}

// ─── VitalEstimate ───────────────────────────────────────────────────

/// A single vital-sign estimate (BPM + confidence + status).
///
/// Python:
/// ```python
/// from wifi_densepose import VitalEstimate, VitalStatus
/// est = VitalEstimate(72.4, confidence=0.9, status=VitalStatus.Valid)
/// print(est.value_bpm, est.confidence, est.status)
/// ```
#[pyclass(from_py_object, frozen, name = "VitalEstimate")]
#[derive(Clone)]
pub struct PyVitalEstimate {
    inner: VitalEstimate,
}

#[pymethods]
impl PyVitalEstimate {
    #[new]
    fn new(value_bpm: f64, confidence: f64, status: PyVitalStatus) -> Self {
        Self {
            inner: VitalEstimate {
                value_bpm,
                confidence,
                status: status.as_rust(),
            },
        }
    }

    #[getter]
    fn value_bpm(&self) -> f64 { self.inner.value_bpm }

    #[getter]
    fn confidence(&self) -> f64 { self.inner.confidence }

    #[getter]
    fn status(&self) -> PyVitalStatus { PyVitalStatus::from_rust(self.inner.status) }

    fn __repr__(&self) -> String {
        format!(
            "VitalEstimate(value_bpm={:.2}, confidence={:.3}, status={:?})",
            self.inner.value_bpm, self.inner.confidence, self.inner.status,
        )
    }
}

impl PyVitalEstimate {
    fn from_rust(e: VitalEstimate) -> Self {
        Self { inner: e }
    }
}

// ─── VitalReading ────────────────────────────────────────────────────

/// Combined HR + BR snapshot from one window of CSI data.
#[pyclass(frozen, name = "VitalReading")]
pub struct PyVitalReading {
    inner: VitalReading,
}

#[pymethods]
impl PyVitalReading {
    #[new]
    fn new(
        respiratory_rate: PyVitalEstimate,
        heart_rate: PyVitalEstimate,
        subcarrier_count: usize,
        signal_quality: f64,
        timestamp_secs: f64,
    ) -> Self {
        Self {
            inner: VitalReading {
                respiratory_rate: respiratory_rate.inner,
                heart_rate: heart_rate.inner,
                subcarrier_count,
                signal_quality,
                timestamp_secs,
            },
        }
    }

    #[getter]
    fn respiratory_rate(&self) -> PyVitalEstimate {
        PyVitalEstimate::from_rust(self.inner.respiratory_rate.clone())
    }

    #[getter]
    fn heart_rate(&self) -> PyVitalEstimate {
        PyVitalEstimate::from_rust(self.inner.heart_rate.clone())
    }

    #[getter]
    fn subcarrier_count(&self) -> usize { self.inner.subcarrier_count }

    #[getter]
    fn signal_quality(&self) -> f64 { self.inner.signal_quality }

    #[getter]
    fn timestamp_secs(&self) -> f64 { self.inner.timestamp_secs }

    fn __repr__(&self) -> String {
        format!(
            "VitalReading(br={:.1}, hr={:.1}, subcarriers={}, quality={:.3})",
            self.inner.respiratory_rate.value_bpm,
            self.inner.heart_rate.value_bpm,
            self.inner.subcarrier_count,
            self.inner.signal_quality,
        )
    }
}

// ─── BreathingExtractor ──────────────────────────────────────────────

/// Extracts respiratory rate (6–30 BPM) from per-subcarrier amplitude
/// residuals via 0.1–0.5 Hz bandpass + zero-crossing analysis.
///
/// Python:
/// ```python
/// from wifi_densepose import BreathingExtractor
///
/// br = BreathingExtractor.esp32_default()  # 56 subcarriers, 100 Hz, 30s window
/// # or: BreathingExtractor(n_subcarriers=56, sample_rate=100.0, window_secs=30.0)
///
/// # Feed residuals from your preprocessor (one frame at a time)
/// est = br.extract(residuals=[0.01, -0.02, …], weights=[])  # equal weights
/// if est is not None:
///     print(est.value_bpm, est.confidence)
/// ```
#[pyclass(name = "BreathingExtractor")]
pub struct PyBreathingExtractor {
    inner: BreathingExtractor,
}

#[pymethods]
impl PyBreathingExtractor {
    /// Construct with explicit parameters.
    #[new]
    #[pyo3(signature = (n_subcarriers, sample_rate, window_secs=30.0))]
    fn new(n_subcarriers: usize, sample_rate: f64, window_secs: f64) -> Self {
        Self {
            inner: BreathingExtractor::new(n_subcarriers, sample_rate, window_secs),
        }
    }

    /// ESP32 defaults: 56 subcarriers, 100 Hz, 30-second window.
    #[staticmethod]
    fn esp32_default() -> Self {
        Self { inner: BreathingExtractor::esp32_default() }
    }

    /// Extract respiratory rate from a vector of per-subcarrier
    /// residuals + per-subcarrier weights. GIL is released during the
    /// DSP loop so Python threads can do other work concurrently.
    ///
    /// Returns `None` if insufficient history has been accumulated.
    fn extract(&mut self, py: Python<'_>, residuals: Vec<f64>, weights: Vec<f64>) -> Option<PyVitalEstimate> {
        // GIL release: see ADR-117 §7 and the Q5 tokio audit. The DSP
        // loop is pure sync, no Python objects touched, safe to run
        // without the GIL.
        let est = py.detach(|| self.inner.extract(&residuals, &weights));
        est.map(PyVitalEstimate::from_rust)
    }

    fn __repr__(&self) -> String {
        format!("BreathingExtractor(0.1–0.5 Hz bandpass)")
    }
}

// ─── HeartRateExtractor ──────────────────────────────────────────────

/// Extracts heart rate (40–120 BPM) from per-subcarrier amplitude
/// residuals and per-subcarrier unwrapped phases (radians) via
/// 0.8–2.0 Hz bandpass + autocorrelation peak detection.
///
/// Python:
/// ```python
/// from wifi_densepose import HeartRateExtractor
///
/// hr = HeartRateExtractor.esp32_default()  # 56 subcarriers, 100 Hz, 15s window
///
/// # Feed residuals and matching unwrapped phases from your preprocessor.
/// # Like BreathingExtractor's weights, phases=[] means "no per-subcarrier
/// # coherence information available" and falls back to equal weighting
/// # across all subcarriers -- it does NOT silently drop every frame.
/// est = hr.extract(residuals=[0.01, -0.02, …], phases=[0.0, 0.01, …])
/// if est is not None:
///     print(est.value_bpm, est.confidence)
/// ```
#[pyclass(name = "HeartRateExtractor")]
pub struct PyHeartRateExtractor {
    inner: HeartRateExtractor,
}

#[pymethods]
impl PyHeartRateExtractor {
    /// Construct with explicit parameters.
    #[new]
    #[pyo3(signature = (n_subcarriers, sample_rate, window_secs=15.0))]
    fn new(n_subcarriers: usize, sample_rate: f64, window_secs: f64) -> Self {
        Self {
            inner: HeartRateExtractor::new(n_subcarriers, sample_rate, window_secs),
        }
    }

    /// ESP32 defaults: 56 subcarriers, 100 Hz, 15-second window.
    #[staticmethod]
    fn esp32_default() -> Self {
        Self { inner: HeartRateExtractor::esp32_default() }
    }

    /// Extract heart rate from per-subcarrier residuals and matching
    /// per-subcarrier unwrapped phases (radians). A short or empty `phases`
    /// slice falls back to equal weighting for any subcarrier missing phase
    /// data (issue #1423) -- it does not truncate the number of subcarriers
    /// fused, and does not silently return `None` for every frame. GIL
    /// released during DSP.
    fn extract(
        &mut self,
        py: Python<'_>,
        residuals: Vec<f64>,
        phases: Vec<f64>,
    ) -> Option<PyVitalEstimate> {
        let est = py.detach(|| self.inner.extract(&residuals, &phases));
        est.map(PyVitalEstimate::from_rust)
    }

    fn __repr__(&self) -> String {
        format!("HeartRateExtractor(0.8–2.0 Hz bandpass)")
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVitalStatus>()?;
    m.add_class::<PyVitalEstimate>()?;
    m.add_class::<PyVitalReading>()?;
    m.add_class::<PyBreathingExtractor>()?;
    m.add_class::<PyHeartRateExtractor>()?;
    Ok(())
}
